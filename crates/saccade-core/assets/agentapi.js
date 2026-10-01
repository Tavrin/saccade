/* Shared by the report and the viewer (and so by `saccade serve` sessions):
 * the view state in the URL hash, the window.saccade API for browser
 * automation, the "copy link" button and the proposed-decision chip. */
(function () {
  'use strict';
  var VERSION = 1;
  var LAYOUTS = ['side', 'swipe', 'flicker', 'heatmap', 'diff'];
  var CHANNELS = ['rgb', 'r', 'g', 'b', 'luma'];

  function num(v) { var n = typeof v === 'number' ? v : (typeof v === 'string' && v.trim() !== '' ? Number(v) : NaN); return isFinite(n) ? n : null; }
  function clamp(n, lo, hi) { return Math.max(lo, Math.min(hi, n)); }
  function round(n, d) { var k = Math.pow(10, d); return Math.round(n * k) / k; }

  // Validates a partial state: returns only the keys whose value is valid.
  // `nameKey` is `set` (viewer) or `entry` (report); the alternative spelling is accepted too.
  function sanitize(src, nameKey) {
    var out = {}, v, n;
    var name = src[nameKey] != null ? src[nameKey] : (src.set != null ? src.set : src.entry);
    if (typeof name === 'string' && name) out[nameKey] = name;
    if (typeof src.layout === 'string' && LAYOUTS.indexOf(src.layout) >= 0) out.layout = src.layout;
    if ((n = num(src.split)) !== null) out.split = round(clamp(n, 0, 1), 3);
    v = src.vertical;
    if (v === true || v === 1 || v === '1' || v === 'true') out.vertical = true;
    else if (v === false || v === 0 || v === '0' || v === 'false') out.vertical = false;
    if (src.zoom === 'fit') out.zoom = 'fit';
    else if ((n = num(src.zoom)) !== null && n > 0) out.zoom = round(Math.min(n, 64), 3);
    v = src.at;
    if (typeof v === 'string') v = v.split(',');
    if (Array.isArray(v) && v.length === 2 && num(v[0]) !== null && num(v[1]) !== null) out.at = [round(num(v[0]), 1), round(num(v[1]), 1)];
    if ((n = num(src.heat)) !== null) out.heat = round(clamp(n, 0, 1), 2);
    if ((n = num(src.signed)) !== null) out.signed = round(clamp(n, 0, 1), 2);
    v = src.mask;
    if (v === true || v === 1 || v === '1' || v === 'true') out.mask = true;
    else if (v === false || v === 0 || v === '0' || v === 'false') out.mask = false;
    if (typeof src.channel === 'string' && CHANNELS.indexOf(src.channel) >= 0) out.channel = src.channel;
    if ((n = num(src.ev)) !== null) out.ev = round(clamp(n, -16, 16), 2);
    if ((n = num(src.contrast)) !== null) out.contrast = round(clamp(n, .5, 4), 2);
    v = src.roi;
    if (typeof v === 'string') v = v.split(',');
    if (v === null) out.roi = null;
    else if (Array.isArray(v) && v.length === 4) {
      var r = v.map(num);
      if (r.every(function (x) { return x !== null && x >= 0 && Math.floor(x) === x; }) && r[2] > 0 && r[3] > 0) out.roi = r;
    }
    if (src.hotspot === null || src.hotspot === 0 || src.hotspot === '0') out.hotspot = null;
    else if ((n = num(src.hotspot)) !== null && n >= 1 && Math.floor(n) === n) out.hotspot = n;
    return out;
  }

  function parseHash(hash, nameKey) {
    var src = {};
    String(hash || '').replace(/^#/, '').split('&').forEach(function (pair) {
      var i = pair.indexOf('=');
      if (i <= 0) return;
      var k = pair.slice(0, i), v;
      try { v = decodeURIComponent(pair.slice(i + 1)); } catch (e) { return; }
      src[k] = v;
    });
    return sanitize(src, nameKey);
  }

  function enc(s) { return encodeURIComponent(s).replace(/%2F/gi, '/').replace(/%2C/gi, ','); }

  // The hash of a full state. Defaults are left out, except the name and the
  // layout; a blind page never writes heat or hotspot.
  function formatHash(st, nameKey, defaults, blind) {
    var p = [];
    if (st[nameKey] != null) p.push(nameKey + '=' + enc(st[nameKey]));
    p.push('layout=' + st.layout);
    if (st.layout === 'swipe' && st.split !== defaults.split) p.push('split=' + st.split);
    if (st.layout === 'swipe' && st.vertical) p.push('vertical=1');
    if (st.zoom !== 'fit') {
      p.push('zoom=' + st.zoom);
      if (st.at) p.push('at=' + st.at[0] + ',' + st.at[1]);
    }
    if (!blind && st.heat > 0) p.push('heat=' + st.heat);
    if (!blind && st.signed > 0) p.push('signed=' + st.signed);
    if (!blind && st.mask) p.push('mask=1');
    if (st.channel !== 'rgb') p.push('channel=' + st.channel);
    if (st.ev !== 0) p.push('ev=' + st.ev);
    if (st.contrast != null && st.contrast !== 1) p.push('contrast=' + st.contrast);
    if (st.roi) p.push('roi=' + st.roi.join(','));
    if (!blind && st.hotspot) p.push('hotspot=' + st.hotspot);
    return p.join('&');
  }

  function nextFrame() {
    return new Promise(function (res) {
      var done = false, fin = function () { if (!done) { done = true; res(); } };
      if (typeof requestAnimationFrame === 'function') requestAnimationFrame(function () { requestAnimationFrame(fin); });
      setTimeout(fin, 120);
    });
  }

  // adapter: { nameKey, defaults, blind, names(), get(), apply(partial) -> Promise,
  //            step(d) -> Promise<boolean>, canvas() -> Promise<HTMLCanvasElement> }
  function create(adapter) {
    var nameKey = adapter.nameKey, listeners = [], timer = 0, applying = false, live = false;

    function get() { return adapter.get(); }
    function hash() { return formatHash(get(), nameKey, adapter.defaults, adapter.blind); }
    function writeHash() {
      clearTimeout(timer); timer = 0;
      try { history.replaceState(null, '', '#' + hash()); } catch (e) { /* sandboxed: ignore */ }
    }
    function emit() {
      var st = get();
      listeners.slice().forEach(function (fn) { try { fn(st); } catch (e) { /* a listener must not break the page */ } });
    }
    // Called by the page after any change of the view state.
    function changed() {
      if (applying || !live) return;
      clearTimeout(timer);
      timer = setTimeout(function () { writeHash(); emit(); }, 150);
    }
    function set(partial) {
      var clean = sanitize(partial && typeof partial === 'object' ? partial : {}, nameKey);
      applying = true;
      return Promise.resolve(adapter.apply(clean)).catch(function () { /* ignore unusable parts */ })
        .then(nextFrame).then(function () { applying = false; writeHash(); emit(); return get(); });
    }
    function applyHash() {
      var st = parseHash(location.hash, nameKey);
      var done = Object.keys(st).length ? set(st) : Promise.resolve(get());
      return done.then(function (v) { live = true; return v; });
    }

    var api = {
      version: VERSION,
      get: get,
      set: set,
      sets: function () { return adapter.names(); },
      entries: function () { return adapter.names(); },
      next: function () { return Promise.resolve(adapter.step(1)).then(nextFrame).then(function () { writeHash(); emit(); return get(); }); },
      prev: function () { return Promise.resolve(adapter.step(-1)).then(nextFrame).then(function () { writeHash(); emit(); return get(); }); },
      snapshot: function () {
        return Promise.resolve(adapter.canvas()).then(function (canvas) {
          try { return canvas.toDataURL('image/png'); } catch (e) {
            throw new Error('snapshot blocked: the browser tainted the canvas, which it does for file:// pages; serve the directory over http (python3 -m http.server) or run `saccade snapshot`');
          }
        });
      },
      on: function (ev, fn) {
        if (ev !== 'change' || typeof fn !== 'function') return function () {};
        listeners.push(fn);
        return function () { var i = listeners.indexOf(fn); if (i >= 0) listeners.splice(i, 1); };
      },
      link: function () { return location.href.split('#')[0] + '#' + hash(); }
    };
    api.hash = hash;
    api.changed = changed;
    api.applyHash = applyHash;
    window.addEventListener('hashchange', function () { var st = parseHash(location.hash, nameKey); if (Object.keys(st).length) set(st); });
    window.saccade = api;
    return api;
  }

  // Copies the current link, hash included, and reports it with a toast.
  function copyLink(api) {
    var link = api.link(), UI = window.__saccadeUI;
    try { history.replaceState(null, '', link.slice(link.indexOf('#'))); } catch (e) { /* ignore */ }
    return UI.copy(link).then(function (ok) { UI.toast(ok ? 'Link to this view copied' : 'Could not copy the link'); return ok; });
  }

  // The "Copy link to this view" button.
  function bindCopy(btn, api) {
    var label = btn.textContent;
    btn.addEventListener('click', function () {
      copyLink(api).then(function (ok) {
        btn.textContent = ok ? 'Link copied' : 'Copy failed';
        setTimeout(function () { btn.textContent = label; }, 1600);
      });
    });
  }

  // ---- proposed decisions ----------------------------------------------------

  // name -> proposals[] from the decisions sidecar (`saccade decide` writes it
  // next to the report or view); sets of a decisions file keep their decision.
  function sidecar() {
    var d = window.__saccadeDecisions, out = {};
    if (d && Array.isArray(d.sets)) d.sets.forEach(function (s) { out[s.name] = s; });
    return out;
  }
  // The proposal a person can confirm: the newest answer to the accept question.
  function acceptProposal(list) {
    var best = null;
    (list || []).forEach(function (p) { if (p.question === 'accept' && (!best || p.timestamp_ms >= best.timestamp_ms)) best = p; });
    return best;
  }
  function chipText(p) {
    var t = 'agent proposed: ' + p.answer.replace('_', ' ');
    if (p.prob != null) t += ' ' + Number(p.prob).toFixed(2);
    return t + ' (' + p.source + ')';
  }
  // Chips for every proposal, plus confirm / override buttons for the accept one.
  // `onVerdict('accept'|'reject')` records the person's decision.
  function proposalBar(list, current, onVerdict) {
    var wrap = document.createElement('div');
    wrap.className = 'agent-bar';
    var ap = acceptProposal(list);
    (list || []).forEach(function (p) {
      var c = document.createElement('span');
      c.className = 'agent-chip' + (p === ap ? ' main' : '');
      c.textContent = (p.question === 'accept' ? '' : p.question.replace('_', ' ') + ': ') + (p.question === 'accept' ? chipText(p) : p.answer.replace(/_/g, ' ') + (p.prob != null ? ' ' + Number(p.prob).toFixed(2) : '') + ' (' + p.source + ')');
      c.title = 'Recorded by `saccade decide`; advice, not a decision' + (p.note ? ': ' + p.note : '');
      if (p.hotspot) c.textContent += ' #' + p.hotspot;
      wrap.appendChild(c);
    });
    if (ap && (ap.answer === 'accept' || ap.answer === 'reject')) {
      var opposite = ap.answer === 'accept' ? 'reject' : 'accept';
      var yes = document.createElement('button'), no = document.createElement('button');
      yes.type = no.type = 'button';
      yes.className = no.className = 'btn small';
      yes.textContent = 'Confirm (y)'; no.textContent = 'Override: ' + opposite + ' (n)';
      yes.setAttribute('data-agent', 'confirm'); no.setAttribute('data-agent', 'override');
      yes.disabled = no.disabled = false;
      yes.addEventListener('click', function () { onVerdict(ap.answer); });
      no.addEventListener('click', function () { onVerdict(opposite); });
      if (current) { yes.setAttribute('aria-pressed', String(current === ap.answer)); no.setAttribute('aria-pressed', String(current === opposite)); }
      wrap.appendChild(yes); wrap.appendChild(no);
    }
    return { el: wrap, proposal: ap && (ap.answer === 'accept' || ap.answer === 'reject') ? ap : null };
  }

  window.__saccadeAgent = {
    version: VERSION, parseHash: parseHash, formatHash: formatHash, sanitize: sanitize, create: create,
    bindCopy: bindCopy, copyLink: copyLink, sidecar: sidecar, acceptProposal: acceptProposal, proposalBar: proposalBar
  };
})();
