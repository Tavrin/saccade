(function () {
  'use strict';
  var D = JSON.parse(document.getElementById('flipdiff-data').textContent);
  var $ = function (id) { return document.getElementById(id); };
  var LS_KEY = 'flipdiff-view.v1:' + D.id;
  var LETTERS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';

  var S = {
    set: 0, visible: [], sort: 'name', diffOnly: false, layout: 'grid', tool: 'pan', scale: 1, tx: 0, ty: 0, fit: true,
    exposure: 0, contrast: 1, channel: 'rgb', opacity: 0.6, rate: 2, paused: false,
    swipe: 50, vertical: false, heat: false, hot: true, hold: false, lastLayout: 'swipe', fs: false, revealed: false, chosen: [], flickerIdx: 0, cursor: null, roiDraft: null, hsel: null, hselT: ''
  };
  var dec = {};           // name -> SetDecision-shaped entry
  var pixCache = {};      // set index -> {rgb:[ImageData|null], flip:[Uint8Array|null], promise}
  var timer = null;
  var viewports = [];
  var agent = null;       // window.flipdiff (see agentapi.js)
  var props = {};         // set name -> proposals recorded by `flipdiff decide`
  function notify() { if (agent) agent.changed(); }

  // ---------- helpers ----------
  function el(tag, attrs, kids) {
    var n = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) {
      if (k === 'text') n.textContent = attrs[k];
      else if (k === 'class') n.className = attrs[k];
      else n.setAttribute(k, attrs[k]);
    });
    (kids || []).forEach(function (c) { n.appendChild(typeof c === 'string' ? document.createTextNode(c) : c); });
    return n;
  }
  function url(p) { return p.split('/').map(encodeURIComponent).join('/'); }
  function cur() { return D.sets[S.set]; }
  // Blind mode embeds only neutral labels (P1, P2, ...); the true labels come from
  // blind-key.json, loaded through the file picker when the judge reveals.
  // The key maps each set's pane positions (P1, P2, ...) to the true labels: every
  // set shuffles its panes, so the mapping is per set (KEYSETS[set.name][i]).
  var KEY = null, KEYSETS = null;
  function trueLabel(i, set) {
    if (set && KEYSETS && KEYSETS[set.name] && KEYSETS[set.name][i] != null) return KEYSETS[set.name][i];
    return KEY && KEY[i] != null ? KEY[i] : D.labels[i];
  }
  function trueOf(neutral, set) { var i = D.labels.indexOf(neutral); return i < 0 ? neutral : trueLabel(i, set); }
  function hideInfo() { return D.blind && !S.revealed; }
  function letter(set, i) { return LETTERS[set.order.indexOf(i)] || '?'; }
  function paneName(set, i) {
    if (!D.blind) return D.labels[i];
    return S.revealed ? trueLabel(i, set) + ' (' + letter(set, i) + ')' : letter(set, i);
  }
  function present(set) { return set.order.filter(function (i) { return set.panes[i].path; }); }
  // Which directories have this set's image (labels are neutral or hidden in blind mode).
  function labelsWhere(set, has) {
    return set.panes.map(function (p, i) { return !!p.path === has ? D.labels[i] : null; }).filter(Boolean);
  }
  // The one directory that has the image, when only one does.
  function soloLabel(set) {
    if (D.blind || set.panes.length < 2) return null;
    var have = labelsWhere(set, true);
    return have.length === 1 ? have[0] : null;
  }
  // Why a comparison layout has nothing to show: names the directory that lacks the image.
  function needTwo(set, what, tickHint) {
    if (present(set).length >= 2) return tickHint;
    var miss = labelsWhere(set, false);
    if (hideInfo() || !miss.length) return what + ' needs two images, but this set has only ' + present(set).length + '.';
    return miss.join(', ') + (miss.length > 1 ? ' have' : ' has') + ' no ' + set.name + '. ' + what + ' needs two images.';
  }
  function refDims(set) {
    var r = set.panes[D.reference];
    if (r && r.width) return [r.width, r.height];
    for (var i = 0; i < set.panes.length; i++) if (set.panes[i].width) return [set.panes[i].width, set.panes[i].height];
    return [1, 1];
  }
  function fmt(v, d) { return v == null ? '–' : v.toFixed(d == null ? 4 : d); }
  function entry(name) {
    if (!dec[name]) dec[name] = { name: name, decision: null, chosen_label: null, no_difference: false, note: '', roi: null, timestamp_ms: 0 };
    return dec[name];
  }
  function touch(e) { e.timestamp_ms = Date.now(); save(); }
  function isDecided(e) { return !!(e && (e.decision || e.chosen_label || e.no_difference)); }

  // ---------- persistence ----------
  function load() {
    try {
      var raw = localStorage.getItem(LS_KEY);
      if (!raw) return;
      var o = JSON.parse(raw);
      if (o && o.dec) dec = o.dec;
    } catch (e) { /* storage unavailable: run without persistence */ }
  }
  function save() {
    try { localStorage.setItem(LS_KEY, JSON.stringify({ dec: dec, revealed: S.revealed })); } catch (e) { /* ignore */ }
    serveSync();
  }
  // The decisions document: what was decided, and what it was decided on. `dirs`
  // (empty in a blind view until `flipdiff unblind` fills it from the key) and the
  // per-set image hashes let `flipdiff approve` refuse to promote anything else.
  function buildDecisions() {
    var dirs = D.dirs || [];
    return {
      schema: 'flipdiff-decisions.v1', seed: D.seed, labels: D.labels, blind: D.blind, dirs: dirs,
      sets: D.sets.map(function (s) {
        var e = dec[s.name] || {};
        var ci = e.chosen_label ? D.labels.indexOf(e.chosen_label) : -1;
        var out = {
          name: s.name, decision: e.decision || null, chosen_label: e.chosen_label || null,
          no_difference: !!e.no_difference, note: e.note || '', roi: e.roi || null, timestamp_ms: e.timestamp_ms || 0,
          chosen_dir: ci >= 0 && !D.blind ? (dirs[ci] || null) : null,
          sha256: s.panes.map(function (p) { return p.sha256 || null; })
        };
        // Answers `flipdiff decide` recorded travel with the file, so saving never drops them.
        if (props[s.name] && props[s.name].length) out.proposals = props[s.name];
        return out;
      })
    };
  }
  // Under `flipdiff serve` (window.FLIPDIFF_SERVE = {token, session}), also POST the decisions file, debounced.
  var serveTimer = 0;
  function serveSync() {
    var sv = window.FLIPDIFF_SERVE;
    if (!sv || !sv.session) return;
    clearTimeout(serveTimer);
    serveTimer = setTimeout(function () {
      var out = buildDecisions();
      fetch('/api/session/' + sv.session + '/decisions', { method: 'POST', headers: { 'X-Flipdiff-Token': sv.token, 'Content-Type': 'application/json' }, body: JSON.stringify(out) }).catch(function () { /* offline: localStorage keeps it */ });
    }, 500);
  }

  // ---------- pixels (inspector + ROI) ----------
  function loadImg(src) {
    return new Promise(function (res, rej) {
      var im = new Image();
      im.onload = function () { res(im); };
      im.onerror = function () { rej(new Error('image load failed')); };
      im.src = src;
    });
  }
  function loadScript(src) {
    return new Promise(function (res, rej) {
      var s = document.createElement('script');
      s.onload = res; s.onerror = function () { rej(new Error('script load failed')); };
      s.src = src; document.head.appendChild(s);
    });
  }
  function readImage(src, w, h) {
    return loadImg(src).then(function (im) {
      var c = document.createElement('canvas'); c.width = w; c.height = h;
      var g = c.getContext('2d', { willReadFrequently: true });
      g.drawImage(im, 0, 0);
      return g.getImageData(0, 0, w, h);
    });
  }
  // Inspector data is loaded per set, only once the inspector is first used. The
  // set's script holds lossless PNG data URIs (same-origin, so the canvas is never
  // tainted on file://), downsampled by an integer factor when a pane is larger
  // than 2048 px; `ix` maps an image pixel to its index in that data.
  function pixelsFor(si) {
    var c = pixCache[si];
    if (c) return c.promise;
    var set = D.sets[si];
    c = pixCache[si] = { rgb: [], flip: [], scale: [] };
    c.promise = loadScript(url(set.pixels_script)).then(function () {
      var px = (window.__flipdiffPx || {})[si] || {};
      var chain = Promise.resolve();
      set.panes.forEach(function (p, i) {
        var f = (px.scale && px.scale[i]) || 1;
        var w = Math.ceil(p.width / f), h = Math.ceil(p.height / f);
        c.scale[i] = f;
        chain = chain.then(function () { return px.rgb && px.rgb[i] ? readImage(px.rgb[i], w, h) : null; })
          .catch(function () { return null; })
          .then(function (v) { c.rgb[i] = v; })
          .then(function () { return px.flip && px.flip[i] ? readImage(px.flip[i], w, h) : null; })
          .catch(function () { return null; })
          .then(function (id) {
            if (!id) { c.flip[i] = null; return; }
            var a = new Uint8Array(w * h);
            for (var k = 0; k < a.length; k++) a[k] = id.data[k * 4];
            c.flip[i] = a;
          });
      });
      return chain.then(function () { delete (window.__flipdiffPx || {})[si]; return c; });
    }).catch(function () {
      set.panes.forEach(function (_, i) { c.rgb[i] = null; c.flip[i] = null; c.scale[i] = 1; });
      return c;
    });
    return c.promise;
  }
  function ix(c, set, i, x, y) {
    var f = c.scale[i] || 1;
    return Math.floor(y / f) * Math.ceil(set.panes[i].width / f) + Math.floor(x / f);
  }

  // ---------- transform ----------
  function vpWidth() { return viewports.length ? viewports[0].clientWidth : 1; }
  function fitScale() { return vpWidth() / refDims(cur())[0]; }
  function clampT() {
    var d = refDims(cur()), s = S.scale;
    var vw = vpWidth(), vh = vw * d[1] / d[0];
    var minx = Math.min(0, vw - d[0] * s), miny = Math.min(0, vh - d[1] * s);
    S.tx = Math.max(minx, Math.min(0, S.tx));
    S.ty = Math.max(miny, Math.min(0, S.ty));
  }
  function applyTransform() {
    if (S.fit) { S.scale = fitScale(); S.tx = 0; S.ty = 0; }
    var s = S.scale;
    clampT();
    var t = 'translate(' + S.tx + 'px,' + S.ty + 'px) scale(' + s + ')';
    var layers = document.querySelectorAll('.layer');
    for (var i = 0; i < layers.length; i++) {
      layers[i].style.transform = t;
      layers[i].style.setProperty('--inv', String(1 / s));
    }
    $('stage').classList.toggle('pix', s > 1.0001);
    // With no image on stage (nothing to swipe) there is no scale to report.
    $('zval').textContent = viewports.length ? (s >= 10 ? s.toFixed(0) : s.toFixed(2)) + '×' : '–';
    var zb = document.querySelectorAll('#seg-zoom button');
    for (var j = 0; j < zb.length; j++) {
      var z = zb[j].getAttribute('data-z');
      zb[j].setAttribute('aria-pressed', String(z === 'fit' ? S.fit : (!S.fit && Math.abs(S.scale - Number(z)) < 1e-6)));
    }
    notify();
  }
  function zoomAt(k, cx, cy) {
    var ns = Math.max(fitScale(), Math.min(64, S.scale * k));
    k = ns / S.scale;
    S.tx = cx - (cx - S.tx) * k;
    S.ty = cy - (cy - S.ty) * k;
    S.scale = ns;
    S.fit = Math.abs(ns - fitScale()) < 1e-6;
    applyTransform();
  }
  function setZoom(z) {
    if (z === 'fit') { S.fit = true; applyTransform(); return; }
    var vw = vpWidth(), d = refDims(cur()), vh = vw * d[1] / d[0];
    S.fit = false;
    var ns = Math.max(Number(z), fitScale());
    // zoom about the viewport centre
    var cx = vw / 2, cy = vh / 2, k = ns / S.scale;
    S.tx = cx - (cx - S.tx) * k; S.ty = cy - (cy - S.ty) * k; S.scale = ns;
    applyTransform();
  }

  // ---------- display filter ----------
  function applyFilter() {
    var f = [];
    if (S.exposure !== 0) f.push('brightness(' + Math.pow(2, S.exposure).toFixed(4) + ')');
    if (S.contrast !== 1) f.push('contrast(' + S.contrast + ')');
    if (S.channel !== 'rgb') f.push('url(#ch-' + S.channel + ')');
    $('stage').style.setProperty('--f', f.length ? f.join(' ') : 'none');
    $('expv').textContent = (S.exposure > 0 ? '+' : '') + S.exposure.toFixed(1) + ' EV';
    $('conv').textContent = '×' + S.contrast.toFixed(2);
    notify();
  }

  // ---------- stage ----------
  // Numbered hotspot boxes of pane i, in image pixels (they zoom with their layer).
  function hotBoxes(p, i) {
    if (!S.hot || hideInfo()) return [];
    return (p.hotspots || []).map(function (hs, k) {
      var r = hs.rect_px;
      var b = el('div', { class: 'hsp', 'data-pane': String(i), 'data-h': String(k), title: 'Hotspot ' + (k + 1) + ': click to zoom to it' }, [el('span', { text: String(k + 1) })]);
      b.style.left = r[0] + 'px'; b.style.top = r[1] + 'px'; b.style.width = r[2] + 'px'; b.style.height = r[3] + 'px';
      return b;
    });
  }
  function layerFor(set, i, withHeat, heatPath, noBoxes) {
    var p = set.panes[i];
    var layer = el('div', { class: 'layer' });
    layer.style.width = p.width + 'px'; layer.style.height = p.height + 'px';
    var img = el('img', { class: 'base', src: url(p.path), width: String(p.width), height: String(p.height), alt: paneName(set, i) + ' – ' + set.name, draggable: 'false' });
    layer.appendChild(img);
    var hp = heatPath || (withHeat && p.heatmap);
    if (hp) {
      layer.appendChild(el('img', { class: 'heat', src: url(hp), width: String(p.width), height: String(p.height), alt: '', draggable: 'false' }));
    }
    layer.appendChild(el('div', { class: 'roi', hidden: '' }));
    if (!noBoxes) hotBoxes(p, i).forEach(function (b) { layer.appendChild(b); });
    return layer;
  }
  function wrapFor(set, i, withHeat, heatPath, noBoxes) {
    var w = el('div', { class: 'wrap' });
    w.appendChild(layerFor(set, i, withHeat, heatPath, noBoxes));
    return w;
  }
  function frameWide(p) {
    var hs = p.hotspots || [];
    return hs.length > 0 && hs[0].rect_frac[2] * hs[0].rect_frac[3] >= 0.5;
  }
  function paneHead(set, i) {
    var p = set.panes[i];
    var right = '', full = null;
    if (!hideInfo()) {
      if (i === D.reference) right = 'reference';
      else if (p.metrics) { right = 'mean ' + fmt(p.metrics.mean) + ' · p95 ' + fmt(p.metrics.p95) + ' · max ' + fmt(p.metrics.max); full = 'FLIP ' + right; }
      else if (p.error) right = p.error;
    }
    var head = el('div', { class: 'pane-h' }, [el('b', { text: paneName(set, i), title: paneName(set, i) }), el('span', { text: right, title: full || right })]);
    if (!hideInfo() && frameWide(p)) head.appendChild(el('span', { class: 'wide', text: 'frame-wide change', title: 'The largest hotspot covers at least half of the frame' }));
    return head;
  }
  function vpEl(kids) {
    var v = el('div', { class: 'vp' }, kids);
    viewports.push(v);
    return v;
  }
  function missingPane(set, i) {
    var d = refDims(set);
    var m = el('div', { class: 'missing', text: 'not present in ' + (hideInfo() ? 'this set' : D.labels[i]) });
    m.style.setProperty('--w', d[0]); m.style.setProperty('--h', d[1]);
    return el('div', { class: 'pane' }, [paneHead(set, i), m]);
  }

  function renderStage() {
    var set = cur(), stage = $('stage');
    clearInterval(timer); timer = null;
    viewports = [];
    stage.textContent = '';
    var d = refDims(set);
    stage.style.setProperty('--w', d[0]); stage.style.setProperty('--h', d[1]); stage.style.setProperty('--ar', d[0] / d[1]);
    stage.classList.toggle('tool-roi', S.tool === 'roi');
    var single = S.layout !== 'grid' && S.layout !== 'overlay';
    stage.classList.toggle('single', single);
    var ch = S.chosen.filter(function (i) { return set.panes[i].path; });

    if (S.layout === 'grid') {
      set.order.forEach(function (i) {
        if (!set.panes[i].path) { stage.appendChild(missingPane(set, i)); return; }
        stage.appendChild(el('div', { class: 'pane' }, [paneHead(set, i), vpEl([wrapFor(set, i, false)])]));
      });
    } else if (S.layout === 'overlay') {
      set.order.forEach(function (i) {
        var p = set.panes[i];
        if (!p.path) { stage.appendChild(missingPane(set, i)); return; }
        stage.appendChild(el('div', { class: 'pane' }, [paneHead(set, i), vpEl([wrapFor(set, i, true)])]));
      });
    } else if (S.layout === 'swipe') {
      if (ch.length < 2) { stage.appendChild(el('p', { class: 'note-msg', text: needTwo(set, 'Swipe', 'Swipe needs two images: tick two under "Show".') })); }
      else {
        var a = ch[0], b = ch[1];
        var wb = wrapFor(set, b, false, S.heat && !hideInfo() ? heatFor(set, a, b) : null, true);
        wb.id = 'swipe-top';
        var div = el('div', { class: 'divider', id: 'swipe-div' }, [el('i', { class: 'grip' })]);
        // Hotspot boxes sit in their own unclipped layer, so the divider never hides them.
        var hp = b === D.reference ? a : b;
        var hw = el('div', { class: 'wrap hotwrap' });
        var hl = el('div', { class: 'layer' });
        hl.style.width = set.panes[hp].width + 'px'; hl.style.height = set.panes[hp].height + 'px';
        hotBoxes(set.panes[hp], hp).forEach(function (x) { hl.appendChild(x); });
        hw.appendChild(hl);
        var v = vpEl([wrapFor(set, a, false, null, true), wb, hw, div,
          el('span', { class: 'corner l', text: paneName(set, a) }), el('span', { class: 'corner r', text: paneName(set, b) })]);
        v.classList.add('swipe');
        v.classList.toggle('vert', S.vertical);
        stage.appendChild(el('div', { class: 'pane' }, [el('div', { class: 'pane-h' }, [el('b', { text: paneName(set, a) + ' | ' + paneName(set, b), title: paneName(set, a) + ' | ' + paneName(set, b) }), el('span', { text: '' })]), v]));
        paintSwipe(v);   // after attaching: it looks the layers up by id
      }
    } else if (S.layout === 'flicker') {
      if (ch.length < 2) { stage.appendChild(el('p', { class: 'note-msg', text: needTwo(set, 'Flicker', 'Flicker needs at least two images: tick them under "Show".') })); }
      else {
        var wraps = ch.map(function (i) { return wrapFor(set, i, false); });
        var corner = el('span', { class: 'corner l' });
        var head = el('b');
        var vv = vpEl(wraps.concat([corner]));
        stage.appendChild(el('div', { class: 'pane' }, [el('div', { class: 'pane-h' }, [head, el('span', { text: ch.length + ' images, ' + S.rate + ' Hz' })]), vv]));
        var show = function () {
          S.flickerIdx = S.flickerIdx % ch.length;
          wraps.forEach(function (w, k) { w.style.visibility = k === S.flickerIdx ? 'visible' : 'hidden'; });
          corner.textContent = paneName(set, ch[S.flickerIdx]);
          head.textContent = corner.textContent;
        };
        show();
        if (!S.paused) timer = setInterval(function () { S.flickerIdx = (S.flickerIdx + 1) % ch.length; show(); }, 1000 / S.rate);
      }
    }
    S.hold = false;
    stage.appendChild(el('div', { class: 'fs-hud' }, [el('button', { type: 'button', class: 'btn small', id: 'fs-exit', text: 'Exit full screen (f)' })]));
    $('fs-exit').addEventListener('click', function () { setFullscreen(false); });
    applyFilter();
    stage.style.setProperty('--opa', S.opacity);
    applyTransform();
    drawRoi();
    renderInspector();
  }

  // The FLIP heatmap that belongs to the capture side of a swipe: FLIP(reference, pane).
  function heatFor(set, a, b) {
    if (b !== D.reference && set.panes[b].heatmap) return set.panes[b].heatmap;
    if (a !== D.reference && set.panes[a].heatmap) return set.panes[a].heatmap;
    return null;
  }
  // Position the swipe divider and clip the top pane, horizontally or vertically.
  function paintSwipe(vp) {
    var t = $('swipe-top'), d = $('swipe-div');
    if (!t || !d) return;
    var v = vp || d.closest('.vp');
    v.classList.toggle('vert', S.vertical);
    if (S.vertical) {
      t.style.clipPath = 'inset(' + S.swipe + '% 0 0 0)';
      d.style.left = ''; d.style.top = 'calc(' + S.swipe + '% - 1px)';
    } else {
      t.style.clipPath = 'inset(0 0 0 ' + S.swipe + '%)';
      d.style.top = ''; d.style.left = 'calc(' + S.swipe + '% - 1px)';
    }
  }
  function setSwipe(pct) {
    S.swipe = Math.max(0, Math.min(100, Math.round(pct * 10) / 10));
    $('swipe').value = String(S.swipe);
    paintSwipe();
    notify();
  }
  // Zoom the viewports to hotspot k of pane i, leaving a margin around the box.
  function zoomHot(i, k) {
    var set = cur(), hs = (set.panes[i].hotspots || [])[k];
    if (!hs || !viewports.length) return;
    var r = hs.rect_px, vw = vpWidth(), d = refDims(set), vh = vw * d[1] / d[0];
    var want = 0.8 * Math.min(vw / Math.max(r[2], 4), vh / Math.max(r[3], 4));
    var ns = Math.max(fitScale(), Math.min(64, want));
    S.fit = Math.abs(ns - fitScale()) < 1e-6;
    S.scale = ns;
    S.tx = vw / 2 - (r[0] + r[2] / 2) * ns;
    S.ty = vh / 2 - (r[1] + r[3] / 2) * ns;
    applyTransform();
    S.hsel = k + 1; S.hselT = tkey();
    $('stage').scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }
  function swipeFromPointer(vp, e) {
    var rc = vp.getBoundingClientRect();
    setSwipe(S.vertical ? (e.clientY - rc.top) / rc.height * 100 : (e.clientX - rc.left) / rc.width * 100);
  }
  function nearDivider(vp, e) {
    var rc = vp.getBoundingClientRect();
    var pos = S.vertical ? e.clientY - rc.top : e.clientX - rc.left;
    var at = (S.vertical ? rc.height : rc.width) * S.swipe / 100;
    return Math.abs(pos - at) <= 28;
  }

  // ---------- hold to compare ----------
  function holdStart() {
    var set = cur();
    if (S.hold || hideInfo() || !set.panes[D.reference] || !set.panes[D.reference].path || S.layout === 'flicker') return;
    S.hold = true;
    $('hold').setAttribute('aria-pressed', 'true');
    viewports.forEach(function (vp) {
      var w = wrapFor(set, D.reference, false);
      w.classList.add('holdwrap');
      vp.appendChild(w);
      vp.appendChild(el('span', { class: 'corner hold', text: paneName(set, D.reference) + ' (held)' }));
    });
    applyFilter(); applyTransform();
  }
  function holdEnd() {
    if (!S.hold) return;
    S.hold = false;
    $('hold').setAttribute('aria-pressed', 'false');
    var xs = document.querySelectorAll('.holdwrap, .corner.hold');
    for (var i = 0; i < xs.length; i++) xs[i].remove();
  }

  // ---------- full screen ----------
  // Fullscreen API on the stage; when it is missing or refused, a fixed-position CSS fallback.
  function fsActive() { return document.fullscreenElement === $('stage') || $('stage').classList.contains('fs-css'); }
  function paintFs() {
    S.fs = fsActive();
    $('stage').classList.toggle('fs-on', S.fs);
    $('fs').setAttribute('aria-pressed', String(S.fs));
    if (viewports.length) applyTransform();
  }
  function setFullscreen(on) {
    var st = $('stage');
    if (!on) {
      st.classList.remove('fs-css');
      if (document.fullscreenElement && document.exitFullscreen) document.exitFullscreen();
      paintFs();
      return;
    }
    var fallback = function () { st.classList.add('fs-css'); paintFs(); };
    if (st.requestFullscreen) {
      var r = st.requestFullscreen();
      if (r && r.catch) r.catch(fallback);
    } else fallback();
  }

  // ---------- help overlay ----------
  var KEYS = [
    ['y / n', 'Confirm / override the decision an agent proposed'],
    ['← / →', 'Move the swipe divider 5% (Shift: 1%); outside swipe, previous/next set'],
    ['[ / ]', 'Previous / next image set'],
    ['Space', 'Toggle flicker'],
    ['v', 'Vertical / horizontal split (swipe)'],
    ['h', 'Heatmap layer on the capture side (swipe)'],
    ['1 2 4 8', 'Zoom 1×, 2×, 4×, 8×'],
    ['0', 'Zoom to fit'],
    ['f', 'Full screen'],
    ['c (hold)', 'Show the reference while held'],
    ['?', 'This help'],
    ['Esc', 'Close help / leave full screen']
  ];
  function toggleHelp(force) {
    var h = $('help');
    var show = force == null ? !h || h.hidden : force;
    if (!h) {
      h = el('div', { id: 'help', class: 'help', role: 'dialog', 'aria-label': 'Keyboard shortcuts', hidden: '' });
      var card = el('div', { class: 'help-card' }, [el('h2', { text: 'Keyboard shortcuts' })]);
      var dl = el('dl');
      KEYS.forEach(function (k) { dl.appendChild(el('dt', null, [el('kbd', { text: k[0] })])); dl.appendChild(el('dd', { text: k[1] })); });
      card.appendChild(dl);
      card.appendChild(el('p', { class: 'hint', text: 'Drag on a swipe image to move the divider; when zoomed, drag away from it to pan. Wheel or pinch to zoom.' }));
      var x = el('button', { type: 'button', class: 'btn small', text: 'Close' });
      x.addEventListener('click', function () { toggleHelp(false); });
      card.appendChild(x);
      h.appendChild(card);
      h.addEventListener('click', function (e) { if (e.target === h) toggleHelp(false); });
    }
    (document.fullscreenElement || document.body).appendChild(h);
    h.hidden = !show;
    if (show) h.querySelector('button').focus();
  }

  // ---------- ROI ----------
  function roiRect() {
    var e = dec[cur().name];
    return S.roiDraft || (e && e.roi) || null;
  }
  function drawRoi() {
    var r = roiRect();
    var boxes = document.querySelectorAll('.roi');
    for (var i = 0; i < boxes.length; i++) {
      if (!r || r.w <= 0 || r.h <= 0) { boxes[i].hidden = true; continue; }
      boxes[i].hidden = false;
      boxes[i].style.left = r.x + 'px'; boxes[i].style.top = r.y + 'px';
      boxes[i].style.width = r.w + 'px'; boxes[i].style.height = r.h + 'px';
    }
    notify();
  }
  function roiStats(c, set, r) {
    var out = {};
    set.panes.forEach(function (p, i) {
      var id = c.rgb[i];
      if (!id || !p.width) { out[i] = null; return; }
      var x0 = Math.max(0, r.x), y0 = Math.max(0, r.y), x1 = Math.min(p.width, r.x + r.w), y1 = Math.min(p.height, r.y + r.h);
      var n = 0, sr = 0, sg = 0, sb = 0, sf = 0, fm = c.flip[i];
      for (var y = y0; y < y1; y++) for (var x = x0; x < x1; x++) {
        var k = ix(c, set, i, x, y);
        sr += id.data[k * 4]; sg += id.data[k * 4 + 1]; sb += id.data[k * 4 + 2];
        if (fm) sf += fm[k];
        n++;
      }
      out[i] = n ? { n: n, r: sr / n, g: sg / n, b: sb / n, flip: fm ? sf / n / 255 : null } : null;
    });
    return out;
  }

  // ---------- inspector ----------
  function inspectorNote(set) {
    var f = 1;
    set.panes.forEach(function (p) { if (p.inspector_scale > f) f = p.inspector_scale; });
    return f > 1 ? 'Inspector at 1/' + f + ' res: values are ' + f + '×' + f + ' averages (data capped at 2048 px).' : '';
  }
  function inspRows(set, c, pos, stats, t, head) {
    t.textContent = '';
    t.appendChild(head);
    set.order.forEach(function (i) {
      var p = set.panes[i];
      var rgbTxt = '–', flipTxt = '–', sw = null;
      if (p.path && pos) {
        var id = c && c.rgb[i];
        if (id && pos.x < p.width && pos.y < p.height) {
          var k = ix(c, set, i, pos.x, pos.y) * 4;
          rgbTxt = id.data[k] + ' ' + id.data[k + 1] + ' ' + id.data[k + 2];
          sw = 'rgb(' + id.data[k] + ',' + id.data[k + 1] + ',' + id.data[k + 2] + ')';
        } else if (!c) rgbTxt = '…';
        else if (!id) rgbTxt = 'n/a';
        var fm = c && c.flip[i];
        if (hideInfo()) flipTxt = 'hidden';
        else if (i === D.reference) flipTxt = 'ref';
        else if (fm && pos.x < p.width && pos.y < p.height) flipTxt = (fm[ix(c, set, i, pos.x, pos.y)] / 255).toFixed(3);
      } else if (!p.path) rgbTxt = 'missing';
      var sp = el('span', { class: 'sw' }); if (sw) sp.style.background = sw;
      var tr = el('tr', null, [el('td', null, [sp, paneName(set, i)]), el('td', { text: rgbTxt }), el('td', { text: flipTxt })]);
      if (stats) {
        var s = stats[i];
        tr.appendChild(el('td', { text: s ? s.r.toFixed(1) + ' ' + s.g.toFixed(1) + ' ' + s.b.toFixed(1) : '–' }));
        tr.appendChild(el('td', { text: hideInfo() ? 'hidden' : (i === D.reference ? 'ref' : (s && s.flip != null ? s.flip.toFixed(4) : '–')) }));
      }
      t.appendChild(tr);
    });
  }
  function renderInspector() {
    var set = cur(), t = $('insp'), si = S.set;
    var r = roiRect(), pos = S.cursor;
    var hint = $('insp-hint');
    var head = el('tr', null, [el('th', { text: 'Image' }), el('th', { text: 'RGB' }), el('th', { text: 'FLIP' })]);
    var hasRoi = r && r.w > 0 && r.h > 0;
    if (hasRoi) { head.appendChild(el('th', { text: 'ROI mean RGB' })); head.appendChild(el('th', { text: 'ROI mean FLIP' })); }
    hint.textContent = pos ? 'Pixel (' + pos.x + ', ' + pos.y + ')' : 'Hover (or tap) an image to read pixel values.';
    if (hasRoi) hint.textContent += (pos ? ' · ' : '') + 'ROI ' + r.x + ',' + r.y + ' ' + r.w + '×' + r.h + ' (' + (r.w * r.h) + ' px)';
    var note = inspectorNote(set);
    if (note) hint.textContent += ' · ' + note;
    // No pixel data is fetched until a pixel or a region is actually inspected.
    var used = !!pos || hasRoi;
    var ready = pixCache[si] && pixCache[si].done ? pixCache[si] : null;
    inspRows(set, ready, pos, null, t, head);
    if (!used) return;
    pixelsFor(si).then(function (c) {
      c.done = true;
      if (si !== S.set) return;
      inspRows(set, c, pos, hasRoi ? roiStats(c, set, r) : null, t, head);
    });
  }
  function toImage(vp, e) {
    var rc = vp.getBoundingClientRect();
    return { x: (e.clientX - rc.left - S.tx) / S.scale, y: (e.clientY - rc.top - S.ty) / S.scale };
  }
  function cursorFrom(vp, e) {
    var p = toImage(vp, e), d = refDims(cur());
    var x = Math.floor(p.x), y = Math.floor(p.y);
    if (x < 0 || y < 0 || x >= d[0] || y >= d[1]) return;
    S.cursor = { x: x, y: y };
    renderInspector();
  }

  // ---------- pointer interaction ----------
  var ptrs = {}, gesture = null;
  function ptrCount() { return Object.keys(ptrs).length; }
  function onDown(e) {
    var vp = e.target.closest ? e.target.closest('.vp') : null;
    if (!vp) return;
    ptrs[e.pointerId] = { x: e.clientX, y: e.clientY };
    try { vp.setPointerCapture(e.pointerId); } catch (err) { /* ignore */ }
    if (ptrCount() === 2) { gesture = { kind: 'pinch', d: pinchDist(), vp: vp }; return; }
    var roi = S.tool === 'roi' || e.shiftKey;
    var hb = !roi && e.target.closest ? e.target.closest('.hsp') : null;
    var sw = !roi && !hb && S.layout === 'swipe' && vp.classList.contains('swipe') && !S.hold && (S.fit || nearDivider(vp, e));
    if (sw) { gesture = { kind: 'divider', vp: vp, moved: true }; swipeFromPointer(vp, e); vp.classList.add('drag'); return; }
    gesture = { kind: roi ? 'roi' : 'pan', vp: vp, sx: e.clientX, sy: e.clientY, tx: S.tx, ty: S.ty, moved: false };
    if (roi) { var p = toImage(vp, e); gesture.p0 = p; }
    if (hb) gesture.hot = [Number(hb.getAttribute('data-pane')), Number(hb.getAttribute('data-h'))];
    if (!roi) vp.classList.add('drag');
  }
  function pinchDist() {
    var k = Object.keys(ptrs); var a = ptrs[k[0]], b = ptrs[k[1]];
    return Math.hypot(a.x - b.x, a.y - b.y) || 1;
  }
  function onMove(e) {
    var vp = e.target.closest ? e.target.closest('.vp') : null;
    if (ptrs[e.pointerId]) { ptrs[e.pointerId].x = e.clientX; ptrs[e.pointerId].y = e.clientY; }
    if (!gesture) { if (vp && e.pointerType === 'mouse') cursorFrom(vp, e); return; }
    var rc = gesture.vp.getBoundingClientRect();
    if (gesture.kind === 'pinch' && ptrCount() >= 2) {
      var k = Object.keys(ptrs), a = ptrs[k[0]], b = ptrs[k[1]];
      var nd = pinchDist();
      zoomAt(nd / gesture.d, (a.x + b.x) / 2 - rc.left, (a.y + b.y) / 2 - rc.top);
      gesture.d = nd;
    } else if (gesture.kind === 'divider') {
      swipeFromPointer(gesture.vp, e);
      if (e.pointerType === 'mouse') cursorFrom(gesture.vp, e);
    } else if (gesture.kind === 'pan') {
      var dx = e.clientX - gesture.sx, dy = e.clientY - gesture.sy;
      if (Math.abs(dx) + Math.abs(dy) > 3) gesture.moved = true;
      if (gesture.moved) { S.fit = false; S.tx = gesture.tx + dx; S.ty = gesture.ty + dy; applyTransform(); }
      if (e.pointerType === 'mouse') cursorFrom(gesture.vp, e);
    } else if (gesture.kind === 'roi') {
      var p = toImage(gesture.vp, e), d = refDims(cur());
      var x0 = Math.max(0, Math.min(d[0], Math.min(gesture.p0.x, p.x))), x1 = Math.max(0, Math.min(d[0], Math.max(gesture.p0.x, p.x)));
      var y0 = Math.max(0, Math.min(d[1], Math.min(gesture.p0.y, p.y))), y1 = Math.max(0, Math.min(d[1], Math.max(gesture.p0.y, p.y)));
      S.roiDraft = { x: Math.floor(x0), y: Math.floor(y0), w: Math.ceil(x1) - Math.floor(x0), h: Math.ceil(y1) - Math.floor(y0) };
      drawRoi();
    }
  }
  function onUp(e) {
    delete ptrs[e.pointerId];
    var g = gesture;
    if (!g) return;
    if (g.kind === 'pinch') { if (ptrCount() < 2) gesture = null; return; }
    gesture = null;
    g.vp.classList.remove('drag');
    if (g.kind === 'roi') {
      var r = S.roiDraft; S.roiDraft = null;
      var en = entry(cur().name);
      en.roi = r && r.w >= 2 && r.h >= 2 ? r : null;
      touch(en);
      drawRoi(); renderInspector(); renderDecision();
    } else if (!g.moved) {
      if (g.hot) zoomHot(g.hot[0], g.hot[1]);
      else cursorFrom(g.vp, e);
    }
  }
  function onWheel(e) {
    var vp = e.target.closest ? e.target.closest('.vp') : null;
    if (!vp) return;
    e.preventDefault();
    var rc = vp.getBoundingClientRect();
    var dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaY;
    zoomAt(Math.exp(-dy * 0.0015), e.clientX - rc.left, e.clientY - rc.top);
  }

  // ---------- chrome: sets, toolbar, decision ----------
  function verdictText(v) { return v === 'needs-work' ? 'needs work' : v; }
  function badgeFor(set) {
    var e = dec[set.name];
    if (!isDecided(e)) return { text: 'undecided', cls: '' };
    if (D.blind && !S.revealed) return { text: 'judged', cls: '' };
    if (e.decision) return { text: verdictText(e.decision), cls: e.decision };
    return { text: e.no_difference ? 'no difference' : 'prefers ' + trueOf(e.chosen_label, set), cls: '' };
  }
  // ---------- triage: order, filter, status chips ----------
  var RANK = { error: 0, missing: 1, changed: 2, identical: 3 };
  function hasTriage() { return !D.blind && D.sets.some(function (s) { return s.status; }); }
  function hasFlip() { return hasTriage() && D.sets.some(function (s) { return s.worst_flip != null; }); }
  function byName(a, b) { return D.sets[a].name < D.sets[b].name ? -1 : D.sets[a].name > D.sets[b].name ? 1 : 0; }
  function worstKey(s) { return s.status === 'error' ? 0 : s.worst_flip > 0 ? 1 : s.status === 'missing' ? 2 : 3; }
  function computeOrder() {
    var idx = D.sets.map(function (_, i) { return i; });
    if (S.diffOnly && hasTriage()) idx = idx.filter(function (i) { return D.sets[i].status !== 'identical'; });
    if (S.hideSolo) idx = idx.filter(function (i) { return !soloLabel(D.sets[i]); });
    idx.sort(function (a, b) {
      var sa = D.sets[a], sb = D.sets[b];
      // Sets only one directory has cannot be compared: always last.
      var oa = soloLabel(sa) ? 1 : 0, ob = soloLabel(sb) ? 1 : 0;
      if (oa !== ob) return oa - ob;
      if (hasTriage() && S.sort === 'worst') {
        // errors, then measured differences by size, then sets with no FLIP value, then identical ones
        var d = worstKey(sa) - worstKey(sb);
        if (d) return d;
        var wa = sa.worst_flip == null ? -1 : sa.worst_flip, wb = sb.worst_flip == null ? -1 : sb.worst_flip;
        if (wa !== wb) return wb - wa;
      } else if (hasTriage() && S.sort === 'status') {
        var d2 = (RANK[sa.status] || 0) - (RANK[sb.status] || 0);
        if (d2) return d2;
      }
      return byName(a, b);
    });
    S.visible = idx;
  }
  function curPos() { return S.visible.indexOf(S.set); }
  function step(d) {
    var p = curPos(), q = p < 0 ? 0 : p + d;
    if (q >= 0 && q < S.visible.length) selectSet(S.visible[q]);
  }
  function chipFor(s) {
    if (!s.status) return null;
    var txt = s.status === 'changed' ? 'differs' : s.status;
    if (s.status === 'missing') {
      var solo = soloLabel(s);
      txt = solo ? 'only in ' + solo : 'missing in ' + labelsWhere(s, false).join(', ');
    }
    var f = s.worst_flip == null ? '' : ' ' + fmt(s.worst_flip, 4);
    return { cls: s.status, text: txt, flip: f.trim() };
  }
  function renderTriage() {
    var boxes = document.querySelectorAll('.triage');
    for (var b = 0; b < boxes.length; b++) fillTriage(boxes[b]);
  }
  function fillTriage(box) {
    box.textContent = '';
    box.hidden = !hasTriage();
    if (!hasTriage()) return;
    var sel = el('select', { 'aria-label': 'Order image sets' });
    [['worst', 'Worst first'], ['name', 'Name'], ['status', 'Status']].forEach(function (o) {
      if (o[0] === 'worst' && !hasFlip()) return;
      var op = el('option', { value: o[0], text: o[1] }); if (o[0] === S.sort) op.selected = true; sel.appendChild(op);
    });
    sel.addEventListener('change', function () { S.sort = sel.value; refreshOrder(); });
    var cb = el('input', { type: 'checkbox' }); cb.checked = S.diffOnly;
    cb.addEventListener('change', function () { S.diffOnly = cb.checked; refreshOrder(); });
    box.appendChild(el('label', { class: 'sortl' }, [el('span', { text: 'Order' }), sel]));
    box.appendChild(el('label', { class: 'difl' }, [cb, el('span', { text: 'Differences only' })]));
    var solos = D.sets.filter(function (s) { return soloLabel(s); }).length;
    if (solos) {
      var hs = el('input', { type: 'checkbox' }); hs.checked = !!S.hideSolo;
      hs.addEventListener('change', function () { S.hideSolo = hs.checked; refreshOrder(); });
      box.appendChild(el('label', { class: 'difl', title: 'Sets that only one directory has (nothing to compare)' }, [hs, el('span', { text: 'Hide unmatched (' + solos + ')' })]));
    }
  }
  // Re-sort; the current set stays selected while it is visible, else the first one is shown.
  function refreshOrder() {
    computeOrder();
    renderTriage();
    if (!S.visible.length) { renderSets(); return; }
    if (curPos() < 0) selectSet(S.visible[0]); else renderSets();
  }
  function setHasWarnings(s) { return s.panes.some(function (p) { return (p.warnings || []).length; }); }
  function renderWarnings() {
    var set = cur(), card = $('warn-card'), ul = $('warnlist');
    ul.textContent = '';
    set.panes.forEach(function (p, i) {
      (p.warnings || []).forEach(function (w) {
        var bad = /non-finite/.test(w);
        ul.appendChild(el('li', { class: bad ? 'nf' : '', text: paneName(set, i) + ': ' + w.replace(/^image /, '') }));
      });
      var pr = p.properties;
      if (pr && pr.negative_count) ul.appendChild(el('li', { text: paneName(set, i) + ': ' + pr.negative_count + ' negative samples' }));
    });
    card.hidden = !ul.children.length;
  }
  function renderSets() {
    var ol = $('setlist'), sel = $('setsel');
    ol.textContent = ''; sel.textContent = '';
    S.visible.forEach(function (i) {
      var s = D.sets[i];
      var b = badgeFor(s), c = chipFor(s);
      var line = el('span', { class: 'line' });
      if (c) {
        line.appendChild(el('span', { class: 'chip st-' + c.cls, text: c.text }));
        if (c.flip) line.appendChild(el('span', { class: 'fv', text: c.flip, title: 'Worst mean FLIP against the reference' }));
      }
      if (setHasWarnings(s)) line.appendChild(el('span', { class: 'chip st-error', text: '\u26A0 warning', title: 'An image is all black, all white or has non-finite samples' }));
      line.appendChild(el('span', { class: 'badge ' + b.cls, text: b.text }));
      var btn = el('button', { type: 'button', title: s.name }, [el('span', { class: 'n', text: s.name }), line]);
      if (i === S.set) btn.setAttribute('aria-current', 'true');
      btn.addEventListener('click', function () { selectSet(i); });
      ol.appendChild(el('li', null, [btn]));
      var o = el('option', { value: String(i), text: s.name + (c ? ' – ' + c.text + (c.flip ? ' ' + c.flip : '') : '') + ' – ' + b.text });
      if (i === S.set) o.selected = true;
      sel.appendChild(o);
    });
    if (!S.visible.length) ol.appendChild(el('li', { class: 'none', text: 'No sets differ from the reference.' }));
    var p = curPos();
    $('count').textContent = (p < 0 ? 0 : p + 1) + ' / ' + S.visible.length + (S.visible.length < D.sets.length ? ' (of ' + D.sets.length + ')' : '');
    $('prev').disabled = p <= 0;
    $('next').disabled = p < 0 || p >= S.visible.length - 1;
  }
  function renderChosen() {
    var set = cur(), box = $('chosen');
    box.textContent = '';
    present(set).forEach(function (i) {
      var cb = el('input', { type: 'checkbox' });
      cb.checked = S.chosen.indexOf(i) >= 0;
      cb.addEventListener('change', function () {
        S.chosen = present(set).filter(function (k) { return k === i ? cb.checked : S.chosen.indexOf(k) >= 0; });
        renderStage();
      });
      box.appendChild(el('label', null, [cb, paneName(set, i)]));
    });
  }
  function renderToolbar() {
    function mark(id, val) {
      var bs = document.querySelectorAll('#' + id + ' button');
      for (var i = 0; i < bs.length; i++) bs[i].setAttribute('aria-pressed', String(bs[i].getAttribute('data-v') === val));
    }
    mark('seg-layout', S.layout); mark('seg-tool', S.tool); mark('seg-chan', S.channel);
    notify();
    $('b-overlay').disabled = hideInfo();
    $('g-swipe').hidden = S.layout !== 'swipe';
    $('g-flick').hidden = S.layout !== 'flicker';
    $('g-over').hidden = S.layout !== 'overlay' && !(S.layout === 'swipe' && S.heat);
    $('b-heat').setAttribute('aria-pressed', String(S.heat));
    $('b-heat').disabled = hideInfo();
    $('b-vert').setAttribute('aria-pressed', String(S.vertical));
    $('swipe').value = String(S.swipe);
    $('b-hot').setAttribute('aria-pressed', String(S.hot));
    $('b-hot').disabled = hideInfo();
    $('hold').disabled = hideInfo() || S.layout === 'flicker';
    $('g-show').hidden = S.layout !== 'swipe' && S.layout !== 'flicker';
    $('ratev').textContent = S.rate + ' Hz';
    $('pause').setAttribute('aria-pressed', String(S.paused));
    $('pause').textContent = S.paused ? 'Resume' : 'Pause';
    renderChosen();
  }
  // Hotspots of the current set, each non-reference pane against the reference.
  function renderHotspots() {
    var set = cur(), card = $('hot-card'), t = $('hotlist');
    t.textContent = '';
    var rows = [];
    if (!hideInfo()) set.order.forEach(function (i) {
      (set.panes[i].hotspots || []).forEach(function (hs, k) { rows.push({ i: i, k: k, hs: hs }); });
    });
    card.hidden = !rows.length;
    if (!rows.length) return;
    t.appendChild(el('tr', null, ['Image', '#', 'Position', 'Size (px)', 'Share of error', 'Mean FLIP', 'Max FLIP'].map(function (c) { return el('th', { text: c }); })));
    rows.forEach(function (r) {
      var hs = r.hs;
      var tr = el('tr', { class: 'hrow', tabindex: '0', title: 'Zoom to this hotspot' }, [
        el('td', { text: paneName(set, r.i) }), el('td', { text: String(r.k + 1) }), el('td', { text: hs.position }),
        el('td', { text: hs.rect_px[2] + '×' + hs.rect_px[3] }),
        el('td', { text: (hs.share_of_total_error * 100).toFixed(1) + '%' }),
        el('td', { text: fmt(hs.mean_flip) }), el('td', { text: fmt(hs.max_flip) })]);
      var go = function () { zoomHot(r.i, r.k); };
      tr.addEventListener('click', go);
      tr.addEventListener('keydown', function (e) { if (e.key === 'Enter') { e.preventDefault(); go(); } });
      t.appendChild(tr);
    });
    var wide = set.order.filter(function (i) { return frameWide(set.panes[i]); });
    $('hot-hint').textContent = wide.length ? 'Frame-wide change: ' + wide.map(function (i) { return paneName(set, i); }).join(', ') + ' (the largest hotspot covers at least half of the frame).' : '';
  }
  function renderDecision() {
    var set = cur(), box = $('dec'), e = entry(set.name);
    box.textContent = '';
    function btn(text, on, fn) {
      var b = el('button', { type: 'button', class: 'btn', 'aria-pressed': String(on), text: text });
      b.addEventListener('click', fn);
      return b;
    }
    if (D.blind) {
      var row = el('div', { class: 'dec-row' }, [el('span', { class: 'lbl', text: 'Preferred image' })]);
      present(set).forEach(function (i) {
        row.appendChild(btn(letter(set, i), e.chosen_label === D.labels[i] && !e.no_difference, function () {
          e.chosen_label = D.labels[i]; e.no_difference = false; touch(e); afterDecision();
        }));
      });
      row.appendChild(btn('No visible difference', e.no_difference, function () {
        e.no_difference = true; e.chosen_label = null; touch(e); afterDecision();
      }));
      box.appendChild(row);
    }
    var row2 = el('div', { class: 'dec-row' }, [el('span', { class: 'lbl', text: 'Verdict' })]);
    [['accept', 'Accept'], ['reject', 'Reject'], ['needs-work', 'Needs work']].forEach(function (v) {
      row2.appendChild(btn(v[1], e.decision === v[0], function () {
        e.decision = e.decision === v[0] ? null : v[0]; touch(e); afterDecision();
      }));
    });
    box.appendChild(row2);
    var ta = el('textarea', { 'aria-label': 'Note', placeholder: 'Note (optional)' });
    ta.value = e.note || '';
    ta.addEventListener('input', function () { e.note = ta.value; touch(e); });
    box.appendChild(ta);
    var meta = el('p', { class: 'dec-meta' });
    meta.textContent = e.roi ? 'ROI saved: ' + e.roi.x + ',' + e.roi.y + ' ' + e.roi.w + '×' + e.roi.h : 'No ROI. Choose "Region" and drag, or Shift-drag.';
    box.appendChild(meta);
    var presets = cur().presets || [];
    if (presets.length) {
      var prow = el('div', { class: 'dec-row' }, [el('span', { class: 'lbl', text: 'Regions' })]);
      presets.forEach(function (p) {
        var pb = el('button', { type: 'button', class: 'btn small', text: p.name, title: p.x + ',' + p.y + ' ' + p.w + '×' + p.h });
        pb.addEventListener('click', function () {
          e.roi = { x: p.x, y: p.y, w: p.w, h: p.h }; touch(e); drawRoi(); renderInspector(); renderDecision();
        });
        prow.appendChild(pb);
      });
      box.appendChild(prow);
    }
    if (e.roi) {
      var clr = el('button', { type: 'button', class: 'btn small', text: 'Clear ROI' });
      clr.addEventListener('click', function () { e.roi = null; touch(e); drawRoi(); renderInspector(); renderDecision(); });
      box.appendChild(clr);
    }
    updateReveal();
    renderProposal();
  }
  function afterDecision() { renderSets(); renderDecision(); }
  function updateReveal() {
    var b = $('reveal');
    b.hidden = !D.blind || S.revealed;
    var left = D.sets.filter(function (s) { return !isDecided(dec[s.name]); }).length;
    b.disabled = left > 0;
    b.title = left > 0 ? left + ' image set(s) still undecided' : 'Pick blind-key.json to show which directory each image came from';
  }

  function selectSet(i) {
    if (i < 0 || i >= D.sets.length) return;
    S.set = i; S.cursor = null; S.roiDraft = null; S.flickerIdx = 0;
    S.chosen = present(cur());
    S.fit = true;
    renderSets(); renderToolbar(); renderStage(); renderDecision(); renderWarnings(); renderConfigDiff(); renderHotspots();
  }
  function rerenderAll() { renderSets(); renderToolbar(); renderStage(); renderDecision(); renderMeta(); renderWarnings(); renderConfigDiff(); renderHotspots(); }

  // Metadata-sidecar differences of the current set, each pane against the reference.
  function renderConfigDiff() {
    var set = cur(), card = $('cfg-card'), t = $('cfgdiff'), hint = $('cfg-hint');
    t.textContent = '';
    var cols = [], vals = {}, keys = [];
    set.panes.forEach(function (p, i) {
      (p.meta_diff || []).forEach(function (d) {
        if (keys.indexOf(d.key) < 0) keys.push(d.key);
        (vals[d.key] = vals[d.key] || { ref: d.baseline })[i] = d.capture;
        if (cols.indexOf(i) < 0) cols.push(i);
      });
    });
    var errs = [];
    set.panes.forEach(function (p, i) { if (p.meta_error) errs.push(D.labels[i] + ': ' + p.meta_error); });
    card.hidden = !keys.length && !errs.length;
    hint.textContent = errs.join(' | ');
    if (!keys.length) return;
    keys.sort();
    cols.sort(function (a, b) { return a - b; });
    var head = el('tr', null, [el('th', { text: 'Key' }), el('th', { text: D.labels[D.reference] + ' (ref)' })]);
    cols.forEach(function (i) { head.appendChild(el('th', { text: D.labels[i] })); });
    t.appendChild(head);
    keys.forEach(function (k) {
      var row = el('tr', null, [el('td', { class: 'mk', text: k }), el('td', { class: 'mv' + (vals[k].ref === '<absent>' ? ' na' : ''), text: vals[k].ref })]);
      cols.forEach(function (i) {
        var v = vals[k][i] === undefined ? vals[k].ref : vals[k][i];
        row.appendChild(el('td', { class: 'mv' + (v === '<absent>' ? ' na' : '') + (v !== vals[k].ref ? ' diff' : ''), text: v }));
      });
      t.appendChild(row);
    });
  }

  function renderMeta() {
    var m = $('meta'); m.textContent = '';
    function add(k, v) { m.appendChild(el('div', null, [el('dt', { text: k }), el('dd', { text: v })])); }
    if (D.blind && !S.revealed) add('Mode', 'blind (labels hidden)');
    else add('Directories', D.labels.map(function (l, i) { return trueLabel(i) + (i === D.reference && !D.blind ? ' (ref)' : ''); }).join(', '));
    add('Sets', String(D.sets.length));
    add('Seed', String(D.seed));
    add('Version', D.tool_version);
  }

  // ---------- export ----------
  function exportDecisions() {
    var out = buildDecisions();
    var blob = new Blob([JSON.stringify(out, null, 2) + '\n'], { type: 'application/json' });
    var u = URL.createObjectURL(blob);
    var a = el('a', { href: u, download: 'flipdiff-decisions.v1.json' });
    document.body.appendChild(a); a.click(); a.remove();
    setTimeout(function () { URL.revokeObjectURL(u); }, 4000);
  }

  // ---------- wiring ----------
  function seg(id, attr, fn) {
    $(id).addEventListener('click', function (e) {
      var b = e.target.closest('button');
      if (b && !b.disabled) fn(b.getAttribute(attr));
    });
  }
  function typing(t) {
    if (!t || !t.tagName) return false;
    if (t.isContentEditable || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT') return true;
    return t.tagName === 'INPUT' && !/^(range|checkbox|radio|button)$/.test(t.type);
  }
  function toggleVertical() { S.vertical = !S.vertical; renderToolbar(); paintSwipe(); }
  function toggleHeat() { S.heat = !S.heat; renderToolbar(); renderStage(); }
  function onKey(e) {
    if (e.ctrlKey || e.metaKey || e.altKey || typing(e.target)) return;
    var k = e.key, amt = e.shiftKey ? 1 : 5;
    if (k === 'Escape') { if ($('help') && !$('help').hidden) toggleHelp(false); else if (S.fs) setFullscreen(false); return; }
    if (k === '?') { e.preventDefault(); toggleHelp(); return; }
    if (k === 'ArrowLeft' || k === 'ArrowRight') {
      var dir = k === 'ArrowLeft' ? -1 : 1;
      if (S.layout === 'swipe') { e.preventDefault(); setSwipe(S.swipe + dir * amt); }
      else step(dir);
    } else if (k === '[') step(-1);
    else if (k === ']') step(1);
    else if (k === ' ') {
      e.preventDefault();
      if (e.repeat) return;
      if (S.layout === 'flicker') S.layout = S.lastLayout;
      else { S.lastLayout = S.layout; S.layout = 'flicker'; }
      renderToolbar(); renderStage();
    } else if (k === 'v' || k === 'V') { if (S.layout === 'swipe') toggleVertical(); }
    else if (k === 'h' || k === 'H') { if (S.layout === 'swipe') toggleHeat(); }
    else if (k === 'f' || k === 'F') setFullscreen(!fsActive());
    else if (k === 'c' || k === 'C') { if (!e.repeat) holdStart(); }
    else if (k === '0') setZoom('fit');
    else if (k === '1' || k === '2' || k === '4' || k === '8') setZoom(k);
    else if (k === 'y' || k === 'Y' || k === 'n' || k === 'N') {
      var ap = proposal();
      if (ap) setVerdict((k === 'y' || k === 'Y') === (ap.answer === 'accept') ? 'accept' : 'reject');
    }
  }

  // ---------- agent API: hash state, window.flipdiff, proposed decisions ----------
  var A = window.__flipdiffAgent;
  var LAYOUT_OUT = { grid: 'side', overlay: 'heatmap', swipe: 'swipe', flicker: 'flicker' };
  var LAYOUT_IN = { side: 'grid', heatmap: 'overlay', swipe: 'swipe', flicker: 'flicker' };
  function tkey() { return [S.tx.toFixed(2), S.ty.toFixed(2), S.scale.toFixed(4)].join(); }
  // The pane whose hotspots `hotspot=n` counts: the first non-reference image.
  function primaryPane() {
    var set = cur();
    for (var k = 0; k < set.order.length; k++) {
      var i = set.order[k];
      if (i !== D.reference && set.panes[i].path) return i;
    }
    return -1;
  }
  function agentGet() {
    var set = cur(), vw = vpWidth(), d = refDims(set), vh = vw * d[1] / d[0], r = roiRect();
    var heat = !hideInfo() && (S.layout === 'overlay' || (S.layout === 'swipe' && S.heat)) ? S.opacity : 0;
    return {
      set: set.name, layout: LAYOUT_OUT[S.layout] || 'side', split: Math.round(S.swipe * 10) / 1000, vertical: S.vertical,
      zoom: S.fit ? 'fit' : Math.round(S.scale * 1000) / 1000,
      at: S.fit ? null : [Math.round((vw / 2 - S.tx) / S.scale), Math.round((vh / 2 - S.ty) / S.scale)],
      heat: heat, channel: S.channel === 'l' ? 'luma' : S.channel, ev: Math.round(S.exposure * 100) / 100,
      roi: r && r.w > 0 && r.h > 0 ? [r.x, r.y, r.w, r.h] : null,
      hotspot: !hideInfo() && S.hsel && S.hselT === tkey() ? S.hsel : null
    };
  }
  function agentApply(p) {
    if (p.set != null) {
      var idx = -1;
      D.sets.forEach(function (s, i) { if (s.name === p.set) idx = i; });
      if (idx >= 0 && idx !== S.set) {
        if (S.visible.indexOf(idx) < 0) { S.diffOnly = false; computeOrder(); renderTriage(); }
        selectSet(idx);
      }
    }
    if (p.layout) { S.layout = LAYOUT_IN[p.layout]; if (S.layout === 'overlay' && hideInfo()) S.layout = 'grid'; }
    if (p.split != null) { S.swipe = Math.round(p.split * 1000) / 10; $('swipe').value = String(S.swipe); }
    if (p.vertical != null) S.vertical = p.vertical;
    if (p.channel) S.channel = p.channel === 'luma' ? 'l' : p.channel;
    if (p.ev != null) { S.exposure = p.ev; $('exp').value = String(p.ev); }
    if (p.heat != null && !hideInfo()) {
      if (p.heat > 0) { S.opacity = p.heat; $('opa').value = String(p.heat); if (S.layout === 'swipe') S.heat = true; }
      else if (S.layout === 'swipe') S.heat = false;
    }
    renderToolbar(); renderStage(); applyFilter();
    var vw = vpWidth(), d = refDims(cur()), vh = vw * d[1] / d[0];
    if (p.zoom === 'fit') setZoom('fit');
    else if (p.zoom != null || (p.at && !S.fit)) {
      S.fit = false;
      S.scale = Math.max(fitScale(), p.zoom != null ? p.zoom : S.scale);
      var at = p.at || [d[0] / 2, d[1] / 2];
      S.tx = vw / 2 - at[0] * S.scale; S.ty = vh / 2 - at[1] * S.scale;
      applyTransform();
    }
    if ('roi' in p) {
      if (p.roi) S.roiDraft = { x: p.roi[0], y: p.roi[1], w: p.roi[2], h: p.roi[3] };
      else { S.roiDraft = null; var en = dec[cur().name]; if (en && en.roi) { en.roi = null; touch(en); } }
      drawRoi(); renderInspector();
    }
    if (p.hotspot && !hideInfo()) { var pi = primaryPane(); if (pi >= 0) zoomHot(pi, p.hotspot - 1); }
  }
  // The stage as a canvas: images with the current transform, the swipe split, display filter, boxes and labels.
  function drawVp(g, vp, sr, filter) {
    var r = vp.getBoundingClientRect(), ox = r.left - sr.left, oy = r.top - sr.top;
    g.save(); g.beginPath(); g.rect(ox, oy, r.width, r.height); g.clip();
    for (var i = 0; i < vp.children.length; i++) {
      var w = vp.children[i];
      if (!w.classList.contains('wrap') || w.classList.contains('hotwrap') || w.classList.contains('holdwrap') || w.style.visibility === 'hidden') continue;
      var base = w.querySelector('img.base'), heat = w.querySelector('img.heat');
      if (!base) continue;
      var bw = Number(base.getAttribute('width')), bh = Number(base.getAttribute('height'));
      g.save();
      if (w.id === 'swipe-top') {
        g.beginPath();
        if (S.vertical) g.rect(ox, oy + r.height * S.swipe / 100, r.width, r.height); else g.rect(ox + r.width * S.swipe / 100, oy, r.width, r.height);
        g.clip();
      }
      g.filter = filter && filter !== 'none' ? filter : 'none';
      g.drawImage(base, ox + S.tx, oy + S.ty, bw * S.scale, bh * S.scale);
      if (heat) { g.globalAlpha = S.opacity; g.drawImage(heat, ox + S.tx, oy + S.ty, bw * S.scale, bh * S.scale); }
      g.restore();
    }
    var dv = vp.querySelector('.divider');
    if (dv) {
      g.fillStyle = '#fff'; g.strokeStyle = '#000'; g.lineWidth = 1;
      var pos = (S.vertical ? r.height : r.width) * S.swipe / 100;
      if (S.vertical) { g.fillRect(ox, oy + pos - 1, r.width, 2); } else { g.fillRect(ox + pos - 1, oy, 2, r.height); }
    }
    Array.prototype.forEach.call(vp.querySelectorAll('.hsp, .roi'), function (b) {
      if (b.hidden) return;
      var br = b.getBoundingClientRect();
      g.lineWidth = 2; g.strokeStyle = b.classList.contains('roi') ? '#00c8ff' : '#ffb000';
      g.strokeRect(br.left - sr.left + 1, br.top - sr.top + 1, br.width - 2, br.height - 2);
      if (b.classList.contains('hsp')) { g.fillStyle = '#ffb000'; g.font = 'bold 12px sans-serif'; g.fillText(b.textContent, br.left - sr.left + 4, br.top - sr.top + 14); }
    });
    Array.prototype.forEach.call(vp.querySelectorAll('.corner'), function (c) {
      var cr = c.getBoundingClientRect();
      g.fillStyle = 'rgba(0,0,0,0.7)'; g.fillRect(cr.left - sr.left, cr.top - sr.top, cr.width, cr.height);
      g.fillStyle = '#fff'; g.font = '12px sans-serif'; g.fillText(c.textContent, cr.left - sr.left + 5, cr.top - sr.top + cr.height * 0.72);
    });
    g.restore();
  }
  function agentCanvas() {
    var st = $('stage'), sr = st.getBoundingClientRect();
    var imgs = Array.prototype.slice.call(st.querySelectorAll('img.base, img.heat'));
    return Promise.all(imgs.map(function (im) { return im.decode ? im.decode().catch(function () { /* drawn blank */ }) : null; })).then(function () {
      var c = document.createElement('canvas');
      c.width = Math.max(1, Math.round(sr.width)); c.height = Math.max(1, Math.round(sr.height));
      var g = c.getContext('2d');
      g.fillStyle = getComputedStyle(document.body).backgroundColor || '#000';
      g.fillRect(0, 0, c.width, c.height);
      g.imageSmoothingEnabled = !(S.scale > 1.0001);
      var f = st.style.getPropertyValue('--f');
      Array.prototype.forEach.call(st.querySelectorAll('.vp'), function (vp) { drawVp(g, vp, sr, f); });
      Array.prototype.forEach.call(st.querySelectorAll('.pane-h b'), function (b) {
        var br = b.getBoundingClientRect();
        g.fillStyle = getComputedStyle(document.body).color; g.font = 'bold 13px sans-serif';
        g.fillText(b.textContent, br.left - sr.left, br.top - sr.top + 13);
      });
      return c;
    });
  }
  function mergeDecisions(d) {
    if (!d || !Array.isArray(d.sets)) return;
    d.sets.forEach(function (s) {
      props[s.name] = s.proposals || [];
      if (s.decision && !(dec[s.name] && dec[s.name].decision)) {
        var e = entry(s.name);
        e.decision = s.decision;
        if (!e.note && s.note) e.note = s.note;
      }
    });
  }
  function proposal() {
    var ap = A.acceptProposal(props[cur().name]);
    return ap && (ap.answer === 'accept' || ap.answer === 'reject') ? ap : null;
  }
  function setVerdict(v) { var e = entry(cur().name); e.decision = v; touch(e); afterDecision(); }
  // The chip for what an agent proposed for this set, above the stage.
  function renderProposal() {
    var slot = $('agent-slot');
    if (!slot) return;
    slot.textContent = '';
    var list = props[cur().name] || [];
    slot.hidden = !list.length;
    if (list.length) slot.appendChild(A.proposalBar(list, (dec[cur().name] || {}).decision, setVerdict).el);
  }

  function init() {
    load();
    mergeDecisions(window.__flipdiffDecisions);
    var sv0 = window.FLIPDIFF_SERVE;
    if (sv0 && sv0.session) {
      fetch('/api/session/' + sv0.session + '/decisions').then(function (r) { return r.ok ? r.json() : null; })
        .then(function (d) { if (d) { mergeDecisions(d); renderSets(); renderDecision(); } }).catch(function () { /* offline: the page works without proposals */ });
    }
    if (!D.sets.length) { $('stage').appendChild(el('p', { class: 'note-msg', text: 'No images found in the given directories.' })); return; }
    seg('seg-layout', 'data-v', function (v) { S.layout = v; renderToolbar(); renderStage(); });
    seg('seg-tool', 'data-v', function (v) { S.tool = v; renderToolbar(); $('stage').classList.toggle('tool-roi', v === 'roi'); });
    seg('seg-chan', 'data-v', function (v) { S.channel = v; renderToolbar(); applyFilter(); });
    seg('seg-zoom', 'data-z', function (v) { setZoom(v); });
    $('exp').addEventListener('input', function (e) { S.exposure = Number(e.target.value); applyFilter(); });
    $('con').addEventListener('input', function (e) { S.contrast = Number(e.target.value); applyFilter(); });
    $('reset-view').addEventListener('click', function () {
      S.exposure = 0; S.contrast = 1; S.channel = 'rgb'; $('exp').value = '0'; $('con').value = '1';
      renderToolbar(); applyFilter();
    });
    $('swipe').addEventListener('input', function (e) { setSwipe(Number(e.target.value)); });
    $('b-vert').addEventListener('click', function () { toggleVertical(); });
    $('b-heat').addEventListener('click', function () { toggleHeat(); });
    $('b-hot').addEventListener('click', function () { S.hot = !S.hot; renderToolbar(); renderStage(); });
    $('fs').addEventListener('click', function () { setFullscreen(!fsActive()); });
    $('help-btn').addEventListener('click', function () { toggleHelp(); });
    var hb = $('hold');
    hb.addEventListener('pointerdown', function (e) { e.preventDefault(); try { hb.setPointerCapture(e.pointerId); } catch (err) { /* ignore */ } holdStart(); });
    ['pointerup', 'pointercancel', 'lostpointercapture'].forEach(function (n) { hb.addEventListener(n, holdEnd); });
    hb.addEventListener('contextmenu', function (e) { e.preventDefault(); });
    hb.addEventListener('keydown', function (e) { if (e.key === 'Enter') { e.preventDefault(); holdStart(); } });
    hb.addEventListener('keyup', function (e) { if (e.key === 'Enter') holdEnd(); });
    window.addEventListener('blur', holdEnd);
    document.addEventListener('fullscreenchange', paintFs);
    $('rate').addEventListener('input', function (e) { S.rate = Number(e.target.value); renderToolbar(); renderStage(); });
    $('pause').addEventListener('click', function () { S.paused = !S.paused; renderToolbar(); renderStage(); });
    $('opa').addEventListener('input', function (e) { S.opacity = Number(e.target.value); $('stage').style.setProperty('--opa', S.opacity); });
    $('prev').addEventListener('click', function () { step(-1); });
    $('next').addEventListener('click', function () { step(1); });
    $('setsel').addEventListener('change', function (e) { selectSet(Number(e.target.value)); });
    $('export').addEventListener('click', exportDecisions);
    $('reveal').addEventListener('click', function () { $('keyfile').click(); });
    $('keyfile').addEventListener('change', function (e) {
      var f = e.target.files && e.target.files[0];
      e.target.value = '';
      if (!f) return;
      var r = new FileReader();
      r.onload = function () {
        try {
          var k = JSON.parse(String(r.result));
          if (!k || !Array.isArray(k.labels) || k.labels.length !== D.labels.length || k.seed !== D.seed) {
            window.alert('That file is not the blind key of this view (labels or seed differ).');
            return;
          }
          KEY = k.labels; KEYSETS = k.sets || null; S.revealed = true; rerenderAll();
        } catch (err) { window.alert('Could not read the blind key: ' + err); }
      };
      r.readAsText(f);
    });
    var st = $('stage');
    st.addEventListener('pointerdown', onDown);
    st.addEventListener('pointermove', onMove);
    st.addEventListener('pointerup', onUp);
    st.addEventListener('pointercancel', onUp);
    st.addEventListener('wheel', onWheel, { passive: false });
    window.addEventListener('resize', function () { if (S.fit) applyTransform(); else { clampT(); applyTransform(); } });
    document.addEventListener('keydown', onKey);
    document.addEventListener('keyup', function (e) {
      if (e.key === 'c' || e.key === 'C') holdEnd();
      else if (e.key === ' ' && !typing(e.target)) e.preventDefault();
    });
    renderMeta();
    S.sort = hasFlip() ? 'worst' : 'name';
    var bl = window.FLIPDIFF_SERVE && window.FLIPDIFF_SERVE.browse;
    if (bl) { $('browse').href = bl; $('browse').hidden = false; }
    computeOrder();
    renderTriage();
    selectSet(S.visible.length ? S.visible[0] : 0);
    agent = A.create({
      nameKey: 'set', blind: D.blind, defaults: { split: 0.5 },
      get: agentGet, apply: agentApply,
      names: function () {
        return D.sets.map(function (s) { return { name: s.name, status: hideInfo() ? null : (s.status || null), value: hideInfo() || s.worst_flip == null ? null : s.worst_flip }; });
      },
      step: step, canvas: agentCanvas
    });
    A.bindCopy($('copy-link'), agent);
    agent.applyHash();
  }
  init();
})();
