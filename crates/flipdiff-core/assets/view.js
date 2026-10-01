(function () {
  'use strict';
  var D = JSON.parse(document.getElementById('flipdiff-data').textContent);
  var $ = function (id) { return document.getElementById(id); };
  var LS_KEY = 'flipdiff-view.v1:' + D.id;
  var LETTERS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';

  var S = {
    set: 0, layout: 'grid', tool: 'pan', scale: 1, tx: 0, ty: 0, fit: true,
    exposure: 0, contrast: 1, channel: 'rgb', opacity: 0.6, rate: 2, paused: false,
    swipe: 50, revealed: false, chosen: [], flickerIdx: 0, cursor: null, roiDraft: null
  };
  var dec = {};           // name -> SetDecision-shaped entry
  var pixCache = {};      // set index -> {rgb:[ImageData|null], flip:[Uint8Array|null], promise}
  var timer = null;
  var viewports = [];

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
  var KEY = null;
  function trueLabel(i) { return KEY && KEY[i] != null ? KEY[i] : D.labels[i]; }
  function trueOf(neutral) { var i = D.labels.indexOf(neutral); return i < 0 ? neutral : trueLabel(i); }
  function hideInfo() { return D.blind && !S.revealed; }
  function letter(set, i) { return LETTERS[set.order.indexOf(i)] || '?'; }
  function paneName(set, i) {
    if (!D.blind) return D.labels[i];
    return S.revealed ? trueLabel(i) + ' (' + letter(set, i) + ')' : letter(set, i);
  }
  function present(set) { return set.order.filter(function (i) { return set.panes[i].path; }); }
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
      return g.getImageData(0, 0, w, h);   // throws SecurityError when the canvas is tainted
    });
  }
  function pixelsFor(si) {
    var c = pixCache[si];
    if (c) return c.promise;
    var set = D.sets[si];
    c = pixCache[si] = { rgb: [], flip: [] };
    c.promise = (function () {
      var uris = null;
      function rgbOf(i) {
        var p = set.panes[i];
        if (!p.path || !p.width) return Promise.resolve(null);
        var direct = uris ? Promise.reject(new Error('use data')) : readImage(url(p.path), p.width, p.height);
        return direct.catch(function () {
          // file:// images taint the canvas in Chromium: fall back to data URIs
          // from the per-set script, which are same-origin by definition.
          var ready = uris ? Promise.resolve() : loadScript(url(set.pixels_script)).then(function () {
            uris = (window.__flipdiffPx || {})[si] || [];
          });
          return ready.then(function () {
            return uris[i] ? readImage(uris[i], p.width, p.height) : null;
          });
        });
      }
      function flipOf(i) {
        var p = set.panes[i];
        if (!p.flip) return Promise.resolve(null);
        return readImage(p.flip, p.width, p.height).then(function (id) {
          var a = new Uint8Array(p.width * p.height);
          for (var k = 0; k < a.length; k++) a[k] = id.data[k * 4];
          return a;
        });
      }
      var chain = Promise.resolve();
      set.panes.forEach(function (_, i) {
        chain = chain.then(function () { return rgbOf(i); }).then(function (v) { c.rgb[i] = v; })
          .catch(function () { c.rgb[i] = null; })
          .then(function () { return flipOf(i); }).then(function (v) { c.flip[i] = v; })
          .catch(function () { c.flip[i] = null; });
      });
      return chain.then(function () { return c; });
    })();
    return c.promise;
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
    $('zval').textContent = (s >= 10 ? s.toFixed(0) : s.toFixed(2)) + '×';
    var zb = document.querySelectorAll('#seg-zoom button');
    for (var j = 0; j < zb.length; j++) {
      var z = zb[j].getAttribute('data-z');
      zb[j].setAttribute('aria-pressed', String(z === 'fit' ? S.fit : (!S.fit && Math.abs(S.scale - Number(z)) < 1e-6)));
    }
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
  }

  // ---------- stage ----------
  function layerFor(set, i, withHeat) {
    var p = set.panes[i];
    var layer = el('div', { class: 'layer' });
    layer.style.width = p.width + 'px'; layer.style.height = p.height + 'px';
    var img = el('img', { class: 'base', src: url(p.path), width: String(p.width), height: String(p.height), alt: paneName(set, i) + ' – ' + set.name, draggable: 'false' });
    layer.appendChild(img);
    if (withHeat && p.heatmap) {
      layer.appendChild(el('img', { class: 'heat', src: url(p.heatmap), width: String(p.width), height: String(p.height), alt: '', draggable: 'false' }));
    }
    layer.appendChild(el('div', { class: 'roi', hidden: '' }));
    return layer;
  }
  function wrapFor(set, i, withHeat) {
    var w = el('div', { class: 'wrap' });
    w.appendChild(layerFor(set, i, withHeat));
    return w;
  }
  function paneHead(set, i) {
    var p = set.panes[i];
    var right = '';
    if (!hideInfo()) {
      if (i === D.reference) right = 'reference';
      else if (p.metrics) right = 'FLIP mean ' + fmt(p.metrics.mean) + ' · p95 ' + fmt(p.metrics.p95) + ' · max ' + fmt(p.metrics.max);
      else if (p.error) right = p.error;
    }
    return el('div', { class: 'pane-h' }, [el('b', { text: paneName(set, i) }), el('span', { text: right })]);
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
      if (ch.length < 2) { stage.appendChild(el('p', { class: 'note-msg', text: 'Swipe needs two images: tick two under "Show".' })); }
      else {
        var a = ch[0], b = ch[1];
        var wb = wrapFor(set, b, false);
        wb.style.clipPath = 'inset(0 0 0 ' + S.swipe + '%)';
        wb.id = 'swipe-top';
        var div = el('div', { class: 'divider', id: 'swipe-div' });
        div.style.left = 'calc(' + S.swipe + '% - 1px)';
        var v = vpEl([wrapFor(set, a, false), wb, div,
          el('span', { class: 'corner l', text: paneName(set, a) }), el('span', { class: 'corner r', text: paneName(set, b) })]);
        stage.appendChild(el('div', { class: 'pane' }, [el('div', { class: 'pane-h' }, [el('b', { text: paneName(set, a) + ' | ' + paneName(set, b) }), el('span', { text: '' })]), v]));
      }
    } else if (S.layout === 'flicker') {
      if (ch.length < 2) { stage.appendChild(el('p', { class: 'note-msg', text: 'Flicker needs at least two images: tick them under "Show".' })); }
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
    applyFilter();
    stage.style.setProperty('--opa', S.opacity);
    applyTransform();
    drawRoi();
    renderInspector();
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
  }
  function roiStats(c, set, r) {
    var out = {};
    set.panes.forEach(function (p, i) {
      var id = c.rgb[i];
      if (!id || !p.width) { out[i] = null; return; }
      var x0 = Math.max(0, r.x), y0 = Math.max(0, r.y), x1 = Math.min(p.width, r.x + r.w), y1 = Math.min(p.height, r.y + r.h);
      var n = 0, sr = 0, sg = 0, sb = 0, sf = 0, fm = c.flip[i];
      for (var y = y0; y < y1; y++) for (var x = x0; x < x1; x++) {
        var k = y * p.width + x;
        sr += id.data[k * 4]; sg += id.data[k * 4 + 1]; sb += id.data[k * 4 + 2];
        if (fm) sf += fm[k];
        n++;
      }
      out[i] = n ? { n: n, r: sr / n, g: sg / n, b: sb / n, flip: fm ? sf / n / 255 : null } : null;
    });
    return out;
  }

  // ---------- inspector ----------
  function renderInspector() {
    var set = cur(), t = $('insp'), si = S.set;
    var r = roiRect(), pos = S.cursor;
    var hint = $('insp-hint');
    t.textContent = '';
    var head = el('tr', null, [el('th', { text: 'Image' }), el('th', { text: 'RGB' }), el('th', { text: 'FLIP' })]);
    var hasRoi = r && r.w > 0 && r.h > 0;
    if (hasRoi) { head.appendChild(el('th', { text: 'ROI mean RGB' })); head.appendChild(el('th', { text: 'ROI mean FLIP' })); }
    t.appendChild(head);
    hint.textContent = pos ? 'Pixel (' + pos.x + ', ' + pos.y + ')' : 'Hover (or tap) an image to read pixel values.';
    if (hasRoi) hint.textContent += (pos ? ' · ' : '') + 'ROI ' + r.x + ',' + r.y + ' ' + r.w + '×' + r.h + ' (' + (r.w * r.h) + ' px)';
    pixelsFor(si).then(function (c) {
      if (si !== S.set) return;
      var stats = hasRoi ? roiStats(c, set, r) : null;
      t.textContent = '';
      t.appendChild(head);
      set.order.forEach(function (i) {
        var p = set.panes[i];
        var rgbTxt = '–', flipTxt = '–', sw = null;
        if (p.path && pos) {
          var id = c.rgb[i];
          if (id && pos.x < p.width && pos.y < p.height) {
            var k = (pos.y * p.width + pos.x) * 4;
            rgbTxt = id.data[k] + ' ' + id.data[k + 1] + ' ' + id.data[k + 2];
            sw = 'rgb(' + id.data[k] + ',' + id.data[k + 1] + ',' + id.data[k + 2] + ')';
          } else if (!id) rgbTxt = 'n/a';
          var fm = c.flip[i];
          if (hideInfo()) flipTxt = 'hidden';
          else if (i === D.reference) flipTxt = 'ref';
          else if (fm && pos.x < p.width && pos.y < p.height) flipTxt = (fm[pos.y * p.width + pos.x] / 255).toFixed(3);
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
    gesture = { kind: roi ? 'roi' : 'pan', vp: vp, sx: e.clientX, sy: e.clientY, tx: S.tx, ty: S.ty, moved: false };
    if (roi) { var p = toImage(vp, e); gesture.p0 = p; }
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
    } else if (!g.moved) cursorFrom(g.vp, e);
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
    return { text: e.no_difference ? 'no difference' : 'prefers ' + trueOf(e.chosen_label), cls: '' };
  }
  function renderSets() {
    var ol = $('setlist'), sel = $('setsel');
    ol.textContent = ''; sel.textContent = '';
    D.sets.forEach(function (s, i) {
      var b = badgeFor(s);
      var btn = el('button', { type: 'button' }, [el('span', { class: 'n', text: s.name }), el('span', { class: 'badge ' + b.cls, text: b.text })]);
      if (i === S.set) btn.setAttribute('aria-current', 'true');
      btn.addEventListener('click', function () { selectSet(i); });
      ol.appendChild(el('li', null, [btn]));
      var o = el('option', { value: String(i), text: s.name + ' – ' + b.text });
      if (i === S.set) o.selected = true;
      sel.appendChild(o);
    });
    $('count').textContent = (S.set + 1) + ' / ' + D.sets.length;
    $('prev').disabled = S.set === 0;
    $('next').disabled = S.set >= D.sets.length - 1;
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
    $('b-overlay').disabled = hideInfo();
    $('g-swipe').hidden = S.layout !== 'swipe';
    $('g-flick').hidden = S.layout !== 'flicker';
    $('g-over').hidden = S.layout !== 'overlay';
    $('g-show').hidden = S.layout !== 'swipe' && S.layout !== 'flicker';
    $('ratev').textContent = S.rate + ' Hz';
    $('pause').setAttribute('aria-pressed', String(S.paused));
    $('pause').textContent = S.paused ? 'Resume' : 'Pause';
    renderChosen();
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
    if (e.roi) {
      var clr = el('button', { type: 'button', class: 'btn small', text: 'Clear ROI' });
      clr.addEventListener('click', function () { e.roi = null; touch(e); drawRoi(); renderInspector(); renderDecision(); });
      box.appendChild(clr);
    }
    updateReveal();
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
    renderSets(); renderToolbar(); renderStage(); renderDecision();
  }
  function rerenderAll() { renderSets(); renderToolbar(); renderStage(); renderDecision(); renderMeta(); }

  function renderMeta() {
    var m = $('meta'); m.textContent = '';
    function add(k, v) { m.appendChild(el('div', null, [el('dt', { text: k }), el('dd', { text: v })])); }
    if (D.blind && !S.revealed) add('Mode', 'blind (labels hidden)');
    else add('Directories', D.labels.map(function (l, i) { return trueLabel(i) + (i === D.reference ? ' (ref)' : ''); }).join(', '));
    add('Sets', String(D.sets.length));
    add('Seed', String(D.seed));
    add('Version', D.tool_version);
  }

  // ---------- export ----------
  function exportDecisions() {
    var out = {
      schema: 'flipdiff-decisions.v1', seed: D.seed, labels: D.labels, blind: D.blind,
      sets: D.sets.map(function (s) {
        var e = dec[s.name] || {};
        return {
          name: s.name, decision: e.decision || null, chosen_label: e.chosen_label || null,
          no_difference: !!e.no_difference, note: e.note || '', roi: e.roi || null, timestamp_ms: e.timestamp_ms || 0
        };
      })
    };
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
  function init() {
    load();
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
    $('swipe').addEventListener('input', function (e) {
      S.swipe = Number(e.target.value);
      var t = $('swipe-top'), d = $('swipe-div');
      if (t) t.style.clipPath = 'inset(0 0 0 ' + S.swipe + '%)';
      if (d) d.style.left = 'calc(' + S.swipe + '% - 1px)';
    });
    $('rate').addEventListener('input', function (e) { S.rate = Number(e.target.value); renderToolbar(); renderStage(); });
    $('pause').addEventListener('click', function () { S.paused = !S.paused; renderToolbar(); renderStage(); });
    $('opa').addEventListener('input', function (e) { S.opacity = Number(e.target.value); $('stage').style.setProperty('--opa', S.opacity); });
    $('prev').addEventListener('click', function () { selectSet(S.set - 1); });
    $('next').addEventListener('click', function () { selectSet(S.set + 1); });
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
          KEY = k.labels; S.revealed = true; rerenderAll();
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
    document.addEventListener('keydown', function (e) {
      var t = e.target && e.target.tagName;
      if (t === 'TEXTAREA' || t === 'INPUT' || t === 'SELECT' || e.ctrlKey || e.metaKey || e.altKey) return;
      if (e.key === 'ArrowLeft') selectSet(S.set - 1);
      else if (e.key === 'ArrowRight') selectSet(S.set + 1);
    });
    renderMeta();
    selectSet(0);
  }
  init();
})();
