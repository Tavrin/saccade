(function () {
  'use strict';
  var D = JSON.parse(document.getElementById('flipdiff-data').textContent);
  var UI = window.__flipdiffUI;
  var $ = function (id) { return document.getElementById(id); };
  var LS_KEY = 'flipdiff-view.v1:' + D.id;
  var LETTERS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';

  var S = {
    set: 0, visible: [], sort: 'name', diffOnly: false, layout: 'grid', tool: 'pan', scale: 1, tx: 0, ty: 0, fit: true,
    exposure: 0, contrast: 1, channel: 'rgb', opacity: 0.6, rate: 2, paused: false,
    swipe: 50, vertical: false, layer: 'none', mask: false, insp: false, hot: true, hold: false, lastLayout: 'swipe', fs: false, revealed: false, chosen: [], flickerIdx: 0, cursor: null, roiDraft: null, hsel: null, hselT: ''
  };
  var dec = {};           // name -> SetDecision-shaped entry
  var pixCache = {};      // set index -> {rgb:[ImageData|null], flip:[Uint8Array|null], promise}
  var timer = null;
  var viewports = [];
  var agent = null;       // window.flipdiff (see agentapi.js)
  var props = {};         // set name -> proposals recorded by `flipdiff decide`
  var reg = UI.use(UI.actions());
  var fsx = null;         // full-screen controller of the stage
  var pop = {};           // the Display and Tools popovers
  var HAS = { signed: false, mask: false, hot: false };  // does any set carry this data
  function notify() { if (agent) agent.changed(); }

  // ---------- helpers ----------
  function el(tag, attrs, kids) { return UI.h.apply(null, [tag, attrs].concat(kids || [])); }
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
  // Diagnostics data: the findings per pane, the signed-difference and non-finite images.
  function hasSigned(set) { return !hideInfo() && set.panes.some(function (p) { return p.signed_diff; }); }
  function hasMask(set) { return !hideInfo() && set.panes.some(function (p) { return p.nonfinite_mask; }); }
  function overlayOf(p, kind) { return kind === 'signed' ? p.signed_diff : p.heatmap; }
  // The overlay kind drawn in the current layout: 'heat', 'signed' or null.
  function layerKind() {
    if (hideInfo()) return null;
    if (S.layout === 'overlay') return 'heat';
    if (S.layout === 'diff') return 'signed';
    return S.layout === 'swipe' && S.layer !== 'none' ? S.layer : null;
  }
  // The non-reference pane of a swipe pair that carries the overlay.
  function overlayPane(set, a, b, kind) {
    if (b !== D.reference && overlayOf(set.panes[b], kind)) return b;
    if (a !== D.reference && overlayOf(set.panes[a], kind)) return a;
    return -1;
  }
  // The pane with the largest mean FLIP against the reference (the one whose findings lead the set).
  function worstPane(set) {
    var best = -1, bv = -1;
    set.panes.forEach(function (p, i) {
      if (i === D.reference || !p.metrics) return;
      if (p.metrics.mean > bv) { bv = p.metrics.mean; best = i; }
    });
    return best;
  }

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

  // ---------- pixels (inspector, status bar and ROI) ----------
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
  // Pixel data is loaded per set, only once a pixel or a region is first inspected. The
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
    var d = refDims(cur()), s = S.scale, vw = vpWidth();
    var t = UI.xf.clamp(S.tx, S.ty, d[0] * s, d[1] * s, vw, vw * d[1] / d[0]);
    S.tx = t[0]; S.ty = t[1];
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
    var zb = document.querySelectorAll('#seg-zoom button');
    for (var j = 0; j < zb.length; j++) {
      var z = zb[j].getAttribute('data-z');
      zb[j].setAttribute('aria-pressed', String(z === 'fit' ? S.fit : (!S.fit && Math.abs(S.scale - Number(z)) < 1e-6)));
    }
    renderStatus();
    notify();
  }
  function zoomAt(k, cx, cy) {
    var ns = Math.max(fitScale(), Math.min(64, S.scale * k));
    k = ns / S.scale;
    var t = UI.xf.about(S.tx, S.ty, k, cx, cy);
    S.tx = t[0]; S.ty = t[1];
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
    var t = UI.xf.about(S.tx, S.ty, ns / S.scale, vw / 2, vh / 2);
    S.tx = t[0]; S.ty = t[1]; S.scale = ns;
    applyTransform();
  }

  // ---------- display filter ----------
  function applyFilter() {
    $('stage').style.setProperty('--f', UI.filterCss(S.exposure, S.contrast, S.channel));
    $('expv').textContent = (S.exposure > 0 ? '+' : '') + S.exposure.toFixed(1) + ' EV';
    $('conv').textContent = '×' + S.contrast.toFixed(2);
    $('display-dot').hidden = S.exposure === 0 && S.contrast === 1 && S.channel === 'rgb';
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
  // Boxes around the clusters of non-finite samples of a pane, shown with the mask.
  function nfBoxes(p) {
    var nf = p.diagnostics && p.diagnostics.nonfinite;
    if (!S.mask || hideInfo() || !nf) return [];
    return (nf.clusters || []).map(function (c, k) {
      var b = el('div', { class: 'nfb', title: c.pixels + ' non-finite pixel(s)' }, [el('span', { text: 'NF ' + (k + 1) })]);
      b.style.left = c.rect_px[0] + 'px'; b.style.top = c.rect_px[1] + 'px'; b.style.width = c.rect_px[2] + 'px'; b.style.height = c.rect_px[3] + 'px';
      return b;
    });
  }
  // One image layer. o.kind picks the overlay (heat or signed) of the pane itself; o.path
  // overrides it (a swipe's overlay belongs to the other pane); o.noBoxes leaves the boxes out.
  function layerFor(set, i, o) {
    o = o || {};
    var p = set.panes[i];
    var layer = el('div', { class: 'layer' });
    layer.style.width = p.width + 'px'; layer.style.height = p.height + 'px';
    layer.appendChild(el('img', { class: 'base', src: url(p.path), width: String(p.width), height: String(p.height), alt: paneName(set, i) + ' – ' + set.name, draggable: 'false' }));
    var op = o.path !== undefined ? o.path : (o.kind ? overlayOf(p, o.kind) : null);
    if (op) layer.appendChild(el('img', { class: 'heat', src: url(op), width: String(p.width), height: String(p.height), alt: '', draggable: 'false' }));
    if (S.mask && !hideInfo() && p.nonfinite_mask) layer.appendChild(el('img', { class: 'mask', src: url(p.nonfinite_mask), width: String(p.width), height: String(p.height), alt: '', draggable: 'false' }));
    layer.appendChild(el('div', { class: 'roi', hidden: '' }));
    if (!o.noBoxes) hotBoxes(p, i).concat(nfBoxes(p)).forEach(function (b) { layer.appendChild(b); });
    return layer;
  }
  function wrapFor(set, i, o) {
    var w = el('div', { class: 'wrap' });
    w.appendChild(layerFor(set, i, o));
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
    var gridLike = S.layout === 'grid' || S.layout === 'overlay' || S.layout === 'diff';
    stage.classList.toggle('single', !gridLike);
    var ch = S.chosen.filter(function (i) { return set.panes[i].path; });

    if (gridLike) {
      var kind = layerKind();
      if (S.layout === 'diff' && !hasSigned(set)) stage.appendChild(el('p', { class: 'note-msg', text: 'No signed difference for this set: the images are identical, only one is present, or diagnostics are off.' }));
      else set.order.forEach(function (i) {
        var p = set.panes[i];
        if (!p.path) { stage.appendChild(missingPane(set, i)); return; }
        stage.appendChild(el('div', { class: 'pane' }, [paneHead(set, i), vpEl([wrapFor(set, i, { kind: kind })])]));
      });
    } else if (S.layout === 'swipe') {
      if (ch.length < 2) { stage.appendChild(el('p', { class: 'note-msg', text: needTwo(set, 'Swipe', 'Swipe needs two images: tick two under "Compare".') })); }
      else {
        var a = ch[0], b = ch[1], lk = layerKind();
        var op = lk ? overlayPane(set, a, b, lk) : -1;
        var wb = wrapFor(set, b, { path: op >= 0 ? overlayOf(set.panes[op], lk) : null, noBoxes: true });
        wb.id = 'swipe-top';
        var div = el('div', { class: 'divider', id: 'swipe-div' }, [el('i', { class: 'grip' })]);
        // Hotspot boxes sit in their own unclipped layer, so the divider never hides them.
        var hp = b === D.reference ? a : b;
        var hw = el('div', { class: 'wrap hotwrap' });
        var hl = el('div', { class: 'layer' });
        hl.style.width = set.panes[hp].width + 'px'; hl.style.height = set.panes[hp].height + 'px';
        hotBoxes(set.panes[hp], hp).concat(nfBoxes(set.panes[hp])).forEach(function (x) { hl.appendChild(x); });
        hw.appendChild(hl);
        var v = vpEl([wrapFor(set, a, { noBoxes: true }), wb, hw, div,
          el('span', { class: 'corner l', text: paneName(set, a) }), el('span', { class: 'corner r', text: paneName(set, b) })]);
        v.classList.add('swipe');
        v.classList.toggle('vert', S.vertical);
        stage.appendChild(el('div', { class: 'pane' }, [el('div', { class: 'pane-h' }, [el('b', { text: paneName(set, a) + ' | ' + paneName(set, b), title: paneName(set, a) + ' | ' + paneName(set, b) }), el('span', { text: '' })]), v]));
        paintSwipe(v);   // after attaching: it looks the layers up by id
      }
    } else if (S.layout === 'flicker') {
      if (ch.length < 2) { stage.appendChild(el('p', { class: 'note-msg', text: needTwo(set, 'Flicker', 'Flicker needs at least two images: tick them under "Compare".') })); }
      else {
        var wraps = ch.map(function (i) { return wrapFor(set, i); });
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
    $('fs-exit').addEventListener('click', function () { fsx.set(false); });
    applyFilter();
    stage.style.setProperty('--opa', S.opacity);
    applyTransform();
    drawRoi();
    renderInspector();
    renderLegend();
  }

  // Colour scales of what the stage draws: the FLIP heatmap, the signed difference, the non-finite mask.
  function renderLegend() {
    var box = $('legend'), set = cur(), k = layerKind();
    box.textContent = '';
    if (k === 'heat') box.appendChild(el('span', { class: 'legend flip', title: 'FLIP error from 0 (dark) to 1 (light)' }, ['0', el('i'), '1 FLIP']));
    if (k === 'signed') {
      var scales = [];
      set.panes.forEach(function (p, i) { if (i !== D.reference && p.diagnostics && p.diagnostics.signed && p.signed_diff) scales.push(p.diagnostics.signed.scale); });
      var s = scales.length ? scales[0] : null;
      var same = scales.every(function (x) { return x === scales[0]; });
      if (s != null) box.appendChild(el('span', { class: 'legend signed', title: 'Luminance change mapped to full colour' + (same ? '' : ' (each image has its own scale)') }, ['−' + UI.fmtScale(s) + ' darker', el('i'), 'brighter +' + UI.fmtScale(s) + (same ? '' : '*')]));
    }
    if (S.mask && hasMask(set)) box.appendChild(el('span', { class: 'legend mask' }, [el('i', { class: 'nan' }), 'NaN ', el('i', { class: 'inf' }), 'Inf ', el('i', { class: 'neg' }), 'negative']));
    box.hidden = !box.children.length;
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
    var ns = Math.max(fitScale(), Math.min(64, UI.xf.fitBox(r[2], r[3], vw, vh, 4, 0.8)));
    var t = UI.xf.centerOn(r[0] + r[2] / 2, r[1] + r[3] / 2, ns, vw, vh);
    S.fit = Math.abs(ns - fitScale()) < 1e-6;
    S.scale = ns; S.tx = t[0]; S.ty = t[1];
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
      var w = wrapFor(set, D.reference);
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

  // ---------- inspector and status bar ----------
  function inspectorNote(set) {
    var f = 1;
    set.panes.forEach(function (p) { if (p.inspector_scale > f) f = p.inspector_scale; });
    return f > 1 ? 'Inspector at 1/' + f + ' res: values are ' + f + '×' + f + ' averages (data capped at 2048 px).' : '';
  }
  // The colour and FLIP value of pane i at `pos`, from the loaded pixel data `c` (null while loading).
  function sample(set, c, i, pos) {
    var p = set.panes[i], out = { rgb: null, rgbTxt: '–', flipTxt: '–' };
    if (!p.path) { out.rgbTxt = 'missing'; return out; }
    if (!pos) return out;
    var id = c && c.rgb[i], inside = pos.x < p.width && pos.y < p.height;
    if (id && inside) {
      var k = ix(c, set, i, pos.x, pos.y) * 4;
      out.rgb = [id.data[k], id.data[k + 1], id.data[k + 2]];
      out.rgbTxt = UI.rgbText(out.rgb);
    } else if (!c) out.rgbTxt = '…';
    else if (!id) out.rgbTxt = 'n/a';
    var fm = c && c.flip[i];
    if (hideInfo()) out.flipTxt = 'hidden';
    else if (i === D.reference) out.flipTxt = 'ref';
    else if (fm && inside) out.flipTxt = (fm[ix(c, set, i, pos.x, pos.y)] / 255).toFixed(3);
    return out;
  }
  function inspRows(set, c, pos, stats, t, head) {
    t.textContent = '';
    t.appendChild(head);
    set.order.forEach(function (i) {
      var s = sample(set, c, i, pos);
      var sp = el('span', { class: 'sw' }); if (s.rgb) sp.style.background = 'rgb(' + s.rgb.join(',') + ')';
      var tr = el('tr', null, [el('td', null, [sp, paneName(set, i)]), el('td', { text: s.rgbTxt }), el('td', { text: s.flipTxt })]);
      if (stats) {
        var st = stats[i];
        tr.appendChild(el('td', { text: st ? st.r.toFixed(1) + ' ' + st.g.toFixed(1) + ' ' + st.b.toFixed(1) : '–' }));
        tr.appendChild(el('td', { text: hideInfo() ? 'hidden' : (i === D.reference ? 'ref' : (st && st.flip != null ? st.flip.toFixed(4) : '–')) }));
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
    $('insp-card').hidden = !(S.insp || hasRoi);
    if (hasRoi) { head.appendChild(el('th', { text: 'ROI mean RGB' })); head.appendChild(el('th', { text: 'ROI mean FLIP' })); }
    hint.textContent = pos ? 'Pixel (' + pos.x + ', ' + pos.y + ')' : 'Hover (or tap) an image to read pixel values.';
    if (hasRoi) hint.textContent += (pos ? ' · ' : '') + 'ROI ' + r.x + ',' + r.y + ' ' + r.w + '×' + r.h + ' (' + (r.w * r.h) + ' px)';
    var note = inspectorNote(set);
    if (note) hint.textContent += ' · ' + note;
    // No pixel data is fetched until a pixel or a region is actually inspected.
    var used = !!pos || hasRoi;
    var ready = pixCache[si] && pixCache[si].done ? pixCache[si] : null;
    inspRows(set, ready, pos, null, t, head);
    renderStatus();
    if (!used) return;
    pixelsFor(si).then(function (c) {
      c.done = true;
      if (si !== S.set) return;
      inspRows(set, c, pos, hasRoi ? roiStats(c, set, r) : null, t, head);
      renderStatus();
    });
  }
  // The status bar under the stage: cursor pixel, colour and FLIP per pane, zoom, pair and layout.
  var LAYOUT_NAME = { grid: 'Side by side', swipe: 'Swipe', flicker: 'Flicker', overlay: 'Heatmap', diff: 'Difference' };
  function renderStatus() {
    var sb = $('sbar'), set = cur(), pos = S.cursor;
    if (!sb || !set) return;
    var c = pixCache[S.set] && pixCache[S.set].done ? pixCache[S.set] : null;
    function item(k, v) { return el('span', { class: 'si' }, [el('span', { class: 'k', text: k }), typeof v === 'string' ? el('b', { text: v }) : v]); }
    function sep() { return el('span', { class: 'sep' }); }
    sb.textContent = '';
    sb.appendChild(item('Pixel', pos ? pos.x + ', ' + pos.y : '–'));
    present(set).forEach(function (i) {
      var s = sample(set, c, i, pos);
      var sw = el('span', { class: 'sw' }); if (s.rgb) sw.style.background = 'rgb(' + s.rgb.join(',') + ')';
      var parts = [sw, el('b', { text: s.rgbTxt })];
      if (i !== D.reference && s.flipTxt !== '–') parts.push(el('span', { class: 'px', text: 'FLIP ' + s.flipTxt }));
      sb.appendChild(sep());
      sb.appendChild(item(paneName(set, i), el('span', { class: 'si' }, parts)));
    });
    sb.appendChild(el('span', { class: 'grow' }));
    sb.appendChild(item('Zoom', viewports.length ? (S.scale >= 10 ? S.scale.toFixed(0) : S.scale.toFixed(2)) + '×' + (S.fit ? ' fit' : '') : '–'));
    var ch = S.chosen.filter(function (i) { return set.panes[i].path; });
    if ((S.layout === 'swipe' || S.layout === 'flicker') && ch.length >= 2) {
      sb.appendChild(sep());
      sb.appendChild(item('Pair', ch.map(function (i) { return paneName(set, i); }).join(' | ')));
    }
    sb.appendChild(sep());
    sb.appendChild(item('Layout', LAYOUT_NAME[S.layout] || S.layout));
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
  var P = UI.pointers(), gesture = null;
  function onDown(e) {
    var vp = e.target.closest ? e.target.closest('.vp') : null;
    if (!vp) return;
    P.add(e);
    try { vp.setPointerCapture(e.pointerId); } catch (err) { /* ignore */ }
    if (P.count() === 2) { gesture = { kind: 'pinch', d: P.dist(), vp: vp }; return; }
    var roi = S.tool === 'roi' || e.shiftKey;
    var hb = !roi && e.target.closest ? e.target.closest('.hsp') : null;
    var sw = !roi && !hb && S.layout === 'swipe' && vp.classList.contains('swipe') && !S.hold && (S.fit || nearDivider(vp, e));
    if (sw) { gesture = { kind: 'divider', vp: vp, moved: true }; swipeFromPointer(vp, e); vp.classList.add('drag'); return; }
    gesture = { kind: roi ? 'roi' : 'pan', vp: vp, sx: e.clientX, sy: e.clientY, tx: S.tx, ty: S.ty, moved: false };
    if (roi) { var p = toImage(vp, e); gesture.p0 = p; }
    if (hb) gesture.hot = [Number(hb.getAttribute('data-pane')), Number(hb.getAttribute('data-h'))];
    if (!roi) vp.classList.add('drag');
  }
  function onMove(e) {
    var vp = e.target.closest ? e.target.closest('.vp') : null;
    P.move(e);
    if (!gesture) { if (vp && e.pointerType === 'mouse') cursorFrom(vp, e); return; }
    var rc = gesture.vp.getBoundingClientRect();
    if (gesture.kind === 'pinch' && P.count() >= 2) {
      var m = P.mid(rc), nd = P.dist();
      zoomAt(nd / gesture.d, m[0], m[1]);
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
    P.remove(e);
    var g = gesture;
    if (!g) return;
    if (g.kind === 'pinch') { if (P.count() < 2) gesture = null; return; }
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
    zoomAt(UI.xf.wheel(e, 0.0015), e.clientX - rc.left, e.clientY - rc.top);
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
  var STATUS_TONE = { changed: 's-fail', identical: 's-pass', missing: 's-missing', error: 's-error' };
  function chipFor(s) {
    if (!s.status) return null;
    var txt = s.status === 'changed' ? 'differs' : s.status, tone = STATUS_TONE[s.status];
    if (s.status === 'missing') {
      var solo = soloLabel(s);
      txt = solo ? 'only in ' + solo : 'missing in ' + labelsWhere(s, false).join(', ');
    }
    // A measured difference is named by the findings of its worst image when there are some.
    var wi = s.status === 'changed' ? worstPane(s) : -1, dg = wi >= 0 ? s.panes[wi].diagnostics : null;
    if (dg) { txt = UI.classLabel(dg.class); tone = UI.classTone(dg.class); }
    var f = s.worst_flip == null ? '' : ' ' + fmt(s.worst_flip, 4);
    return { tone: tone, text: txt, flip: f.trim() };
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
  // The findings of every compared image of the set: class, the generated sentence, tone and shift, timings.
  function renderDiag() {
    var set = cur(), box = $('diag-slot');
    box.textContent = '';
    var cands = hideInfo() ? [] : set.order.filter(function (i) { return i !== D.reference && set.panes[i].diagnostics; });
    cands.forEach(function (i) {
      var node = UI.diagnosticsEl(set.panes[i].diagnostics, { label: cands.length > 1 || D.labels.length > 2 ? paneName(set, i) + ' vs ' + D.labels[D.reference] : null });
      if (node) box.appendChild(node);
    });
    box.hidden = !box.children.length;
  }
  function renderSets() {
    var ol = $('setlist'), sel = $('setsel');
    ol.textContent = ''; sel.textContent = '';
    S.visible.forEach(function (i) {
      var s = D.sets[i];
      var b = badgeFor(s), c = chipFor(s);
      var line = el('span', { class: 'line' });
      if (c) {
        line.appendChild(el('span', { class: 'chip ' + c.tone, text: c.text }));
        if (c.flip) line.appendChild(el('span', { class: 'fv', text: c.flip, title: 'Worst mean FLIP against the reference' }));
      }
      if (setHasWarnings(s)) line.appendChild(el('span', { class: 'chip s-error', text: '⚠ warning', title: 'An image is all black, all white or has non-finite samples' }));
      line.appendChild(el('span', { class: 'dstate ' + b.cls, text: b.text }));
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
    mark('seg-layout', S.layout); mark('seg-tool', S.tool); mark('seg-chan', S.channel); mark('seg-layer', S.layer);
    notify();
    $('b-overlay').disabled = hideInfo();
    $('b-diff').hidden = !hasSigned(cur());
    $('b-layer-signed').hidden = !hasSigned(cur());
    document.querySelector('#seg-layer [data-v="heat"]').disabled = hideInfo();
    $('g-swipe').hidden = S.layout !== 'swipe';
    $('g-flick').hidden = S.layout !== 'flicker';
    $('g-over').hidden = !layerKind();
    $('swipe').value = String(S.swipe);
    $('b-vert').setAttribute('aria-pressed', String(S.vertical));
    $('b-hot').hidden = !HAS.hot || hideInfo();
    $('b-hot').setAttribute('aria-pressed', String(S.hot));
    $('b-mask').hidden = !hasMask(cur());
    $('b-mask').setAttribute('aria-pressed', String(S.mask));
    $('b-insp').setAttribute('aria-pressed', String(S.insp));
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
        row.appendChild(btn(letter(set, i), e.chosen_label === D.labels[i] && !e.no_difference, function () { prefer(i); }));
      });
      row.appendChild(btn('No visible difference', e.no_difference, noDifference));
      box.appendChild(row);
    }
    var row2 = el('div', { class: 'dec-row' }, [el('span', { class: 'lbl', text: 'Verdict' })]);
    [['accept', 'Accept'], ['reject', 'Reject'], ['needs-work', 'Needs work']].forEach(function (v) {
      row2.appendChild(btn(v[1], e.decision === v[0], function () { toggleVerdict(v[0]); }));
    });
    box.appendChild(row2);
    var ta = el('textarea', { 'aria-label': 'Note', placeholder: 'Note (optional)' });
    ta.value = e.note || '';
    ta.addEventListener('input', function () { e.note = ta.value; touch(e); });
    box.appendChild(ta);
    var meta = el('p', { class: 'dec-meta' });
    meta.textContent = e.roi ? 'ROI saved: ' + e.roi.x + ',' + e.roi.y + ' ' + e.roi.w + '×' + e.roi.h : 'No ROI. Choose "Region" under Tools and drag, or Shift-drag.';
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
      clr.addEventListener('click', clearRoi);
      box.appendChild(clr);
    }
    updateReveal();
    renderProposal();
  }
  function toggleVerdict(v) { var e = entry(cur().name); e.decision = e.decision === v ? null : v; touch(e); afterDecision(); }
  function prefer(i) { var e = entry(cur().name); e.chosen_label = D.labels[i]; e.no_difference = false; touch(e); afterDecision(); }
  function noDifference() { var e = entry(cur().name); e.no_difference = true; e.chosen_label = null; touch(e); afterDecision(); }
  function clearRoi() { var e = entry(cur().name); e.roi = null; touch(e); drawRoi(); renderInspector(); renderDecision(); }
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
    renderSets(); renderToolbar(); renderStage(); renderDecision(); renderWarnings(); renderDiag(); renderConfigDiff(); renderHotspots();
  }
  function rerenderAll() { renderSets(); renderToolbar(); renderStage(); renderDecision(); renderMeta(); renderWarnings(); renderDiag(); renderConfigDiff(); renderHotspots(); }
  // Jumps to a set the current filter hides by clearing the filters first.
  function jumpTo(i) {
    if (S.visible.indexOf(i) < 0) { S.diffOnly = false; S.hideSolo = false; computeOrder(); renderTriage(); }
    selectSet(i);
  }

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

  // ---------- actions: one registry drives the keys, the palette and the help ----------
  function setLayout(v) {
    if (v === 'overlay' && hideInfo()) return;
    S.layout = v; renderToolbar(); renderStage();
  }
  function setLayer(v) {
    if (S.layout !== 'swipe') S.layout = 'swipe';
    S.layer = S.layer === v ? 'none' : v;
    renderToolbar(); renderStage();
  }
  function toggleVertical() { S.vertical = !S.vertical; renderToolbar(); paintSwipe(); }
  function toggleFlicker() {
    if (S.layout === 'flicker') S.layout = S.lastLayout;
    else { S.lastLayout = S.layout; S.layout = 'flicker'; }
    renderToolbar(); renderStage();
  }
  function setChannel(v) { S.channel = v; renderToolbar(); applyFilter(); }
  function resetDisplay() {
    S.exposure = 0; S.contrast = 1; S.channel = 'rgb'; $('exp').value = '0'; $('con').value = '1';
    renderToolbar(); applyFilter();
  }
  function setTool(v) { S.tool = v; renderToolbar(); $('stage').classList.toggle('tool-roi', v === 'roi'); }
  function toggleHot() { S.hot = !S.hot; renderToolbar(); renderStage(); }
  function toggleMask() { S.mask = !S.mask; renderToolbar(); renderStage(); }
  function toggleInsp() { S.insp = !S.insp; renderToolbar(); renderInspector(); }
  function copyLink() {
    A.copyLink(agent);
  }
  function snapshot() {
    agent.snapshot().then(function (u) { UI.download('flipdiff-' + cur().name.replace(/[^A-Za-z0-9._-]+/g, '_') + '.png', u); UI.toast('Snapshot downloaded'); })
      .catch(function (e) { UI.toast(String(e && e.message || e).slice(0, 140)); });
  }
  function exportDecisions() { UI.downloadJson('flipdiff-decisions.v1.json', buildDecisions()); }
  function showHelp() {
    UI.help.toggle(reg, [
      ['Drag', 'Move the swipe divider; once zoomed, drag away from it to pan'],
      ['Shift', 'Shift-drag draws a region of interest'],
      ['Wheel', 'Zoom at the pointer (pinch on touch screens)'],
      ['Click', 'Click a numbered box to zoom to that hotspot']
    ], 'Keys act on the set on screen. Everything here is also in the command palette.');
  }
  function openPop(p) { if (pop[p]) pop[p].open(true); }

  function defineActions() {
    var A = reg.add.bind(reg), G = { L: 'Layout', Z: 'Zoom', S: 'Swipe', V: 'Display', T: 'Tools', N: 'Navigate', D: 'Decision', X: 'General' };
    A({ id: 'palette', group: G.X, title: 'Open the command palette', show: ['Ctrl', 'K'], hidden: true, run: function () { UI.palette.open(); } });
    // navigation
    A({ id: 'nav.prev', group: G.N, title: 'Previous set', keys: ['['], run: function () { step(-1); } });
    A({ id: 'nav.next', group: G.N, title: 'Next set', keys: [']'], run: function () { step(1); } });
    A({ id: 'nav.first', group: G.N, title: 'First set', run: function () { if (S.visible.length) selectSet(S.visible[0]); } });
    A({ id: 'nav.last', group: G.N, title: 'Last set', run: function () { if (S.visible.length) selectSet(S.visible[S.visible.length - 1]); } });
    A({ id: 'nav.diffonly', group: G.N, title: 'Show only sets that differ', enabled: hasTriage, on: function () { return S.diffOnly; }, run: function () { S.diffOnly = !S.diffOnly; refreshOrder(); } });
    ['worst', 'name', 'status'].forEach(function (k) {
      A({ id: 'nav.sort.' + k, group: G.N, title: 'Order sets by ' + (k === 'worst' ? 'worst first' : k), enabled: function () { return hasTriage() && (k !== 'worst' || hasFlip()); }, on: function () { return S.sort === k; }, run: function () { S.sort = k; refreshOrder(); } });
    });
    reg.dynamic(function () {
      return D.sets.map(function (s, i) { return { id: 'go.' + i, group: 'Go to', title: s.name, keywords: 'set image jump', on: function () { return i === S.set; }, run: function () { jumpTo(i); } }; });
    });
    // layouts
    [['grid', 'Side by side', null], ['swipe', 'Swipe', null], ['flicker', 'Flicker', null], ['overlay', 'Heatmap', function () { return !hideInfo(); }], ['diff', 'Signed difference', function () { return hasSigned(cur()); }]].forEach(function (l) {
      A({ id: 'layout.' + l[0], group: G.L, title: 'Layout: ' + l[1], enabled: l[2] || undefined, on: function () { return S.layout === l[0]; }, run: function () { setLayout(l[0]); } });
    });
    A({ id: 'layout.flicker.toggle', group: G.L, title: 'Toggle flicker', keys: [' '], show: ['Space'], hidden: true, run: toggleFlicker });
    A({ id: 'flicker.pause', group: G.L, title: 'Pause or resume the flicker', enabled: function () { return S.layout === 'flicker'; }, run: function () { S.paused = !S.paused; renderToolbar(); renderStage(); } });
    // swipe
    A({ id: 'swipe.left', group: G.S, title: 'Move the divider left; previous set outside swipe', keys: ['ArrowLeft'], show: ['ArrowLeft', 'ArrowRight'], help: 'Move the divider 5% (Shift: 1%); previous or next set outside swipe', hidden: true, repeat: true, run: function (e) { if (S.layout === 'swipe') setSwipe(S.swipe - (e.shiftKey ? 1 : 5)); else step(-1); } });
    A({ id: 'swipe.right', group: G.S, title: 'Move the divider right; next set outside swipe', keys: ['ArrowRight'], show: [], hidden: true, repeat: true, run: function (e) { if (S.layout === 'swipe') setSwipe(S.swipe + (e.shiftKey ? 1 : 5)); else step(1); } });
    A({ id: 'swipe.vertical', group: G.S, title: 'Swipe: vertical or horizontal split', keys: ['v'], enabled: function () { return S.layout === 'swipe'; }, on: function () { return S.vertical; }, run: toggleVertical });
    A({ id: 'swipe.heat', group: G.S, title: 'Swipe: heatmap layer', keys: ['h'], enabled: function () { return S.layout === 'swipe' && !hideInfo(); }, on: function () { return S.layer === 'heat'; }, run: function () { setLayer('heat'); } });
    A({ id: 'swipe.signed', group: G.S, title: 'Swipe: signed difference layer', keys: ['d'], enabled: function () { return S.layout === 'swipe' && hasSigned(cur()); }, on: function () { return S.layer === 'signed'; }, run: function () { setLayer('signed'); } });
    // zoom
    A({ id: 'zoom.fit', group: G.Z, title: 'Zoom: fit', keys: ['0'], run: function () { setZoom('fit'); } });
    ['1', '2', '4', '8'].forEach(function (z) { A({ id: 'zoom.' + z, group: G.Z, title: 'Zoom: ' + z + '×', keys: [z], run: function () { setZoom(z); } }); });
    A({ id: 'view.fullscreen', group: G.Z, title: 'Full screen', keys: ['f'], on: function () { return fsx.active(); }, run: function () { fsx.toggle(); } });
    A({ id: 'view.hotspots', group: G.Z, title: 'Show hotspot boxes', enabled: function () { return HAS.hot && !hideInfo(); }, on: function () { return S.hot; }, run: toggleHot });
    A({ id: 'view.mask', group: G.Z, title: 'Show the non-finite mask', keys: ['m'], enabled: function () { return hasMask(cur()); }, on: function () { return S.mask; }, run: toggleMask });
    // display
    [['rgb', 'RGB'], ['r', 'red'], ['g', 'green'], ['b', 'blue'], ['l', 'luma']].forEach(function (c) {
      A({ id: 'display.channel.' + c[0], group: G.V, title: 'Channel: ' + c[1], on: function () { return S.channel === c[0]; }, run: function () { setChannel(c[0]); } });
    });
    A({ id: 'display.ev.up', group: G.V, title: 'Exposure: brighter by 1 stop', run: function () { S.exposure = Math.min(4, S.exposure + 1); $('exp').value = String(S.exposure); applyFilter(); } });
    A({ id: 'display.ev.down', group: G.V, title: 'Exposure: darker by 1 stop', run: function () { S.exposure = Math.max(-4, S.exposure - 1); $('exp').value = String(S.exposure); applyFilter(); } });
    [['up', .1], ['down', -.1]].forEach(function (v) { A({ id: 'display.contrast.' + v[0], group: G.V, title: 'Contrast: ' + (v[1] > 0 ? 'increase' : 'decrease'), run: function () { S.contrast = Math.max(.5, Math.min(4, S.contrast + v[1])); $('con').value = String(S.contrast); applyFilter(); } }); });
    A({ id: 'display.reset', group: G.V, title: 'Reset exposure, contrast and channel', run: resetDisplay });
    A({ id: 'display.open', group: G.V, title: 'Open the Display panel', run: function () { openPop('display'); } });
    // tools
    A({ id: 'tool.pan', group: G.T, title: 'Drag tool: pan', on: function () { return S.tool === 'pan'; }, run: function () { setTool('pan'); } });
    A({ id: 'tool.roi', group: G.T, title: 'Drag tool: draw a region', on: function () { return S.tool === 'roi'; }, run: function () { setTool('roi'); } });
    A({ id: 'tool.roi.clear', group: G.T, title: 'Clear the region of interest', enabled: function () { var e = dec[cur().name]; return !!(e && e.roi); }, run: clearRoi });
    A({ id: 'tool.inspector', group: G.T, title: 'Pixel inspector table', on: function () { return S.insp; }, run: toggleInsp });
    A({ id: 'tool.hold', group: G.T, title: 'Hold to compare with the reference', keys: ['c'], show: ['c'], help: 'Show the reference while held', hidden: true, hold: { down: holdStart, up: holdEnd } });
    A({ id: 'tool.link', group: G.T, title: 'Copy link to this view', run: copyLink });
    A({ id: 'tool.snapshot', group: G.T, title: 'Snapshot: download the stage as PNG', run: snapshot });
    A({ id: 'tool.open', group: G.T, title: 'Open the Tools panel', run: function () { openPop('tools'); } });
    // decisions
    [['accept', 'Accept'], ['reject', 'Reject'], ['needs-work', 'Needs work']].forEach(function (v) {
      A({ id: 'decision.' + v[0], group: G.D, title: 'Decision: ' + v[1], keywords: 'verdict', on: function () { return entry(cur().name).decision === v[0]; }, run: function () { toggleVerdict(v[0]); } });
    });
    A({ id: 'decision.clear', group: G.D, title: 'Decision: clear the verdict', enabled: function () { return !!entry(cur().name).decision; }, run: function () { var e = entry(cur().name); e.decision = null; touch(e); afterDecision(); } });
    if (D.blind) {
      LETTERS.slice(0, D.labels.length).split('').forEach(function (ch, k) {
        A({ id: 'decision.prefer.' + ch, group: G.D, title: 'Prefer image ' + ch, enabled: function () { var set = cur(); return present(set).some(function (i) { return letter(set, i) === ch; }); }, run: function () { var set = cur(); present(set).forEach(function (i) { if (letter(set, i) === ch) prefer(i); }); } });
      });
      A({ id: 'decision.nodiff', group: G.D, title: 'No visible difference', run: noDifference });
    }
    A({ id: 'decision.confirm', group: G.D, title: 'Confirm the proposed decision', keys: ['y'], enabled: function () { return !!proposal(); }, run: function () { var ap = proposal(); if (ap) setVerdict(ap.answer === 'accept' ? 'accept' : 'reject'); } });
    A({ id: 'decision.override', group: G.D, title: 'Override the proposed decision', keys: ['n'], enabled: function () { return !!proposal(); }, run: function () { var ap = proposal(); if (ap) setVerdict(ap.answer === 'accept' ? 'reject' : 'accept'); } });
    // general
    A({ id: 'export', group: G.X, title: 'Export decisions', run: exportDecisions });
    A({ id: 'reveal', group: G.X, title: 'Reveal labels (blind view)', enabled: function () { return D.blind && !S.revealed && !$('reveal').disabled; }, run: function () { $('keyfile').click(); } });
    A({ id: 'help', group: G.X, title: 'Keyboard shortcuts', keys: ['?'], run: showHelp });
    A({ id: 'escape', group: G.X, title: 'Close the panel or leave full screen', keys: ['Escape'], show: ['Esc'], hidden: true, run: function () { if (!UI.escape() && fsx.active()) fsx.set(false); } });
  }

  // ---------- agent API: hash state, window.flipdiff, proposed decisions ----------
  var A = window.__flipdiffAgent;
  var LAYOUT_OUT = { grid: 'side', overlay: 'heatmap', swipe: 'swipe', flicker: 'flicker', diff: 'diff' };
  var LAYOUT_IN = { side: 'grid', heatmap: 'overlay', swipe: 'swipe', flicker: 'flicker', diff: 'diff' };
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
    var lk = layerKind();
    return {
      set: set.name, layout: LAYOUT_OUT[S.layout] || 'side', split: Math.round(S.swipe * 10) / 1000, vertical: S.vertical,
      zoom: S.fit ? 'fit' : Math.round(S.scale * 1000) / 1000,
      at: S.fit ? null : [Math.round((vw / 2 - S.tx) / S.scale), Math.round((vh / 2 - S.ty) / S.scale)],
      heat: lk === 'heat' ? S.opacity : 0, signed: lk === 'signed' ? S.opacity : 0, mask: !!(S.mask && !hideInfo()),
      contrast: S.contrast, channel: S.channel === 'l' ? 'luma' : S.channel, ev: Math.round(S.exposure * 100) / 100,
      roi: r && r.w > 0 && r.h > 0 ? [r.x, r.y, r.w, r.h] : null,
      hotspot: !hideInfo() && S.hsel && S.hselT === tkey() ? S.hsel : null
    };
  }
  function agentApply(p) {
    if (p.set != null) {
      var idx = -1;
      D.sets.forEach(function (s, i) { if (s.name === p.set) idx = i; });
      if (idx >= 0 && idx !== S.set) jumpTo(idx);
    }
    if (p.layout) { S.layout = LAYOUT_IN[p.layout]; if ((S.layout === 'overlay' || S.layout === 'diff') && hideInfo()) S.layout = 'grid'; }
    if (p.split != null) { S.swipe = Math.round(p.split * 1000) / 10; $('swipe').value = String(S.swipe); }
    if (p.vertical != null) S.vertical = p.vertical;
    if (p.channel) S.channel = p.channel === 'luma' ? 'l' : p.channel;
    if (p.contrast != null) { S.contrast = p.contrast; $('con').value = String(p.contrast); }
    if (p.ev != null) { S.exposure = p.ev; $('exp').value = String(p.ev); }
    if (!hideInfo()) {
      ['heat', 'signed'].forEach(function (k) {
        if (p[k] == null) return;
        if (p[k] > 0) { S.opacity = p[k]; $('opa').value = String(p[k]); if (S.layout === 'swipe') S.layer = k; }
        else if (S.layout === 'swipe' && S.layer === k) S.layer = 'none';
      });
      if (p.mask != null) S.mask = !!p.mask;
    }
    renderToolbar(); renderStage(); applyFilter();
    var vw = vpWidth(), d = refDims(cur()), vh = vw * d[1] / d[0];
    if (p.zoom === 'fit') setZoom('fit');
    else if (p.zoom != null || (p.at && !S.fit)) {
      S.fit = false;
      S.scale = Math.max(fitScale(), p.zoom != null ? p.zoom : S.scale);
      var at = p.at || [d[0] / 2, d[1] / 2];
      var t = UI.xf.centerOn(at[0], at[1], S.scale, vw, vh);
      S.tx = t[0]; S.ty = t[1];
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
      var base = w.querySelector('img.base'), heat = w.querySelector('img.heat'), mask = w.querySelector('img.mask');
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
      if (mask) { g.globalAlpha = 1; g.globalCompositeOperation = 'screen'; g.drawImage(mask, ox + S.tx, oy + S.ty, bw * S.scale, bh * S.scale); }
      g.restore();
    }
    var dv = vp.querySelector('.divider');
    if (dv) {
      g.fillStyle = '#fff'; g.strokeStyle = '#000'; g.lineWidth = 1;
      var pos = (S.vertical ? r.height : r.width) * S.swipe / 100;
      if (S.vertical) { g.fillRect(ox, oy + pos - 1, r.width, 2); } else { g.fillRect(ox + pos - 1, oy, 2, r.height); }
    }
    Array.prototype.forEach.call(vp.querySelectorAll('.hsp, .roi, .nfb'), function (b) {
      if (b.hidden) return;
      var br = b.getBoundingClientRect();
      g.lineWidth = 2; g.strokeStyle = b.classList.contains('roi') ? '#00c8ff' : b.classList.contains('nfb') ? '#ff00ff' : '#ffb000';
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
    var imgs = Array.prototype.slice.call(st.querySelectorAll('img.base, img.heat, img.mask'));
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
    D.sets.forEach(function (s) {
      s.panes.forEach(function (p) {
        if (p.signed_diff) HAS.signed = true;
        if (p.nonfinite_mask) HAS.mask = true;
        if ((p.hotspots || []).length) HAS.hot = true;
      });
    });
    fsx = UI.fullscreen($('compare-stage'), function (on) {
      S.fs = on;
      $('fs').setAttribute('aria-pressed', String(on));
      if (viewports.length) applyTransform();
    });
    pop.display = UI.popover($('w-display'), $('display-btn'), $('display-pop'));
    pop.tools = UI.popover($('w-tools'), $('tools-btn'), $('tools-pop'));
    defineActions();
    function seg(id, attr, fn) {
      $(id).addEventListener('click', function (e) {
        var b = e.target.closest('button');
        if (b && !b.disabled) fn(b.getAttribute(attr));
      });
    }
    seg('seg-layout', 'data-v', setLayout);
    seg('seg-layer', 'data-v', function (v) { S.layer = v; renderToolbar(); renderStage(); });
    seg('seg-tool', 'data-v', setTool);
    seg('seg-chan', 'data-v', setChannel);
    seg('seg-zoom', 'data-z', setZoom);
    $('exp').addEventListener('input', function (e) { S.exposure = Number(e.target.value); applyFilter(); });
    $('con').addEventListener('input', function (e) { S.contrast = Number(e.target.value); applyFilter(); });
    $('reset-view').addEventListener('click', resetDisplay);
    $('swipe').addEventListener('input', function (e) { setSwipe(Number(e.target.value)); });
    $('b-vert').addEventListener('click', toggleVertical);
    $('b-hot').addEventListener('click', toggleHot);
    $('b-mask').addEventListener('click', toggleMask);
    $('b-insp').addEventListener('click', toggleInsp);
    $('fs').addEventListener('click', function () { fsx.toggle(); });
    $('help-btn').addEventListener('click', showHelp);
    $('palette-btn').addEventListener('click', function () { UI.palette.open(); });
    $('snap').addEventListener('click', snapshot);
    var hb = $('hold');
    hb.addEventListener('pointerdown', function (e) { e.preventDefault(); try { hb.setPointerCapture(e.pointerId); } catch (err) { /* ignore */ } holdStart(); });
    ['pointerup', 'pointercancel', 'lostpointercapture'].forEach(function (n) { hb.addEventListener(n, holdEnd); });
    hb.addEventListener('contextmenu', function (e) { e.preventDefault(); });
    hb.addEventListener('keydown', function (e) { if (e.key === 'Enter') { e.preventDefault(); holdStart(); } });
    hb.addEventListener('keyup', function (e) { if (e.key === 'Enter') holdEnd(); });
    window.addEventListener('blur', holdEnd);
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
