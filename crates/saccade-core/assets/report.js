(function () {
  "use strict";

  var report = JSON.parse(document.getElementById("saccade-data").textContent);
  var entries = report.entries || [];
  var LB = ((report.config || {}).labels) || { baseline: "baseline", capture: "capture" };
  var IDENTITY = (report.config || {}).mode === "identity";

  var UI = window.__saccadeUI;
  UI.installCvd();
  var reg = UI.use(UI.actions());
  var A = window.__saccadeAgent;
  // The view state the URL hash and window.saccade describe; compare() reads it
  // when it is built and writes it back as the person changes the view.
  var V = { entry: null, layout: "swipe", split: 0.5, vertical: false, zoom: "fit", at: null, heat: 0, signed: 0, mask: false, contrast: 1, channel: "rgb", ev: 0, roi: null, hsel: null, hselT: "", tcur: "", hotspotReq: null };
  var agent = null;
  function notify() { if (agent) agent.changed(); }
  function applyFilter() {
    document.body.style.setProperty("--f", UI.filterCss(V.ev, V.contrast, V.channel));
  }

  var RANK = { fail: 0, error: 1, missing: 2, new: 3, pass: 4 };
  var STATUSES = ["fail", "error", "missing", "new", "pass"];

  var COLUMNS = [
    { key: "status", label: "Status", get: function (e) { return RANK[e.status]; } },
    { key: "name", label: "Name", get: function (e) { return e.name; }, text: true },
    { key: "metric", label: "Metric", sec: true, get: function (e) { return e.metric_used; }, text: true },
    { key: "value", label: "Value", num: true, get: function (e) { return e.value; } },
    { key: "threshold", label: "Threshold", num: true, get: function (e) { return e.threshold; } },
    { key: "mean", label: "Mean", num: true, sec: true, get: function (e) { return e.metrics ? e.metrics.mean : null; } },
    { key: "p95", label: "p95", num: true, sec: true, get: function (e) { return e.metrics ? e.metrics.p95 : null; } },
    { key: "max", label: "Max", num: true, sec: true, get: function (e) { return e.metrics ? e.metrics.max : null; } }
  ];

  // ---- helpers -----------------------------------------------------------

  var h = UI.h;

  function fmt(v) {
    if (v === null || v === undefined || isNaN(v)) return null;
    if (v === 0) return "0";
    return v >= 0.001 ? v.toFixed(4) : v.toExponential(2);
  }

  function heatmapTop(e) {
    return e.buffer ? fmt(e.buffer.heatmap_max) + " " + e.buffer.unit : "1 FLIP";
  }

  // Encode each path segment; the path is relative to index.html.
  function url(p) {
    return "./" + String(p).split("/").map(encodeURIComponent).join("/");
  }

  function fmtTime(unix) {
    var d = new Date(unix * 1000);
    if (isNaN(d.getTime())) return "unknown";
    return d.toISOString().replace("T", " ").slice(0, 16) + " UTC";
  }

  // Hotspots are listed and drawn for failing entries only.
  function hotspotsOf(e) { return e.status === "fail" && e.hotspots ? e.hotspots : []; }
  function frameWide(e) {
    var hs = hotspotsOf(e);
    return hs.length > 0 && hs[0].rect_frac[2] * hs[0].rect_frac[3] >= 0.5;
  }
  function pct(v) { return (v * 100).toFixed(v < 0.1 ? 1 : 0) + "%"; }

  // ---- header ------------------------------------------------------------

  function renderHeader() {
    // Metadata is rendered from the report in Rust, including identity proof scope.
    var t = report.totals || {};
    var chips = document.getElementById("chips");
    chips.appendChild(h("span", { class: "chip total", role: "listitem" }, h("b", { text: String(t.total || 0) }), "total"));
    STATUSES.forEach(function (s) {
      var n = t[s] || 0;
      chips.appendChild(h("span", { class: "chip s-" + s + (n ? "" : " zero"), role: "listitem" }, h("b", { text: String(n) }), s));
    });
  }

  // ---- table state ---------------------------------------------------------

  var hasIssues = entries.some(function (e) { return e.status !== "pass"; });
  var state = {
    filter: hasIssues ? "issues" : "all",
    sort: null, // {key, dir} or null for the default order
    regions: true, // draw region rectangles over the images
    hotspots: true, // draw numbered hotspot boxes over the images
    open: {}
  };
  var timers = [];

  function cmpDefault(a, b) {
    var d = RANK[a.status] - RANK[b.status];
    if (d) return d;
    var av = a.value === null ? -1 : a.value, bv = b.value === null ? -1 : b.value;
    if (av !== bv) return bv - av;
    return a.name < b.name ? -1 : a.name > b.name ? 1 : 0;
  }

  function sorted(list) {
    var out = list.slice();
    var s = state.sort;
    if (!s) return out.sort(cmpDefault);
    var col = COLUMNS.filter(function (c) { return c.key === s.key; })[0];
    var sign = s.dir === "asc" ? 1 : -1;
    return out.sort(function (a, b) {
      var av = col.get(a), bv = col.get(b);
      var an = av === null || av === undefined, bn = bv === null || bv === undefined;
      if (an !== bn) return an ? 1 : -1; // empty values always last
      var r = 0;
      if (!an) r = col.text ? String(av).localeCompare(String(bv)) : av - bv;
      return r ? r * sign : cmpDefault(a, b);
    });
  }

  function onSort(key) {
    var col = COLUMNS.filter(function (c) { return c.key === key; })[0];
    var first = col.num ? "desc" : "asc";
    var second = first === "asc" ? "desc" : "asc";
    var s = state.sort;
    if (!s || s.key !== key) state.sort = { key: key, dir: first };
    else if (s.dir === first) state.sort = { key: key, dir: second };
    else state.sort = null; // third click: back to the default order
    render();
  }

  function renderHead() {
    var tr = document.getElementById("thead-row");
    tr.textContent = "";
    COLUMNS.forEach(function (c) {
      var s = state.sort;
      var th = h("th", {
        class: (c.num ? "num " : "") + (c.sec ? "col-sec" : ""),
        scope: "col",
        "aria-sort": s && s.key === c.key ? (s.dir === "asc" ? "ascending" : "descending") : "none"
      }, h("button", { type: "button", onclick: function () { onSort(c.key); } }, c.label));
      tr.appendChild(th);
    });
  }

  // ---- rows ----------------------------------------------------------------

  function numCell(v, extra) {
    var s = fmt(v);
    return h("td", { class: "num" + (extra ? " " + extra : ""), title: v === null || v === undefined ? null : String(v) },
      s === null ? h("span", { class: "na", text: "—" }) : s);
  }

  function valueCell(e) {
    var td = numCell(e.value);
    if (e.value !== null && e.threshold > 0) {
      var pct = Math.max(1, Math.min(100, (e.value / (e.threshold * 2)) * 100));
      td.appendChild(h("span", { class: "meter s-" + e.status },
        h("i", { style: "width:" + pct.toFixed(1) + "%" }), h("u")));
    }
    return td;
  }

  function toggle(name) {
    if (state.open[name]) {
      delete state.open[name];
      if (V.entry === name) V.entry = Object.keys(state.open)[0] || null;
    } else {
      state.open[name] = true;
      V.entry = name;
    }
    render();
    notify();
  }

  function renderRow(e) {
    var open = !!state.open[e.name];
    var tr = h("tr", { class: "row" + (open ? " open" : ""), onclick: function (ev) {
      if (ev.target.closest("button")) return;
      toggle(e.name);
    } },
      h("td", null, h("span", { class: "st s-" + e.status, text: e.status })),
      h("td", { class: "name" }, h("button", { type: "button", "aria-expanded": open ? "true" : "false", onclick: function () { toggle(e.name); } }, e.name),
        (e.meta_diff || []).length ? h("span", { class: "badge cfg", title: e.meta_diff.map(function (d) { return d.key; }).join(", "), text: "config differs" }) : null,
        (e.warnings || []).length ? h("span", { class: "badge warn", title: e.warnings.join("\n"), text: "warning" }) : null,
        frameWide(e) ? h("span", { class: "badge wide", title: "The largest hotspot covers at least half of the frame", text: "frame-wide change" }) : null),
      h("td", { class: "metric col-sec", text: e.metric_used }),
      valueCell(e),
      numCell(e.threshold),
      numCell(e.metrics ? e.metrics.mean : null, "col-sec"),
      numCell(e.metrics ? e.metrics.p95 : null, "col-sec"),
      numCell(e.metrics ? e.metrics.max : null, "col-sec")
    );
    return tr;
  }

  function render() {
    timers.forEach(clearInterval);
    timers = [];
    cmps = [];
    renderHead();
    var shown = entries.filter(function (e) { return state.filter === "all" || e.status !== "pass"; });
    var body = document.getElementById("tbody");
    body.textContent = "";
    sorted(shown).forEach(function (e) {
      body.appendChild(renderRow(e));
      if (state.open[e.name]) body.appendChild(renderDetail(e));
    });
    cmps.forEach(function (c) { c.refresh(); });
    document.getElementById("empty").hidden = shown.length > 0;
    document.getElementById("count").textContent = "showing " + shown.length + " of " + entries.length;
    document.getElementById("f-issues").setAttribute("aria-pressed", state.filter === "issues");
    document.getElementById("f-all").setAttribute("aria-pressed", state.filter === "all");
  }

  // ---- detail --------------------------------------------------------------

  function placeholder(e, what) {
    return h("div", { class: "ph" }, h("div", null, h("b", { text: "no " + what }), "status: " + e.status));
  }

  // An image inside a .zbox; records the natural width for 1x/2x sizing.
  function zimg(src, alt, e, onHot) {
    var box = h("div", { class: "zbox" });
    if (e.metrics && e.metrics.width) box.style.setProperty("--nw", e.metrics.width);
    var img = h("img", { src: url(src), alt: alt, loading: "lazy", decoding: "async" });
    img.addEventListener("load", function () {
      box.style.setProperty("--nw", img.naturalWidth);
      markUpscaled(box.closest(".d"));
    });
    box.appendChild(img);
    regionBoxes(e).forEach(function (r) { box.appendChild(r); });
    hotspotsOf(e).forEach(function (hs, i) {
      var r = hs.rect_frac;
      box.appendChild(h("button", {
        type: "button", class: "hsp", title: "Hotspot " + (i + 1) + ": zoom the compare stage to it",
        "aria-label": "Zoom to hotspot " + (i + 1),
        style: "left:" + (r[0] * 100) + "%;top:" + (r[1] * 100) + "%;width:" + (r[2] * 100) + "%;height:" + (r[3] * 100) + "%",
        onclick: function () { if (onHot) onHot(i); }
      }, h("span", { text: String(i + 1) })));
    });
    if (src === e.paths.capture && e.paths.nonfinite_mask) {
      box.appendChild(h("img", { class: "side-mask", src: url(e.paths.nonfinite_mask), alt: "Non-finite mask", draggable: "false" }));
      var dims = e.metrics || {};
      (((e.diagnostics || {}).nonfinite || {}).clusters || []).forEach(function (c, i) {
        var r = c.rect_px;
        box.appendChild(h("div", { class: "side-nfb", title: c.pixels + " affected pixels", style: "left:" + r[0] / dims.width * 100 + "%;top:" + r[1] / dims.height * 100 + "%;width:" + r[2] / dims.width * 100 + "%;height:" + r[3] / dims.height * 100 + "%" }, h("span", { text: "NF " + (i + 1) })));
      });
    }
    return box;
  }

  // Region rectangles as percentages of the image, so every zoom level lines up.
  function regionBoxes(e) {
    var m = e.metrics;
    if (!m || !m.width || !m.height) return [];
    return (e.regions || []).map(function (r) {
      var rp = r.rect_px;
      return h("div", {
        class: "rgn" + (r.status === "fail" ? " s-fail" : ""),
        style: "left:" + (rp[0] / m.width * 100) + "%;top:" + (rp[1] / m.height * 100) + "%;width:" +
          (rp[2] / m.width * 100) + "%;height:" + (rp[3] / m.height * 100) + "%"
      }, h("span", { text: r.name }));
    });
  }

  function regionTable(e) {
    var rs = e.regions || [];
    if (!rs.length) return null;
    var head = ["Region", "Rect (px)", "Metric", "Value", "Threshold", "Status"];
    return h("div", { class: "rtab-wrap" }, h("table", { class: "rtab" },
      h("caption", { text: "Regions" }),
      h("thead", null, h("tr", null, head.map(function (t, i) { return h("th", { class: (i >= 3 && i < 5 ? "num" : "") + (i === 1 || i === 2 ? " col-sec" : ""), scope: "col", text: t }); }))),
      h("tbody", null, rs.map(function (r) {
        var st = r.status || "info";
        return h("tr", null,
          h("td", { text: r.name }),
          h("td", { class: "col-sec", text: r.rect_px.join(", ") }),
          h("td", { class: "col-sec", text: r.metric_used }),
          numCell(r.value),
          numCell(r.threshold),
          h("td", null, h("span", { class: "st" + (r.status ? " s-" + r.status : ""), text: st })));
      }))));
  }

  // Non-fatal observations (all-black or all-white sides, non-finite samples) and
  // per-side non-finite counts, in the validity area at the top of the detail.
  function warnBox(e) {
    var ws = (e.warnings || []).slice(), lines = [];
    [[LB.baseline, e.baseline_properties], [LB.capture, e.properties]].forEach(function (sp) {
      var p = sp[1];
      if (!p) return;
      var bad = (p.nan_count || 0) + (p.inf_count || 0);
      if (bad || p.negative_count) {
        lines.push(h("li", { class: bad ? "nf" : "", text: sp[0] + ": " + (p.nan_count || 0) + " NaN, " + (p.inf_count || 0) + " Inf, " + (p.negative_count || 0) + " negative samples" }));
      }
    });
    if (!ws.length && !lines.length) return null;
    return h("div", { class: "cfgwarn warnbox", role: "note" },
      h("strong", { text: "\u26A0 Warnings: check the images before trusting the numbers" }),
      h("ul", null, ws.map(function (w) { return h("li", { text: w }); }).concat(lines)));
  }

  function metaTable(e) {
    var ds = e.meta_diff || [];
    if (!ds.length) return null;
    return h("div", { class: "cfgwarn", role: "note" },
      h("strong", { text: "\u26A0 Configuration differs: this comparison may not be like for like" }),
      h("div", { class: "rtab-wrap" }, h("table", { class: "rtab mtab" },
      h("thead", null, h("tr", null, ["Key", LB.baseline, LB.capture].map(function (t) { return h("th", { scope: "col", text: t }); }))),
      h("tbody", null, ds.map(function (d) {
        return h("tr", null,
          h("td", { class: "mk", text: d.key }),
          h("td", { class: d.baseline === "<absent>" ? "mv na" : "mv", text: d.baseline }),
          h("td", { class: d.capture === "<absent>" ? "mv na" : "mv", text: d.capture }));
      })))));
  }

  function markUpscaled(d) {
    if (!d) return;
    var img = d.querySelector(".zbox img");
    var box = d.querySelector(".zbox");
    if (!img || !box) return;
    d.classList.toggle("up", d.classList.contains("zoom-fit") && box.clientWidth > img.naturalWidth);
  }

  function pane(e, label, path, extra, onHot) {
    var cap = h("figcaption", null, h("span", { text: label }), extra || null);
    var body = path ? h("div", { class: "vp" }, zimg(path, label + " of " + e.name, e, onHot)) : placeholder(e, label);
    return h("figure", { class: "pane" }, cap, body);
  }

  function hotspotTable(e, onHot) {
    var hs = hotspotsOf(e);
    if (!hs.length) return null;
    var head = ["#", "Position", "Size (px)", "Share of error", "Mean FLIP", "Max FLIP"];
    return h("div", { class: "rtab-wrap" }, h("table", { class: "rtab htab" },
      h("caption", { text: "Hotspots (click a row to zoom the compare stage to it)" }),
      h("thead", null, h("tr", null, head.map(function (t, i) { return h("th", { class: i >= 3 ? "num" : "", scope: "col", text: t }); }))),
      h("tbody", null, hs.map(function (x, i) {
        return h("tr", { class: "hrow", onclick: function () { onHot(i); } },
          h("td", null, h("button", { type: "button", class: "hn", "aria-label": "Zoom to hotspot " + (i + 1), text: String(i + 1) })),
          h("td", { text: x.position }),
          h("td", { text: x.rect_px[2] + "×" + x.rect_px[3] }),
          h("td", { class: "num", text: pct(x.share_of_total_error) }),
          numCell(x.mean_flip),
          numCell(x.max_flip));
      }))));
  }

  function badges(e) {
    var p = e.properties;
    var b = h("div", { class: "badges" });
    if (frameWide(e)) b.appendChild(h("span", { class: "badge wide", title: "The largest hotspot covers at least half of the frame", text: "frame-wide change" }));
    if ((e.meta_diff || []).length) b.appendChild(h("span", { class: "badge cfg", text: "config differs" }));
    if (!p) { b.appendChild(h("span", { class: "badge", text: "no decodable " + LB.capture })); return b; }
    if (e.bit_identical === true) b.appendChild(h("span", { class: "badge ident", text: "bit-identical" }));
    else if (e.bit_identical === false) b.appendChild(h("span", { class: "badge", text: "not bit-identical" }));
    if (p.is_all_black) b.appendChild(h("span", { class: "badge warn", text: "ALL BLACK" }));
    if (p.is_all_white) b.appendChild(h("span", { class: "badge warn", text: "ALL WHITE" }));
    b.appendChild(h("span", { class: "badge", text: "lum min/mean/max " + p.min_luminance.toFixed(3) + " / " + p.mean_luminance.toFixed(3) + " / " + p.max_luminance.toFixed(3), title: "Image luminance (0 to 1) of the capture: minimum, mean, maximum. Not FLIP values." }));
    if (e.metrics) b.appendChild(h("span", { class: "badge", text: e.metrics.width + "×" + e.metrics.height }));
    return b;
  }

  // ---- compare stage: swipe / flicker with synced zoom and pan ---------------

  var cmps = [];          // live compare controllers, rebuilt on every render()
  var activeCmp = null;   // the one the keyboard shortcuts drive

  var pixelScript = null;
  function reportPixels() {
    if (!pixelScript) pixelScript = new Promise(function (resolve) {
      var script = document.createElement('script'); script.src = 'report-pixels.js';
      script.onload = resolve; script.onerror = resolve; document.head.appendChild(script);
    });
    return pixelScript;
  }

  function compare(e, sidePanes) {
    var hasHeat = !!e.paths.heatmap, hasSigned = !!e.paths.signed_diff;
    var tool = "pan", inspector = false, cursor = null, pixelData = null;
    var x = 50, k = 1, tx = 0, ty = 0, vertical = false, heat = false, applied = false;
    var ptrs = {}, gesture = null;

    var stage = h("div", { class: "stage", tabindex: "0", role: "group",
      "aria-label": "Comparison of " + LB.baseline + " and " + LB.capture + ". Drag to move the divider; arrow keys also work." });
    if (e.metrics && e.metrics.width && e.metrics.height) stage.style.setProperty("--ar", e.metrics.width / e.metrics.height);
    var base = h("img", { class: "base", src: url(e.paths.baseline), alt: LB.baseline, decoding: "async", draggable: "false" });
    base.addEventListener("load", function () {
      if (base.naturalWidth && base.naturalHeight) stage.style.setProperty("--ar", base.naturalWidth / base.naturalHeight);
      applyView();
      paint();
    });
    var capImg = h("img", { src: url(e.paths.capture), alt: LB.capture, decoding: "async", draggable: "false" });
    var heatImg = hasHeat || hasSigned ? h("img", { class: "hm", src: url(e.paths.heatmap || e.paths.signed_diff), alt: "FLIP heatmap", decoding: "async", draggable: "false" }) : null;
    var maskImg = e.paths.nonfinite_mask ? h("img", { class: "mask-img", src: url(e.paths.nonfinite_mask), alt: "Non-finite mask", hidden: true }) : null;
    var cap = h("div", { class: "cap" }, capImg, heatImg);
    var nfBoxes = (((e.diagnostics || {}).nonfinite || {}).clusters || []).map(function (c, i) {
      return h("div", { class: "nfb", hidden: true, title: c.pixels + " affected pixels" }, h("span", { text: "NF " + (i + 1) }));
    });
    var grip = h("i", { class: "grip" });
    var line = h("div", { class: "line" }, grip);
    var tagL = h("span", { class: "tag l", text: LB.baseline });
    var tagR = h("span", { class: "tag r", text: LB.capture });
    var roiEl = h("div", { class: "roi", hidden: true });
    stage.append(base, cap, line, tagL, tagR, roiEl);
    if (maskImg) stage.appendChild(maskImg);
    nfBoxes.forEach(function (b) { stage.appendChild(b); });
    var hots = hotspotsOf(e).map(function (hs, i) {
      var b = h("button", { type: "button", class: "hsp", title: "Hotspot " + (i + 1) + ": zoom to it", "aria-label": "Zoom to hotspot " + (i + 1) }, h("span", { text: String(i + 1) }));
      b.addEventListener("pointerdown", function (ev) { ev.stopPropagation(); });
      b.addEventListener("click", function () { zoomBox(i); });
      stage.appendChild(b);
      return b;
    });

    var slider = h("input", { type: "range", min: "0", max: "100", step: "0.1", value: "50", "aria-label": "Swipe position: " + LB.baseline + " left, " + LB.capture + " right" });
    var opa = h("input", { type: "range", min: "0", max: "1", step: "0.05", value: "0.6", hidden: true, class: "opa", "aria-label": "Overlay opacity" });
    stage.style.setProperty("--opa", "0.6");

    function setX(p, quiet) {
      x = Math.max(0, Math.min(100, Math.round(p * 10) / 10));
      stage.style.setProperty("--x", x + "%");
      slider.value = String(x);
      if (!quiet) { V.split = x / 100; notify(); }
    }
    setX(V.split * 100, true);
    slider.addEventListener("input", function () { setX(Number(slider.value)); });
    opa.addEventListener("input", function () { stage.style.setProperty("--opa", opa.value); if (layer === "signed") V.signed = Number(opa.value); else V.heat = Number(opa.value); notify(); });

    // ---- zoom and pan: one transform shared by every layer ----
    var zoomBtns = {};
    var ZS = [["fit", "Fit"], ["1", "1×"], ["2", "2×"], ["4", "4×"], ["8", "8×"]];
    function natW() { return base.naturalWidth || (e.metrics && e.metrics.width) || 1; }
    function natH() { return base.naturalHeight || (e.metrics && e.metrics.height) || 1; }
    function absK(z) { return Math.max(1, Number(z) * natW() / (stage.clientWidth || 1)); }
    // Applies the page-level zoom (V.zoom, V.at) to this stage.
    function applyView() {
      var w = stage.clientWidth, hh = stage.clientHeight;
      if (V.zoom === "fit" || !w) { k = 1; tx = 0; ty = 0; }
      else {
        k = absK(V.zoom);
        var at = V.at || [natW() / 2, natH() / 2];
        tx = w / 2 - at[0] / natW() * w * k; ty = hh / 2 - at[1] / natH() * hh * k;
      }
      applied = true;
      if (V.hotspotReq && hotspotsOf(e).length >= V.hotspotReq) { var n = V.hotspotReq; V.hotspotReq = null; zoomBox(n - 1); }
    }
    function paint() {
      if (V.layout === "side") { renderStatus(); return; }
      var w = stage.clientWidth, hh = stage.clientHeight;
      if (k <= 1.0001) { k = 1; tx = 0; ty = 0; }
      var t = UI.xf.clamp(tx, ty, w * k, hh * k, w, hh); tx = t[0]; ty = t[1];
      stage.style.setProperty("--t", "translate(" + tx + "px," + ty + "px) scale(" + k + ")");
      stage.classList.toggle("pix", k > 1.0001);
      hotspotsOf(e).forEach(function (hs, i) {
        var r = hs.rect_frac, st = hots[i].style;
        st.left = (tx + r[0] * w * k) + "px"; st.top = (ty + r[1] * hh * k) + "px";
        st.width = (r[2] * w * k) + "px"; st.height = (r[3] * hh * k) + "px";
      });
      nfBoxes.forEach(function (b, i) {
        var r = e.diagnostics.nonfinite.clusters[i].rect_px;
        b.hidden = !V.mask;
        b.style.left = (tx + r[0] / natW() * w * k) + "px"; b.style.top = (ty + r[1] / natH() * hh * k) + "px";
        b.style.width = r[2] / natW() * w * k + "px"; b.style.height = r[3] / natH() * hh * k + "px";
      });
      ZS.forEach(function (z) {
        var on = z[0] === "fit" ? k === 1 : k > 1 && Math.abs(k - absK(z[0])) < 1e-3;
        zoomBtns[z[0]].setAttribute("aria-pressed", on ? "true" : "false");
      });
      if (V.roi && w) {
        var q = V.roi, st2 = roiEl.style;
        roiEl.hidden = false;
        st2.left = (tx + q[0] / natW() * w * k) + "px"; st2.top = (ty + q[1] / natH() * hh * k) + "px";
        st2.width = (q[2] / natW() * w * k) + "px"; st2.height = (q[3] / natH() * hh * k) + "px";
      } else roiEl.hidden = true;
      if (applied && w) {
        V.tcur = k.toFixed(4) + "," + tx.toFixed(1) + "," + ty.toFixed(1);
        if (k <= 1.0001) { V.zoom = "fit"; V.at = null; }
        else {
          V.zoom = Math.round(k * w / natW() * 1000) / 1000;
          V.at = [Math.round((w / 2 - tx) / (w * k) * natW()), Math.round((hh / 2 - ty) / (hh * k) * natH())];
        }
        notify();
      }
      renderStatus();
    }
    function zoomAt(f, cx, cy) {
      var nk = Math.max(1, Math.min(256, k * f)), r = nk / k;
      var t = UI.xf.about(tx, ty, r, cx, cy); tx = t[0]; ty = t[1]; k = nk;
      paint();
    }
    // Zoom and centre the stage on hotspot i, leaving a margin around the box.
    function zoomBox(i) {
      var r = hotspotsOf(e)[i].rect_frac, w = stage.clientWidth, hh = stage.clientHeight;
      var bw = Math.max(r[2] * w, 8), bh = Math.max(r[3] * hh, 8);
      var nk = Math.max(1, Math.min(256, 0.8 * Math.min(w / bw, hh / bh)));
      k = nk;
      tx = w / 2 - (r[0] + r[2] / 2) * w * nk;
      ty = hh / 2 - (r[1] + r[3] / 2) * hh * nk;
      paint();
      V.hsel = i + 1; V.hselT = V.tcur;
    }
    function setZoom(z) {
      if (V.layout === "side") { V.zoom = z === "fit" ? "fit" : Number(z); V.at = null; paintSide(); renderStatus(); notify(); return; }
      if (z === "fit") { k = 1; paint(); return; }
      var nk = absK(z), r = nk / k, cx = stage.clientWidth / 2, cy = stage.clientHeight / 2;
      var t = UI.xf.about(tx, ty, r, cx, cy); tx = t[0]; ty = t[1]; k = nk;
      paint();
    }

    // ---- modes ----
    var mode = "swipe", paused = false, showCap = false, timer = null;
    var pauseBtn = h("button", { type: "button", class: "btn", hidden: true, text: "Pause" });
    var bSide = h("button", { type: "button", "aria-pressed": "false", "data-layout": "side", text: "Side", title: "Only the images side by side, no compare stage" });
    var bSwipe = h("button", { type: "button", "aria-pressed": "true", "data-layout": "swipe", text: "Swipe" });
    var bFlick = h("button", { type: "button", "aria-pressed": "false", "data-layout": "flicker", text: "Flicker", title: "Space" });
    var bDiff = hasSigned ? h("button", { type: "button", "data-layout": "diff", text: "Difference" }) : null;
    var bHeatL = hasHeat ? h("button", { type: "button", "aria-pressed": "false", "data-layout": "heatmap", text: "Heatmap", title: "The capture under its FLIP heatmap" }) : null;
    function paintFlick() {
      stage.classList.toggle("showcap", showCap);
      tagL.textContent = showCap ? LB.capture : LB.baseline;
    }
    function stop() {
      if (timer !== null) { clearInterval(timer); timers = timers.filter(function (t) { return t !== timer; }); timer = null; }
    }
    function start() {
      stop();
      timer = setInterval(function () { showCap = !showCap; paintFlick(); }, 500);
      timers.push(timer);
    }
    function setMode(m) {
      mode = m;
      stage.classList.toggle("flick", m === "flicker");
      slider.hidden = m === "flicker";
      pauseBtn.hidden = m !== "flicker";
      if (m === "flicker") {
        showCap = false; paintFlick();
        if (!paused) start();
      } else {
        stop();
        tagL.textContent = LB.baseline;
        stage.classList.remove("showcap");
      }
    }
    // side | swipe | flicker | heatmap: the page-level layout (V.layout).
    function paintSide() {
      var detail = root.closest(".d");
      if (!detail) return;
      detail.classList.toggle("zoom-fit", V.zoom === "fit");
      detail.style.setProperty("--pane-scale", V.zoom === "fit" ? 1 : V.zoom);
      ZS.forEach(function (z) { zoomBtns[z[0]].setAttribute("aria-pressed", String(String(V.zoom) === z[0])); });
    }
    function setLayout(l, quiet) {
      if ((l === "heatmap" && !hasHeat) || (l === "diff" && !hasSigned)) l = "swipe";
      V.layout = l;
      root.classList.toggle("side", l === "side");
      paintSide();
      setMode(l === "flicker" ? "flicker" : "swipe");
      if (l === "heatmap" || l === "diff") { setX(0, true); setLayer(l === "diff" ? "signed" : "heat", true); }
      else {
        setX(V.split * 100, true);
        setLayer(l === "swipe" ? (V.signed > 0 ? "signed" : V.heat > 0 ? "heat" : "none") : "none", true);
      }
      slider.hidden = l !== "swipe";
      vertBtn.hidden = l !== "swipe";
      if (heatBtn) heatBtn.hidden = l !== "swipe";
      if (signedBtn) signedBtn.hidden = l !== "swipe";
      line.hidden = l !== "swipe";
      renderStatus();
      [bSide, bSwipe, bFlick, bHeatL, bDiff].forEach(function (b) { if (b) b.setAttribute("aria-pressed", b.getAttribute("data-layout") === l ? "true" : "false"); });
      if (!quiet) notify();
    }
    [bSide, bSwipe, bFlick, bHeatL, bDiff].forEach(function (b) { if (b) b.addEventListener("click", function () { setLayout(b.getAttribute("data-layout")); }); });
    pauseBtn.addEventListener("click", function () {
      paused = !paused;
      pauseBtn.textContent = paused ? "Play" : "Pause";
      if (paused) stop(); else start();
    });

    var vertBtn = h("button", { type: "button", class: "btn small", "aria-pressed": "false", title: "Vertical / horizontal split (v)", text: "Vertical" });
    function setVertical(v, quiet) {
      vertical = v;
      if (!quiet) { V.vertical = v; notify(); }
      stage.classList.toggle("vert", v);
      vertBtn.setAttribute("aria-pressed", v ? "true" : "false");
    }
    vertBtn.addEventListener("click", function () { setVertical(!vertical); });

    var heatBtn = hasHeat ? h("button", { type: "button", class: "btn small", "aria-pressed": "false", title: "FLIP heatmap on the capture side (h)", text: "Heatmap" }) : null;
    var signedBtn = hasSigned ? h("button", { type: "button", class: "btn small", text: "Difference", "aria-pressed": "false" }) : null;
    var layer = "none";
    var legend = h("span", { class: "legend", hidden: true });
    var maskLegend = h("span", { class: "legend mask", hidden: !V.mask }, h("i", { class: "nan" }), "NaN", h("i", { class: "inf" }), "Inf", h("i", { class: "neg" }), "negative");
    function setLayer(kind, quiet) {
      if ((kind === "heat" && !hasHeat) || (kind === "signed" && !hasSigned)) return;
      layer = kind; heat = kind !== "none";
      if (heatImg) {
        heatImg.src = url(kind === "signed" ? e.paths.signed_diff : e.paths.heatmap || e.paths.signed_diff);
        heatImg.alt = kind === "signed" ? "Signed luminance difference" : "FLIP heatmap";
      }
      stage.classList.toggle("heat", heat);
      if (heatBtn) heatBtn.setAttribute("aria-pressed", String(kind === "heat"));
      if (signedBtn) signedBtn.setAttribute("aria-pressed", String(kind === "signed"));
      opa.hidden = !heat; legend.hidden = !heat;
      legend.className = "legend " + (kind === "signed" ? "signed" : "flip");
      var scale = e.diagnostics && e.diagnostics.signed ? UI.fmtScale(e.diagnostics.signed.scale) : "?";
      legend.replaceChildren(kind === "signed" ? "−" + scale + " darker" : "0", h("i"), kind === "signed" ? "brighter +" + scale : heatmapTop(e));
      if (!quiet) { V.heat = kind === "heat" ? Number(opa.value) : 0; V.signed = kind === "signed" ? Number(opa.value) : 0; notify(); }
    }
    function setHeat(on, quiet) { setLayer(on ? "heat" : "none", quiet); }
    if (heatBtn) heatBtn.addEventListener("click", function () { setHeat(layer !== "heat"); });
    if (signedBtn) signedBtn.addEventListener("click", function () { setLayer(layer === "signed" ? "none" : "signed"); });

    // ---- full screen: the Fullscreen API on the whole .cmp, with a CSS fallback ----
    var root;
    var fsBtn = h("button", { type: "button", class: "btn small", "aria-pressed": "false", title: "Full screen (f)", text: "Full screen" });
    var fsx;
    function fsActive() { return fsx.active(); }
    function setFullscreen(on) { fsx.set(on); }
    fsBtn.addEventListener("click", function () { fsx.toggle(); });
    window.addEventListener("resize", function () { if (root.isConnected) paint(); });

    // ---- pointer input: drag moves the divider; pan when zoomed; pinch zooms ----
    function nearLine(ev) {
      var rc = stage.getBoundingClientRect();
      var pos = vertical ? ev.clientY - rc.top : ev.clientX - rc.left;
      return Math.abs(pos - (vertical ? rc.height : rc.width) * x / 100) <= 28;
    }
    function xFrom(ev) {
      var rc = stage.getBoundingClientRect();
      setX(vertical ? (ev.clientY - rc.top) / rc.height * 100 : (ev.clientX - rc.left) / rc.width * 100);
    }
    function pinchDist() {
      var ks = Object.keys(ptrs), a = ptrs[ks[0]], b = ptrs[ks[1]];
      return Math.hypot(a.x - b.x, a.y - b.y) || 1;
    }
    stage.addEventListener("pointerdown", function (ev) {
      if (ev.pointerType === "mouse" && ev.button !== 0) return;
      ptrs[ev.pointerId] = { x: ev.clientX, y: ev.clientY };
      try { stage.setPointerCapture(ev.pointerId); } catch (err) { /* ignore */ }
      if (Object.keys(ptrs).length === 2) { gesture = { kind: "pinch", d: pinchDist() }; return; }
      if (ev.shiftKey || tool === "roi") { var q = pixelAt(ev); gesture = { kind: "roi", start: q }; return; }
      if (V.layout === "swipe" && (k === 1 || nearLine(ev))) { gesture = { kind: "divider" }; xFrom(ev); }
      else gesture = { kind: "pan", sx: ev.clientX, sy: ev.clientY, tx: tx, ty: ty };
      stage.classList.add("drag");
    });
    stage.addEventListener("pointermove", function (ev) {
      if (ptrs[ev.pointerId]) { ptrs[ev.pointerId].x = ev.clientX; ptrs[ev.pointerId].y = ev.clientY; }
      cursor = pixelAt(ev); loadPixels(); renderStatus();
      if (!gesture) return;
      if (gesture.kind === "pinch") {
        if (Object.keys(ptrs).length < 2) return;
        var ks = Object.keys(ptrs), a = ptrs[ks[0]], b = ptrs[ks[1]], rc = stage.getBoundingClientRect(), nd = pinchDist();
        zoomAt(nd / gesture.d, (a.x + b.x) / 2 - rc.left, (a.y + b.y) / 2 - rc.top);
        gesture.d = nd;
      } else if (gesture.kind === "roi") {
        var q = pixelAt(ev), a = gesture.start;
        V.roi = [Math.min(q[0], a[0]), Math.min(q[1], a[1]), Math.abs(q[0] - a[0]) + 1, Math.abs(q[1] - a[1]) + 1]; paint();
      } else if (gesture.kind === "divider") xFrom(ev);
      else { tx = gesture.tx + ev.clientX - gesture.sx; ty = gesture.ty + ev.clientY - gesture.sy; paint(); }
    });
    function up(ev) {
      delete ptrs[ev.pointerId];
      if (gesture && gesture.kind === "pinch" && Object.keys(ptrs).length >= 1) { gesture = null; return; }
      gesture = null;
      stage.classList.remove("drag");
    }
    stage.addEventListener("pointerup", up);
    stage.addEventListener("pointercancel", up);
    // Plain wheel scrolls the page until the stage is zoomed; Ctrl/Cmd+wheel (and trackpad pinch) always zooms.
    stage.addEventListener("wheel", function (ev) {
      if (!(ev.ctrlKey || ev.metaKey || k > 1.0001)) return;
      ev.preventDefault();
      var rc = stage.getBoundingClientRect();
      zoomAt(UI.xf.wheel(ev, ev.ctrlKey ? 0.01 : 0.0015), ev.clientX - rc.left, ev.clientY - rc.top);
    }, { passive: false });

    var chanSel = h("select", { "aria-label": "Display channel", title: "Machado 2009, severity 1.0. PRE-CHECK display aid, not certification or formal compliance." }, [["rgb", "RGB"], ["r", "R"], ["g", "G"], ["b", "B"], ["luma", "Luma"]].concat(UI.cvdChoices).map(function (o) { return h("option", { value: o[0], text: o[1] }); }));
    chanSel.value = V.channel;
    chanSel.addEventListener("change", function () { V.channel = chanSel.value; applyFilter(); notify(); });
    var evInp = h("input", { type: "range", min: "-4", max: "4", step: "0.1", value: String(V.ev), class: "evr", "aria-label": "Exposure in stops", title: "Exposure (EV)" });
    evInp.addEventListener("input", function () { V.ev = Number(evInp.value); applyFilter(); notify(); });
    var zoomSeg = h("div", { class: "seg", role: "group", "aria-label": "Compare zoom" }, ZS.map(function (z) {
      var b = h("button", { type: "button", "data-cz": z[0], "aria-pressed": z[0] === "fit" ? "true" : "false", text: z[1], title: z[0] === "fit" ? "Fit (0)" : "Zoom " + z[1] + " (" + z[0] + ")" });
      b.addEventListener("click", function () { setZoom(z[0]); });
      zoomBtns[z[0]] = b;
      return b;
    }));

    var conInp = h("input", { type: "range", min: ".5", max: "4", step: ".05", value: String(V.contrast), "aria-label": "Display contrast", oninput: function () { V.contrast = Number(conInp.value); applyFilter(); notify(); } });
    function popover(title, children) {
      var btn = h("button", { type: "button", class: "btn", text: title });
      var panel = h("div", { class: "pop", role: "group", "aria-label": title }, h("h3", { text: title }), children);
      var wrap = h("div", { class: "pop-wrap" }, btn, panel);
      var ctl = UI.popover(wrap, btn, panel); wrap.openPanel = function () { ctl.open(true); }; return wrap;
    }
    function row(label, control) { return h("div", { class: "row" }, h("span", { class: "lbl", text: label }), control); }
    var display = popover("Display", [row("Channel", chanSel), row("Exposure", evInp), row("Contrast", conInp),
      h("button", { type: "button", class: "btn small", text: "Reset", onclick: function () { V.ev = 0; V.contrast = 1; V.channel = "rgb"; evInp.value = "0"; conInp.value = "1"; chanSel.value = "rgb"; applyFilter(); notify(); } })]);
    function setTool(v) { tool = v; roiBtn.setAttribute("aria-pressed", String(v === "roi")); }
    var roiBtn = h("button", { type: "button", class: "btn small", text: "Draw region", "aria-pressed": "false", onclick: function () { setTool(tool === "roi" ? "pan" : "roi"); } });
    var inspectBtn = h("button", { type: "button", class: "btn small", text: "Inspector", "aria-pressed": "false", onclick: function () { inspector = !inspector; inspectBtn.setAttribute("aria-pressed", String(inspector)); loadPixels(); renderStatus(); } });
    var inspect = h("div", { class: "inspect", hidden: true });
    function hold(on) { stage.classList.toggle("held", on); }
    var holdBtn = h("button", { type: "button", class: "btn small", text: "Hold to compare" });
    holdBtn.addEventListener("pointerdown", function () { hold(true); });
    ["pointerup", "pointercancel", "pointerleave", "blur"].forEach(function (n) { holdBtn.addEventListener(n, function () { hold(false); }); });
    holdBtn.addEventListener("keydown", function (ev) { if (ev.key === "Enter") hold(true); });
    holdBtn.addEventListener("keyup", function () { hold(false); });
    var tools = popover("Tools", [row("Region", roiBtn), row("Pixels", inspectBtn), row("Reference", holdBtn),
      row("Share", h("span", null, h("button", { type: "button", class: "btn small", text: "Copy link", onclick: function () { A.copyLink(agent); } }),
      h("button", { type: "button", class: "btn small", text: "Snapshot", onclick: snapshot }))),
      h("button", { type: "button", class: "btn small", text: "Clear region", onclick: function () { V.roi = null; paint(); } })]);
    var hotBtn = h("button", { type: "button", class: "btn small", text: "Hotspots", "aria-pressed": String(state.hotspots), onclick: toggleHotspots });
    function toggleHotspots() { state.hotspots = !state.hotspots; document.querySelectorAll(".d").forEach(function (d) { d.classList.toggle("hots", state.hotspots); }); hotBtn.setAttribute("aria-pressed", String(state.hotspots)); }
    var maskBtn = maskImg ? h("button", { type: "button", class: "btn small", text: "Non-finite", "aria-pressed": String(V.mask), onclick: toggleMask }) : null;
    function toggleMask() { if (!maskImg) return; V.mask = !V.mask; root.classList.toggle("nfmask", V.mask); maskLegend.hidden = !V.mask; notify(); if (maskImg) maskImg.hidden = !V.mask; if (maskBtn) maskBtn.setAttribute("aria-pressed", String(V.mask)); paint(); }
    var status = h("div", { class: "sbar", "aria-label": "Pixel readout" });
    root = h("div", { class: "cmp" + (V.mask ? " nfmask" : "") }, h("div", { class: "cmp-cap", role: "toolbar", "aria-label": "Comparison controls" },
      h("div", { class: "seg", role: "group", "aria-label": "Compare mode" }, bSide, bSwipe, bFlick, bHeatL, bDiff), zoomSeg,
      slider, vertBtn, heatBtn, signedBtn, opa, pauseBtn, h("span", { class: "grow" }), hotBtn, maskBtn, display, tools, fsBtn),
      h("div", { class: "legends" }, legend, maskImg ? maskLegend : null), h("div", { class: "cmp-wrap" }, stage), sidePanes, status, inspect);
    fsx = UI.fullscreen(root, function (on) { fsBtn.setAttribute("aria-pressed", String(on)); fsBtn.textContent = on ? "Exit full screen" : "Full screen"; paint(); });
    function pixelAt(ev) {
      var rc = stage.getBoundingClientRect();
      return [Math.max(0, Math.min(natW() - 1, Math.floor((ev.clientX - rc.left - tx) / (rc.width * k) * natW()))), Math.max(0, Math.min(natH() - 1, Math.floor((ev.clientY - rc.top - ty) / (rc.height * k) * natH())))];
    }
    var loadingPixels = false;
    function loadPixels() {
      if (pixelData || loadingPixels) return;
      loadingPixels = true;
      reportPixels().then(function () {
      var data = window.__saccadeReportPixels || {};
      return Promise.all([base, capImg, heatImg].map(function (im) {
        if (!im) return null;
        var src = data[e.name] && data[e.name][im === base ? 0 : im === capImg ? 1 : 2];
        var raw = new Image();
        if (im === heatImg && !e.paths.heatmap) return null;
        raw.src = src || (im === heatImg ? url(e.paths.heatmap) : im.src);
        return raw.decode().then(function () { var c = document.createElement("canvas"); c.width = natW(); c.height = natH(); var g = c.getContext("2d", { willReadFrequently: true }); g.drawImage(raw, 0, 0, c.width, c.height); return g.getImageData(0, 0, c.width, c.height); }).catch(function () { return null; });
      }));
      }).then(function (ps) { pixelData = ps; renderStatus(); });
    }
    function renderStatus() {
      if (!status) return;
      function item(key, value) { return h("span", { class: "si" }, h("span", { class: "k", text: key }), h("b", { text: value })); }
      var rgb = ["–", "–"], flip = "–";
      if (cursor && pixelData) {
        var ix = (cursor[1] * natW() + cursor[0]) * 4;
        [0, 1].forEach(function (i) { if (pixelData[i]) rgb[i] = UI.rgbText(pixelData[i].data.slice(ix, ix + 3)); });
        if (pixelData[2]) { var d = pixelData[2].data; flip = "≈" + UI.magmaValue(d[ix], d[ix + 1], d[ix + 2]).toFixed(3); }
      }
      status.replaceChildren(item("Pixel", cursor ? cursor.join(", ") : "move over the image"), item(LB.baseline, rgb[0]), item(LB.capture, rgb[1]), item("FLIP", flip), h("span", { class: "grow" }), item("Zoom", V.zoom === "fit" ? "Fit" : V.zoom + "×"), item("Pair", LB.baseline + " / " + LB.capture), item("Layout", V.layout === "diff" ? "signed difference" : V.layout));
      inspect.hidden = !inspector; if (inspector) inspect.textContent = (cursor ? "Pixel " + cursor.join(", ") : "Move over the image") + " · " + LB.baseline + ": " + rgb[0] + " · " + LB.capture + ": " + rgb[1] + " · FLIP " + flip + (V.roi ? " · ROI " + V.roi.join(", ") : "");
    }
    function snapshot() { activeCmp = api; V.entry = e.name; agent.snapshot().then(function (u) { UI.download("saccade-snapshot.png", u); UI.toast("Snapshot downloaded"); }).catch(function (err) { UI.toast(err.message); }); }

    // The stage as a canvas: both images with the current transform, the split, heatmap, filter, boxes and labels.
    function toCanvas() {
      if (V.layout === "side") {
        var pictures = Array.prototype.filter.call(sidePanes.querySelectorAll("img"), function (im) { return getComputedStyle(im).display !== "none"; });
        return Promise.all(pictures.map(function (im) { return im.decode().catch(function () {}); })).then(function () {
          var c = document.createElement("canvas"), r = sidePanes.getBoundingClientRect();
          c.width = Math.max(1, r.width); c.height = Math.max(1, r.height);
          var g = c.getContext("2d");
          pictures.forEach(function (im, i) {
            var ir = im.getBoundingClientRect(), vr = im.closest(".vp").getBoundingClientRect();
            g.save(); g.beginPath(); g.rect(vr.left - r.left, vr.top - r.top, vr.width, vr.height); g.clip();
            if (im.classList.contains("side-mask")) g.globalCompositeOperation = "screen";
            g.filter = im.classList.contains("side-mask") || im.alt === "heatmap" ? "none" : UI.filterCss(V.ev, V.contrast, V.channel);
            g.drawImage(im, ir.left - r.left, ir.top - r.top, ir.width, ir.height);
            if (!im.classList.contains("side-mask")) {
              g.filter = "none";
              im.parentNode.querySelectorAll(".hsp, .rgn, .side-nfb").forEach(function (box) {
                if (getComputedStyle(box).display === "none") return;
                var br = box.getBoundingClientRect();
                g.strokeStyle = box.classList.contains("side-nfb") ? "#ff00ff" : box.classList.contains("rgn") ? "#ffd400" : "#25d3ee";
                g.lineWidth = 2; g.strokeRect(br.left - r.left, br.top - r.top, br.width, br.height);
              });
            }
            g.restore();
            g.fillStyle = "#596171"; g.font = "12px sans-serif";
            if (!im.classList.contains("side-mask")) g.fillText(im.alt, vr.left - r.left + 4, vr.top - r.top - 5);
          });
          return c;
        });
      }
      var w = stage.clientWidth, hh = stage.clientHeight;
      var imgs = [base, capImg].concat(heatImg && heat ? [heatImg] : [], maskImg && V.mask ? [maskImg] : []);
      return Promise.all(imgs.map(function (im) { return im.decode ? im.decode().catch(function () { /* drawn blank */ }) : null; })).then(function () {
        var c = document.createElement("canvas");
        c.width = Math.max(1, w); c.height = Math.max(1, hh);
        var g = c.getContext("2d");
        g.imageSmoothingEnabled = !(k > 1.0001);
        var f = getComputedStyle(document.body).getPropertyValue("--f").trim();
        var clip = vertical ? [0, hh * x / 100, w, hh] : [w * x / 100, 0, w, hh];
        function draw(img, alpha, clipped) {
          g.save();
          if (clipped) { g.beginPath(); g.rect(clip[0], clip[1], clip[2], clip[3]); g.clip(); }
          g.globalAlpha = alpha; if (img === maskImg) g.globalCompositeOperation = "screen"; g.filter = img === heatImg || img === maskImg ? "none" : f && f !== "none" ? f : "none";
          g.drawImage(img, tx, ty, w * k, hh * k);
          g.restore();
        }
        if (mode === "flicker") draw(showCap ? capImg : base, 1, false);
        else {
          draw(base, 1, false); draw(capImg, 1, true);
          if (heatImg && stage.classList.contains("heat")) draw(heatImg, Number(opa.value), true);
          g.fillStyle = "#fff";
          if (vertical) g.fillRect(0, hh * x / 100 - 1, w, 2); else g.fillRect(w * x / 100 - 1, 0, 2, hh);
        }
        if (maskImg && V.mask) draw(maskImg, .8, false);
        Array.prototype.forEach.call(stage.querySelectorAll(".hsp, .nfb, .roi, .tag"), function (b) {
          if (b.hidden || !b.getClientRects().length || getComputedStyle(b).display === "none" || b.classList.contains("r") && mode === "flicker") return;
          var br = b.getBoundingClientRect(), sr = stage.getBoundingClientRect();
          var bx = br.left - sr.left, by = br.top - sr.top;
          if (b.classList.contains("tag")) {
            g.fillStyle = "rgba(0,0,0,0.7)"; g.fillRect(bx, by, br.width, br.height);
            g.fillStyle = "#fff"; g.font = "12px sans-serif"; g.fillText(b.textContent, bx + 5, by + br.height * 0.72);
          } else {
            g.lineWidth = 2; g.strokeStyle = b.classList.contains("roi") ? "#00c8ff" : b.classList.contains("nfb") ? "#ff00ff" : "#25d3ee";
            g.strokeRect(bx + 1, by + 1, br.width - 2, br.height - 2);
          }
        });
        return c;
      });
    }

    var api = {
      root: root,
      canvas: toCanvas,
      refresh: function () { applyView(); paintSide(); paint(); },
      entry: e.name,
      inspectAt: function (pos) { cursor = pos; loadPixels(); renderStatus(); },
      nudge: function (d) { if (mode === "swipe") setX(x + d); },
      flicker: function () { setLayout(V.layout === "flicker" ? "swipe" : "flicker"); },
      layout: setLayout, layer: setLayer, mask: toggleMask, hot: toggleHotspots, hold: hold,
      display: display.openPanel, tools: tools.openPanel, resetDisplay: function () { display.querySelector("button.btn.small").click(); }, contrast: function (d) { V.contrast = Math.max(.5, Math.min(4, V.contrast + d)); conInp.value = String(V.contrast); applyFilter(); notify(); }, pause: function () { if (mode === "flicker") pauseBtn.click(); }, clearRoi: function () { V.roi = null; paint(); }, roi: function () { roiBtn.click(); }, inspector: function () { inspectBtn.click(); }, snapshot: snapshot,
      vertical: function () { setVertical(!vertical); },
      heat: function () { setHeat(!heat); },
      zoom: setZoom,
      zoomBox: zoomBox,
      fullscreen: function () { setFullscreen(!fsActive()); },
      exitFullscreen: function () { if (fsActive()) setFullscreen(false); },
      isFullscreen: fsActive
    };
    root.cmpApi = api;
    cmps.push(api);
    if (V.vertical) setVertical(true, true);
    if (maskImg) maskImg.hidden = !V.mask;
    if (V.heat > 0 && hasHeat) { opa.value = String(V.heat); stage.style.setProperty("--opa", String(V.heat)); setHeat(true, true); }
    setLayout(V.layout, true);
    function activate() { activeCmp = api; V.entry = e.name; notify(); }
    root.addEventListener("pointerenter", activate);
    root.addEventListener("focusin", activate);
    root.addEventListener("pointerdown", activate);
    return root;
  }

  // The compare the keyboard drives: the last one touched, else the first on screen.
  function currentCmp() {
    var live = cmps.filter(function (c) { return c.root.isConnected; });
    if (activeCmp && live.indexOf(activeCmp) >= 0) return activeCmp;
    var vh = window.innerHeight;
    for (var i = 0; i < live.length; i++) {
      var r = live[i].root.getBoundingClientRect();
      if (r.bottom > 0 && r.top < vh) return live[i];
    }
    return live[0] || null;
  }

  function toggleHelp(force) { UI.help.toggle(reg, [["Drag", "Move divider or pan when zoomed"], ["Shift-drag", "Draw a region"], ["Ctrl + wheel", "Zoom at the pointer"]], "Keys drive the comparison you last touched.", force); }
  function defineActions() {
    function add(id, title, keys, run, group) { reg.add({ id: id, title: title, keys: keys || [], group: group || "Tools", run: run }); }
    function cmp(id, title, keys, fn, group) { reg.add({ id: id, title: title, keys: keys || [], group: group || "Compare", repeat: id === "swipe.left" || id === "swipe.right", enabled: function () { return !!currentCmp(); }, run: function (ev) { var c = currentCmp(); if (c) fn(c, ev || {}); } }); }
    ["side", "swipe", "flicker", "heatmap", "diff"].forEach(function (l) { cmp("layout." + l, "Layout: " + (l === "diff" ? "signed difference" : l), [], function (c) { c.layout(l); }, "Layout"); });
    ["fit", "1", "2", "4", "8"].forEach(function (z) { cmp("zoom." + z, "Zoom: " + z, [z === "fit" ? "0" : z], function (c) { c.zoom(z); }, "Zoom"); });
    cmp("swipe.left", "Divider left", ["ArrowLeft"], function (c, ev) { c.nudge(ev.shiftKey ? -1 : -5); });
    cmp("swipe.right", "Divider right", ["ArrowRight"], function (c, ev) { c.nudge(ev.shiftKey ? 1 : 5); });
    cmp("flicker", "Toggle flicker", [" "], function (c) { c.flicker(); });
    cmp("vertical", "Vertical / horizontal split", ["v"], function (c) { c.vertical(); });
    cmp("heat", "Swipe heatmap layer", ["h"], function (c) { c.heat(); });
    cmp("signed", "Swipe signed difference layer", ["d"], function (c) { c.layer(V.signed ? "none" : "signed"); });
    cmp("mask", "Non-finite mask and cluster boxes", ["m"], function (c) { c.mask(); });
    cmp("hot", "Toggle hotspot boxes", [], function (c) { c.hot(); });
    cmp("fullscreen", "Full screen", ["f"], function (c) { c.fullscreen(); });
    cmp("display", "Display settings", [], function (c) { c.display(); });
    cmp("tools", "Tools panel", [], function (c) { c.tools(); });
    cmp("roi", "Draw a region of interest", [], function (c) { c.roi(); });
    cmp("inspector", "Pixel inspector", [], function (c) { c.inspector(); });
    reg.add({ id: "hold", title: "Hold to compare", group: "Tools", keys: ["c"], hold: { down: function () { var c = currentCmp(); if (c) c.hold(true); }, up: function () { var c = currentCmp(); if (c) c.hold(false); } } });
    window.addEventListener("blur", function () { var c = currentCmp(); if (c) c.hold(false); });
    cmp("snapshot", "Snapshot: download PNG", [], function (c) { c.snapshot(); });
    ["rgb", "r", "g", "b", "luma", "protan", "deutan", "tritan"].forEach(function (ch) { add("channel." + ch, "Channel: " + ch, [], function () { agent.set({ channel: ch }); }, "Display"); });
    [-1, 0, 1].forEach(function (ev) { add("exposure." + ev, "Exposure: " + ev + " EV", [], function () { agent.set({ ev: ev }); }, "Display"); });
    cmp("display.reset", "Reset display", [], function (c) { c.resetDisplay(); }, "Display");
    cmp("contrast.up", "Contrast: increase", [], function (c) { c.contrast(.1); }, "Display");
    cmp("contrast.down", "Contrast: decrease", [], function (c) { c.contrast(-.1); }, "Display");
    cmp("roi.clear", "Clear the region of interest", [], function (c) { c.clearRoi(); });
    cmp("flicker.pause", "Pause or resume flicker", [], function (c) { c.pause(); });
    cmp("swipe.plain", "Swipe: plain layer", [], function (c) { c.layer("none"); });
    add("nav.prev", "Previous set", ["["], function () { agentStep(-1); }, "Navigate");
    add("nav.next", "Next set", ["]"], function () { agentStep(1); }, "Navigate");
    reg.dynamic(function () { return entries.map(function (e) { return { id: "go." + e.name, title: e.name, group: "Go to", run: function () { agent.set({ entry: e.name }); } }; }); });
    ["accept", "reject", "needs-work"].forEach(function (v) { add("decision." + v, "Decision: " + v, [], function () { var c = currentCmp(); if (c) setVerdict(c.entry, v); }, "Decision"); });
    ["y", "n"].forEach(function (key) { reg.add({ id: "proposal." + key, title: key === "y" ? "Confirm proposed decision" : "Override proposed decision", keys: [key], group: "Decision", enabled: function () { return !!proposalFor(V.entry); }, run: function () { var ap = proposalFor(V.entry); if (ap) setVerdict(V.entry, (key === "y") === (ap.answer === "accept") ? "accept" : "reject"); } }); });
    add("copy", "Copy link to this view", [], function () { A.copyLink(agent); });
    add("export", "Export decisions", [], exportDecisions);
    add("filter", "Toggle failures only", [], function () { state.filter = state.filter === "all" ? "issues" : "all"; render(); }, "Navigate");
    add("help", "Keyboard shortcuts", ["?"], function () { toggleHelp(); });
    add("escape", "Close panel or leave full screen", ["Escape"], function () { if (!UI.escape()) { var c = currentCmp(); if (c) c.exitFullscreen(); } });
  }

  function metricsGrid(e) {
    var m = e.metrics;
    if (!m) return null;
    var items = [
      ["mean", fmt(m.mean)], ["p50", fmt(m.p50)], ["p95", fmt(m.p95)], ["p99", fmt(m.p99)], ["max", fmt(m.max)],
      ["> 0.1", (m.frac_above_0_1 * 100).toFixed(2) + "%"], ["> 0.5", (m.frac_above_0_5 * 100).toFixed(2) + "%"]
    ];
    return h("dl", { class: "mx" }, items.map(function (kv) {
      return h("div", null, h("dt", { text: kv[0] }), h("dd", { text: String(kv[1]) }));
    }));
  }

  function renderDetail(e) {
    var d = h("div", { class: "d zoom-fit" + (state.regions ? " regions" : "") });
    var cmpApi = null;
    function goHot(i) {
      if (!cmpApi) return;
      cmpApi.zoomBox(i);
      cmpApi.root.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
    d.className += state.hotspots ? " hots" : "";
    var diag = UI.diagnosticsEl(e.diagnostics, { label: e.status }); if (diag) d.appendChild(diag);
    d.appendChild(decisionBar(e));
    var wb = warnBox(e);
    if (wb) d.appendChild(wb);
    var mt = metaTable(e);
    if (mt) d.appendChild(mt);
    d.appendChild(badges(e));
    if (e.error) d.appendChild(h("div", { class: "err-box", role: "alert", text: e.error }));
    var p = e.paths || {};
    var sidePanes = h("div", { class: "panes" },
      pane(e, LB.baseline, p.baseline, null, goHot),
      pane(e, LB.capture, p.capture, null, goHot),
      pane(e, "heatmap", p.heatmap, h("span", { class: "legend flip", title: "Error, 0 (dark) to " + heatmapTop(e) + " (light)" }, "0", h("i"), heatmapTop(e)), goHot));
    if (p.baseline && p.capture) { var cr = compare(e, sidePanes); cmpApi = cr.cmpApi; d.appendChild(cr);
      d.querySelectorAll(".panes .zbox").forEach(function (box) { box.addEventListener("pointermove", function (ev) { var r = box.getBoundingClientRect(); var im = box.querySelector("img"); cmpApi.inspectAt([Math.floor((ev.clientX - r.left) / r.width * im.naturalWidth), Math.floor((ev.clientY - r.top) / r.height * im.naturalHeight)]); }); }); }
    if (!cmpApi) d.appendChild(sidePanes);
    var ht = hotspotTable(e, goHot);
    if (ht) d.appendChild(ht);
    var mg = metricsGrid(e);
    if (mg) d.appendChild(mg);
    var rt = regionTable(e);
    if (rt) d.appendChild(rt);
    var tr = h("tr", { class: "detail" }, h("td", { colspan: String(innerWidth <= 720 ? COLUMNS.filter(function (c) { return !c.sec; }).length : COLUMNS.length) }, d));
    return tr;
  }

  // ---- decisions: what a person decided, and what an agent proposed ---------

  var dec = {}, props = {};
  var LS_KEY = "saccade-report.v1:" + report.generated_at_unix + ":" + entries.length;
  function loadDecisions() {
    try { var o = JSON.parse(localStorage.getItem(LS_KEY) || "null"); if (o && o.dec) dec = o.dec; } catch (err) { /* no storage: decisions last for the visit */ }
    var side = A.sidecar();
    Object.keys(side).forEach(function (name) {
      props[name] = side[name].proposals || [];
      if (side[name].decision && !(dec[name] && dec[name].decision)) dec[name] = { decision: side[name].decision, note: side[name].note || "", timestamp_ms: side[name].timestamp_ms || 0 };
    });
  }
  function setVerdict(name, v) {
    dec[name] = { decision: v, note: (dec[name] && dec[name].note) || "", timestamp_ms: Date.now() };
    try { localStorage.setItem(LS_KEY, JSON.stringify({ dec: dec })); } catch (err) { /* ignore */ }
    render();
  }
  // The decisions file, as `saccade approve --decisions` reads it; proposals travel along.
  function reviewerExposure() {
    var v = document.getElementById('reviewer-context').value;
    return { mapping_access: true ? {availability:'available',value:true} : {availability:'missing',reason:'mapping exposure not declared'}, implementation_context: v === 'unknown' ? {availability:'missing',reason:'implementation exposure not declared'} : {availability:'available',value:v === 'true'}, reviewer: null };
  }
  function buildDecisions() {
    var dirs = report.baseline_dir && report.capture_dir ? [report.baseline_dir, report.capture_dir] : [];
    return {
      reviewer_exposure: reviewerExposure(), saw_model_proposals: {availability:'available',value:Object.keys(props).some(function(k){return props[k].length > 0;})}, schema: "saccade-decisions.v1", seed: 0, labels: [LB.baseline, LB.capture], blind: false, dirs: dirs,
      sets: entries.filter(function (e) { return dec[e.name] || (props[e.name] || []).length; }).map(function (e) {
        var d = dec[e.name] || {};
        var o = { name: e.name, decision: d.decision || null, chosen_label: null, no_difference: false, note: d.note || "", roi: null, timestamp_ms: d.timestamp_ms || 0, chosen_dir: null, sha256: dirs.length ? [e.baseline_sha256 || null, e.capture_sha256 || null] : [] };
        if ((props[e.name] || []).length) o.proposals = props[e.name];
        return o;
      })
    };
  }
  function exportDecisions() { UI.downloadJson("saccade-decisions.v1.json", buildDecisions()); }
  function decisionBar(e) {
    var cur = (dec[e.name] || {}).decision || null;
    var list = props[e.name] || [];
    var bar = A.proposalBar(list, cur, function (v) { setVerdict(e.name, v); }).el;
    var mk = function (v, label) {
      return h("button", { type: "button", class: "btn small", "aria-pressed": cur === v ? "true" : "false", text: label, onclick: function () { setVerdict(e.name, cur === v ? null : v); } });
    };
    var row = h("div", { class: "d-bar decbar" }, h("span", { class: "lbl", text: "Decision" }), mk("accept", "Accept"), mk("reject", "Reject"), mk("needs-work", "Needs work"));
    return h("div", { class: "decwrap" }, list.length ? bar : null, row);
  }
  function proposalFor(name) {
    var ap = name ? A.acceptProposal(props[name]) : null;
    return ap && (ap.answer === "accept" || ap.answer === "reject") ? ap : null;
  }

  // ---- window.saccade and the URL hash --------------------------------------

  function shownEntries() {
    var shown = entries.filter(function (e) { return state.filter === "all" || e.status !== "pass"; });
    return sorted(shown);
  }
  function activeCmpFor(name) {
    var live = cmps.filter(function (c) { return c.root.isConnected && c.entry === name; });
    return live[0] || null;
  }
  function agentGet() {
    return {
      entry: V.entry, layout: V.layout, split: V.split, vertical: V.vertical, zoom: V.zoom, at: V.zoom === "fit" ? null : V.at,
      heat: V.heat, signed: V.signed, mask: V.mask, contrast: V.contrast, channel: V.channel, ev: V.ev, roi: V.roi,
      hotspot: V.hsel && V.hselT === V.tcur ? V.hsel : null
    };
  }
  function agentApply(p) {
    ["layout", "split", "vertical", "zoom", "heat", "signed", "mask", "contrast", "channel", "ev", "roi"].forEach(function (key) { if (key in p) V[key] = p[key]; });
    if ("at" in p) V.at = p.at;
    if (p.zoom === "fit") V.at = null;
    V.hotspotReq = p.hotspot || null;
    if (p.hotspot === null) { V.hsel = null; V.hotspotReq = null; }
    if (p.entry != null && entries.some(function (e) { return e.name === p.entry; })) {
      state.open = {}; state.open[p.entry] = true; V.entry = p.entry;
      if (state.filter === "issues" && entries.some(function (e) { return e.name === p.entry && e.status === "pass"; })) state.filter = "all";
    }
    applyFilter();
    render();
    var row = V.entry && document.querySelector("tr.row.open");
    if (row && row.scrollIntoView) row.scrollIntoView({ block: "nearest" });
  }
  function agentStep(d) {
    var list = shownEntries().map(function (e) { return e.name; });
    if (!list.length) return false;
    var i = list.indexOf(V.entry), q = i < 0 ? (d > 0 ? 0 : list.length - 1) : i + d;
    if (q < 0 || q >= list.length) return false;
    agentApply({ entry: list[q] });
    return true;
  }
  function agentCanvas() {
    var c = V.entry ? activeCmpFor(V.entry) : null;
    if (!c) return Promise.reject(new Error("no compare stage is open: call saccade.set({entry: name}) first (the entry needs both a baseline and a capture image)"));
    return c.canvas();
  }

  // ---- wiring ----------------------------------------------------------------

  document.getElementById("f-issues").addEventListener("click", function () { state.filter = "issues"; render(); });
  document.getElementById("f-all").addEventListener("click", function () { state.filter = "all"; render(); });
  window.addEventListener("resize", function () {
    document.querySelectorAll(".d").forEach(markUpscaled);
    document.querySelectorAll("tr.detail > td").forEach(function (td) { td.colSpan = innerWidth <= 720 ? COLUMNS.filter(function (c) { return !c.sec; }).length : COLUMNS.length; });
  });

  loadDecisions();
  var init = A.parseHash(location.hash, "entry");
  ["layout", "split", "vertical", "zoom", "at", "heat", "signed", "mask", "contrast", "channel", "ev", "roi"].forEach(function (key) { if (key in init) V[key] = init[key]; });
  if (init.entry && entries.some(function (e) { return e.name === init.entry; })) {
    V.entry = init.entry; state.open[init.entry] = true;
    if (entries.some(function (e) { return e.name === init.entry && e.status === "pass"; })) state.filter = "all";
  }
  if (init.hotspot) V.hotspotReq = init.hotspot;
  applyFilter();
  if (report.perf_diff || (report.perf_errors || []).length) {
    var perfHost = document.createElement("section"); document.querySelector(".review-summary").after(perfHost);
    window.saccadePerf.diff(perfHost, report.perf_diff, report.combined_verdict);
    (report.perf_errors || []).forEach(function(e) {var p=document.createElement("p");p.textContent=e.path+": "+e.message;perfHost.appendChild(p);});
  }
  renderHeader();
  render();
  agent = A.create({
    nameKey: "entry", blind: false, defaults: { split: 0.5 },
    get: agentGet, apply: agentApply, step: agentStep, canvas: agentCanvas,
    names: function () { return entries.map(function (e) { return { name: e.name, status: e.status, value: e.value }; }); }
  });
  A.bindCopy(document.getElementById("copy-link"), agent);
  document.getElementById("export").addEventListener("click", exportDecisions);
  defineActions();
  document.getElementById("palette-btn").addEventListener("click", function () { UI.palette.open(); });
  agent.applyHash();
})();
