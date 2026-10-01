(function () {
  'use strict';
  var D = JSON.parse(document.getElementById('run-data').textContent);
  var UI = window.__saccadeUI, h = UI.h, key = 'saccade-serve-selection';
  var reg = UI.use(UI.actions()); UI.controls(reg);
  function $(id) { return document.getElementById(id); }
  function add(path) {
    var selection = [];
    try { selection = JSON.parse(localStorage.getItem(key) || '[]'); } catch (_) {}
    if (!Array.isArray(selection)) selection = [];
    var image = /\.(png|jpe?g|exr|hdr)$/i;
    if (selection.some(function (p) { return typeof p !== 'string' || image.test(p) !== image.test(path); })) {
      $('notice').textContent = 'The compare tray holds a different kind of selection. Clear it in Browse first.'; return false;
    }
    if (selection.indexOf(path) < 0) {
      if (selection.length >= 6) { $('notice').textContent = 'The compare tray is full (6 items).'; return false; }
      selection.push(path);
    }
    try { localStorage.setItem(key, JSON.stringify(selection)); } catch (_) { $('notice').textContent = 'The compare tray could not be saved.'; return false; }
    $('notice').textContent = 'Added to the compare tray (' + selection.length + '/6).'; return true;
  }
  $('runpath').textContent = D.path || 'Archive root';
  $('add').addEventListener('click', function () { add(D.path); });
  $('with').addEventListener('click', function () { if (add(D.path)) location.href = '/'; });
  if (D.meta_error) $('meta-note').textContent = D.meta_error;
  else if (!D.meta || !Object.keys(D.meta).length) $('meta-note').textContent = 'No metadata sidecar.';
  Object.keys(D.meta || {}).forEach(function (k) { $('metadata').appendChild(h('tr', {}, h('th', { scope: 'row', text: k }), h('td', { text: D.meta[k] }))); });
  $('count').textContent = '(' + D.images.images.length + ')'; $('truncated').hidden = !D.images.truncated;
  D.images.images.forEach(function (image) {
    var path = (D.path ? D.path + '/' : '') + image.name;
    var open = '/image?path=' + encodeURIComponent(path);
    var button = h('button', { class: 'btn small', type: 'button', text: '+ Compare image', onclick: function () { add(path); } });
    $('images').appendChild(h('article', { class: 'runimage' },
      h('a', { href: open, 'aria-label': 'Open ' + image.name }, h('img', { loading: 'lazy', alt: image.name, src: '/thumb?path=' + encodeURIComponent(path) })),
      h('a', { href: open, text: image.name }), button));
  });
  reg.add({ id: 'copy', title: 'Copy link', group: 'Tools', run: function () { UI.copy(location.href).then(function (ok) { UI.toast(ok ? 'Link copied' : 'Copy failed'); }); } });
  reg.add({ id: 'escape', title: 'Close panel', keys: ['Escape'], run: UI.escape });
  $('palette-btn').addEventListener('click', function () { UI.palette.open(); });
})();
