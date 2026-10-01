(function () {
  "use strict";

  var report = JSON.parse(document.getElementById("flipdiff-data").textContent);
  var entries = report.entries || [];
  var LB = ((report.config || {}).labels) || { baseline: "baseline", capture: "capture" };
  var IDENTITY = (report.config || {}).mode === "identity";

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

  // Element builder. Children that are strings become text nodes, so report
  // content is never interpreted as HTML.
  function h(tag, attrs) {
    var el = document.createElement(tag);
    if (attrs) {
      Object.keys(attrs).forEach(function (k) {
        var v = attrs[k];
        if (v === null || v === undefined || v === false) return;
        if (k === "class") el.className = v;
        else if (k === "text") el.textContent = v;
        else if (k.indexOf("on") === 0) el.addEventListener(k.slice(2), v);
        else el.setAttribute(k, v === true ? "" : v);
      });
    }
    (function add(list) {
      for (var i = 0; i < list.length; i++) {
        var c = list[i];
        if (c === null || c === undefined || c === false) continue;
        if (Array.isArray(c)) add(c);
        else el.appendChild(typeof c === "string" ? document.createTextNode(c) : c);
      }
    })(Array.prototype.slice.call(arguments, 2));
    return el;
  }

  function fmt(v) {
    if (v === null || v === undefined || isNaN(v)) return null;
    if (v === 0) return "0";
    return v >= 0.001 ? v.toFixed(4) : v.toExponential(2);
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
    var cfg = report.config || {};
    var meta = document.getElementById("meta");
    [
      ["version", "v" + report.tool_version],
      ["generated", fmtTime(report.generated_at_unix)],
      ["threshold", fmt(cfg.default_threshold) || String(cfg.default_threshold)],
      ["metric", cfg.default_metric],
      ["ppd", String(cfg.pixels_per_degree)],
      ["mode", (cfg.mode || "regression") + " (" + LB.baseline + " vs " + LB.capture + ")"]
    ].forEach(function (kv) {
      meta.appendChild(h("div", null, h("dt", { text: kv[0] }), h("dd", { text: String(kv[1]) })));
    });
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
    if (state.open[name]) delete state.open[name];
    else state.open[name] = true;
    render();
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
    document.getElementById("empty").hidden = shown.length > 0;
    document.getElementById("count").textContent = "showing " + shown.length + " of " + entries.length;
    document.getElementById("f-issues").setAttribute("aria-pressed", state.filter === "issues");
    document.getElementById("f-all").setAttribute("aria-pressed", state.filter === "all");
  }

  // ---- detail --------------------------------------------------------------

  var ZOOMS = [["1", "1×"], ["2", "2×"], ["fit", "Fit"]];
  var zoom = "fit";

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

  function metaTable(e) {
    var ds = e.meta_diff || [];
    if (!ds.length) return null;
    return h("div", { class: "rtab-wrap" }, h("table", { class: "rtab mtab" },
      h("caption", { text: "Configuration differences" }),
      h("thead", null, h("tr", null, ["Key", LB.baseline, LB.capture].map(function (t) { return h("th", { scope: "col", text: t }); }))),
      h("tbody", null, ds.map(function (d) {
        return h("tr", null,
          h("td", { class: "mk", text: d.key }),
          h("td", { class: d.baseline === "<absent>" ? "mv na" : "mv", text: d.baseline }),
          h("td", { class: d.capture === "<absent>" ? "mv na" : "mv", text: d.capture }));
      }))));
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
    b.appendChild(h("span", { class: "badge", text: "lum mean " + p.mean_luminance.toFixed(3) }));
    b.appendChild(h("span", { class: "badge", text: "min " + p.min_luminance.toFixed(3) }));
    b.appendChild(h("span", { class: "badge", text: "max " + p.max_luminance.toFixed(3) }));
    if (e.metrics) b.appendChild(h("span", { class: "badge", text: e.metrics.width + "×" + e.metrics.height }));
    return b;
  }

  // ---- compare stage: swipe / flicker with synced zoom and pan ---------------

  var cmps = [];          // live compare controllers, rebuilt on every render()
  var activeCmp = null;   // the one the keyboard shortcuts drive

  function compare(e) {
    var hasHeat = !!e.paths.heatmap;
    var x = 50, k = 1, tx = 0, ty = 0, vertical = false, heat = false;
    var ptrs = {}, gesture = null;

    var stage = h("div", { class: "stage", tabindex: "0", role: "group",
      "aria-label": "Comparison of " + LB.baseline + " and " + LB.capture + ". Drag to move the divider; arrow keys also work." });
    if (e.metrics && e.metrics.width && e.metrics.height) stage.style.setProperty("--ar", e.metrics.width / e.metrics.height);
    var base = h("img", { class: "base", src: url(e.paths.baseline), alt: LB.baseline, decoding: "async", draggable: "false" });
    base.addEventListener("load", function () {
      if (base.naturalWidth && base.naturalHeight) stage.style.setProperty("--ar", base.naturalWidth / base.naturalHeight);
      paint();
    });
    var capImg = h("img", { src: url(e.paths.capture), alt: LB.capture, decoding: "async", draggable: "false" });
    var heatImg = hasHeat ? h("img", { class: "hm", src: url(e.paths.heatmap), alt: "FLIP heatmap", decoding: "async", draggable: "false" }) : null;
    var cap = h("div", { class: "cap" }, capImg, heatImg);
    var grip = h("i", { class: "grip" });
    var line = h("div", { class: "line" }, grip);
    var tagL = h("span", { class: "tag l", text: LB.baseline });
    var tagR = h("span", { class: "tag r", text: LB.capture });
    stage.append(base, cap, line, tagL, tagR);
    var hots = hotspotsOf(e).map(function (hs, i) {
      var b = h("button", { type: "button", class: "hsp", title: "Hotspot " + (i + 1) + ": zoom to it", "aria-label": "Zoom to hotspot " + (i + 1) }, h("span", { text: String(i + 1) }));
      b.addEventListener("pointerdown", function (ev) { ev.stopPropagation(); });
      b.addEventListener("click", function () { zoomBox(i); });
      stage.appendChild(b);
      return b;
    });

    var slider = h("input", { type: "range", min: "0", max: "100", step: "0.1", value: "50", "aria-label": "Swipe position: " + LB.baseline + " left, " + LB.capture + " right" });
    var opa = h("input", { type: "range", min: "0", max: "1", step: "0.05", value: "0.6", hidden: true, class: "opa", "aria-label": "Heatmap opacity" });
    stage.style.setProperty("--opa", "0.6");

    function setX(p) {
      x = Math.max(0, Math.min(100, Math.round(p * 10) / 10));
      stage.style.setProperty("--x", x + "%");
      slider.value = String(x);
    }
    setX(50);
    slider.addEventListener("input", function () { setX(Number(slider.value)); });
    opa.addEventListener("input", function () { stage.style.setProperty("--opa", opa.value); });

    // ---- zoom and pan: one transform shared by every layer ----
    var zoomBtns = {};
    var ZS = [["fit", "Fit"], ["1", "1×"], ["2", "2×"], ["4", "4×"], ["8", "8×"]];
    function absK(z) { return Math.max(1, Number(z) * (base.naturalWidth || (e.metrics && e.metrics.width) || 1) / (stage.clientWidth || 1)); }
    function paint() {
      var w = stage.clientWidth, hh = stage.clientHeight;
      if (k <= 1.0001) { k = 1; tx = 0; ty = 0; }
      tx = Math.max(w - w * k, Math.min(0, tx));
      ty = Math.max(hh - hh * k, Math.min(0, ty));
      stage.style.setProperty("--t", "translate(" + tx + "px," + ty + "px) scale(" + k + ")");
      stage.classList.toggle("pix", k > 1.0001);
      hotspotsOf(e).forEach(function (hs, i) {
        var r = hs.rect_frac, st = hots[i].style;
        st.left = (tx + r[0] * w * k) + "px"; st.top = (ty + r[1] * hh * k) + "px";
        st.width = (r[2] * w * k) + "px"; st.height = (r[3] * hh * k) + "px";
      });
      ZS.forEach(function (z) {
        var on = z[0] === "fit" ? k === 1 : k > 1 && Math.abs(k - absK(z[0])) < 1e-3;
        zoomBtns[z[0]].setAttribute("aria-pressed", on ? "true" : "false");
      });
    }
    function zoomAt(f, cx, cy) {
      var nk = Math.max(1, Math.min(256, k * f)), r = nk / k;
      tx = cx - (cx - tx) * r; ty = cy - (cy - ty) * r; k = nk;
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
    }
    function setZoom(z) {
      if (z === "fit") { k = 1; paint(); return; }
      var nk = absK(z), r = nk / k, cx = stage.clientWidth / 2, cy = stage.clientHeight / 2;
      tx = cx - (cx - tx) * r; ty = cy - (cy - ty) * r; k = nk;
      paint();
    }

    // ---- modes ----
    var mode = "swipe", paused = false, showCap = false, timer = null;
    var pauseBtn = h("button", { type: "button", class: "btn", hidden: true, text: "Pause" });
    var bSwipe = h("button", { type: "button", "aria-pressed": "true", text: "Swipe" });
    var bFlick = h("button", { type: "button", "aria-pressed": "false", text: "Flicker", title: "Space" });
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
      bSwipe.setAttribute("aria-pressed", m === "swipe");
      bFlick.setAttribute("aria-pressed", m === "flicker");
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
    bSwipe.addEventListener("click", function () { setMode("swipe"); });
    bFlick.addEventListener("click", function () { setMode("flicker"); });
    pauseBtn.addEventListener("click", function () {
      paused = !paused;
      pauseBtn.textContent = paused ? "Play" : "Pause";
      if (paused) stop(); else start();
    });

    var vertBtn = h("button", { type: "button", class: "btn small", "aria-pressed": "false", title: "Vertical / horizontal split (v)", text: "Vertical" });
    function setVertical(v) {
      vertical = v;
      stage.classList.toggle("vert", v);
      vertBtn.setAttribute("aria-pressed", v ? "true" : "false");
    }
    vertBtn.addEventListener("click", function () { setVertical(!vertical); });

    var heatBtn = hasHeat ? h("button", { type: "button", class: "btn small", "aria-pressed": "false", title: "FLIP heatmap on the capture side (h)", text: "Heatmap" }) : null;
    function setHeat(on) {
      if (!hasHeat) return;
      heat = on;
      stage.classList.toggle("heat", on);
      heatBtn.setAttribute("aria-pressed", on ? "true" : "false");
      opa.hidden = !on;
    }
    if (heatBtn) heatBtn.addEventListener("click", function () { setHeat(!heat); });

    // ---- full screen: the Fullscreen API on the whole .cmp, with a CSS fallback ----
    var root;
    var fsBtn = h("button", { type: "button", class: "btn small", "aria-pressed": "false", title: "Full screen (f)", text: "Full screen" });
    function fsActive() { return document.fullscreenElement === root || root.classList.contains("fs-css"); }
    function paintFs() {
      var on = fsActive();
      root.classList.toggle("fs-on", on);
      fsBtn.setAttribute("aria-pressed", on ? "true" : "false");
      fsBtn.textContent = on ? "Exit full screen" : "Full screen";
      paint();
    }
    function setFullscreen(on) {
      if (!on) {
        root.classList.remove("fs-css");
        if (document.fullscreenElement && document.exitFullscreen) document.exitFullscreen();
        paintFs();
        return;
      }
      var fallback = function () { root.classList.add("fs-css"); paintFs(); };
      if (root.requestFullscreen) {
        var r = root.requestFullscreen();
        if (r && r.catch) r.catch(fallback);
      } else fallback();
    }
    fsBtn.addEventListener("click", function () { setFullscreen(!fsActive()); });
    document.addEventListener("fullscreenchange", function () { if (root.isConnected) paintFs(); });
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
      if (mode === "swipe" && (k === 1 || nearLine(ev))) { gesture = { kind: "divider" }; xFrom(ev); }
      else gesture = { kind: "pan", sx: ev.clientX, sy: ev.clientY, tx: tx, ty: ty };
      stage.classList.add("drag");
    });
    stage.addEventListener("pointermove", function (ev) {
      if (ptrs[ev.pointerId]) { ptrs[ev.pointerId].x = ev.clientX; ptrs[ev.pointerId].y = ev.clientY; }
      if (!gesture) return;
      if (gesture.kind === "pinch") {
        if (Object.keys(ptrs).length < 2) return;
        var ks = Object.keys(ptrs), a = ptrs[ks[0]], b = ptrs[ks[1]], rc = stage.getBoundingClientRect(), nd = pinchDist();
        zoomAt(nd / gesture.d, (a.x + b.x) / 2 - rc.left, (a.y + b.y) / 2 - rc.top);
        gesture.d = nd;
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
      var rc = stage.getBoundingClientRect(), dy = ev.deltaMode === 1 ? ev.deltaY * 16 : ev.deltaY;
      zoomAt(Math.exp(-dy * (ev.ctrlKey ? 0.01 : 0.0015)), ev.clientX - rc.left, ev.clientY - rc.top);
    }, { passive: false });

    var zoomSeg = h("div", { class: "seg", role: "group", "aria-label": "Compare zoom" }, ZS.map(function (z) {
      var b = h("button", { type: "button", "data-cz": z[0], "aria-pressed": z[0] === "fit" ? "true" : "false", text: z[1], title: z[0] === "fit" ? "Fit (0)" : "Zoom " + z[1] + " (" + z[0] + ")" });
      b.addEventListener("click", function () { setZoom(z[0]); });
      zoomBtns[z[0]] = b;
      return b;
    }));

    root = h("div", { class: "cmp" },
      h("div", { class: "cmp-cap" },
        h("span", { text: "Compare" }),
        h("div", { class: "seg", role: "group", "aria-label": "Compare mode" }, bSwipe, bFlick),
        pauseBtn,
        h("span", { class: "grow" }),
        fsBtn,
        h("button", { type: "button", class: "btn small", title: "Keyboard shortcuts (?)", "aria-label": "Keyboard shortcuts", text: "?", onclick: function () { toggleHelp(); } })),
      h("div", { class: "cmp-tools" }, zoomSeg, vertBtn, heatBtn, opa),
      h("div", { class: "cmp-wrap" }, stage),
      slider);

    var api = {
      root: root,
      nudge: function (d) { if (mode === "swipe") setX(x + d); },
      flicker: function () { setMode(mode === "flicker" ? "swipe" : "flicker"); },
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
    root.addEventListener("pointerenter", function () { activeCmp = api; });
    root.addEventListener("focusin", function () { activeCmp = api; });
    root.addEventListener("pointerdown", function () { activeCmp = api; });
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

  var KEYS = [
    ["← / →", "Move the divider 5% (Shift: 1%)"],
    ["Space", "Toggle flicker"],
    ["v", "Vertical / horizontal split"],
    ["h", "Heatmap layer on the capture side"],
    ["1 2 4 8", "Zoom 1×, 2×, 4×, 8×"],
    ["0", "Zoom to fit"],
    ["f", "Full screen"],
    ["?", "This help"],
    ["Esc", "Close help / leave full screen"]
  ];
  function toggleHelp(force) {
    var hp = document.getElementById("help");
    var show = force === undefined ? !hp || hp.hidden : force;
    if (!hp) {
      var close = h("button", { type: "button", class: "btn small", text: "Close", onclick: function () { toggleHelp(false); } });
      var dl = h("dl");
      KEYS.forEach(function (kv) { dl.append(h("dt", null, h("kbd", { text: kv[0] })), h("dd", { text: kv[1] })); });
      hp = h("div", { id: "help", class: "help", role: "dialog", "aria-label": "Keyboard shortcuts", hidden: true },
        h("div", { class: "help-card" }, h("h2", { text: "Keyboard shortcuts" }), dl,
          h("p", { class: "hint", text: "Keys drive the comparison you last touched. Drag the image to move the divider; once zoomed, drag away from it to pan. Ctrl + wheel or pinch zooms." }),
          close));
      hp.addEventListener("click", function (ev) { if (ev.target === hp) toggleHelp(false); });
    }
    (document.fullscreenElement || document.body).appendChild(hp);
    hp.hidden = !show;
    if (show) hp.querySelector("button").focus();
  }

  function typing(t) {
    if (!t || !t.tagName) return false;
    if (t.isContentEditable || t.tagName === "TEXTAREA" || t.tagName === "SELECT") return true;
    return t.tagName === "INPUT" && !/^(range|checkbox|radio|button)$/.test(t.type);
  }
  document.addEventListener("keydown", function (ev) {
    if (ev.ctrlKey || ev.metaKey || ev.altKey || typing(ev.target)) return;
    var key = ev.key, hp = document.getElementById("help");
    if (key === "Escape") {
      if (hp && !hp.hidden) toggleHelp(false);
      else { var c0 = currentCmp(); if (c0) c0.exitFullscreen(); }
      return;
    }
    if (key === "?") { ev.preventDefault(); toggleHelp(); return; }
    var c = currentCmp();
    if (!c) return;
    if (key === "ArrowLeft" || key === "ArrowRight") { ev.preventDefault(); c.nudge((key === "ArrowLeft" ? -1 : 1) * (ev.shiftKey ? 1 : 5)); }
    else if (key === " ") { ev.preventDefault(); if (!ev.repeat) c.flicker(); }
    else if (key === "v" || key === "V") c.vertical();
    else if (key === "h" || key === "H") c.heat();
    else if (key === "f" || key === "F") c.fullscreen();
    else if (key === "0") c.zoom("fit");
    else if (key === "1" || key === "2" || key === "4" || key === "8") c.zoom(key);
  });
  // Space activates a focused button on keyup; keep it from doing so after it toggled flicker.
  document.addEventListener("keyup", function (ev) {
    if (ev.key === " " && !typing(ev.target) && currentCmp()) ev.preventDefault();
  });

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
    var d = h("div", { class: "d zoom-" + zoom + (state.regions ? " regions" : "") });
    var zoomBtns = ZOOMS.map(function (z) {
      return h("button", { type: "button", "aria-pressed": zoom === z[0] ? "true" : "false", "data-z": z[0], text: z[1], onclick: function () {
        zoom = z[0];
        // Apply to every open detail and keep the toggle state in sync.
        document.querySelectorAll(".d").forEach(function (el) {
          el.className = el.className.replace(/zoom-\S+/, "zoom-" + zoom);
          el.querySelectorAll("[data-z]").forEach(function (b) { b.setAttribute("aria-pressed", b.getAttribute("data-z") === zoom); });
          markUpscaled(el);
        });
      } });
    });
    var rgnBtn = (e.regions || []).length ? h("div", { class: "seg", role: "group", "aria-label": "Regions" },
      h("button", { type: "button", "aria-pressed": state.regions ? "true" : "false", text: "Regions", onclick: function (ev) {
        state.regions = !state.regions;
        var on = state.regions;
        document.querySelectorAll(".d").forEach(function (el) { el.classList.toggle("regions", on); });
        document.querySelectorAll(".d-bar .seg[aria-label=Regions] button").forEach(function (b) { b.setAttribute("aria-pressed", on); });
      } })) : null;
    var cmpApi = null;
    function goHot(i) {
      if (!cmpApi) return;
      cmpApi.zoomBox(i);
      cmpApi.root.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
    var hspBtn = hotspotsOf(e).length ? h("div", { class: "seg", role: "group", "aria-label": "Hotspots" },
      h("button", { type: "button", "aria-pressed": state.hotspots ? "true" : "false", text: "Hotspots", title: "Numbered boxes where the error is concentrated", onclick: function () {
        state.hotspots = !state.hotspots;
        var on = state.hotspots;
        document.querySelectorAll(".d").forEach(function (el) { el.classList.toggle("hots", on); });
        document.querySelectorAll(".d-bar .seg[aria-label=Hotspots] button").forEach(function (b) { b.setAttribute("aria-pressed", on); });
      } })) : null;
    d.className += state.hotspots ? " hots" : "";
    d.appendChild(h("div", { class: "d-bar" },
      h("div", { class: "seg", role: "group", "aria-label": "Zoom" }, zoomBtns),
      rgnBtn,
      hspBtn,
      badges(e)));
    if (e.error) d.appendChild(h("div", { class: "err-box", role: "alert", text: e.error }));
    var p = e.paths || {};
    d.appendChild(h("div", { class: "panes" },
      pane(e, LB.baseline, p.baseline, null, goHot),
      pane(e, LB.capture, p.capture, null, goHot),
      pane(e, "heatmap", p.heatmap, h("span", { class: "legend", title: "FLIP error, 0 (dark) to 1 (light)" }), goHot)));
    if (p.baseline && p.capture) { var cr = compare(e); cmpApi = cr.cmpApi; d.appendChild(cr); }
    var ht = hotspotTable(e, goHot);
    if (ht) d.appendChild(ht);
    var mg = metricsGrid(e);
    if (mg) d.appendChild(mg);
    var rt = regionTable(e);
    if (rt) d.appendChild(rt);
    var mt = metaTable(e);
    if (mt) d.appendChild(mt);
    var tr = h("tr", { class: "detail" }, h("td", { colspan: String(COLUMNS.length) }, d));
    return tr;
  }

  // ---- wiring ----------------------------------------------------------------

  document.getElementById("f-issues").addEventListener("click", function () { state.filter = "issues"; render(); });
  document.getElementById("f-all").addEventListener("click", function () { state.filter = "all"; render(); });
  window.addEventListener("resize", function () {
    document.querySelectorAll(".d").forEach(markUpscaled);
  });

  renderHeader();
  render();
})();
