// Color customizer injected into soundcloud.com by the desktop shell.
// SoundCloud styles itself through CSS custom properties on <body> (theme-dark / theme-light),
// so recoloring means overriding those tokens. Settings live in the page's localStorage.
(() => {
  if (window.top !== window || !/(^|\.)soundcloud\.com$/.test(location.hostname)) return;
  if (window.__scTheme) return;

  const KEY = 'scdesk.theme.v1';
  const SC_ORANGE_HUE = 20;

  const DEFAULTS = {
    enabled: false,
    accent: '#ff5500',
    bg: '#121212',
    panel: '#303030',
    text: '#ffffff',
    muted: '#999999',
    link: '#699fff',
    tintWaveform: true,
    customCss: '',
  };

  const PRESETS = {
    'SoundCloud': { accent: '#ff5500', bg: '#121212', panel: '#303030', text: '#ffffff', muted: '#999999', link: '#699fff' },
    'Midnight':   { accent: '#8b5cf6', bg: '#0d0b14', panel: '#1d1929', text: '#ece9f5', muted: '#8f88a6', link: '#a78bfa' },
    'Dracula':    { accent: '#ff79c6', bg: '#282a36', panel: '#44475a', text: '#f8f8f2', muted: '#a4a8c4', link: '#8be9fd' },
    'Nord':       { accent: '#88c0d0', bg: '#2e3440', panel: '#3b4252', text: '#eceff4', muted: '#a3abbc', link: '#81a1c1' },
    'Mocha':      { accent: '#cba6f7', bg: '#1e1e2e', panel: '#313244', text: '#cdd6f4', muted: '#a6adc8', link: '#89b4fa' },
    'Rosé Pine':  { accent: '#ebbcba', bg: '#191724', panel: '#26233a', text: '#e0def4', muted: '#908caa', link: '#9ccfd8' },
    'Forest':     { accent: '#1db954', bg: '#0f1410', panel: '#1f2a21', text: '#e8f0e9', muted: '#8fa393', link: '#5fd38a' },
    'Ember':      { accent: '#f43f5e', bg: '#140c0c', panel: '#2a1818', text: '#fbeaea', muted: '#b49393', link: '#fb923c' },
  };

  // Which SoundCloud tokens each user-facing color drives.
  const TOKENS = {
    accent: ['special-color', 'font-special-color', 'button-special-background-color', 'checkbox-checked-background-color',
      'checkbox-checked-border-color', 'toggle-on-body-color', 'toggle-on-body-hover-color', 'button-secondary-selected-font-color',
      'button-secondary-selected-active-font-color', 'button-tertiary-selected-font-color', 'button-tertiary-selected-active-font-color'],
    bg: ['surface-color', 'background-surface-color', 'background-dark-color', 'black-color', 'font-dark-color',
      'button-dark-background-color', 'button-light-font-color', 'button-primary-font-color', 'button-primary-active-font-color',
      'button-primary-disabled-font-color', 'button-primary-loading-icon-color', 'button-tertiary-background-color',
      'button-tertiary-active-background-color', 'button-tertiary-selected-background-color',
      'button-tertiary-selected-active-background-color', 'button-tertiary-disabled-background-color',
      'button-tertiary-loading-background-color', 'tag-selected-font-color', 'toggle-off-handle-color', 'toggle-on-handle-color'],
    panel: ['highlight-color', 'background-highlight-color', 'button-secondary-background-color',
      'button-secondary-active-background-color', 'button-secondary-selected-background-color',
      'button-secondary-selected-active-background-color', 'button-secondary-loading-background-color',
      'button-secondary-disabled-background-color', 'button-primary-selected-background-color',
      'button-primary-selected-active-background-color', 'input-default-background-color', 'input-focused-background-color',
      'input-invalid-background-color', 'input-placeholder-background-color', 'tag-default-background-color'],
    text: ['primary-color', 'font-primary-color', 'font-light-color', 'link-primary-color', 'background-light-color',
      'button-primary-background-color', 'button-primary-active-background-color', 'button-primary-loading-background-color',
      'button-primary-disabled-background-color', 'button-primary-selected-font-color', 'button-primary-selected-active-font-color',
      'button-secondary-font-color', 'button-secondary-active-font-color', 'button-secondary-disabled-font-color',
      'button-secondary-loading-icon-color', 'button-tertiary-font-color', 'button-tertiary-active-font-color',
      'button-tertiary-disabled-font-color', 'button-tertiary-loading-icon-color', 'button-light-background-color',
      'button-dark-font-color', 'button-special-font-color', 'tag-default-font-color', 'tag-selected-background-color',
      'input-default-font-color', 'input-focused-font-color', 'input-disabled-font-color'],
    muted: ['secondary-color', 'font-secondary-color', 'link-secondary-color', 'input-placeholder-font-color',
      'input-focused-border-color', 'toggle-off-body-color', 'toggle-off-body-hover-color'],
    link: ['link-color', 'font-link-color', 'link-standard-color', 'link-standard-hover-color'],
  };
  const RGB_TOKENS = {
    accent: ['special-rgb'], bg: ['surface-rgb', 'black-rgb'], panel: ['highlight-rgb'],
    text: ['primary-rgb'], muted: ['secondary-rgb'], link: ['link-rgb'],
  };
  // Translucent variants SoundCloud uses for hover/active states.
  const ALPHA_TOKENS = [
    ['text', 0.4, ['overlay-color', 'overlay-default-color', 'link-primary-hover-color', 'button-primary-selected-hover-font-color',
      'button-secondary-hover-font-color', 'button-tertiary-hover-font-color', 'tag-default-font-hover-color']],
    ['text', 0.6, ['link-primary-active-color', 'link-primary-disabled-color', 'button-secondary-loading-font-color',
      'button-tertiary-loading-font-color']],
    ['text', 0.15, ['imageBorder-color', 'input-disabled-background-color']],
    ['muted', 0.4, ['link-secondary-hover-color']],
    ['muted', 0.6, ['link-secondary-active-color', 'link-secondary-disabled-color']],
    ['accent', 0.4, ['button-secondary-selected-hover-font-color', 'button-tertiary-selected-hover-font-color']],
    ['link', 0.6, ['link-standard-active-color', 'link-standard-disabled-color']],
    ['bg', 0.4, ['button-primary-hover-font-color', 'tag-selected-font-hover-color', 'toggle-on-handle-hover-color',
      'toggle-off-handle-hover-color']],
    ['bg', 0.6, ['button-primary-loading-font-color']],
  ];

  const hexToRgb = (hex) => {
    let h = hex.replace('#', '');
    if (h.length === 3) h = [...h].map((c) => c + c).join('');
    const n = parseInt(h, 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  };
  const hueOf = (hex) => {
    const [r, g, b] = hexToRgb(hex).map((v) => v / 255);
    const max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min;
    if (!d) return 0;
    const h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
    return (h * 60 + 360) % 360;
  };
  const isHex = (v) => typeof v === 'string' && /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.test(v);

  // Userstyles from Stylus / userstyles.world wrap rules in @-moz-document blocks; unwrap them.
  const unwrapMozDocument = (css) => {
    const re = /@-moz-document[^{]*\{/g;
    let out = '', i = 0, m;
    while ((m = re.exec(css))) {
      out += css.slice(i, m.index);
      let depth = 1, j = re.lastIndex;
      while (j < css.length && depth) {
        if (css[j] === '{') depth++;
        else if (css[j] === '}') depth--;
        j++;
      }
      out += css.slice(re.lastIndex, depth ? j : j - 1);
      i = re.lastIndex = j;
    }
    return out + css.slice(i);
  };

  const load = () => {
    try { return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(KEY) || '{}') }; } catch { return { ...DEFAULTS }; }
  };
  const save = () => { try { localStorage.setItem(KEY, JSON.stringify(state)); } catch {} };
  let state = load();

  const buildCss = (t) => {
    let css = '';
    if (t.enabled) {
      const decls = [];
      for (const [k, names] of Object.entries(TOKENS)) for (const n of names) decls.push(`--${n}:${t[k]}!important`);
      for (const [k, names] of Object.entries(RGB_TOKENS)) for (const n of names) decls.push(`--${n}:${hexToRgb(t[k]).join(',')}!important`);
      for (const [k, a, names] of ALPHA_TOKENS) for (const n of names) decls.push(`--${n}:rgba(${hexToRgb(t[k]).join(',')},${a})!important`);
      css += `html body{${decls.join(';')}}\n`;
      css += `html body,.l-app,.header,.l-container{background-color:var(--surface-color)}\n`;
      css += `.playControls__bg,.playControls__inner{background-color:var(--highlight-color)!important}\n`;
      css += `::selection{background:rgba(${hexToRgb(t.accent).join(',')},.35)}\n`;
      if (t.tintWaveform) {
        // Waveforms are canvas-drawn in SoundCloud orange; rotate their hue toward the accent (approximate).
        const delta = Math.round(hueOf(t.accent) - SC_ORANGE_HUE);
        if (delta) css += `.waveform canvas,[class*="waveform"] canvas{filter:hue-rotate(${delta}deg)}\n`;
      }
    }
    if (t.customCss) css += unwrapMozDocument(t.customCss);
    return css;
  };

  const styleEl = document.createElement('style');
  styleEl.id = 'scdesk-theme';
  const mountStyle = () => {
    if (!document.documentElement) return false;
    if (!styleEl.isConnected) document.documentElement.appendChild(styleEl);
    return true;
  };
  const apply = () => { styleEl.textContent = buildCss(state); mountStyle(); };
  apply();
  if (!mountStyle()) {
    const mo = new MutationObserver(() => { if (mountStyle()) mo.disconnect(); });
    mo.observe(document, { childList: true });
  }

  // ---------- Panel (shadow DOM so SoundCloud's CSS can't touch it) ----------
  const FIELDS = [['accent', 'Accent'], ['bg', 'Background'], ['panel', 'Panels & player bar'],
    ['text', 'Text'], ['muted', 'Secondary text'], ['link', 'Links']];
  let host, root;

  const panelHtml = `
<style>
  :host{all:initial}
  .wrap{position:fixed;top:56px;right:16px;z-index:2147483647;width:320px;max-height:calc(100vh - 140px);overflow:auto;
    background:#17171b;color:#eee;border:1px solid #2c2c33;border-radius:12px;box-shadow:0 12px 40px rgba(0,0,0,.5);
    font:13px/1.4 system-ui,-apple-system,"Segoe UI",sans-serif;padding:14px 14px 12px}
  .row{display:flex;align-items:center;justify-content:space-between;gap:8px;margin:6px 0}
  h2{font-size:15px;margin:0;font-weight:600}
  h3{font-size:11px;text-transform:uppercase;letter-spacing:.06em;color:#8a8a94;margin:14px 0 6px;font-weight:600}
  button{font:inherit;color:#eee;background:#2a2a31;border:1px solid #393942;border-radius:7px;padding:5px 10px;cursor:pointer}
  button:hover{background:#34343c}
  .x{background:none;border:none;font-size:18px;line-height:1;padding:2px 6px;color:#999}
  .presets{display:grid;grid-template-columns:repeat(4,1fr);gap:6px}
  .preset{padding:6px 4px;font-size:11px;display:flex;flex-direction:column;align-items:center;gap:4px}
  .sw{display:flex;height:10px;width:100%;border-radius:3px;overflow:hidden}
  .sw i{flex:1}
  label{flex:1}
  .pick{display:flex;align-items:center;gap:6px}
  input[type=color]{width:28px;height:24px;border:1px solid #393942;border-radius:5px;background:none;padding:0;cursor:pointer}
  input[type=text]{width:72px;font:12px ui-monospace,Consolas,monospace;color:#eee;background:#0f0f12;border:1px solid #393942;border-radius:5px;padding:4px 6px}
  textarea{width:100%;box-sizing:border-box;min-height:90px;font:12px ui-monospace,Consolas,monospace;color:#eee;background:#0f0f12;
    border:1px solid #393942;border-radius:7px;padding:8px;resize:vertical}
  .btns{display:flex;flex-wrap:wrap;gap:6px;margin-top:8px}
  .hint{color:#8a8a94;font-size:11px;margin-top:6px}
  .switch{position:relative;width:34px;height:20px;flex:none}
  .switch input{opacity:0;width:0;height:0}
  .switch span{position:absolute;inset:0;background:#3a3a42;border-radius:20px;transition:.15s;cursor:pointer}
  .switch span:before{content:"";position:absolute;width:14px;height:14px;left:3px;top:3px;background:#fff;border-radius:50%;transition:.15s}
  .switch input:checked+span{background:var(--acc,#f50)}
  .switch input:checked+span:before{transform:translateX(14px)}
  .toast{color:#9fe0b0;font-size:11px;min-height:14px;margin-top:6px}
</style>
<div class="wrap" part="panel">
  <div class="row"><h2>Theme</h2><button class="x" data-act="close" title="Close (Esc)">×</button></div>
  <div class="row"><label>Use custom colors</label><span class="switch"><input type="checkbox" id="enabled"><span></span></span></div>
  <h3>Presets</h3>
  <div class="presets">${Object.entries(PRESETS).map(([name, p]) => `
    <button class="preset" data-preset="${name}"><span class="sw">${['bg', 'panel', 'accent', 'text'].map((k) => `<i style="background:${p[k]}"></i>`).join('')}</span>${name}</button>`).join('')}
  </div>
  <h3>Colors</h3>
  ${FIELDS.map(([k, label]) => `
  <div class="row"><label for="c-${k}">${label}</label><span class="pick"><input type="color" id="c-${k}" data-key="${k}"><input type="text" id="t-${k}" data-key="${k}" maxlength="7" spellcheck="false"></span></div>`).join('')}
  <div class="row"><label for="tintWaveform">Tint waveforms to accent</label><span class="switch"><input type="checkbox" id="tintWaveform"><span></span></span></div>
  <h3>Custom CSS / userstyle</h3>
  <textarea id="customCss" spellcheck="false" placeholder="Paste CSS or a userstyle (e.g. from userstyles.world). Applied even when custom colors are off."></textarea>
  <div class="btns"><button data-act="loadcss">Load .css file…</button><input type="file" id="file" accept=".css,text/css" hidden></div>
  <h3>Share</h3>
  <div class="btns">
    <button data-act="export">Copy theme code</button>
    <button data-act="import">Paste theme code</button>
    <button data-act="reset">Reset</button>
  </div>
  <div class="toast" id="toast"></div>
  <div class="hint">Ctrl+, toggles this panel. Also in the tray menu.</div>
</div>`;

  const $ = (sel) => root.querySelector(sel);
  const toast = (msg) => { const t = $('#toast'); t.textContent = msg; clearTimeout(toast.t); toast.t = setTimeout(() => (t.textContent = ''), 2500); };

  const sync = () => {
    if (!root) return;
    $('#enabled').checked = state.enabled;
    $('#tintWaveform').checked = state.tintWaveform;
    for (const [k] of FIELDS) { $(`#c-${k}`).value = state[k]; $(`#t-${k}`).value = state[k]; }
    if ($('#customCss') !== root.activeElement) $('#customCss').value = state.customCss;
    $('.wrap').style.setProperty('--acc', state.accent);
  };
  const update = (patch) => { state = { ...state, ...patch }; save(); apply(); sync(); };

  const build = () => {
    host = document.createElement('scdesk-theme-panel');
    root = host.attachShadow({ mode: 'open' });
    root.innerHTML = panelHtml;
    root.addEventListener('input', (e) => {
      const el = e.target, k = el.dataset.key;
      if (k && el.type === 'color') update({ [k]: el.value, enabled: true });
      else if (k && el.type === 'text') { const v = el.value.trim(); if (isHex(v)) update({ [k]: v.toLowerCase(), enabled: true }); }
      else if (el.id === 'customCss') update({ customCss: el.value });
    });
    root.addEventListener('change', (e) => {
      const el = e.target;
      if (el.id === 'enabled' || el.id === 'tintWaveform') update({ [el.id]: el.checked });
      else if (el.id === 'file' && el.files[0]) el.files[0].text().then((css) => { update({ customCss: css }); toast('Loaded ' + el.files[0].name); el.value = ''; });
    });
    root.addEventListener('click', async (e) => {
      const btn = e.target.closest('button');
      if (!btn) return;
      if (btn.dataset.preset) return update({ ...PRESETS[btn.dataset.preset], enabled: true });
      switch (btn.dataset.act) {
        case 'close': return close();
        case 'loadcss': return $('#file').click();
        case 'reset': return update({ ...DEFAULTS });
        case 'export': {
          const code = btoa(JSON.stringify(state));
          try { await navigator.clipboard.writeText(code); toast('Theme code copied'); } catch { prompt('Copy this theme code:', code); }
          return;
        }
        case 'import': {
          let code = '';
          try { code = await navigator.clipboard.readText(); } catch {}
          if (!code || !code.trim()) code = prompt('Paste a theme code:') || '';
          try {
            const t = JSON.parse(atob(code.trim()));
            const clean = {};
            for (const k of Object.keys(DEFAULTS)) if (k in t && typeof t[k] === typeof DEFAULTS[k] && (!(k in TOKENS) || isHex(t[k]))) clean[k] = t[k];
            update(clean); toast('Theme imported');
          } catch { toast('That is not a valid theme code'); }
        }
      }
    });
    root.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Escape') close(); });
    // Keep SoundCloud's global shortcuts (space = play/pause, etc.) from firing while typing in the panel.
    root.addEventListener('keyup', (e) => e.stopPropagation());
    root.addEventListener('keypress', (e) => e.stopPropagation());
  };

  const open = () => { if (!host) build(); if (!host.isConnected) document.documentElement.appendChild(host); sync(); };
  const close = () => host && host.remove();
  const toggle = () => (host && host.isConnected ? close() : open());

  window.addEventListener('keydown', (e) => {
    if (e.ctrlKey && !e.shiftKey && !e.altKey && e.key === ',') { e.preventDefault(); e.stopPropagation(); toggle(); }
  }, true);

  // Palette button in SoundCloud's header. The header is re-rendered on navigation, so re-add it when missing.
  const addHeaderButton = () => {
    const right = document.querySelector('.header__right');
    if (!right || right.querySelector('.scdesk-theme-btn')) return;
    const b = document.createElement('button');
    b.className = 'scdesk-theme-btn';
    b.title = 'Theme (Ctrl+,)';
    b.style.cssText = 'float:left;height:46px;width:40px;display:flex;align-items:center;justify-content:center;background:none;border:0;cursor:pointer;color:var(--font-secondary-color,#999)';
    b.innerHTML = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22a10 10 0 1 1 10-10c0 2.8-2.2 4-4 4h-1.8a1.7 1.7 0 0 0-1.2 2.9 1.7 1.7 0 0 1-1.2 2.9z"/><circle cx="7.5" cy="10.5" r="1.2" fill="currentColor"/><circle cx="11" cy="6.8" r="1.2" fill="currentColor"/><circle cx="15.8" cy="8.2" r="1.2" fill="currentColor"/></svg>';
    b.addEventListener('mouseenter', () => (b.style.color = 'var(--font-primary-color,#fff)'));
    b.addEventListener('mouseleave', () => (b.style.color = 'var(--font-secondary-color,#999)'));
    b.addEventListener('click', toggle);
    right.prepend(b);
  };
  let queued = false;
  const watchHeader = () => {
    new MutationObserver(() => {
      if (queued) return;
      queued = true;
      requestAnimationFrame(() => { queued = false; addHeaderButton(); });
    }).observe(document.body, { childList: true, subtree: true });
    addHeaderButton();
  };
  if (document.body) watchHeader();
  else document.addEventListener('DOMContentLoaded', watchHeader, { once: true });

  window.__scTheme = { open, close, toggle };
})();
