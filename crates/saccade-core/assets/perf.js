(function () {
  'use strict';
  function node(tag, text, cls) { var e = document.createElement(tag); if (text != null) e.textContent = text; if (cls) e.className = cls; return e; }
  function num(v, digits) { return v == null ? '—' : v.toFixed(digits == null ? 4 : digits); }
  function signed(v) { return v == null ? '—' : (v > 0 ? '+' : '') + num(v); }
  function noise(t, k) {
    if (t.status !== 'paired') return 'not comparable';
    if (t.beyond_noise == null) return 'repeat noise unknown';
    if (t.noise_threshold == null && t.noise_floor == null) return 'within minimum delta (repeat noise unknown)';
    var limit = t.noise_threshold == null ? t.noise_floor * k : t.noise_threshold;
    return (t.beyond_noise ? 'beyond' : 'within') + ' threshold ' + num(limit) + ' ms';
  }
  var colors = ['var(--accent)', 'var(--pass)', 'var(--new)', 'var(--missing)', 'var(--error)', 'var(--darker)', 'var(--brighter)'];
  function bars(host, d) {
    var wrap = node('div', null, 'perf-bars'), legend = node('div', null, 'perf-legend');
    var roots = d.terms.filter(function (t) { return t.before_kind !== 'scope' || t.after_kind !== 'scope'; });
    var scale = Math.max(d.frame.before || 0, d.frame.after || 0,
      roots.reduce(function (s,t) { return s + (t.before_kind !== 'scope' ? t.before || 0 : 0); },0),
      roots.reduce(function (s,t) { return s + (t.after_kind !== 'scope' ? t.after || 0 : 0); },0), 0.000001);
    ['before','after'].forEach(function (side) {
      var line = node('div',null,'perf-bar-line'), stack = node('div',null,'perf-stack');
      line.appendChild(node('span',side + ' · ' + num(d.frame[side]) + ' ms'));
      roots.forEach(function (t,i) {
        if (t[side] == null || t[side + '_kind'] === 'scope') return;
        var seg = node('span',t[side] / scale > 0.09 ? t.label : '', 'perf-segment');
        seg.style.width = (t[side] / scale * 100) + '%'; seg.style.background = colors[i % colors.length];
        seg.title = t.id + ' (' + t[side + '_kind'] + '): ' + num(t[side]) + ' ms'; stack.appendChild(seg);
      });
      var remainder = d['unattributed_' + side];
      if (remainder > 0) { var seg = node('span','unattributed','perf-segment'); seg.style.width = remainder / scale * 100 + '%'; seg.style.background = 'var(--muted)'; seg.title = 'unattributed: ' + num(remainder) + ' ms'; stack.appendChild(seg); }
      stack.setAttribute('role','img'); stack.setAttribute('aria-label',side + ' additive frame composition; scopes excluded');
      line.appendChild(stack); wrap.appendChild(line);
    });
    roots.filter(function(t) { return Math.max(t.before || 0,t.after || 0) / scale > 0.01; }).forEach(function(t) {
      var item = node('span'), key = node('i',null,'perf-key'); key.style.background = colors[roots.indexOf(t) % colors.length]; item.appendChild(key); item.appendChild(node('span',t.label)); legend.appendChild(item);
    });
    wrap.appendChild(legend); host.appendChild(wrap);
  }
  function table(host, headers, rows, draw) {
    var wrap = node('div',null,'perf-scroll'), t = node('table',null,'perf-table'), head = node('thead'), tr = node('tr'), body = node('tbody');
    var order = 0, ascending = true;
    function paint() {
      body.replaceChildren();
      var sorted = rows.slice().sort(function(a,b) {
        var av = headers[order][1](a), bv = headers[order][1](b);
        if (av == null) return bv == null ? 0 : 1; if (bv == null) return -1;
        var c = typeof av === 'number' ? av-bv : String(av).localeCompare(String(bv)); return ascending ? c : -c;
      });
      draw(body,sorted);
    }
    headers.forEach(function(h,i) { var th = node('th'), button = node('button',h[0]); button.type = 'button'; button.addEventListener('click',function() { ascending = order === i ? !ascending : true; order = i; head.querySelectorAll('th').forEach(function(e,j) { e.setAttribute('aria-sort', j === i ? (ascending ? 'ascending' : 'descending') : 'none'); }); paint(); }); th.appendChild(button); tr.appendChild(th); });
    head.appendChild(tr); t.appendChild(head); t.appendChild(body); wrap.appendChild(t); host.appendChild(wrap); paint(); return t;
  }
  function counters(host, list) {
    if (!list || !list.length) return;
    var details = node('details'), summary = node('summary','Counters (' + list.length + ')'); details.appendChild(summary);
    list.forEach(function(c) { details.appendChild(node('p',c.name + ': ' + num(c.before) + ' → ' + num(c.after) + ' · ' + signed(c.delta) + ' · ' + (c.delta_pct == null ? 'relative move not comparable' : signed(c.delta_pct) + '%') + ' · ' + c.status)); }); host.appendChild(details);
  }
  function diff(host, d, verdict, label) {
    var panel = node('section',null,'perf-panel'); panel.appendChild(node('h3',label || 'Performance evidence'));
    if (verdict) panel.appendChild(node('p',verdict));
    if (!d) { panel.appendChild(node('p','Performance unavailable')); host.appendChild(panel); return; }
    d.warnings.forEach(function(w) { panel.appendChild(node('p',w,'perf-warning')); });
    panel.appendChild(node('p','Frame: ' + signed(d.frame.delta) + ' ms · ' + noise(d.frame,d.noise_k) + '. Timer quantum: ' + num(d.resolution_ms,6) + ' ms; ' + d.resolution_ticks + ' ticks. Minimum delta: ' + num(d.min_delta_ms) + ' ms and ' + num(d.min_delta_pct,2) + '% of baseline frame (' + num(d.minimum_delta_ms) + ' ms).'));
    var unpaired = d.terms.filter(function(t){return t.status !== 'paired';}).sort(function(a,b){return Math.max(b.before || 0,b.after || 0)-Math.max(a.before || 0,a.after || 0) || a.id.localeCompare(b.id);});
    if (unpaired.length) {
      panel.appendChild(node('h4','Terms differ (' + unpaired.length + ' not comparable)'));
      unpaired.forEach(function(t){var prominent = Math.max(t.before || 0,t.after || 0) > d.minimum_delta_ms; panel.appendChild(node('p',t.label + ' [' + t.id + '] · ' + t.status + ' · ' + num(t.before) + ' → ' + num(t.after) + ' ms' + (prominent ? ' · above minimum delta' : ''),prominent ? 'perf-warning' : ''));});
    }
    panel.appendChild(node('p','Unattributed: ' + num(d.unattributed_before) + ' → ' + num(d.unattributed_after) + ' ms. Scopes are nested and excluded from frame totals.'));
    bars(panel,d);
    table(panel,[['Term',function(t){return t.id;}],['Kind',function(t){return t.kind;}],['Before ms',function(t){return t.before;}],['After ms',function(t){return t.after;}],['Δ ms',function(t){return t.delta;}],['Δ %',function(t){return t.delta_pct;}],['Share before',function(t){return t.share_before;}],['Share after',function(t){return t.share_after;}],['Effective threshold',function(t){return t.noise_threshold;}]],d.terms,function(body,sorted) {
      var visited = new Set();
      function row(t,depth) {
        if (visited.has(t.id)) return; visited.add(t.id);
        var tr = node('tr',null,depth ? 'perf-scope' : ''), name = node('td',t.label + (t.id !== t.label ? ' [' + t.id + ']' : ''));
        name.style.paddingLeft = (12 + depth * 22) + 'px'; counters(name,t.counters); tr.appendChild(name);
        [t.kind,num(t.before),num(t.after),signed(t.delta),t.delta_pct == null ? '—' : signed(t.delta_pct) + '%',t.share_before == null ? '—' : num(t.share_before * 100,2) + '%',t.share_after == null ? '—' : num(t.share_after * 100,2) + '%',t.status + ' · ' + noise(t,d.noise_k)].forEach(function(v,i) { tr.appendChild(node('td',v,i === 7 && t.beyond_noise ? 'perf-beyond' : '')); }); body.appendChild(tr);
        sorted.filter(function(c) {return c.parent === t.id;}).forEach(function(c){row(c,depth+1);});
      }
      sorted.filter(function(t) { return !t.parent || !sorted.some(function(p){return p.id===t.parent;}); }).forEach(function(t){row(t,0);});
      sorted.forEach(function(t){row(t,0);});
    }); host.appendChild(panel);
  }
  function ablation(host, model) {
    host.replaceChildren(); var arms = model.arms || model.runs || [];
    var at = table(host,[['Arm',function(a){return a.label;}],['Image',function(a){return a.image_verdict;}],['Flag',function(a){return a.flag;}],['Frame Δ ms',function(a){return a.frame_delta;}],['Beyond-noise terms',function(a){return a.top_deltas.length;}],['Config differs',function(a){return a.config_differs.length;}],['Combined verdict',function(a){return a.combined_verdict;}]],arms,function(body,sorted) {
      sorted.forEach(function(a) { var tr=node('tr'), td=node('td');
        if(a.report_html){var link=node('a',a.label);link.href=a.report_html;td.appendChild(link);}else{td.textContent=a.label;}tr.appendChild(td);
        [a.image_verdict,a.flag,signed(a.frame_delta)].forEach(function(v){tr.appendChild(node('td',v,'perf-copy'));});
        var moves = node('td',null,'perf-copy');
        if (!a.top_deltas.length) moves.textContent = 'none established';
        a.top_deltas.forEach(function(t){var move=node('div',t.label+' '+signed(t.delta)+' ms (threshold '+num(t.noise_threshold)+')','perf-mover');move.title=t.id+' · '+noise(t,a.perf_diff ? a.perf_diff.noise_k : 3);moves.appendChild(move);});tr.appendChild(moves);
        [a.config_differs.join(', ') || 'none',a.combined_verdict].forEach(function(v){tr.appendChild(node('td',v,'perf-copy'));});body.appendChild(tr);
      });
    });
    at.classList.add("perf-ablation");
    arms.forEach(function(a){
      (a.validity_findings || []).forEach(function(f){host.appendChild(node('p',a.label+': '+f,'perf-warning'));});
      (a.next_actions || []).forEach(function(action){host.appendChild(node('p','Next action: '+action,'perf-warning'));});
      if(a.repeat_stability && a.validity_findings && a.validity_findings.length){
        Object.keys(a.repeat_stability.max_flip_by_image || {}).forEach(function(name){
          if((a.repeat_stability.unstable_images || []).indexOf(name) >= 0){
            var hashes = (a.repeat_stability.image_hashes || {})[name] || [];
            host.appendChild(node('p',name+': repeat max FLIP '+num(a.repeat_stability.max_flip_by_image[name])+'; SHA-256 '+hashes.join(', '),'perf-warning'));
          }
        });
      }
      diff(host,a.perf_diff,a.combined_verdict,a.label);
      (a.perf_errors || []).forEach(function(e){host.appendChild(node('p',e.path+': '+e.message,'perf-warning'));});
    });
  }
  window.saccadePerf = {diff:diff,ablation:ablation};
}());
