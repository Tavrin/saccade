// Showcase interactivity: swipe comparisons, layer and entry pickers, hotspot
// zoom, copy buttons and the theme toggle. The page renders without it.
(() => {
  'use strict';
  const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const store = {
    get(k) { try { return localStorage.getItem(k); } catch (_) { return null; } },
    set(k, v) { try { localStorage.setItem(k, v); } catch (_) { /* storage blocked */ } },
  };

  // ---- theme ----
  const root = document.documentElement;
  const saved = store.get('flipdiff-showcase-theme');
  if (saved === 'light' || saved === 'dark') root.dataset.theme = saved;
  const themeBtn = document.getElementById('theme');
  const isDark = () => root.dataset.theme ? root.dataset.theme === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches;
  const syncTheme = () => { if (themeBtn) themeBtn.setAttribute('aria-label', isDark() ? 'Switch to light theme' : 'Switch to dark theme'); };
  if (themeBtn) {
    syncTheme();
    themeBtn.addEventListener('click', () => {
      root.dataset.theme = isDark() ? 'light' : 'dark';
      store.set('flipdiff-showcase-theme', root.dataset.theme);
      syncTheme();
    });
  }

  // ---- copy buttons ----
  document.querySelectorAll('button.copy').forEach((b) => {
    b.addEventListener('click', async () => {
      const text = b.closest('.term').querySelector('pre').dataset.copy || b.closest('.term').querySelector('pre').innerText;
      try { await navigator.clipboard.writeText(text); b.textContent = 'Copied'; } catch (_) { b.textContent = 'Select and copy'; }
      setTimeout(() => { b.textContent = 'Copy'; }, 1400);
    });
  });

  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const fmt = (v) => (v === null || v === undefined ? '-' : Number(v).toFixed(5));
  const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
  const widgets = {};

  function Widget(el) {
    const data = JSON.parse(document.getElementById(el.dataset.src).textContent);
    const stage = el.querySelector('.stage');
    const div = stage.querySelector('.div');
    const base = stage.querySelector('.ly.base img');
    const top = stage.querySelector('.ly.top img');
    const boxes = stage.querySelector('.boxes');
    const tagR = el.querySelector('.tag.r');
    const tagL = el.querySelector('.tag.l');
    const layerSeg = el.querySelector('[data-role=layer]');
    const boxBtn = el.querySelector('[data-role=boxes]');
    const zoomBtn = el.querySelector('[data-role=zoom]');
    const pickers = [...document.querySelectorAll(`[data-for="${el.id}"] [data-pick]`)];
    const nameEl = el.querySelector('.cmp-bar .name');
    const valEl = el.querySelector('.cmp-bar .val');
    const chipEl = el.querySelector('.cmp-bar .chip');
    const footEl = el.querySelector('.cmp-foot');
    let idx = data.default || 0;
    let layer = 'cap';
    let zoomed = el.dataset.zoom !== undefined;
    let x = 50;
    let touched = false;

    const setX = (v) => { x = clamp(v, 0, 100); stage.style.setProperty('--x', x + '%'); div.setAttribute('aria-valuenow', Math.round(x)); };
    const entry = () => data.entries[idx];

    function renderBoxes() {
      const e = entry();
      boxes.innerHTML = (e.hs || []).map((h, i) => {
        const low = h[1] < 0.08 ? ' low' : '';
        return `<div class="hb${low}" style="left:${h[0] * 100}%;top:${h[1] * 100}%;width:${h[2] * 100}%;height:${h[3] * 100}%"><span>${i + 1} · ${Math.round(h[4] * 100)}%</span></div>`;
      }).join('');
    }
    function applyZoom() {
      const e = entry();
      const h = e.hs && e.hs[0];
      if (!zoomed || !h) {
        stage.style.removeProperty('--tf'); stage.classList.remove('zoomed');
        boxes.querySelectorAll('.hb').forEach((b) => { b.style.borderWidth = ''; b.firstChild.style.transform = ''; });
        return;
      }
      const s = clamp(0.42 / Math.max(h[2], h[3]), 1.5, 8);
      const cx = h[0] + h[2] / 2, cy = h[1] + h[3] / 2;
      const tx = clamp(0.5 - s * cx, 1 - s, 0), ty = clamp(0.5 - s * cy, 1 - s, 0);
      stage.style.setProperty('--tf', `translate(${tx * 100}%, ${ty * 100}%) scale(${s})`);
      stage.classList.add('zoomed');
      boxes.querySelectorAll('.hb').forEach((b) => { b.style.borderWidth = (2 / s) + 'px'; b.firstChild.style.transform = `scale(${1 / s})`; b.firstChild.style.transformOrigin = '0 100%'; });
    }
    function render() {
      const e = entry();
      stage.parentElement.style.setProperty('--ar', e.ar);
      base.src = e.b; base.alt = `${data.left} ${e.name}`;
      const heat = layer === 'heat' && e.h;
      top.src = heat ? e.h : e.c; top.alt = `${heat ? (e.hlabel || 'FLIP heatmap') : data.right} ${e.name}`;
      tagR.textContent = heat ? (e.hlabel || 'FLIP heatmap') : (e.right || data.right);
      tagL.textContent = e.left || data.left;
      renderBoxes(); applyZoom();
      if (layerSeg) layerSeg.querySelectorAll('button').forEach((b) => {
        b.setAttribute('aria-pressed', String(b.dataset.layer === layer));
        if (b.dataset.layer === 'heat') b.disabled = !e.h;
      });
      if (zoomBtn) { zoomBtn.disabled = !(e.hs && e.hs.length); zoomBtn.setAttribute('aria-pressed', String(zoomed && !zoomBtn.disabled)); }
      if (boxBtn) boxBtn.disabled = !(e.hs && e.hs.length);
      if (nameEl) nameEl.textContent = e.name;
      if (valEl) valEl.textContent = e.metric ? `${e.metric} ${fmt(e.value)} (limit ${e.thr})` : '';
      if (chipEl) { chipEl.className = `chip s-${e.status}`; chipEl.textContent = e.status === 'fail' ? 'FAIL' : e.status; }
      if (footEl && e.foot !== undefined) footEl.innerHTML = e.foot;
      pickers.forEach((p) => {
        const on = Number(p.dataset.pick) === idx;
        if (p.tagName === 'TR') p.setAttribute('aria-selected', String(on));
        else if (p.tagName.toLowerCase() === 'rect') p.classList.toggle('sel', on);
        else p.setAttribute('aria-pressed', String(on));
      });
    }

    // drag on the stage
    let dragging = false;
    const fromEvent = (ev) => { const r = stage.getBoundingClientRect(); setX(((ev.clientX - r.left) / r.width) * 100); };
    stage.addEventListener('pointerdown', (ev) => {
      if (ev.button !== 0) return;
      touched = true; dragging = true; stage.setPointerCapture(ev.pointerId); fromEvent(ev); div.focus({ preventScroll: true });
    });
    stage.addEventListener('pointermove', (ev) => { if (dragging) fromEvent(ev); });
    const stop = () => { dragging = false; };
    stage.addEventListener('pointerup', stop);
    stage.addEventListener('pointercancel', stop);
    div.addEventListener('keydown', (ev) => {
      const step = ev.shiftKey ? 1 : 5;
      if (ev.key === 'ArrowLeft') { setX(x - step); ev.preventDefault(); }
      else if (ev.key === 'ArrowRight') { setX(x + step); ev.preventDefault(); }
      else if (ev.key === 'Home') { setX(0); ev.preventDefault(); }
      else if (ev.key === 'End') { setX(100); ev.preventDefault(); }
      else return;
      touched = true;
    });
    if (layerSeg) layerSeg.addEventListener('click', (ev) => {
      const b = ev.target.closest('button'); if (!b || b.disabled) return;
      layer = b.dataset.layer; touched = true; render();
    });
    if (boxBtn) boxBtn.addEventListener('click', () => {
      const off = stage.classList.toggle('noboxes'); boxBtn.setAttribute('aria-pressed', String(!off)); touched = true;
    });
    if (zoomBtn) zoomBtn.addEventListener('click', () => { zoomed = !zoomed; touched = true; render(); });
    pickers.forEach((p) => {
      const go = () => { idx = Number(p.dataset.pick); touched = true; render(); };
      p.addEventListener('click', go);
      if (p.tagName === 'TR' || p.tagName.toLowerCase() === 'rect') p.addEventListener('keydown', (ev) => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); go(); } });
    });

    // one short sweep so the divider reads as draggable
    function intro() {
      if (reduced || touched) return;
      const keys = [[0, 50], [700, 82], [1500, 24], [2300, 58]];
      const t0 = performance.now();
      const ease = (t) => t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
      const step = (now) => {
        if (touched) return;
        const t = now - t0;
        let i = 0; while (i < keys.length - 2 && t > keys[i + 1][0]) i++;
        const [ta, va] = keys[i], [tb, vb] = keys[i + 1];
        const k = clamp((t - ta) / (tb - ta), 0, 1);
        setX(va + (vb - va) * ease(k));
        if (t < keys[keys.length - 1][0]) requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    }
    if (el.dataset.intro !== undefined && 'IntersectionObserver' in window) {
      const io = new IntersectionObserver((es) => { if (es.some((e) => e.isIntersecting)) { io.disconnect(); setTimeout(intro, 500); } }, { threshold: 0.6 });
      io.observe(stage);
    }

    setX(Number(el.dataset.split || 50));
    render();
    return {
      setSplit: setX,
      setLayer(l) { layer = l; render(); },
      setZoom(z) { zoomed = z; render(); },
      pick(i) { idx = i; render(); },
      stopIntro() { touched = true; },
    };
  }

  document.querySelectorAll('.cmp[data-src]').forEach((el) => { widgets[el.id] = Widget(el); });
  window.showcase = widgets;
})();
