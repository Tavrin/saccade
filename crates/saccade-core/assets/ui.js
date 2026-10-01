/* Shared by every saccade page (report, view, serve, runs, inbox): the element
 * builder, one action registry that drives the keyboard, the command palette
 * and the help dialog, popovers, toasts, full screen, the pointer and zoom math
 * the two image stages share, and the display filter. Pages register actions;
 * nothing here knows what a report or a set is. */
(function () {
  'use strict';
  var UI = window.__saccadeUI = {};

  // ---- element builder: strings are text nodes, so data is never parsed as HTML ----
  UI.h = function (tag, attrs) {
    var el = document.createElement(tag);
    if (attrs) {
      Object.keys(attrs).forEach(function (k) {
        var v = attrs[k];
        if (v === null || v === undefined || v === false) return;
        if (k === 'class' || k === 'cls') el.className = v;
        else if (k === 'text') el.textContent = v;
        else if (k.indexOf('on') === 0) el.addEventListener(k.slice(2), v);
        else el.setAttribute(k, v === true ? '' : v);
      });
    }
    (function add(list) {
      for (var i = 0; i < list.length; i++) {
        var c = list[i];
        if (c === null || c === undefined || c === false) continue;
        if (Array.isArray(c)) add(c);
        else el.appendChild(typeof c === 'string' ? document.createTextNode(c) : c);
      }
    })(Array.prototype.slice.call(arguments, 2));
    return el;
  };
  var h = UI.h;

  UI.typing = function (t) {
    if (!t || !t.tagName) return false;
    if (t.isContentEditable || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT') return true;
    return t.tagName === 'INPUT' && !/^(range|checkbox|radio|button)$/.test(t.type);
  };

  // The element new overlays attach to: the full-screen element while one is up, else <body>.
  function host() { return document.fullscreenElement || document.body; }

  // ---- keys ----
  var KEY_NAMES = { ArrowLeft: '←', ArrowRight: '→', ArrowUp: '↑', ArrowDown: '↓', ' ': 'Space', Escape: 'Esc', Enter: '↵' };
  function prettyKey(k) { return KEY_NAMES[k] || k; }
  UI.keysEl = function (list) {
    return h('span', { class: 'keys' }, (list || []).map(function (k) { return h('kbd', { text: prettyKey(k) }); }));
  };
  function normKey(k) { return k.length === 1 ? k.toLowerCase() : k; }

  // ---- action registry ----
  // def: { id, title, group, keys: [bound key names], show: [display names],
  //        run(ev), enabled(), on(), hold: {down, up}, repeat, keywords }
  // Every registered action is searchable, including keyboard-only actions.
  UI.actions = function () {
    var list = [], byKey = {}, dyn = [];
    var api = {
      add: function (def) {
        list.push(def);
        (def.keys || []).forEach(function (k) { byKey[normKey(k)] = def; });
        return api;
      },
      all: function () { return list.slice(); },
      byId: function (id) { for (var i = 0; i < list.length; i++) if (list[i].id === id) return list[i]; return null; },
      // fn() returns extra palette items built from the page's current data (jump to a set by name).
      dynamic: function (fn) { dyn.push(fn); return api; },
      items: function () {
        var out = list.slice();
        dyn.forEach(function (fn) { out = out.concat(fn() || []); });
        return out;
      },
      forKey: function (ev) { return byKey[normKey(ev.key)] || null; }
    };
    return api;
  };

  // Data-driven pages register their live DOM controls as palette commands too.
  UI.controls = function (reg, root) {
    reg.dynamic(function () {
      var host = root || document;
      return Array.prototype.filter.call(host.querySelectorAll('button, a.btn, select, input[type=checkbox]'), function (n) {
        return n.id !== 'palette-btn' && !n.closest('.pal, .help') && n.getClientRects().length;
      }).reduce(function (out, n, i) {
        var label = n.getAttribute('aria-label') || (n.tagName === 'SELECT' && n.closest('label') && n.closest('label').childNodes[0].textContent.trim()) || n.textContent.trim() || (n.closest('label') && n.closest('label').textContent.trim()) || n.title;
        if (!label) return out;
        if (n.tagName === 'SELECT') {
          Array.prototype.forEach.call(n.options, function (o) {
            out.push({ id: 'control.' + i + '.' + o.value, title: label + ': ' + o.textContent, group: 'Settings', enabled: function () { return !n.disabled; }, run: function () { n.value = o.value; n.dispatchEvent(new Event('change', { bubbles: true })); } });
          });
        } else out.push({ id: 'control.' + i, title: label, group: 'Page', enabled: function () { return !n.disabled; }, run: function () { n.click(); } });
        return out;
      }, []);
    });
  };

  var registry = null;
  UI.use = function (reg) { registry = reg; return reg; };
  function enabled(d) { return !d.enabled || d.enabled(); }

  document.addEventListener('keydown', function (ev) {
    if ((ev.ctrlKey || ev.metaKey) && !ev.altKey && (ev.key === 'k' || ev.key === 'K')) {
      ev.preventDefault();
      if (palette.isOpen()) palette.close(); else palette.open();
      return;
    }
    if (!registry || ev.ctrlKey || ev.metaKey || ev.altKey) return;
    if (palette.isOpen() || UI.help.isOpen() || UI.typing(ev.target)) return;
    // Let a focused control activate itself, once.
    if ((ev.key === ' ' || ev.key === 'Enter') && ev.target.closest && ev.target.closest('button, a, input')) return;
    var d = registry.forKey(ev);
    if (!d || !enabled(d)) return;
    if (d.prevent !== false) ev.preventDefault();
    if (d.hold) { if (!ev.repeat) d.hold.down(ev); return; }
    if (ev.repeat && !d.repeat) return;
    if (d.run) d.run(ev);
  });
  document.addEventListener('keyup', function (ev) {
    if (!registry || UI.typing(ev.target)) return;
    var d = registry.forKey(ev);
    if (d && d.hold) d.hold.up(ev);
    // Space activates a focused button on keyup; keep it from doing so after it ran an action.
    else if (d && ev.key === ' ' && d.prevent !== false && !(ev.target.closest && ev.target.closest('button, a, input'))) ev.preventDefault();
  });

  // ---- layers: Escape closes the topmost one ----
  UI.escape = function () {
    if (palette.isOpen()) { palette.close(); return true; }
    if (UI.help.isOpen()) { UI.help.hide(); return true; }
    return popovers.closeAll();
  };

  // ---- fuzzy match ----
  // Every space-separated token must match as a substring or, failing that, as a
  // subsequence. Returns {score, idx} (idx: matched positions in `s`) or null.
  function fuzzyToken(tok, s) {
    var at = s.indexOf(tok);
    if (at >= 0) {
      var idx = [];
      for (var i = 0; i < tok.length; i++) idx.push(at + i);
      return { score: 100 + tok.length * 4 - at * 0.5 + (at === 0 || /[\s/_.\-:]/.test(s.charAt(at - 1)) ? 20 : 0), idx: idx };
    }
    var pos = [], last = -2, sc = 0, j = 0;
    for (var k = 0; k < s.length && j < tok.length; k++) {
      if (s.charAt(k) !== tok.charAt(j)) continue;
      sc += 4 + (k === last + 1 ? 6 : 0) + (k === 0 || /[\s/_.\-:]/.test(s.charAt(k - 1)) ? 8 : 0);
      pos.push(k); last = k; j++;
    }
    return j === tok.length ? { score: sc - (pos[pos.length - 1] - pos[0]) * 0.3, idx: pos } : null;
  }
  UI.fuzzy = function (query, title, extra) {
    var toks = query.toLowerCase().split(/\s+/).filter(Boolean);
    if (!toks.length) return { score: 0, idx: [] };
    var t = title.toLowerCase(), all = (extra + ' ' + title).toLowerCase(), score = 0, idx = [];
    for (var i = 0; i < toks.length; i++) {
      var m = fuzzyToken(toks[i], t), bonus = 0;
      if (!m) { m = fuzzyToken(toks[i], all); bonus = -30; if (!m) return null; m = { score: m.score, idx: [] }; }
      score += m.score + bonus;
      idx = idx.concat(m.idx);
    }
    return { score: score, idx: idx };
  };

  // ---- command palette ----
  var palette = UI.palette = (function () {
    var root = null, input = null, list = null, items = [], shown = [], sel = 0, prevFocus = null;
    function itemText(it) { return (it.group || '') + ' ' + (it.keywords || ''); }
    function highlight(title, idx) {
      var span = h('span', { class: 'pal-t' });
      if (!idx.length) { span.textContent = title; return span; }
      var set = {}; idx.forEach(function (i) { set[i] = true; });
      var run = '', on = false;
      function flush() {
        if (!run) return;
        span.appendChild(on ? h('mark', { text: run }) : document.createTextNode(run));
        run = '';
      }
      for (var i = 0; i < title.length; i++) {
        var m = !!set[i];
        if (m !== on) { flush(); on = m; }
        run += title.charAt(i);
      }
      flush();
      return span;
    }
    function filter() {
      var q = input.value.trim();
      var scored = [];
      items.forEach(function (it, n) {
        var r = UI.fuzzy(q, it.title, itemText(it));
        if (r) scored.push({ it: it, r: r, n: n });
      });
      if (q) scored.sort(function (a, b) { return (b.r.score - a.r.score) || (a.n - b.n); });
      shown = scored;
      sel = Math.min(sel, Math.max(0, shown.length - 1));
      draw();
    }
    function draw() {
      list.textContent = '';
      if (!shown.length) { list.appendChild(h('li', { class: 'pal-empty', role: 'presentation', text: 'No matching action' })); input.removeAttribute('aria-activedescendant'); return; }
      shown.forEach(function (s, n) {
        var it = s.it, off = !enabled(it);
        var on = it.on && it.on();
        var li = h('li', { class: 'pal-i' + (off ? ' off' : ''), role: 'option', id: 'pal-o' + n, 'data-action': it.id, 'aria-disabled': off ? 'true' : 'false', 'aria-selected': n === sel ? 'true' : 'false' },
          h('span', { class: 'pal-g', text: it.group || '' }),
          highlight(it.title + (on ? ' (on)' : ''), s.r.idx),
          (it.show || it.keys) && (it.show || it.keys).length ? UI.keysEl(it.show || it.keys) : null);
        li.addEventListener('mousemove', function () { if (sel !== n) { sel = n; mark(); } });
        li.addEventListener('click', function () { run(n); });
        list.appendChild(li);
      });
      mark();
    }
    function mark() {
      var os = list.querySelectorAll('.pal-i');
      for (var i = 0; i < os.length; i++) os[i].setAttribute('aria-selected', i === sel ? 'true' : 'false');
      var cur = os[sel];
      if (cur) { input.setAttribute('aria-activedescendant', cur.id); if (cur.scrollIntoView) cur.scrollIntoView({ block: 'nearest' }); }
    }
    function run(n) {
      var s = shown[n];
      if (!s || !enabled(s.it)) return;
      api.close();
      setTimeout(function () { if (s.it.run) s.it.run({ shiftKey: false }); else if (s.it.hold) { s.it.hold.down(); setTimeout(s.it.hold.up, 800); } }, 0);
    }
    function onKey(ev) {
      var k = ev.key;
      if (k === 'Escape') { ev.preventDefault(); ev.stopPropagation(); api.close(); }
      else if (k === 'ArrowDown' || (k === 'Tab' && !ev.shiftKey) || (ev.ctrlKey && k === 'n')) { ev.preventDefault(); if (shown.length) { sel = (sel + 1) % shown.length; mark(); } }
      else if (k === 'ArrowUp' || (k === 'Tab' && ev.shiftKey) || (ev.ctrlKey && k === 'p')) { ev.preventDefault(); if (shown.length) { sel = (sel - 1 + shown.length) % shown.length; mark(); } }
      else if (k === 'PageDown') { ev.preventDefault(); sel = Math.min(shown.length - 1, sel + 8); mark(); }
      else if (k === 'PageUp') { ev.preventDefault(); sel = Math.max(0, sel - 8); mark(); }
      else if (k === 'Home' && ev.ctrlKey) { ev.preventDefault(); sel = 0; mark(); }
      else if (k === 'End' && ev.ctrlKey) { ev.preventDefault(); sel = Math.max(0, shown.length - 1); mark(); }
      else if (k === 'Enter') { ev.preventDefault(); run(sel); }
    }
    var api = {
      isOpen: function () { return !!root && !root.hidden; },
      open: function () {
        if (!registry) return;
        popovers.closeAll();
        UI.help.hide();
        items = registry.items();
        prevFocus = document.activeElement;
        if (!root) {
          input = h('input', { class: 'pal-in', type: 'text', role: 'combobox', 'aria-expanded': 'true', 'aria-controls': 'pal-list', 'aria-autocomplete': 'list', 'aria-label': 'Command palette', placeholder: 'Type a command, a setting or a name…', spellcheck: 'false', autocomplete: 'off' });
          list = h('ul', { class: 'pal-list', id: 'pal-list', role: 'listbox', 'aria-label': 'Actions' });
          root = h('div', { class: 'pal', role: 'dialog', 'aria-modal': 'true', 'aria-label': 'Command palette' },
            h('div', { class: 'pal-box' }, input, list,
              h('div', { class: 'pal-foot' },
                h('span', null, h('kbd', { text: '↑' }), ' ', h('kbd', { text: '↓' }), ' move'),
                h('span', null, h('kbd', { text: '↵' }), ' run'),
                h('span', null, h('kbd', { text: 'Esc' }), ' close'))));
          input.addEventListener('input', function () { sel = 0; filter(); });
          input.addEventListener('keydown', onKey);
          root.addEventListener('mousedown', function (ev) { if (ev.target === root) api.close(); });
        }
        host().appendChild(root);
        root.hidden = false;
        input.value = ''; sel = 0;
        filter();
        input.focus();
      },
      close: function () {
        if (!root || root.hidden) return;
        root.hidden = true;
        var f = prevFocus; prevFocus = null;
        if (f && f.focus && document.contains(f)) f.focus();
      }
    };
    return api;
  })();

  // ---- popovers (Display, Tools, ...) ----
  var popovers = (function () {
    var open = [];
    function closeAll(except) {
      var any = false;
      open.slice().forEach(function (p) { if (p !== except) { p.close(); any = true; } });
      return any;
    }
    document.addEventListener('pointerdown', function (ev) {
      open.slice().forEach(function (p) { if (!p.wrap.contains(ev.target)) p.close(); });
    }, true);
    window.addEventListener('resize', function () { open.slice().forEach(function (p) { p.place(); }); });
    document.addEventListener('scroll', function () { open.slice().forEach(function (p) { p.place(); }); }, true);
    return { open: open, closeAll: closeAll };
  })();

  // wrap: .pop-wrap holding `button` and `panel` (.pop). Returns {open, close, toggle, isOpen}.
  UI.popover = function (wrap, button, panel, onOpen) {
    panel.hidden = true;
    button.setAttribute('aria-haspopup', 'true');
    button.setAttribute('aria-expanded', 'false');
    var p = {
      wrap: wrap,
      isOpen: function () { return !panel.hidden; },
      place: function () {
        if (panel.hidden) return;
        var anchor = button.getBoundingClientRect();
        panel.style.position = 'fixed';
        panel.style.bottom = 'auto';
        panel.style.right = 'auto';
        var r = panel.getBoundingClientRect();
        panel.style.left = Math.max(8, Math.min(anchor.right - r.width, innerWidth - r.width - 8)) + 'px';
        panel.style.top = Math.max(8, Math.min(anchor.bottom + 6, innerHeight - r.height - 8)) + 'px';
      },
      open: function (focus) {
        popovers.closeAll(p);
        if (onOpen) onOpen();
        panel.hidden = false;
        button.setAttribute('aria-expanded', 'true');
        if (popovers.open.indexOf(p) < 0) popovers.open.push(p);
        p.place();
        if (focus) {
          var f = panel.querySelector('button:not(:disabled), input, select');
          if (f) f.focus();
        }
      },
      close: function (refocus) {
        if (panel.hidden) return;
        panel.hidden = true;
        button.setAttribute('aria-expanded', 'false');
        var i = popovers.open.indexOf(p);
        if (i >= 0) popovers.open.splice(i, 1);
        if (refocus) button.focus();
      },
      toggle: function (focus) { if (p.isOpen()) p.close(); else p.open(focus); }
    };
    button.addEventListener('click', function (ev) { p.toggle(ev.detail === 0); });
    panel.addEventListener('keydown', function (ev) {
      if (ev.key === 'Escape') { ev.preventDefault(); ev.stopPropagation(); p.close(true); }
    });
    wrap.addEventListener('focusout', function (ev) {
      if (ev.relatedTarget && !wrap.contains(ev.relatedTarget)) p.close();
    });
    return p;
  };

  // ---- toasts ----
  UI.toast = function (msg) {
    var box = document.querySelector('.toasts');
    var hostEl = host();
    if (!box) box = h('div', { class: 'toasts', role: 'status', 'aria-live': 'polite' });
    if (box.parentNode !== hostEl) hostEl.appendChild(box);
    var t = h('div', { class: 'toast', text: msg });
    box.appendChild(t);
    setTimeout(function () { if (t.parentNode) t.parentNode.removeChild(t); }, 2400);
  };

  // ---- clipboard and downloads ----
  UI.copy = function (text) {
    function fallback() {
      var ta = h('textarea', { readonly: '', style: 'position:fixed;opacity:0' });
      ta.value = text;
      document.body.appendChild(ta); ta.select();
      var ok = false;
      try { ok = document.execCommand('copy'); } catch (e) { ok = false; }
      ta.remove();
      return ok;
    }
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text).then(function () { return true; }, function () { return fallback(); });
    }
    return Promise.resolve(fallback());
  };
  UI.download = function (name, href, revoke) {
    var a = h('a', { href: href, download: name });
    document.body.appendChild(a); a.click(); a.remove();
    if (revoke) setTimeout(function () { URL.revokeObjectURL(href); }, 4000);
  };
  UI.downloadJson = function (name, obj) {
    var blob = new Blob([JSON.stringify(obj, null, 2) + '\n'], { type: 'application/json' });
    UI.download(name, URL.createObjectURL(blob), true);
  };

  // ---- help dialog, generated from the registry ----
  UI.help = (function () {
    var root = null, previousFocus = null;
    var api = {
      isOpen: function () { return !!root && !root.hidden; },
      hide: function () { if (!api.isOpen()) return; root.hidden = true; if (previousFocus && previousFocus.isConnected) previousFocus.focus(); previousFocus = null; },
      // notes: [[keys, text]] extra lines (mouse gestures); tip: a closing sentence.
      toggle: function (reg, notes, tip, force) {
        var show = force === undefined ? !api.isOpen() : force;
        if (!show) { api.hide(); return; }
        palette.close();
        previousFocus = document.activeElement;
        if (root && root.parentNode) root.parentNode.removeChild(root);
        var groups = [], by = {};
        reg.all().forEach(function (d) {
          if (!(d.show || d.keys || []).length && !d.help) return;
          var g = d.group || 'General';
          if (!by[g]) { by[g] = []; groups.push(g); }
          by[g].push(d);
        });
        var cols = h('div', { class: 'help-cols' });
        groups.forEach(function (g) {
          var dl = h('dl');
          by[g].forEach(function (d) {
            dl.appendChild(h('dt', null, UI.keysEl(d.show || d.keys)));
            dl.appendChild(h('dd', { text: d.help || d.title }));
          });
          cols.appendChild(h('section', null, h('h3', { text: g }), dl));
        });
        if (notes && notes.length) {
          var dl2 = h('dl');
          notes.forEach(function (n) { dl2.appendChild(h('dt', null, h('span', { class: 'keys' }, h('kbd', { text: n[0] })))); dl2.appendChild(h('dd', { text: n[1] })); });
          cols.appendChild(h('section', null, h('h3', { text: 'Mouse and touch' }), dl2));
        }
        var close = h('button', { type: 'button', class: 'btn small', text: 'Close', onclick: function () { api.hide(); } });
        root = h('div', { class: 'help', role: 'dialog', 'aria-modal': 'true', 'aria-label': 'Keyboard shortcuts' },
          h('div', { class: 'help-card' },
            h('h2', { text: 'Keyboard shortcuts' }),
            h('p', { class: 'hint' }, 'Press ', h('kbd', { text: 'Ctrl' }), ' ', h('kbd', { text: 'K' }), ' (', h('kbd', { text: '⌘' }), ' ', h('kbd', { text: 'K' }), ' on a Mac) to search every action.'),
            cols, tip ? h('p', { class: 'hint', text: tip }) : null, close));
        root.addEventListener('click', function (ev) { if (ev.target === root) api.hide(); });
        root.addEventListener('keydown', function (ev) { if (ev.key === 'Tab') { ev.preventDefault(); close.focus(); } else if (ev.key === 'Escape') { ev.preventDefault(); ev.stopPropagation(); api.hide(); } });
        host().appendChild(root);
        close.focus();
      }
    };
    return api;
  })();

  // ---- full screen: the Fullscreen API on `root`, with a fixed-position CSS fallback ----
  var fsInstances = [];
  document.addEventListener('fullscreenchange', function () {
    fsInstances = fsInstances.filter(function (f) { return f.root.isConnected; });
    fsInstances.forEach(function (f) { f.sync(); });
    // overlays follow the full-screen element so they stay visible
    ['.toasts', '.pal', '.help'].forEach(function (s) {
      var n = document.querySelector(s);
      if (n && n.parentNode !== host()) host().appendChild(n);
    });
  });
  UI.fullscreen = function (root, onChange) {
    var f = {
      root: root,
      active: function () { return document.fullscreenElement === root || root.classList.contains('fs-css'); },
      sync: function () { root.classList.toggle('fs-on', f.active()); if (onChange) onChange(f.active()); },
      set: function (on) {
        if (!on) {
          root.classList.remove('fs-css');
          if (document.fullscreenElement && document.exitFullscreen) document.exitFullscreen();
          f.sync();
          return;
        }
        var fallback = function () { root.classList.add('fs-css'); f.sync(); };
        if (root.requestFullscreen) {
          var r = root.requestFullscreen();
          if (r && r.catch) r.catch(fallback);
        } else fallback();
      },
      toggle: function () { f.set(!f.active()); }
    };
    fsInstances.push(f);
    return f;
  };

  // ---- pointers: tracks pointers for the pinch gesture ----
  UI.pointers = function () {
    var p = {};
    var api = {
      add: function (ev) { p[ev.pointerId] = { x: ev.clientX, y: ev.clientY }; },
      move: function (ev) { if (p[ev.pointerId]) { p[ev.pointerId].x = ev.clientX; p[ev.pointerId].y = ev.clientY; } },
      remove: function (ev) { delete p[ev.pointerId]; },
      count: function () { return Object.keys(p).length; },
      two: function () { var k = Object.keys(p); return [p[k[0]], p[k[1]]]; },
      dist: function () { var t = api.two(); return Math.hypot(t[0].x - t[1].x, t[0].y - t[1].y) || 1; },
      // midpoint relative to `rc` (a client rect)
      mid: function (rc) { var t = api.two(); return [(t[0].x + t[1].x) / 2 - rc.left, (t[0].y + t[1].y) / 2 - rc.top]; }
    };
    return api;
  };

  // ---- zoom transform math, shared by the viewer stage and the report compare stage ----
  UI.xf = {
    // Keeps content of size cw x ch covering a vw x vh viewport.
    clamp: function (tx, ty, cw, ch, vw, vh) {
      return [Math.max(Math.min(0, vw - cw), Math.min(0, tx)), Math.max(Math.min(0, vh - ch), Math.min(0, ty))];
    },
    // The translation that keeps (cx, cy) fixed when the scale changes by `r`.
    about: function (tx, ty, r, cx, cy) { return [cx - (cx - tx) * r, cy - (cy - ty) * r]; },
    // Scale (viewport px per content px) that frames a box w x h (at least `min`) with `margin`.
    fitBox: function (w, h2, vw, vh, min, margin) { return margin * Math.min(vw / Math.max(w, min), vh / Math.max(h2, min)); },
    // The translation that centres content point (x, y) at scale s in a vw x vh viewport.
    centerOn: function (x, y, s, vw, vh) { return [vw / 2 - x * s, vh / 2 - y * s]; },
    // Zoom factor of one wheel event.
    wheel: function (ev, coef) { return Math.exp(-(ev.deltaMode === 1 ? ev.deltaY * 16 : ev.deltaY) * coef); }
  };

  // ---- display filter: exposure, contrast and channel as one CSS filter ----
  UI.filterCss = function (ev, contrast, channel) {
    var f = [];
    if (ev) f.push('brightness(' + Math.pow(2, ev).toFixed(4) + ')');
    if (contrast !== undefined && contrast !== 1) f.push('contrast(' + contrast + ')');
    if (channel && channel !== 'rgb') f.push('url(#ch-' + (channel === 'luma' ? 'l' : channel) + ')');
    return f.length ? f.join(' ') : 'none';
  };

  // ---- magma colormap: the FLIP heatmap value of a pixel colour (inverse of the map) ----
  var MAGMA = [[0, 0, 4], [28, 16, 68], [79, 18, 123], [129, 37, 129], [181, 54, 122], [229, 80, 100], [251, 135, 97], [254, 194, 135], [252, 253, 191]];
  UI.magmaValue = function (r, g, b) {
    var best = 1e9, val = 0;
    for (var i = 0; i < MAGMA.length - 1; i++) {
      var a = MAGMA[i], c = MAGMA[i + 1], dx = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
      var len = dx[0] * dx[0] + dx[1] * dx[1] + dx[2] * dx[2];
      var t = Math.max(0, Math.min(1, ((r - a[0]) * dx[0] + (g - a[1]) * dx[1] + (b - a[2]) * dx[2]) / len));
      var d = Math.pow(a[0] + dx[0] * t - r, 2) + Math.pow(a[1] + dx[1] * t - g, 2) + Math.pow(a[2] + dx[2] * t - b, 2);
      if (d < best) { best = d; val = (i + t) / (MAGMA.length - 1); }
    }
    return val;
  };

  // ---- small formatting helpers ----
  UI.pct = function (v) { return (v * 100).toFixed(Math.abs(v) < 0.1 ? 1 : 0) + '%'; };
  UI.rgbText = function (d) { return d[0] + ' ' + d[1] + ' ' + d[2]; };
  // diagnostics class -> chip tone and label
  var CLASS_TONE = { identical: 's-pass', noise: 's-muted', global_tone: 's-info', local_structure: 's-fail', mixed: 's-warn', misaligned: 's-warn', broken_frame: 's-error' };
  var CLASS_LABEL = { identical: 'identical', noise: 'noise', global_tone: 'tone shift', local_structure: 'local change', mixed: 'mixed', misaligned: 'misaligned', broken_frame: 'broken frame' };
  UI.classTone = function (c) { return CLASS_TONE[c] || 's-muted'; };
  UI.classLabel = function (c) { return CLASS_LABEL[c] || String(c).replace(/_/g, ' '); };

  // The diagnostics block of one compared pair: class chip, the generated sentence, tone and
  // shift chips and the timing deltas. `d` is `Diagnostics` from the report or the view model.
  UI.diagnosticsEl = function (d, opts) {
    if (!d) return null;
    opts = opts || {};
    var chips = h('div', { class: 'dchips' });
    var t = d.tone, s = d.shift, sg = d.signed;
    if (t) chips.appendChild(h('span', { class: 'badge info', title: 'Global tone fit: ' + (t.model || '') + ', explains ' + UI.pct(t.tone_explained_fraction) + ' of the error', text: 'tone ' + t.summary }));
    if (t && t.white_balance && t.white_balance !== 'neutral') chips.appendChild(h('span', { class: 'badge info', text: t.white_balance }));
    if (s && s.detected) chips.appendChild(h('span', { class: 'badge warn', title: 'Sub-pixel shift (phase correlation, confidence ' + s.confidence.toFixed(2) + '); explains ' + (s.shift_explained_fraction == null ? '?' : UI.pct(s.shift_explained_fraction)) + ' of the error', text: 'shift ' + (s.dx >= 0 ? '+' : '') + s.dx.toFixed(2) + ', ' + (s.dy >= 0 ? '+' : '') + s.dy.toFixed(2) + ' px' }));
    if (sg) chips.appendChild(h('span', { class: 'badge', title: 'Share of pixels that got brighter / darker', text: UI.pct(sg.frac_brighter) + ' brighter · ' + UI.pct(sg.frac_darker) + ' darker' }));
    var nf = d.nonfinite;
    if (nf) chips.appendChild(h('span', { class: 'badge error', title: nf.cluster_count + ' cluster(s) of non-finite samples', text: nf.nan + ' NaN · ' + nf.inf + ' Inf' + (nf.negative ? ' · ' + nf.negative + ' neg' : '') }));
    var perf = h('div', { class: 'perf', 'aria-label': 'Timing change' }, (d.perf || []).map(function (p) {
      var up = p.delta > 0, down = p.delta < 0;
      var pc = p.delta_pct == null ? '' : ' ' + (p.delta_pct < 0 ? '−' : '+') + (Math.abs(p.delta_pct) < 1 ? Math.abs(p.delta_pct).toFixed(1) : Math.abs(p.delta_pct).toFixed(0)) + '%';
      return h('span', { class: 'pd' + (up ? ' up' : down ? ' down' : ''), title: p.key + ': ' + p.baseline + ' → ' + p.capture },
        h('span', { class: 'k', text: p.key }), p.baseline === p.capture ? 'same ' + fmtNum(p.baseline) : fmtNum(p.baseline) + ' → ' + fmtNum(p.capture), pc ? h('b', { text: pc }) : null);
    }));
    var head = h('div', { class: 'dh' }, h('span', { class: 'chip ' + UI.classTone(d.class), text: UI.classLabel(d.class) }),
      opts.label ? h('span', { class: 'lbl', text: opts.label }) : null, perf.children.length ? perf : null);
    return h('div', { class: 'diag ' + UI.classTone(d.class) }, head, d.description ? h('p', { class: 'dcap', text: d.description }) : null, chips.children.length ? chips : null);
  };
  function fmtNum(v) { var a = Math.abs(v); return a >= 100 ? v.toFixed(0) : a >= 10 ? v.toFixed(1) : v.toFixed(2); }
  UI.fmtNum = fmtNum;
  UI.fmtScale = function (v) { return Math.abs(v) >= 10000 ? v.toExponential(2) : v.toFixed(3); };
})();
