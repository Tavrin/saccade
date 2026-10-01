(function () {
  'use strict';
  var PAGE = window.SACCADE_VOTE || {};
  var RUN = PAGE.run, TOKEN = PAGE.token;
  var LS_NAME = 'saccade.voter';
  var $ = function (id) { return document.getElementById(id); };
  var voter = '';
  var items = [];
  var pos = 0;
  var saving = false;

  function lsGet(k) { try { return localStorage.getItem(k) || ''; } catch (e) { return ''; } }
  function lsSet(k, v) { try { localStorage.setItem(k, v); } catch (e) { /* storage may be blocked */ } }
  function msg(text, bad) { var m = $('msg'); m.textContent = text || ''; m.className = bad ? 'hint err' : 'hint'; }
  function api(url, opts) {
    return fetch(url, opts).then(function (r) {
      return r.json().then(function (j) { if (!r.ok) throw new Error(j && j.error ? j.error : 'HTTP ' + r.status); return j; });
    });
  }
  function show(which) {
    ['who', 'stage', 'done'].forEach(function (id) { $(id).hidden = id !== which; });
  }
  function answered() { return items.filter(function (i) { return i.answer; }).length; }
  function progress() {
    var n = answered();
    $('prog').textContent = n + ' of ' + items.length + ' voted';
    $('barfill').style.width = (items.length ? (100 * n / items.length) : 0) + '%';
  }
  function firstOpen(from) {
    for (var k = 0; k < items.length; k++) {
      var j = (from + k) % items.length;
      if (!items[j].answer) return j;
    }
    return -1;
  }
  function render() {
    progress();
    if (!items.length) { show('done'); $('donemsg').textContent = 'This run has nothing to vote on.'; return; }
    if (pos < 0) {
      show('done');
      $('donemsg').textContent = 'You voted on all ' + items.length + ' items. Thank you, ' + voter + '.';
      return;
    }
    var it = items[pos];
    show('stage');
    $('who-label').textContent = 'Voting as ' + voter;
    var kind = $('kind'); kind.textContent = it.kind; kind.className = 'kind ' + it.kind;
    $('limits').textContent = it.kind_limits;
    $('question').textContent = it.text;
    $('context').textContent = it.context || '';
    var img = $('strip');
    if (it.strip) { img.src = it.strip; $('stripbox').hidden = false; } else { img.removeAttribute('src'); $('stripbox').hidden = true; }
    var box = $('choices'); box.textContent = '';
    var keys = [];
    it.choices.forEach(function (c) {
      var b = document.createElement('button');
      b.type = 'button'; b.className = 'btn' + (it.answer === c.answer ? ' picked' : '');
      var k = document.createElement('kbd'); k.textContent = c.key;
      b.appendChild(k); b.appendChild(document.createTextNode(c.label));
      b.addEventListener('click', function () { vote(c.answer); });
      box.appendChild(b);
      keys.push(c.key + ' ' + c.label.toLowerCase());
    });
    $('keys').textContent = 'Keys: ' + keys.join(', ') + (it.pairwise ? '; left and right arrows pick image 1 and 2' : '') + '.';
    $('back').disabled = pos === 0;
  }
  function vote(answer) {
    if (saving || pos < 0 || !items[pos]) return;
    saving = true;
    Array.prototype.forEach.call($('choices').querySelectorAll('button'), function (b) { b.disabled = true; });
    var it = items[pos];
    msg('');
    api('/api/vote/' + RUN + '/vote', {
      method: 'POST',
      headers: { 'X-Saccade-Token': TOKEN, 'Content-Type': 'application/json' },
      body: JSON.stringify({ voter: voter, item: it.id, answer: answer })
    }).then(function () {
      saving = false;
      it.answer = answer;
      var next = firstOpen(pos + 1);
      pos = next;
      render();
    }).catch(function (e) { saving = false; render(); msg('Could not save the vote: ' + e.message, true); });
  }
  function choiceFor(key) {
    if (pos < 0 || !items[pos]) return null;
    var c = items[pos].choices.filter(function (c) { return c.key === key; })[0];
    return c ? c.answer : null;
  }
  document.addEventListener('keydown', function (ev) {
    if (ev.ctrlKey || ev.metaKey || ev.altKey) return;
    var t = ev.target;
    if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')) return;
    if ($('stage').hidden) return;
    var key = ev.key;
    if (key === 'ArrowLeft' && items[pos] && items[pos].pairwise) key = '1';
    else if (key === 'ArrowRight' && items[pos] && items[pos].pairwise) key = '2';
    else if (key === 'u' || key === 'U') key = 'u';
    var a = choiceFor(key);
    if (a) { ev.preventDefault(); vote(a); }
  });
  function load() {
    return api('/api/vote/' + RUN + '/items?voter=' + encodeURIComponent(voter)).then(function (d) {
      items = d.items || [];
      pos = firstOpen(0);
      render();
    }).catch(function (e) { show('who'); msg(e.message, true); });
  }
  $('whoform').addEventListener('submit', function (ev) {
    ev.preventDefault();
    var n = $('name').value.trim();
    if (!n) { msg('Enter a name.', true); return; }
    voter = n; lsSet(LS_NAME, n); msg(''); load();
  });
  $('rename').addEventListener('click', function () { $('name').value = voter; show('who'); });
  $('back').addEventListener('click', function () { if (!saving && pos > 0) { pos -= 1; render(); } });
  $('skip').addEventListener('click', function () {
    if (saving) return;
    var next = firstOpen(pos + 1);
    if (next >= 0) { pos = next; render(); }
  });
  $('runname').textContent = 'run ' + RUN;
  voter = lsGet(LS_NAME);
  if (voter) { $('name').value = voter; load(); } else { show('who'); }
})();
