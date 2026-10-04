(function () {
  'use strict';
  var P = window.SACCADE_PAGE || {};
  var TOKEN = P.token || '';
  var MAX_RUNS = 6;
  var IMG_RE = /\.(png|jpe?g|exr|hdr)$/i;
  var LS_SEL = 'saccade-serve-selection';

  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, kids) {
    var n = document.createElement(tag);
    if (attrs) for (var k in attrs) {
      if (k === 'text') n.textContent = attrs[k];
      else if (k === 'cls') n.className = attrs[k];
      else n.setAttribute(k, attrs[k]);
    }
    (kids || []).forEach(function (c) { if (c) n.appendChild(c); });
    return n;
  }
  function clear(n) { while (n.firstChild) n.removeChild(n.firstChild); }
  function enc(s) { return encodeURIComponent(s); }
  function getJSON(url) {
    return fetch(url, { headers: { 'Accept': 'application/json' } }).then(function (r) {
      return r.json().then(function (j) {
        if (!r.ok) throw new Error(j && j.error ? j.error : ('HTTP ' + r.status));
        return j;
      });
    });
  }
  function when(sec) { return sec ? new Date(sec * 1000).toLocaleString() : ''; }
  function size(b) {
    if (b < 1024) return b + ' B';
    if (b < 1048576) return (b / 1024).toFixed(0) + ' KB';
    return (b / 1048576).toFixed(1) + ' MB';
  }
  function join(a, b) { return a ? a + '/' + b : b; }

  // ---------- progress page ----------
  function progress() {
    $('progress').hidden = false;
    var id = P.session;
    function poll() {
      getJSON('/api/session/' + enc(id) + '/status').then(function (s) {
        // set=, a= and b= (the set and pair to open on) ride along to the session.
        if (s.state === 'ready' && s.url) { location.replace(s.url + location.search); return; }
        if (s.state === 'failed') {
          $('ptitle').textContent = 'The comparison could not be built';
          $('pnote').textContent = s.error || 'unknown error';
          $('pnote').className = 'hint err';
          document.querySelector('#progress .bar').hidden = true;
          return;
        }
        $('pnote').textContent = (s.runs || []).join('  vs  ') +
          (s.elapsed_s != null ? ' — ' + s.elapsed_s + ' s' : '') +
          (s.estimated_sets ? ' — about ' + s.estimated_sets + ' images in the reference run' : '');
        setTimeout(poll, 700);
      }).catch(function (e) {
        $('pnote').textContent = String(e.message || e);
        $('pnote').className = 'hint err';
        if (/unknown session/i.test(String(e.message || e))) {
          $('ptitle').textContent = 'This comparison is not available';
          $('pnote').textContent = 'The link is stale or the cache was cleared. Open it again from the archive.';
          document.querySelector('#progress .bar').hidden = true;
          return;
        }
        setTimeout(poll, 2000);
      });
    }
    poll();
  }

  // ---------- landing page ----------
  var cwd = '';
  var selection = [];
  var meta = null; // last listing

  function loadSel() {
    try { var s = JSON.parse(localStorage.getItem(LS_SEL) || '[]'); if (Array.isArray(s)) selection = s.slice(0, MAX_RUNS); } catch (e) { /* none */ }
  }
  function saveSel() { try { localStorage.setItem(LS_SEL, JSON.stringify(selection)); } catch (e) { /* ignore */ } }

  // With several roots the first path segment names a root and the empty
  // path lists them: the crumbs then start at "Roots", not at the root.
  function renderCrumbs(path, rootName, multi) {
    var c = $('crumbs'); clear(c);
    var parts = path ? path.split('/') : [];
    function go(p) { return function () { navigate(p); }; }
    var top = multi ? 'Roots' : (rootName || '/');
    if (parts.length) {
      var rb = el('button', { type: 'button', text: top });
      rb.addEventListener('click', go(''));
      c.appendChild(rb);
    } else c.appendChild(el('span', { cls: 'cur', text: top }));
    var acc = '';
    parts.forEach(function (p, i) {
      acc = join(acc, p);
      c.appendChild(el('span', { cls: 'sep', text: '/' }));
      if (i === parts.length - 1) c.appendChild(el('span', { cls: 'cur', text: p }));
      else { var b = el('button', { type: 'button', text: p }); b.addEventListener('click', go(acc)); c.appendChild(b); }
    });
  }

  var HIDE_KEY = /time|_ms$|duration|elapsed|timestamp|run\.id/i;
  function shortVal(v) {
    var t = String(v);
    return t.length > 20 ? t.slice(0, 12) + '…' + t.slice(-4) : t;
  }
  // Metadata chips for one run. `prefer` lists the keys worth showing first
  // (those that differ among the listed runs); the rest hides behind "+N".
  function pairsEl(m, limit, prefer) {
    var box = el('div', { cls: 'pairs' });
    if (!m) return box;
    var all = Object.keys(m).filter(function (k) { return !HIDE_KEY.test(k); });
    var first = (prefer || []).filter(function (k) { return all.indexOf(k) >= 0; });
    var keys = first.concat(all.filter(function (k) { return first.indexOf(k) < 0; }));
    var max = limit || 6;
    // With differing keys, show only those; everything else hides behind "+N".
    var shown = (first.length ? first : keys).slice(0, max);
    shown.forEach(function (k) {
      var full = k + '=' + m[k];
      var b = el('button', { type: 'button', cls: 'kv', text: k + '=' + shortVal(m[k]), title: full + ' (click to filter)' });
      b.addEventListener('click', function () { $('metaf').value = full; runSearch(false); });
      box.appendChild(b);
    });
    var rest = keys.filter(function (k) { return shown.indexOf(k) < 0; }).map(function (k) { return k + '=' + m[k]; });
    if (rest.length) box.appendChild(el('span', { cls: 'kv more', text: '+' + rest.length, title: rest.join('\n') }));
    return box;
  }
  // Keys whose value is not the same in every listed run, in first-seen order.
  function differingKeys(runs) {
    var seen = {}, order = [];
    runs.forEach(function (r) {
      Object.keys(r.meta || {}).forEach(function (k) {
        if (HIDE_KEY.test(k)) return;
        if (!seen[k]) { seen[k] = {}; order.push(k); }
      });
    });
    return order.filter(function (k) {
      var vals = {};
      runs.forEach(function (r) { vals[r.meta && k in r.meta ? String(r.meta[k]) : '\u0000absent'] = 1; });
      return Object.keys(vals).length > 1;
    });
  }

  function thumb(path, sample) {
    if (!sample) return el('div', { cls: 'th none', 'aria-hidden': 'true', text: '▸' });
    var i = el('img', { cls: 'th', loading: 'lazy', alt: '', src: '/thumb?path=' + enc(join(path, sample)) });
    i.addEventListener('error', function () { i.style.visibility = 'hidden'; });
    return i;
  }

  // A selection holds either whole runs or single images, never both.
  function isImg(p) { return IMG_RE.test(p); }
  function selKind() {
    var n = selection.filter(isImg).length;
    return !selection.length ? '' : n === selection.length ? 'images' : n === 0 ? 'runs' : 'mixed';
  }
  var btnReg = [];
  function paintBtn(b, path) {
    var on = selection.indexOf(path) >= 0;
    var kind = isImg(path) ? 'images' : 'runs', cur = selKind();
    var clash = !!cur && cur !== kind;
    b.textContent = on ? 'Selected' : '+ Compare';
    b.setAttribute('aria-pressed', on ? 'true' : 'false');
    b.disabled = on || selection.length >= MAX_RUNS || clash;
    b.title = clash ? 'The selection holds ' + (cur === 'images' ? 'single images' : 'runs') + '; clear it to pick ' + (kind === 'images' ? 'an image' : 'a run') : '';
  }
  function paintAll() {
    btnReg = btnReg.filter(function (r) { return document.body.contains(r.b); });
    btnReg.forEach(function (r) { paintBtn(r.b, r.path); });
  }
  function selectBtn(path) {
    var b = el('button', { type: 'button', cls: 'btn small' });
    paintBtn(b, path);
    btnReg.push({ b: b, path: path });
    b.addEventListener('click', function () { addSel(path); });
    return b;
  }

  // All the images of a run, each with its own "+ Compare".
  function imagesPanel(path) {
    var box = el('div', { cls: 'imgpanel' }, [el('p', { cls: 'hint', text: 'Loading images…' })]);
    getJSON('/api/images?path=' + enc(path)).then(function (r) {
      clear(box);
      if (!r.images.length) { box.appendChild(el('p', { cls: 'hint', text: 'No images here.' })); return; }
      var g = el('div', { cls: 'thumbs pick' });
      r.images.forEach(function (it) {
        var full = join(path, it.name);
        var im = el('img', { loading: 'lazy', alt: it.name, src: '/thumb?path=' + enc(full) });
        im.addEventListener('error', function () { im.style.visibility = 'hidden'; });
        g.appendChild(el('figure', null, [im, el('figcaption', { text: it.name, title: it.name + ' · ' + size(it.bytes) }), selectBtn(full)]));
      });
      box.appendChild(g);
      if (r.truncated) box.appendChild(el('p', { cls: 'hint', text: 'Only the first ' + r.images.length + ' images are listed.' }));
    }).catch(function (e) { clear(box); box.appendChild(el('p', { cls: 'hint err', text: String(e.message || e) })); });
    return box;
  }
  // "Images" toggle on a run row: expands the row into its image list.
  function imagesToggle(row, path) {
    var b = el('button', { type: 'button', cls: 'btn small ghost', text: 'Images ▾', 'aria-expanded': 'false' });
    var panel = null;
    b.addEventListener('click', function () {
      var open = row.classList.toggle('open');
      b.setAttribute('aria-expanded', open ? 'true' : 'false');
      b.textContent = open ? 'Images ▴' : 'Images ▾';
      if (open) { panel = imagesPanel(path); row.appendChild(panel); }
      else if (panel) { row.removeChild(panel); panel = null; }
    });
    return b;
  }
  function rowActions(row, path) {
    return el('div', { cls: 'racts' }, [el('a', { cls: 'btn small', href: '/run?path=' + enc(path), text: 'Open run' }), selectBtn(path), imagesToggle(row, path)]);
  }

  function renderRunBox(l) {
    var rb = $('runbox'); clear(rb);
    if (!l.run) { rb.hidden = true; return; }
    rb.hidden = false;
    rb.appendChild(el('div', { cls: 'head' }, [
      el('span', null, [el('b', { text: 'This directory is a run' }), el('span', { cls: 'sub', text: '  ' + l.run.images + (l.run.images === 1 ? ' image, ' : ' images, ') + size(l.run.bytes) })]),
      selectBtn(l.path)
    ]));
    if (l.run.meta_error) rb.appendChild(el('p', { cls: 'hint err', text: 'sidecar: ' + l.run.meta_error }));
    rb.appendChild(pairsEl(l.run.meta, 12));
    rb.appendChild(imagesPanel(l.path));
  }

  function navigate(path, noHash) {
    $('note').textContent = 'Loading…';
    getJSON('/api/ls?path=' + enc(path)).then(function (l) {
      cwd = l.path; meta = l;
      if (!noHash) { try { history.pushState(null, '', '#' + (cwd ? '/' + cwd : '')); } catch (e) { /* ignore */ } }
      $('rootname').textContent = l.root_name;
      renderCrumbs(l.path, l.root_name, l.multi);
      renderRunBox(l);
      var ul = $('dirs'); clear(ul);
      l.dirs.forEach(function (d) {
        var p = join(l.path, d.name);
        var nm = el('button', { type: 'button', cls: 'nm', text: d.name });
        nm.addEventListener('click', function () { navigate(p); });
        var sub = el('span', { cls: 'sub', text: when(d.mtime) });
        var top = el('span', null, [nm, d.has_images ? el('span', { cls: 'badge', text: 'run' }) : null]);
        var row = el('li', { cls: 'row' }, [thumb(p, d.sample), el('div', { cls: 'main' }, [top, sub])]);
        if (d.has_images) row.appendChild(rowActions(row, p));
        ul.appendChild(row);
      });
      $('note').textContent = l.dirs.length + (l.multi && !l.path ? ' roots' : ' sub-directories') + (l.partial ? ' (image probing stopped early: some directories are not classified)' : '') + (l.outside_links ? ' · ' + l.outside_links + ' symlinks leave the served roots and are not shown' : '');
      $('resultscard').hidden = true;
      loadRecent();
    }).catch(function (e) {
      $('note').textContent = String(e.message || e);
      $('note').className = 'hint err';
    });
  }

  function runList(ul, runs) {
    clear(ul);
    var diff = differingKeys(runs);
    runs.forEach(function (r) {
      var nm = el('button', { type: 'button', cls: 'nm', text: r.path });
      nm.addEventListener('click', function () { navigate(r.path); });
      var row = el('li', { cls: 'row' }, [
        thumb(r.path, r.sample),
        el('div', { cls: 'main' }, [nm, el('span', { cls: 'sub', text: r.images + (r.images === 1 ? ' image · ' : ' images · ') + when(r.mtime) }), pairsEl(r.meta, 3, diff)])
      ]);
      row.appendChild(rowActions(row, r.path));
      ul.appendChild(row);
    });
  }

  function runSearch(recent) {
    var url = '/api/search?path=' + enc(cwd) + '&q=' + enc($('q').value) + '&meta=' + enc($('metaf').value.trim());
    if (recent) url += '&recent=1';
    $('resultscard').hidden = false;
    $('resultstitle').textContent = recent ? 'Most recently modified runs here' : 'Search results';
    $('resultsnote').textContent = 'Searching…';
    getJSON(url).then(function (r) {
      runList($('results'), r.runs);
      $('resultsnote').textContent = r.runs.length + ' runs, ' + r.directories_scanned + ' directories scanned' +
        (r.partial ? ' — partial: the walk hit its time or size budget; narrow the directory' : '');
    }).catch(function (e) { $('resultsnote').textContent = String(e.message || e); });
  }

  function loadRecent() {
    $('recentwhere').textContent = 'under /' + cwd;
    $('recentnote').textContent = 'Scanning…';
    getJSON('/api/search?path=' + enc(cwd) + '&recent=1&budget_ms=2000').then(function (r) {
      runList($('recent'), r.runs.slice(0, 8));
      $('recentnote').textContent = r.runs.length ? (r.partial ? 'Partial: bounded walk.' : '') : 'No runs found' + (r.partial ? ' (bounded walk)' : '') + '.';
    }).catch(function (e) { $('recentnote').textContent = String(e.message || e); });
  }

  function loadDecisions() {
    getJSON('/api/decisions').then(function (ds) {
      var ul = $('decs'); clear(ul);
      if (!ds.length) { ul.appendChild(el('li', { cls: 'row' }, [el('span', { cls: 'sub', text: 'No decisions saved yet.' })])); return; }
      ds.forEach(function (d) {
        var title = (d.runs && d.runs.length ? d.runs : d.labels).join(' vs ');
        var t = d.session_url ? el('a', { href: d.session_url, text: title || d.session }) : el('span', { text: title || d.session });
        ul.appendChild(el('li', { cls: 'row' }, [el('div', { cls: 'main' }, [t, el('span', { cls: 'sub', text: d.decided + '/' + d.sets + ' decided · ' + when(d.mtime) })])]));
      });
    }).catch(function () { /* optional */ });
  }

  // ---------- selection ----------
  function addSel(p) {
    if (selection.indexOf(p) >= 0 || selection.length >= MAX_RUNS) return;
    selection.push(p); saveSel(); renderSel(); paintAll();
  }
  function move(i, d) {
    var j = i + d;
    if (j < 0 || j >= selection.length) return;
    var t = selection[i]; selection[i] = selection[j]; selection[j] = t;
    saveSel(); renderSel();
  }
  function renderSel() {
    var ul = $('chips'); clear(ul);
    var dragFrom = -1;
    selection.forEach(function (p, i) {
      var li = el('li', { cls: 'chip', draggable: 'true' });
      li.appendChild(el('span', { cls: 'pos' + (i === 0 ? ' ref' : ''), text: i === 0 ? 'ref' : '#' + (i + 1) }));
      li.appendChild(el('span', { cls: 'p', text: p, title: p }));
      var l = el('button', { type: 'button', cls: 'btn small', 'aria-label': 'Move earlier', text: '↑' });
      var r = el('button', { type: 'button', cls: 'btn small', 'aria-label': 'Move later', text: '↓' });
      var x = el('button', { type: 'button', cls: 'btn small', 'aria-label': 'Remove', text: '×' });
      l.addEventListener('click', function () { move(i, -1); });
      r.addEventListener('click', function () { move(i, 1); });
      x.addEventListener('click', function () { selection.splice(i, 1); saveSel(); renderSel(); paintAll(); });
      [l, r, x].forEach(function (b) { li.appendChild(b); });
      li.addEventListener('dragstart', function (e) { dragFrom = i; try { e.dataTransfer.setData('text/plain', p); e.dataTransfer.effectAllowed = 'move'; } catch (er) { /* ignore */ } });
      li.addEventListener('dragover', function (e) { if (dragFrom >= 0) { e.preventDefault(); li.classList.add('over'); } });
      li.addEventListener('dragleave', function () { li.classList.remove('over'); });
      li.addEventListener('drop', function (e) {
        li.classList.remove('over');
        if (dragFrom < 0 || dragFrom === i) return;
        e.preventDefault(); e.stopPropagation();
        var it = selection.splice(dragFrom, 1)[0];
        selection.splice(i, 0, it);
        dragFrom = -1; saveSel(); renderSel();
      });
      ul.appendChild(li);
    });
    var kind = selKind();
    $('compare').disabled = selection.length < 2 || kind === 'mixed';
    $('compare').textContent = kind === 'runs' && !$('blind').checked ? 'Compare runs' : 'Compare';
    $('selhint').textContent = kind === 'mixed' ? 'The selection mixes runs and single images; clear it and pick one kind.'
      : selection.length < 2 ? 'Select 2 to 6 runs, or 2 to 6 single images (open a run row with "Images"). The first is the FLIP reference; drag chips (or use the arrows) to reorder.'
      : selection.length + (kind === 'images' ? ' images' : ' runs') + '; "' + selection[0] + '" is the reference.' + (kind === 'runs' && !$('blind').checked ? ' Compare opens the run overview.' : '');
  }
  function compare() {
    var kind = selKind();
    // Whole runs open the overview first; single images and blind judging go straight to a viewer.
    if (kind === 'runs' && !$('blind').checked) {
      location.href = '/runs?ref=' + enc(selection[0]) + '&' + selection.slice(1).map(function (p) { return 'run=' + enc(p); }).join('&');
      return;
    }
    var url = '/compare?' + selection.map(function (p) { return 'run=' + enc(p); }).join('&');
    if ($('blind').checked) url += '&blind=1';
    location.href = url;
  }

  // ---------- drag-and-drop upload ----------
  var slots = { a: null, b: null };
  function slotLabel(s) { return s.label + ' (' + s.files.length + (s.files.length === 1 ? ' image' : ' images') + ')'; }
  function renderSlots() {
    ['a', 'b'].forEach(function (k) { $('v' + k).textContent = slots[k] ? slotLabel(slots[k]) : 'empty'; });
    $('upload').disabled = !(slots.a && slots.b);
  }
  function msg(t, bad) { $('dropmsg').textContent = t; $('dropmsg').className = 'dropmsg' + (bad ? ' err' : ''); }

  function walkDir(entry) {
    // directory entries: paths are relative to the dropped directory
    var out = [];
    function rec(e, prefix) {
      if (e.isFile) return new Promise(function (res) {
        e.file(function (f) { if (IMG_RE.test(f.name)) out.push({ name: prefix + f.name, file: f }); res(); }, function () { res(); });
      });
      var reader = e.createReader();
      function more() {
        return new Promise(function (res) { reader.readEntries(res, function () { res([]); }); }).then(function (list) {
          if (!list.length) return;
          return Promise.all(list.map(function (c) { return rec(c, prefix + (e === entry ? '' : e.name + '/')); })).then(more);
        });
      }
      return more();
    }
    return rec(entry, '').then(function () { return { label: entry.name, files: out }; });
  }
  function fromEntry(entry, file) {
    if (entry && entry.isDirectory) return walkDir(entry);
    var f = file || null;
    if (entry && entry.isFile) return new Promise(function (res) { entry.file(function (ff) { res(ff); }, function () { res(null); }); }).then(function (ff) { return single(ff); });
    return Promise.resolve(single(f));
  }
  function single(f) {
    if (!f || !IMG_RE.test(f.name)) return null;
    return { label: f.name, files: [{ name: f.name, file: f }] };
  }
  function assign(items, which) {
    return Promise.all(items).then(function (res) {
      var ok = res.filter(function (x) { return x && x.files.length; });
      if (ok.length < res.length) msg('Some items had no supported image (png, jpg, exr, hdr) and were skipped.', true);
      if (which) { if (ok[0]) slots[which] = ok[0]; }
      else {
        var empty = ['a', 'b'].filter(function (k) { return !slots[k]; });
        var order = ok.length >= 2 ? ['a', 'b'] : empty;
        ok.slice(0, 2).forEach(function (x, i) { if (order[i]) slots[order[i]] = x; });
      }
      renderSlots();
    });
  }
  function dropItems(dt) {
    var items = [];
    if (dt.items && dt.items.length) {
      for (var i = 0; i < dt.items.length; i++) {
        var it = dt.items[i];
        if (it.kind !== 'file') continue;
        var entry = it.webkitGetAsEntry ? it.webkitGetAsEntry() : null;
        items.push(fromEntry(entry, it.getAsFile()));
      }
    } else if (dt.files) {
      for (var j = 0; j < dt.files.length; j++) items.push(Promise.resolve(single(dt.files[j])));
    }
    return items;
  }
  function pickInput(which, dir) {
    var inp = dir ? $('in-dir') : $('in-file');
    inp.value = '';
    inp.onchange = function () {
      var files = Array.prototype.slice.call(inp.files || []);
      if (!files.length) return;
      if (!dir) { var s = single(files[0]); if (s) { slots[which] = s; renderSlots(); } else msg('Not a supported image.', true); return; }
      var list = [];
      files.forEach(function (f) {
        if (!IMG_RE.test(f.name)) return;
        var rel = (f.webkitRelativePath || f.name).split('/').slice(1).join('/') || f.name;
        list.push({ name: rel, file: f });
      });
      var label = files[0].webkitRelativePath ? files[0].webkitRelativePath.split('/')[0] : 'folder';
      if (list.length) { slots[which] = { label: label, files: list }; renderSlots(); } else msg('No supported images in that folder.', true);
    };
    inp.click();
  }
  function rand32() {
    var b = new Uint8Array(16); crypto.getRandomValues(b);
    return Array.prototype.map.call(b, function (x) { return ('0' + x.toString(16)).slice(-2); }).join('');
  }
  function post(url, body) {
    return fetch(url, { method: 'POST', headers: { 'X-Saccade-Token': TOKEN }, body: body }).then(function (r) {
      return r.json().then(function (j) { if (!r.ok) throw new Error(j && j.error ? j.error : 'HTTP ' + r.status); return j; });
    });
  }
  function upload() {
    var id = rand32();
    var jobs = [];
    ['a', 'b'].forEach(function (side) { slots[side].files.forEach(function (f) { jobs.push({ side: side, f: f }); }); });
    $('upload').disabled = true;
    var done = 0;
    function next() {
      if (done >= jobs.length) {
        var la = slots.a.label, lb = slots.b.label;
        if (la === lb) lb += ' (B)';
        return post('/api/upload/open?upload=' + id + '&labels=' + enc(la) + ',' + enc(lb), null);
      }
      var j = jobs[done++];
      msg('Uploading ' + done + ' / ' + jobs.length + ': ' + j.f.name);
      return post('/api/upload?upload=' + id + '&side=' + j.side + '&name=' + enc(j.f.name), j.f.file).then(next);
    }
    next().then(function (r) { location.href = r.url; }).catch(function (e) { msg(String(e.message || e), true); $('upload').disabled = false; });
  }

  function landing() {
    $('landing').hidden = false;
    loadSel(); renderSel(); renderSlots(); loadDecisions();
    $('filters').addEventListener('submit', function (e) { e.preventDefault(); runSearch(false); });
    $('recentbtn').addEventListener('click', function () { runSearch(true); });
    $('compare').addEventListener('click', compare);
    $('clear').addEventListener('click', function () { selection = []; saveSel(); renderSel(); paintAll(); });
    $('blind').addEventListener('change', renderSel);
    $('upload').addEventListener('click', upload);
    document.querySelectorAll('[data-pick]').forEach(function (b) {
      b.addEventListener('click', function () { var p = b.getAttribute('data-pick').split('-'); pickInput(p[0], p[1] === 'dir'); });
    });
    var zone = $('drop');
    ['dragenter', 'dragover'].forEach(function (t) { zone.addEventListener(t, function (e) { if (e.dataTransfer && Array.prototype.indexOf.call(e.dataTransfer.types || [], 'Files') >= 0) { e.preventDefault(); zone.classList.add('over'); } }); });
    zone.addEventListener('dragleave', function (e) { if (e.target === zone) zone.classList.remove('over'); });
    zone.addEventListener('drop', function (e) {
      zone.classList.remove('over');
      if (!e.dataTransfer || Array.prototype.indexOf.call(e.dataTransfer.types || [], 'Files') < 0) return;
      e.preventDefault();
      var slotEl = e.target.closest ? e.target.closest('.slot') : null;
      var which = slotEl ? (slotEl.id === 'slot-a' ? 'a' : 'b') : null;
      var items = dropItems(e.dataTransfer);
      assign(which ? items.slice(0, 1) : items, which);
    });
    window.addEventListener('dragover', function (e) { if (e.dataTransfer && Array.prototype.indexOf.call(e.dataTransfer.types || [], 'Files') >= 0) e.preventDefault(); });
    window.addEventListener('drop', function (e) { if (e.dataTransfer && Array.prototype.indexOf.call(e.dataTransfer.types || [], 'Files') >= 0) e.preventDefault(); });
    window.addEventListener('popstate', function () { navigate(hashPath(), true); });
    navigate(hashPath(), true);
  }
  function hashPath() {
    var h = decodeURIComponent((location.hash || '').replace(/^#\/?/, ''));
    return h;
  }

  var shared = window.__saccadeUI, actions = shared.use(shared.actions());
  shared.controls(actions);
  actions.add({ id: 'navigate.home', group: 'Navigate', title: 'Browse archive roots', run: function () { navigate(''); } });
  actions.add({ id: 'navigate.recent', group: 'Navigate', title: 'Show recent runs', run: function () { runSearch(true); } });
  actions.add({ id: 'copy', group: 'Tools', title: 'Copy link to this page', run: function () { shared.copy(location.href).then(function (ok) { shared.toast(ok ? 'Link copied' : 'Copy failed'); }); } });
  actions.add({ id: 'help', group: 'General', title: 'Keyboard shortcuts', keys: ['?'], run: function () { shared.help.toggle(actions); } });
  actions.add({ id: 'escape', group: 'General', title: 'Close panel', keys: ['Escape'], run: shared.escape });
  $('palette-btn').addEventListener('click', function () { shared.palette.open(); });

  if (P.mode === 'progress') progress(); else landing();
})();

// Isolated inbox badge: independent of archive browsing and selection state.
(() => {
  const badge = document.getElementById('inboxbadge');
  if (!badge) return;
  async function updateInboxBadge() {
    try {
      const response = await fetch('/api/inbox');
      if (response.ok) badge.textContent = 'Inbox (' + (await response.json()).filter(i => i.status === 'open').length + ' open)';
    } catch (_) { /* The archive remains usable if an inbox refresh fails. */ }
  }
  updateInboxBadge(); setInterval(updateInboxBadge, 5000);
})();
