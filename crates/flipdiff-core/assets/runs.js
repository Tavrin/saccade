(function () {
  'use strict';
  // Run overview page. Served (`mode: "serve"`): polls GET /api/runs until every
  // pair is measured. Static (`mode: "static"`): renders the embedded model.
  var C = window.FLIPDIFF_RUNS || {};
  var STATIC = C.mode === 'static';
  var M = null;
  var UI = { sort: 'worst', changed: false, cs: -1, csChanged: false, pos: 50, manual: -1, draft: [], sig: {} };

  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, kids) {
    var n = document.createElement(tag);
    if (attrs) for (var k in attrs) {
      if (attrs[k] == null) continue;
      if (k === 'text') n.textContent = attrs[k];
      else if (k === 'cls') n.className = attrs[k];
      else n.setAttribute(k, attrs[k]);
    }
    (kids || []).forEach(function (c) { if (c) n.appendChild(typeof c === 'string' ? document.createTextNode(c) : c); });
    return n;
  }
  function clear(n) { while (n.firstChild) n.removeChild(n.firstChild); }
  function enc(s) { return encodeURIComponent(s); }
  function fmt(v) { return v == null || !isFinite(v) ? '–' : (v < 0.001 && v > 0 ? v.toFixed(4) : v.toFixed(3)); }
  function plural(n, one, many) { return n + ' ' + (n === 1 ? one : many); }

  // ---------- pairing parameters of this page (index 0 is the reference) ----------
  var PAIRQ = {};
  (function () {
    var re = /(?:^|&)pair(\d+)=([^&]*)/g, m;
    while ((m = re.exec(C.query || '')) !== null) PAIRQ[m[1]] = decodeURIComponent(m[2]);
  })();
  function pageQuery(pairs) {
    var q = C.query || '';
    var base = q.split('&').filter(function (p) { return p && !/^pair\d+=/.test(p); });
    Object.keys(pairs).forEach(function (k) { if (pairs[k]) base.push('pair' + k + '=' + enc(pairs[k])); });
    return base.join('&');
  }
  function repair(i, value) {
    var p = {};
    Object.keys(PAIRQ).forEach(function (k) { p[k] = PAIRQ[k]; });
    if (value) p[i] = value; else delete p[i];
    location.href = '/runs?' + pageQuery(p);
  }
  function manualOf(i) {
    var v = PAIRQ[i] || '';
    if (v.indexOf('manual:') !== 0) return [];
    return v.slice(7).split(',').filter(Boolean).map(function (s) { var a = s.split('-'); return [+a[0], +a[1]]; });
  }

  // ---------- viewer links ----------
  // A session of the reference and the runs `idx`, opened on a set and a pair.
  function viewerUrl(idx, rowName) {
    var paths = [M.ref.path], labels = [M.ref.label], extra = '';
    idx.forEach(function (ci, j) {
      paths.push(M.runs[ci].path); labels.push(M.runs[ci].label);
      if (PAIRQ[ci + 1]) extra += '&pair' + (j + 1) + '=' + enc(PAIRQ[ci + 1]);
    });
    var u = '/compare?runs=' + paths.map(enc).join(',') + '&labels=' + labels.map(enc).join(',') + extra;
    if (rowName != null) u += '&set=' + enc(rowName) + '&a=' + enc(M.ref.label) + '&b=' + enc(labels[labels.length - 1]);
    return u;
  }

  // ---------- severity colour ----------
  var STOPS = [[0, [63, 185, 133]], [0.35, [230, 200, 74]], [0.65, [240, 138, 60]], [1, [214, 51, 74]]];
  function sevOf(mean) {
    if (mean == null || !isFinite(mean)) return 0;
    var lo = Math.log10(1e-4), hi = Math.log10(0.3);
    return Math.max(0, Math.min(1, (Math.log10(Math.max(mean, 1e-5)) - lo) / (hi - lo)));
  }
  function tint(mean) {
    var t = sevOf(mean), i = 0;
    while (i < STOPS.length - 2 && t > STOPS[i + 1][0]) i++;
    var a = STOPS[i], b = STOPS[i + 1], f = (t - a[0]) / (b[0] - a[0]);
    var c = a[1].map(function (v, k) { return Math.round(v + (b[1][k] - v) * f); });
    return 'rgb(' + c.join(',') + ')';
  }

  // ---------- header ----------
  function renderHead() {
    $('reflabel').textContent = 'Reference ' + M.ref.label + ' · ' + plural(M.ref.images.length, 'image', 'images') + ' · ' + plural(M.runs.length, 'run', 'runs') + ' compared';
    if (!STATIC) {
      $('back').hidden = false;
      var a = $('openall');
      a.hidden = false;
      a.href = viewerUrl(M.runs.map(function (_, i) { return i; }), null);
    }
    var p = M.progress, bar = $('progress');
    bar.hidden = p.complete;
    bar.className = 'bar wide det';
    clear(bar);
    bar.appendChild(el('i', { style: 'width:' + (p.total ? Math.round(100 * p.done / p.total) : 0) + '%' }));
    $('mprog').textContent = p.complete ? plural(p.total, 'pair', 'pairs') + ' measured' : 'Measuring ' + p.done + ' of ' + p.total + ' pairs…';
  }

  // ---------- run-level configuration ----------
  function renderCfg() {
    var t = $('cfgtable'); clear(t);
    $('cfgcard').hidden = !M.config_differences.length;
    if (!M.config_differences.length) return;
    var head = el('tr', null, [el('th', { text: 'Key' }), el('th', { text: M.ref.label + ' (ref)' })]);
    M.runs.forEach(function (r) { head.appendChild(el('th', { text: r.label })); });
    t.appendChild(el('thead', null, [head]));
    var body = el('tbody');
    M.config_differences.forEach(function (d) {
      var tr = el('tr', null, [el('td', { cls: 'k', text: d.key })]);
      d.values.forEach(function (v, i) {
        var cls = 'v' + (i > 0 && v !== d.values[0] ? ' diff' : '') + (v === '<absent>' ? ' na' : '');
        tr.appendChild(el('td', { cls: cls, text: v }));
      });
      body.appendChild(tr);
    });
    t.appendChild(body);
  }

  // ---------- per-run summaries ----------
  function stat(cls, n, label, extra) {
    return el('span', { cls: 's ' + cls }, [el('b', { text: String(n) }), ' ' + label, extra || null]);
  }
  function statsLine(r) {
    var parts = [];
    parts.push(stat(r.identical ? 'ok' : '', r.identical, 'identical'));
    parts.push(stat(r.changed ? 'bad' : '', r.changed, 'changed', r.worst ? el('span', null, [' (worst ', el('code', { text: r.worst.name }), ' ' + fmt(r.worst.mean) + ')']) : null));
    parts.push(stat('', r.only_in_ref, 'only-in-ref'));
    parts.push(stat('', r.only_in_run, 'only-in-run'));
    if (r.errors) parts.push(stat('bad', r.errors, 'errors'));
    if (r.pending) parts.push(stat('warn', r.pending, 'pending'));
    parts.push(el('span', { cls: 's' }, ['config differs: ', el('b', { text: String(r.config_differs.length) }), ' ' + (r.config_differs.length === 1 ? 'key' : 'keys')]));
    var line = el('div', { cls: 'stats' });
    parts.forEach(function (p, i) {
      // spaces around the separator are the line-break opportunities
      if (i) { line.appendChild(document.createTextNode(' ')); line.appendChild(el('span', { cls: 'sep', text: '·' })); line.appendChild(document.createTextNode(' ')); }
      line.appendChild(p);
    });
    return line;
  }
  function btn(text, fn, cls) {
    var b = el('button', { type: 'button', cls: 'btn small ' + (cls || ''), text: text });
    b.addEventListener('click', fn);
    return b;
  }
  function renderSummaries() {
    var box = $('summaries'); clear(box);
    M.runs.forEach(function (r, ci) {
      var card = el('article', { cls: 'rcard' + (r.no_visible_effect ? ' noeffect' : '') });
      card.appendChild(el('h3', null, [r.label, r.pairing !== 'name' ? el('span', { cls: 'chip', text: 'paired by ' + (r.pairing === 'position' ? 'position' : 'hand') }) : null]));
      card.appendChild(el('div', { cls: 'path', text: r.path }));
      if (r.no_visible_effect) {
        card.appendChild(el('div', { cls: 'flag noeffect' }, [el('b', { text: 'No visible effect: ' }), 'this run changed nothing. Every image is bit-identical to ' + M.ref.label + '.']));
      }
      var refN = M.ref.images.length;
      if (r.mismatch && r.pairing === 'name') {
        var f = el('div', { cls: 'flag mismatch' }, [el('b', { text: r.name_matches + ' of ' + refN + ' file names match' }), ' between ' + M.ref.label + ' and ' + r.label + ' (' + plural(r.images, 'image', 'images') + ' in the run), so almost nothing was compared.']);
        if (!STATIC) {
          f.appendChild(el('div', { cls: 'acts' }, [
            btn('Pair by position', function () { repair(ci + 1, 'position'); }),
            btn('Pair manually…', function () { openManual(ci); })
          ]));
        }
        card.appendChild(f);
      } else if (r.mismatch) {
        var g = el('div', { cls: 'flag paired' }, [el('b', { text: 'Paired by ' + (r.pairing === 'position' ? 'position' : 'hand') + '. ' }), 'Only ' + r.name_matches + ' of ' + refN + ' file names match.']);
        if (!STATIC) {
          g.appendChild(el('div', { cls: 'acts' }, [
            btn('Match by name instead', function () { repair(ci + 1, null); }),
            btn(r.pairing === 'manual' ? 'Edit pairing…' : 'Pair manually…', function () { openManual(ci); })
          ]));
        }
        card.appendChild(g);
      }
      if (r.config_error) card.appendChild(el('div', { cls: 'flag cfgerr', text: 'Run configuration not read: ' + r.config_error }));
      card.appendChild(statsLine(r));
      var acts = el('div', { cls: 'acts' });
      if (!STATIC) {
        var a = el('a', { cls: 'btn small primary', href: viewerUrl([ci], null), text: 'Open in viewer' });
        acts.appendChild(a);
      }
      acts.appendChild(btn('Contact sheet', function () { UI.cs = ci; renderContact(true); $('contactcard').scrollIntoView({ behavior: 'smooth', block: 'start' }); }));
      card.appendChild(acts);
      box.appendChild(card);
    });
  }

  // ---------- pair by hand ----------
  function openManual(ci) {
    var r = M.runs[ci], refN = M.ref.images.length;
    UI.manual = ci;
    UI.draft = [];
    for (var i = 0; i < refN; i++) UI.draft.push(-1);
    var seed = r.pairing === 'manual' ? manualOf(ci + 1) : [];
    if (!seed.length) {
      // start from the names the two runs already share
      r.run_images.forEach(function (im, j) {
        for (var k = 0; k < refN; k++) if (M.ref.images[k].name === im.name) UI.draft[k] = j;
      });
    }
    seed.forEach(function (p) { if (p[0] < refN) UI.draft[p[0]] = p[1]; });
    renderPair();
    $('pairpanel').scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
  function imgBox(im, cls) {
    return el('div', { cls: 'pimg ' + (cls || '') }, [im.thumb ? el('img', { src: im.thumb, alt: '', loading: 'lazy', draggable: 'false' }) : null, el('span', { cls: 'nm', text: im.name })]);
  }
  function renderPair() {
    var panel = $('pairpanel');
    clear(panel);
    if (UI.manual < 0) { panel.hidden = true; return; }
    panel.hidden = false;
    var r = M.runs[UI.manual];
    var used = {};
    UI.draft.forEach(function (j) { if (j >= 0) used[j] = true; });
    panel.appendChild(el('div', { cls: 'cardhead' }, [
      el('h2', { text: 'Pair ' + r.label + ' with ' + M.ref.label + ' by hand' }),
      el('span', { cls: 'hint', text: 'Drag an image of the run onto a reference image, or pick it from the list.' })
    ]));
    var rows = el('div', { cls: 'pairrows' });
    M.ref.images.forEach(function (im, i) {
      var slot = el('div', { cls: 'pimg slot' });
      var j = UI.draft[i];
      if (j >= 0) {
        var ri = r.run_images[j];
        if (ri.thumb) slot.appendChild(el('img', { src: ri.thumb, alt: '' }));
        slot.appendChild(el('span', { cls: 'nm', text: ri.name }));
      } else slot.appendChild(el('span', { cls: 'ph', text: 'drop an image here' }));
      var sel = el('select', { 'aria-label': 'Image of ' + r.label + ' for ' + im.name });
      sel.appendChild(el('option', { value: '-1', text: '(none)' }));
      r.run_images.forEach(function (x, jj) {
        var o = el('option', { value: String(jj), text: x.name + (used[jj] && UI.draft[i] !== jj ? ' (used)' : '') });
        if (jj === UI.draft[i]) o.selected = true;
        sel.appendChild(o);
      });
      sel.addEventListener('change', function () { assign(i, +sel.value); });
      slot.appendChild(sel);
      slot.addEventListener('dragover', function (e) { e.preventDefault(); slot.classList.add('over'); });
      slot.addEventListener('dragleave', function () { slot.classList.remove('over'); });
      slot.addEventListener('drop', function (e) {
        e.preventDefault(); slot.classList.remove('over');
        var v = e.dataTransfer && e.dataTransfer.getData('text/plain');
        if (v !== '' && v != null && !isNaN(+v)) assign(i, +v);
      });
      rows.appendChild(el('div', { cls: 'prow' }, [imgBox(im), el('span', { cls: 'arrow', text: '↔' }), slot]));
    });
    var tray = el('div', { cls: 'tray' });
    r.run_images.forEach(function (im, j) {
      var b = imgBox(im, used[j] ? 'used' : '');
      b.setAttribute('draggable', 'true');
      b.addEventListener('dragstart', function (e) { try { e.dataTransfer.setData('text/plain', String(j)); e.dataTransfer.effectAllowed = 'move'; } catch (er) { /* ignore */ } });
      tray.appendChild(b);
    });
    panel.appendChild(el('div', { cls: 'pairgrid' }, [
      el('div', null, [el('h2', { text: M.ref.label + ' (reference)' }), rows]),
      el('div', null, [el('h2', { text: r.label + ' images' }), tray])
    ]));
    var pairs = UI.draft.map(function (j, i) { return j >= 0 ? i + '-' + j : null; }).filter(Boolean);
    panel.appendChild(el('div', { cls: 'pairbar' }, [
      btn('Compare ' + plural(pairs.length, 'pair', 'pairs'), function () { repair(UI.manual + 1, 'manual:' + pairs.join(',')); }, 'primary'),
      btn('Fill by position', function () {
        UI.draft = UI.draft.map(function (_, i) { return i < r.run_images.length ? i : -1; });
        renderPair();
      }),
      btn('Clear', function () { UI.draft = UI.draft.map(function () { return -1; }); renderPair(); }),
      btn('Cancel', function () { UI.manual = -1; renderPair(); })
    ]));
    var disabled = panel.querySelector('.pairbar .primary');
    if (disabled && !pairs.length) disabled.disabled = true;
  }
  function assign(i, j) {
    if (j >= 0) UI.draft = UI.draft.map(function (x) { return x === j ? -1 : x; });
    UI.draft[i] = j;
    renderPair();
  }

  // ---------- matrix ----------
  function rowHasDiff(row) {
    return row.cells.some(function (c) { return c.status !== 'identical' && c.status !== 'absent'; });
  }
  function sortedRows() {
    var rows = M.images.map(function (r, i) { return { r: r, i: i }; });
    if (UI.changed) rows = rows.filter(function (x) { return rowHasDiff(x.r); });
    var rank = function (r) { return r.worst != null ? 0 : rowHasDiff(r) ? 1 : 2; };
    rows.sort(function (a, b) {
      if (UI.sort === 'worst') {
        var d = rank(a.r) - rank(b.r);
        if (d) return d;
        if (a.r.worst !== b.r.worst) return (b.r.worst == null ? -1 : b.r.worst) - (a.r.worst == null ? -1 : a.r.worst);
      }
      return a.r.name < b.r.name ? -1 : a.r.name > b.r.name ? 1 : 0;
    });
    return rows;
  }
  function cellEl(row, ri, c, ci) {
    var base = { 'data-ri': String(ri), 'data-ci': String(ci) };
    function mk(tag, cls, kids, extra) {
      var a = { cls: 'cell ' + cls, 'data-ri': base['data-ri'], 'data-ci': base['data-ci'] };
      for (var k in (extra || {})) a[k] = extra[k];
      return el(tag, a, kids);
    }
    var img = c.thumb ? el('img', { src: c.thumb, alt: '', loading: 'lazy' }) : null;
    switch (c.status) {
      case 'identical': return mk('div', 'same', ['=', el('small', { text: 'identical' })], { title: 'Bit-identical to the reference' });
      case 'changed': {
        var mean = c.metrics ? c.metrics.mean : null;
        var kids = [img, el('span', { cls: 'tint' }), el('span', { cls: 'val', text: fmt(mean) })];
        var extra = { style: '--tint:' + tint(mean) };
        if (!STATIC && row.ref_image) { extra.href = viewerUrl([ci], row.name); return mk('a', 'chg', kids, extra); }
        return mk('div', 'chg', kids, extra);
      }
      case 'pending': return mk('div', 'pend', [], { title: 'Measuring…' });
      case 'error': return mk('div', 'err', ['! ' + (c.error ? c.error.slice(0, 60) : 'error')], { title: c.error || 'error' });
      case 'only_in_run': return mk('div', 'dim', [img, el('span', { cls: 'badge2', text: 'only in run' })], { title: 'Only in this run' });
      case 'only_in_ref': return mk('div', 'none', ['–'], { title: 'The run has no such image' });
      default: return mk('div', 'none', ['–'], { title: 'Not in this run' });
    }
  }
  function renderMatrix(force) {
    $('matrixcard').hidden = !M.images.length;
    if (!M.images.length) return;
    var rows = sortedRows();
    var sig = [M.progress.done, UI.sort, UI.changed, rows.length].join('|');
    if (!force && UI.sig.matrix === sig) return;
    UI.sig.matrix = sig;
    var t = $('matrix'); clear(t);
    t.style.setProperty('--n', String(M.runs.length));
    var head = el('tr', null, [el('th', { cls: 'rowh', scope: 'col' }, [el('span', { cls: 'rl', text: 'Image' }), el('span', { cls: 'rs', text: plural(rows.length, 'row', 'rows') })])]);
    M.runs.forEach(function (r) {
      head.appendChild(el('th', { scope: 'col', title: r.summary }, [
        el('span', { cls: 'rl', text: r.label }),
        el('span', { cls: 'rs' + (r.no_visible_effect ? ' fx' : ''), text: r.no_visible_effect ? 'no visible effect' : r.identical + ' identical · ' + r.changed + ' changed' })
      ]));
    });
    t.appendChild(el('thead', null, [head]));
    var body = el('tbody');
    rows.forEach(function (x) {
      var row = x.r;
      var first = el('th', { cls: 'rowh', scope: 'row' }, [el('div', { cls: 'rowname' }, [
        row.ref_image && row.ref_image.thumb ? el('img', { src: row.ref_image.thumb, alt: '', loading: 'lazy' }) : null,
        el('span', { cls: row.ref_image ? '' : 'only', text: row.ref_image ? row.name : row.name + ' (not in the reference)', title: row.name })
      ])]);
      var tr = el('tr', null, [first]);
      row.cells.forEach(function (c, ci) { tr.appendChild(el('td', null, [cellEl(row, x.i, c, ci)])); });
      body.appendChild(tr);
    });
    t.appendChild(body);
    $('mnote').textContent = M.progress.complete ? 'Cells are tinted by mean FLIP, from green (barely visible) to red (large). Hover a cell for its heatmap and values' + (STATIC ? '.' : '; click it to open the viewer on that image.') : '';
  }

  // ---------- hover card ----------
  function tipRows(dl, pairs) {
    pairs.forEach(function (p) { dl.appendChild(el('dt', { text: p[0] })); dl.appendChild(el('dd', { text: p[1] })); });
  }
  function showTip(cell, ev) {
    var row = M.images[+cell.getAttribute('data-ri')], ci = +cell.getAttribute('data-ci');
    var c = row.cells[ci], r = M.runs[ci], tip = $('tip');
    clear(tip);
    tip.appendChild(el('div', { cls: 't', text: row.name }));
    tip.appendChild(el('div', { cls: 'u', text: r.label + (c.run_name ? ' · ' + c.run_name : '') + ' vs ' + M.ref.label }));
    var dl = el('dl');
    if (c.status === 'changed' && c.metrics) {
      if (c.heat) {
        var hi = el('img', { src: c.heat, alt: 'FLIP heatmap' });
        hi.addEventListener('load', function () { moveTip(ev); });  // the card grows once the heatmap is in
        tip.appendChild(hi);
      }
      tipRows(dl, [['mean FLIP', fmt(c.metrics.mean)], ['p95', fmt(c.metrics.p95)], ['p99', fmt(c.metrics.p99)], ['max', fmt(c.metrics.max)], ['size', c.metrics.width + '×' + c.metrics.height]]);
    } else if (c.status === 'identical') tipRows(dl, [['result', 'bit-identical']]);
    else if (c.status === 'error') tipRows(dl, [['error', c.error || 'error']]);
    else if (c.status === 'pending') tipRows(dl, [['result', 'measuring…']]);
    else if (c.status === 'only_in_run') tipRows(dl, [['result', 'only in the run']]);
    else tipRows(dl, [['result', 'missing in the run']]);
    tip.appendChild(dl);
    tip.hidden = false;
    moveTip(ev);
  }
  function moveTip(ev) {
    var tip = $('tip');
    var w = tip.offsetWidth, h = tip.offsetHeight, x = ev.clientX + 16, y = ev.clientY + 16;
    if (x + w > window.innerWidth - 8) x = ev.clientX - w - 16;
    if (y + h > window.innerHeight - 8) y = Math.max(8, window.innerHeight - h - 8);
    tip.style.left = Math.max(8, x) + 'px';
    tip.style.top = Math.max(8, y) + 'px';
  }

  // ---------- contact sheet ----------
  function defaultRun() {
    for (var i = 0; i < M.runs.length; i++) if (M.runs[i].changed) return i;
    return 0;
  }
  function setPos(v) {
    UI.pos = Math.max(0, Math.min(100, v));
    $('sheet').style.setProperty('--pos', UI.pos + '%');
    $('csslider').value = String(Math.round(UI.pos));
    $('csval').textContent = Math.round(UI.pos) + '%';
  }
  function tileEl(row, ci) {
    var c = row.cells[ci], r = M.runs[ci], im = row.ref_image;
    var bad = c.status === 'error';
    var chip;
    if (c.status === 'changed') chip = el('span', { cls: 'v', style: '--tint:' + tint(c.metrics && c.metrics.mean), text: fmt(c.metrics && c.metrics.mean) });
    else if (c.status === 'identical') chip = el('span', { cls: 'v same', text: '=' });
    else if (bad) chip = el('span', { cls: 'v bad', text: 'error' });
    else chip = el('span', { cls: 'v same', text: c.status === 'pending' ? '…' : '–' });
    var stage = el('div', { cls: 'stage', title: bad ? c.error : null });
    var refImg = el('img', { src: im.preview || im.thumb, alt: row.name + ' in ' + M.ref.label, loading: 'lazy' });
    refImg.addEventListener('load', function () { if (refImg.naturalWidth) stage.style.setProperty('--ar', refImg.naturalWidth + ' / ' + refImg.naturalHeight); });
    stage.appendChild(refImg);
    var hasRun = (c.status === 'identical' || c.status === 'changed' || c.status === 'pending') && (c.preview || c.thumb);
    if (hasRun) {
      stage.appendChild(el('div', { cls: 'top' }, [el('img', { src: c.preview || c.thumb, alt: row.name + ' in ' + r.label, loading: 'lazy' })]));
      stage.appendChild(el('div', { cls: 'div' }));
      stage.appendChild(el('span', { cls: 'corner l', text: M.ref.label }));
      stage.appendChild(el('span', { cls: 'corner r', text: r.label }));
    } else {
      stage.appendChild(el('div', { cls: 'gone', text: bad ? (c.error || 'could not be compared') : r.label + ' has no ' + row.name }));
      stage.style.cursor = 'default';
    }
    var tile = el('article', { cls: 'tile' }, [el('header', null, [el('span', { cls: 'n', text: row.name }), chip]), stage]);
    if (!STATIC && hasRun) tile.appendChild(el('a', { cls: 'open', href: viewerUrl([ci], row.name), text: 'Open in viewer →' }));
    return tile;
  }
  function renderContact(force) {
    var rows = M.images.filter(function (r) { return r.ref_image; });
    $('contactcard').hidden = !rows.length;
    if (!rows.length) return;
    if (UI.cs < 0 || UI.cs >= M.runs.length) UI.cs = defaultRun();
    var sel = $('csrun');
    if (sel.options.length !== M.runs.length) {
      clear(sel);
      M.runs.forEach(function (r, i) { sel.appendChild(el('option', { value: String(i), text: r.label })); });
    }
    sel.value = String(UI.cs);
    $('refname').textContent = M.ref.label;
    if (UI.csChanged) rows = rows.filter(function (r) { return r.cells[UI.cs].status !== 'identical'; });
    var sig = [UI.cs, UI.csChanged, rows.map(function (r) { return r.cells[UI.cs].status; }).join(',')].join('|');
    if (!force && UI.sig.sheet === sig) return;
    UI.sig.sheet = sig;
    var sheet = $('sheet'); clear(sheet);
    rows.slice().sort(function (a, b) { return a.name < b.name ? -1 : 1; }).forEach(function (r) { sheet.appendChild(tileEl(r, UI.cs)); });
    if (!rows.length) sheet.appendChild(el('p', { cls: 'hint', text: 'Every image of this run is bit-identical to the reference.' }));
    setPos(UI.pos);
  }
  function wireContact() {
    $('csrun').addEventListener('change', function () { UI.cs = +$('csrun').value; renderContact(true); });
    $('cschanged').addEventListener('change', function () { UI.csChanged = $('cschanged').checked; renderContact(true); });
    $('csslider').addEventListener('input', function () { setPos(+$('csslider').value); });
    document.addEventListener('keydown', function (e) {
      var t = e.target, tag = t && t.tagName;
      if (tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA' || e.altKey || e.ctrlKey || e.metaKey) return;
      if ($('contactcard').hidden) return;
      var step = e.shiftKey ? 10 : 2;
      if (e.key === 'ArrowLeft') { setPos(UI.pos - step); e.preventDefault(); }
      else if (e.key === 'ArrowRight') { setPos(UI.pos + step); e.preventDefault(); }
      else if (e.key === 'Home') { setPos(0); e.preventDefault(); }
      else if (e.key === 'End') { setPos(100); e.preventDefault(); }
    });
    var drag = null;
    $('sheet').addEventListener('pointerdown', function (e) {
      var st = e.target.closest ? e.target.closest('.stage') : null;
      if (!st || !st.querySelector('.top')) return;
      drag = st;
      try { st.setPointerCapture(e.pointerId); } catch (er) { /* ignore */ }
      move(e);
    });
    function move(e) {
      if (!drag) return;
      var rc = drag.getBoundingClientRect();
      setPos(100 * (e.clientX - rc.left) / rc.width);
    }
    $('sheet').addEventListener('pointermove', move);
    ['pointerup', 'pointercancel'].forEach(function (n) { $('sheet').addEventListener(n, function () { drag = null; }); });
  }

  // ---------- page ----------
  function wireMatrix() {
    $('msort').addEventListener('change', function () { UI.sort = $('msort').value; renderMatrix(true); });
    $('mchanged').addEventListener('change', function () { UI.changed = $('mchanged').checked; renderMatrix(true); });
    var m = $('matrix');
    m.addEventListener('mouseover', function (e) {
      var c = e.target.closest ? e.target.closest('.cell[data-ri]') : null;
      if (c) showTip(c, e); else $('tip').hidden = true;
    });
    m.addEventListener('mousemove', function (e) { if (!$('tip').hidden) moveTip(e); });
    m.addEventListener('mouseleave', function () { $('tip').hidden = true; });
    m.addEventListener('focusin', function (e) {
      var c = e.target.closest ? e.target.closest('.cell[data-ri]') : null;
      if (c) { var rc = c.getBoundingClientRect(); showTip(c, { clientX: rc.right, clientY: rc.top }); }
    });
    m.addEventListener('focusout', function () { $('tip').hidden = true; });
  }
  function render() {
    $('loading').hidden = true;
    document.title = 'flipdiff overview: ' + M.ref.label + ' vs ' + M.runs.map(function (r) { return r.label; }).join(', ');
    renderHead();
    renderCfg();
    renderSummaries();
    renderMatrix(false);
    renderContact(false);
    if (UI.manual >= 0 && !$('pairpanel').children.length) renderPair();
  }
  function fail(e) {
    $('loading').hidden = true;
    $('progress').hidden = true;
    var n = $('rerror');
    n.hidden = false;
    n.textContent = String(e && e.message ? e.message : e);
  }
  function poll() {
    fetch(C.api, { headers: { 'Accept': 'application/json' } }).then(function (r) {
      return r.json().then(function (j) { if (!r.ok) throw new Error(j && j.error ? j.error : 'HTTP ' + r.status); return j; });
    }).then(function (m) {
      M = m;
      render();
      if (!m.progress.complete) setTimeout(poll, 600);
    }).catch(fail);
  }
  wireMatrix();
  wireContact();
  if (STATIC) { M = C.model; render(); } else poll();
})();
