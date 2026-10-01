(function () {
  'use strict';
  // Served sessions only. Looks the viewer's controls up by id, so it needs
  // nothing from the viewer's own script besides those ids.
  var sv = window.FLIPDIFF_SERVE || {};
  function ready(fn) {
    if (document.readyState === 'complete') setTimeout(fn, 0);
    else window.addEventListener('load', function () { setTimeout(fn, 0); });
  }
  function click(n) { if (n) n.click(); }
  ready(function () {
    var browse = document.getElementById('browse');
    if (sv.overview && browse && browse.parentNode) {
      var a = document.createElement('a');
      a.className = 'back';
      a.id = 'overview-link';
      a.href = sv.overview;
      a.textContent = 'Run overview';
      browse.parentNode.insertBefore(a, browse.nextSibling);
    }
    var q;
    try { q = new URLSearchParams(location.search); } catch (e) { return; }
    var set = q.get('set'), A = q.get('a'), B = q.get('b');
    if (!set) return;
    var btns = document.querySelectorAll('#setlist button');
    for (var i = 0; i < btns.length; i++) {
      if (btns[i].title === set) { btns[i].click(); break; }
    }
    if (!A || !B) return;
    click(document.querySelector('#seg-layout button[data-v="swipe"]'));
    var labels = document.querySelectorAll('#chosen label');
    for (var j = 0; j < labels.length; j++) {
      var cb = labels[j].querySelector('input');
      var name = labels[j].textContent.trim();
      var want = name === A || name === B;
      if (cb && cb.checked !== want) cb.click();
    }
  });
})();
