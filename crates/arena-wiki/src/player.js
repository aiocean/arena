// The wiki's skill player: one cast of a skill at a foe standing still, as the rules fought it (arena-wiki's
// hits::record, /wiki/play/<skill>.json), drawn tick by tick from the game's own sheets: the fighters' frames, the
// effects as the game places them, and every shape the rules test (hurtboxes, blows, shots, fields, units, hands).
// Arena pixels, y up from the floor. Nothing here decides anything: it only shows what was recorded.
(() => {
  const MARKS = {
    Body: null, // by owner: caster blue, foe pink
    Untouchable: '160,160,170',
    Strike: '255,70,70',
    Shot: '255,70,70',
    Field: '255,160,40',
    Unit: '90,230,120',
    Wrist: '255,220,60',
    Hilt: '255,220,60',
    Launch: '255,220,60',
  };
  const OWNER = ['90,180,255', '255,140,200'];
  const HURT = new Set(['Body', 'Untouchable']);
  const HANDS = new Set(['Wrist', 'Hilt', 'Launch']);

  function el(tag, attrs = {}, kids = []) {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) {
      if (k === 'text') e.textContent = v;
      else e.setAttribute(k, v);
    }
    for (const k of kids) e.append(k);
    return e;
  }

  function image(url) {
    return new Promise((ok) => {
      const i = new Image();
      i.onload = () => ok(i);
      i.onerror = () => ok(null);
      i.src = url;
    });
  }

  async function mount(root) {
    const assets = root.dataset.assets.replace(/\/$/, '');
    const rec = await fetch(root.dataset.src).then((r) => r.json());
    const frames = rec.frames;
    if (!frames.length) return;

    // every page the recording draws from
    const urls = new Set();
    const pageOf = (sheet, page) => `${assets}/${sheet}${page ? '-' + page : ''}.webp`;
    for (const f of frames) {
      f.fighters.forEach((g, i) => g.cell && urls.add(pageOf(rec.sheets[i], g.cell[0])));
      for (const e of f.effects) urls.add(`${assets}/${e.file}`);
    }
    const pics = new Map();
    await Promise.all([...urls].map(async (u) => pics.set(u, await image(u))));

    // one frame of reference for the whole cast: every fighter, shape and effect, the floor
    let x0 = Infinity, x1 = -Infinity, y0 = 0, y1 = -Infinity;
    const grow = (a, b, c, d) => { x0 = Math.min(x0, a); y0 = Math.min(y0, b); x1 = Math.max(x1, c); y1 = Math.max(y1, d); };
    for (const f of frames) {
      for (const g of f.fighters) {
        if (g.cell) {
          const [, , , w, h, dx, dy] = g.cell;
          const l = g.face < 0 ? g.x - (dx + w) : g.x + dx;
          grow(l, g.air - (dy + h), l + w, g.air - dy);
        } else grow(g.x - 30, g.air, g.x + 30, g.air + 240);
      }
      for (const s of f.shapes) {
        if (s.shape.rect) grow(...s.shape.rect);
        else { const [x, y, r] = s.shape.circle; grow(x - r, y - r, x + r, y + r); }
      }
      for (const e of f.effects) {
        const r = Math.hypot(e.cell[2], e.cell[3]) * e.k / 2;
        grow(e.cx - r, e.cy - r, e.cx + r, e.cy + r);
      }
    }
    const pad = 24;
    // a little under the floor at most (a seal's turning corners reach further than anything drawn there)
    x0 -= pad; x1 += pad; y0 = Math.max(y0, -40) - 12; y1 += pad;
    const W = x1 - x0, H = y1 - y0;

    // the controls
    const canvas = el('canvas', { class: 'fx-canvas' });
    const play = el('button', { type: 'button', text: '⏸', title: 'Play or pause (space)' });
    const back = el('button', { type: 'button', text: '⟨', title: 'One tick back (←)' });
    const fwd = el('button', { type: 'button', text: '⟩', title: 'One tick on (→)' });
    const hitBtn = el('button', { type: 'button', text: 'first hit', title: 'The first tick it can hit' });
    const speed = el('select', { title: 'Speed' }, [0.1, 0.25, 0.5, 1].map((s) => el('option', { value: s, text: s + '×' })));
    speed.value = '0.25';
    const scrub = el('input', { type: 'range', min: 0, max: frames.length - 1, value: 0, step: 1, class: 'fx-scrub' });
    const marks = el('canvas', { class: 'fx-marks', height: 6 });
    const info = el('div', { class: 'fx-info' });
    const layers = {};
    const toggles = el('div', { class: 'fx-layers' });
    for (const [k, label] of [['sprites', 'fighters'], ['effects', 'effects'], ['hurt', 'hurtboxes'], ['hit', 'hitboxes'], ['hands', 'hands']]) {
      const c = el('input', { type: 'checkbox' });
      c.checked = true;
      layers[k] = c;
      toggles.append(el('label', {}, [c, ' ' + label]));
    }
    const legend = el('div', { class: 'fx-legend' });
    legend.innerHTML = '<span style="color:rgb(90,180,255)">■</span> caster hurtbox · <span style="color:rgb(255,140,200)">■</span> foe hurtbox · <span style="color:rgb(255,70,70)">■</span> blow / shot · <span style="color:rgb(255,160,40)">■</span> field · <span style="color:rgb(90,230,120)">■</span> unit · <span style="color:rgb(255,220,60)">●</span> hand';
    // the stretches it can hit in (the facts above), each a button to its first tick
    const windows = JSON.parse(root.dataset.windows || '[]');
    const jumps = el('div', { class: 'fx-jumps' });
    if (windows.length) jumps.append('Can hit: ');
    for (const [a, b] of windows) {
      const i = frames.findIndex((f) => f.t >= a - 1e-6);
      const label = Math.abs(b - a) < 1e-9 ? `${a.toFixed(2)} s` : `${a.toFixed(2)}–${b.toFixed(2)} s`;
      const btn = el('button', { type: 'button', text: label, title: 'Jump to its first tick' });
      if (i < 0) btn.disabled = true;
      btn.onclick = () => { setPlaying(false); show(i); };
      jumps.append(btn);
    }
    root.append(
      canvas,
      el('div', { class: 'fx-bar' }, [play, back, fwd, hitBtn, speed, toggles]),
      el('div', { class: 'fx-track' }, [scrub, marks]),
      jumps,
      info,
      legend,
    );

    // where each tick can hit, under the scrubber
    const hitting = frames.map((f) => f.shapes.some((s) => !HURT.has(s.mark) && !HANDS.has(s.mark)) || f.sweeps.length > 0);
    const firstHit = hitting.indexOf(true);
    if (firstHit < 0) hitBtn.disabled = true;
    function drawMarks() {
      const w = (marks.width = marks.clientWidth || 600);
      const m = marks.getContext('2d');
      m.clearRect(0, 0, w, 6);
      m.fillStyle = 'rgb(255,70,70)';
      hitting.forEach((h, i) => h && m.fillRect(Math.floor((i / frames.length) * w), 0, Math.max(1, Math.ceil(w / frames.length)), 6));
    }

    const ctx = canvas.getContext('2d');
    function size() {
      const css = Math.min(root.clientWidth || 800, 960);
      const k = css / W;
      const dpr = window.devicePixelRatio || 1;
      canvas.style.width = css + 'px';
      canvas.style.height = H * k + 'px';
      canvas.width = Math.round(css * dpr);
      canvas.height = Math.round(H * k * dpr);
      return k * dpr;
    }
    let scale = size();
    // arena (y up) to canvas
    const X = (x) => (x - x0) * scale;
    const Y = (y) => (y1 - y) * scale;

    function rect(b, rgb, fill) {
      const [a, c, d, e] = [X(b[0]), Y(b[3]), X(b[2]), Y(b[1])];
      if (fill) { ctx.fillStyle = `rgba(${rgb},${fill})`; ctx.fillRect(a, c, d - a, e - c); }
      ctx.strokeStyle = `rgb(${rgb})`;
      ctx.strokeRect(a + 0.5, c + 0.5, d - a, e - c);
    }

    function draw(i) {
      const f = frames[i];
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.fillStyle = '#14121c';
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.imageSmoothingEnabled = false;
      ctx.lineWidth = Math.max(1, scale * 0.75);
      // the floor
      ctx.strokeStyle = 'rgb(110,100,130)';
      ctx.beginPath(); ctx.moveTo(0, Y(0) + 0.5); ctx.lineTo(canvas.width, Y(0) + 0.5); ctx.stroke();
      if (layers.sprites.checked) {
        f.fighters.forEach((g, n) => {
          if (!g.cell) return;
          const [page, sx, sy, w, h, dx, dy] = g.cell;
          const img = pics.get(pageOf(rec.sheets[n], page));
          if (!img) return;
          ctx.save();
          ctx.translate(X(g.x), Y(g.air));
          if (g.face < 0) ctx.scale(-1, 1);
          ctx.drawImage(img, sx, sy, w, h, dx * scale, dy * scale, w * scale, h * scale);
          ctx.restore();
        });
      }
      if (layers.effects.checked) {
        for (const e of f.effects) {
          const img = pics.get(`${assets}/${e.file}`);
          if (!img) continue;
          const [sx, sy, w, h] = e.cell;
          ctx.save();
          ctx.translate(X(e.cx), Y(e.cy));
          ctx.rotate(e.angle);
          if (e.flip) ctx.scale(-1, 1);
          const k = e.k * scale;
          ctx.drawImage(img, sx, sy, w, h, (-w * k) / 2, (-h * k) / 2, w * k, h * k);
          ctx.restore();
        }
      }
      for (const s of f.shapes) {
        const hurt = HURT.has(s.mark), hand = HANDS.has(s.mark);
        if ((hurt && !layers.hurt.checked) || (hand && !layers.hands.checked) || (!hurt && !hand && !layers.hit.checked)) continue;
        const rgb = s.mark === 'Body' ? OWNER[s.owner ?? 0] : MARKS[s.mark] || '255,70,70';
        if (s.shape.rect) {
          if (s.mark === 'Untouchable') ctx.setLineDash([4, 3]);
          rect(s.shape.rect, rgb, hurt ? 0 : 0.18);
          ctx.setLineDash([]);
        } else {
          const [x, y, r] = s.shape.circle;
          ctx.strokeStyle = `rgb(${rgb})`;
          ctx.fillStyle = `rgba(${rgb},${hand ? 1 : 0.18})`;
          ctx.beginPath();
          ctx.arc(X(x), Y(y), Math.max(r * scale, hand ? 2.5 * scale : 3), 0, Math.PI * 2);
          ctx.fill();
          if (!hand) ctx.stroke();
        }
      }
      if (layers.hit.checked) {
        for (const [ax, ay, bx, by, r] of f.sweeps) {
          ctx.strokeStyle = 'rgba(255,70,70,0.9)';
          ctx.lineWidth = Math.max(2, r * 2 * scale);
          ctx.globalAlpha = 0.35;
          ctx.lineCap = 'round';
          ctx.beginPath(); ctx.moveTo(X(ax), Y(ay)); ctx.lineTo(X(bx), Y(by)); ctx.stroke();
          ctx.globalAlpha = 1;
          ctx.lineWidth = Math.max(1, scale * 0.75);
        }
      }
      // the words: when, and what each fighter shows
      const who = ['Caster', 'Foe'];
      const lines = [`t = ${f.t.toFixed(3)} s · tick ${i + 1} / ${frames.length}${hitting[i] ? ' · <b class="fx-can-hit">can hit</b>' : ''}`];
      f.fighters.forEach((g, n) => {
        let part = '';
        if (g.window) part = g.i < g.window[0] ? 'wind-up' : g.i <= g.window[1] ? 'active' : 'follow-through';
        const act = g.act ? ` · act ${g.act.spell}: ${g.act.phase} ${g.act.t.toFixed(3)} / ${(g.act.phase === 'windup' ? g.act.windup : g.act.recover).toFixed(3)} s` : '';
        lines.push(`${who[n]}: <code>${g.clip || '-'}</code> frame ${g.i + 1} / ${g.n}${part ? ` (${part}${g.window ? `, active ${g.window[0] + 1}–${g.window[1] + 1}` : ''})` : ''}${act}`);
      });
      info.innerHTML = lines.join('<br>');
    }

    let at = 0, playing = true, acc = 0, last = null, hold = 0;
    function show(i) {
      at = Math.max(0, Math.min(frames.length - 1, i));
      scrub.value = at;
      draw(at);
    }
    function setPlaying(p) { playing = p; play.textContent = p ? '⏸' : '▶'; }
    play.onclick = () => setPlaying(!playing);
    back.onclick = () => { setPlaying(false); show(at - 1); };
    fwd.onclick = () => { setPlaying(false); show(at + 1); };
    hitBtn.onclick = () => { setPlaying(false); show(firstHit); };
    scrub.oninput = () => { setPlaying(false); show(+scrub.value); };
    for (const c of Object.values(layers)) c.onchange = () => draw(at);
    root.tabIndex = 0;
    root.addEventListener('keydown', (e) => {
      if (e.key === ' ') { e.preventDefault(); setPlaying(!playing); }
      else if (e.key === 'ArrowLeft') { e.preventDefault(); back.onclick(); }
      else if (e.key === 'ArrowRight') { e.preventDefault(); fwd.onclick(); }
    });
    window.addEventListener('resize', () => { scale = size(); drawMarks(); draw(at); });

    // played only while on screen
    let visible = false;
    new IntersectionObserver((es) => { visible = es[0].isIntersecting; }).observe(root);
    function tick(now) {
      if (last !== null && playing && visible) {
        const dt = (now - last) / 1000;
        if (at >= frames.length - 1) {
          hold += dt;
          if (hold > 0.8) { hold = 0; acc = 0; show(0); }
        } else {
          acc += dt * +speed.value;
          const step = Math.floor(acc / rec.tick);
          if (step > 0) { acc -= step * rec.tick; show(at + step); }
        }
      }
      last = now;
      requestAnimationFrame(tick);
    }
    drawMarks();
    show(0);
    requestAnimationFrame(tick);
  }

  for (const root of document.querySelectorAll('.fx-player')) {
    mount(root).catch((e) => { root.textContent = 'The player could not load: ' + e; });
  }
})();
