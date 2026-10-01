(function () {
  "use strict";

  var report = JSON.parse(document.getElementById("flipdiff-data").textContent);
  var entries = report.entries || [];

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

  // ---- header ------------------------------------------------------------

  function renderHeader() {
    var cfg = report.config || {};
    var meta = document.getElementById("meta");
    [
      ["version", "v" + report.tool_version],
      ["generated", fmtTime(report.generated_at_unix)],
      ["threshold", fmt(cfg.default_threshold) || String(cfg.default_threshold)],
      ["metric", cfg.default_metric],
      ["ppd", String(cfg.pixels_per_degree)]
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
      h("td", { class: "name" }, h("button", { type: "button", "aria-expanded": open ? "true" : "false", onclick: function () { toggle(e.name); } }, e.name)),
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
  function zimg(src, alt, e) {
    var box = h("div", { class: "zbox" });
    if (e.metrics && e.metrics.width) box.style.setProperty("--nw", e.metrics.width);
    var img = h("img", { src: url(src), alt: alt, loading: "lazy", decoding: "async" });
    img.addEventListener("load", function () {
      box.style.setProperty("--nw", img.naturalWidth);
      markUpscaled(box.closest(".d"));
    });
    box.appendChild(img);
    return box;
  }

  function markUpscaled(d) {
    if (!d) return;
    var img = d.querySelector(".zbox img");
    var box = d.querySelector(".zbox");
    if (!img || !box) return;
    d.classList.toggle("up", d.classList.contains("zoom-fit") && box.clientWidth > img.naturalWidth);
  }

  function pane(e, label, path, extra) {
    var cap = h("figcaption", null, h("span", { text: label }), extra || null);
    var body = path ? h("div", { class: "vp" }, zimg(path, label + " of " + e.name, e)) : placeholder(e, label);
    return h("figure", { class: "pane" }, cap, body);
  }

  function badges(e) {
    var p = e.properties;
    var b = h("div", { class: "badges" });
    if (!p) { b.appendChild(h("span", { class: "badge", text: "no decodable capture" })); return b; }
    if (p.is_all_black) b.appendChild(h("span", { class: "badge warn", text: "ALL BLACK" }));
    if (p.is_all_white) b.appendChild(h("span", { class: "badge warn", text: "ALL WHITE" }));
    b.appendChild(h("span", { class: "badge", text: "lum mean " + p.mean_luminance.toFixed(3) }));
    b.appendChild(h("span", { class: "badge", text: "min " + p.min_luminance.toFixed(3) }));
    b.appendChild(h("span", { class: "badge", text: "max " + p.max_luminance.toFixed(3) }));
    if (e.metrics) b.appendChild(h("span", { class: "badge", text: e.metrics.width + "×" + e.metrics.height }));
    return b;
  }

  function compare(e) {
    var stage = h("div", { class: "stage zbox" });
    if (e.metrics && e.metrics.width) stage.style.setProperty("--nw", e.metrics.width);
    var base = h("img", { src: url(e.paths.baseline), alt: "baseline", decoding: "async" });
    base.addEventListener("load", function () {
      stage.style.setProperty("--nw", base.naturalWidth);
      markUpscaled(stage.closest(".d"));
    });
    var cap = h("img", { class: "cap", src: url(e.paths.capture), alt: "capture", decoding: "async" });
    var line = h("div", { class: "line" });
    var tagL = h("span", { class: "tag l", text: "baseline" });
    var tagR = h("span", { class: "tag r", text: "capture" });
    stage.append(base, cap, line, tagL, tagR);

    var slider = h("input", { type: "range", min: "0", max: "100", value: "50", "aria-label": "Swipe position: baseline left, capture right" });
    slider.addEventListener("input", function () { stage.style.setProperty("--x", slider.value + "%"); });

    var mode = "swipe";
    var paused = false;
    var showCap = false;
    var timer = null;
    var pauseBtn = h("button", { type: "button", class: "btn", hidden: true, text: "Pause" });
    var bSwipe = h("button", { type: "button", "aria-pressed": "true", text: "Swipe" });
    var bFlick = h("button", { type: "button", "aria-pressed": "false", text: "Flicker" });

    function paintFlick() {
      stage.classList.toggle("showcap", showCap);
      tagL.textContent = showCap ? "capture" : "baseline";
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
        tagL.textContent = "baseline";
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

    return h("div", { class: "cmp" },
      h("div", { class: "cmp-cap" },
        h("span", { text: "Compare" }),
        h("div", { class: "seg", role: "group", "aria-label": "Compare mode" }, bSwipe, bFlick),
        pauseBtn),
      h("div", { class: "cmp-wrap" }, stage),
      slider);
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
    var d = h("div", { class: "d zoom-" + zoom });
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
    d.appendChild(h("div", { class: "d-bar" },
      h("div", { class: "seg", role: "group", "aria-label": "Zoom" }, zoomBtns),
      badges(e)));
    if (e.error) d.appendChild(h("div", { class: "err-box", role: "alert", text: e.error }));
    var p = e.paths || {};
    d.appendChild(h("div", { class: "panes" },
      pane(e, "baseline", p.baseline),
      pane(e, "capture", p.capture),
      pane(e, "heatmap", p.heatmap, h("span", { class: "legend", title: "FLIP error, 0 (dark) to 1 (light)" }))));
    if (p.baseline && p.capture) d.appendChild(compare(e));
    var mg = metricsGrid(e);
    if (mg) d.appendChild(mg);
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
