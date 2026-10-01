(() => {
  'use strict';
  const items = document.getElementById('items'), error = document.getElementById('error');
  let signature = '', busy = false;
  const element = (tag, text, cls) => { const n = document.createElement(tag); n.textContent = text; if (cls) n.className = cls; return n; };
  async function refresh() {
    if (busy) return;
    try {
      const response = await fetch('/api/inbox');
      if (!response.ok) throw new Error('Cannot load inbox (' + response.status + ')');
      const data = await response.json(), next = JSON.stringify(data);
      document.getElementById('count').textContent = data.filter(i => i.status === 'open').length + ' open';
      // Preserve the user's unsaved note while periodic refreshes run.
      if (next === signature || document.activeElement?.tagName === 'TEXTAREA') return;
      signature = next; items.replaceChildren(); error.textContent = '';
      if (!data.length) items.append(element('p', 'No questions yet.', 'hint'));
      for (const item of data) {
        const card = element('article', '', item.status); card.id = item.id;
        card.append(element('span', item.status, 'status'), element('h2', item.question));
        if (item.from) card.append(element('p', 'From ' + item.from, 'meta'));
        if (item.context) card.append(element('p', item.context, 'context'));
        // Defense in depth: stored data is never HTML, and links remain same-origin.
        if (item.link && item.link.startsWith('/') && !item.link.startsWith('//') && !item.link.includes('\\')) {
          const link = element('a', 'Open evidence', 'link'); link.href = item.link; card.append(link);
        }
        if (item.status === 'answered') {
          card.append(element('p', 'Answer: ' + item.answer));
          if (item.note) card.append(element('p', item.note, 'note'));
        } else {
          const note = element('textarea', ''); note.placeholder = 'Optional note'; note.setAttribute('aria-label', 'Note for ' + item.question); card.append(note);
          for (const answer of item.allowed_answers) {
            const button = element('button', answer); button.type = 'button'; card.append(button);
            button.addEventListener('click', async () => {
              busy = true; card.querySelectorAll('button').forEach(b => b.disabled = true);
              try {
                const r = await fetch('/api/inbox/' + encodeURIComponent(item.id) + '/answer', {method:'POST', headers:{'Content-Type':'application/json','X-Saccade-Token':window.INBOX_TOKEN}, body:JSON.stringify({answer,note:note.value || null})});
                if (!r.ok) throw new Error('Answer not saved: ' + await r.text());
                signature = ''; note.blur();
              } catch (e) { error.textContent = e.message; }
              finally { busy = false; card.querySelectorAll('button').forEach(b => b.disabled = false); refresh(); }
            });
          }
        }
        items.append(card);
      }
      if (location.hash) document.getElementById(location.hash.slice(1))?.scrollIntoView();
    } catch (e) { error.textContent = e.message; }
  }
  refresh(); setInterval(refresh, 3000);
})();
