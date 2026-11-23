// Account switcher: added to the bottom of SoundCloud's profile dropdown when signed in, and behind
// an "Accounts" button next to "Sign in" when signed out (a newly added account starts signed out).
// The app pushes the account list in with __scdeskSetAccounts and polls __scdeskTakeAccountRequest
// once a second to act on clicks.
(() => {
  if (window.top !== window || !/(^|\.)soundcloud\.com$/.test(location.hostname)) return;

  let accounts = [];
  let active = '';
  let request = null;

  const style = document.createElement('style');
  style.textContent = `
    .scdesk-acct-sep { border-top: 1px solid var(--highlight-color, #303030); margin: 6px 0; }
    .scdesk-acct-link { display: flex !important; align-items: center; gap: 8px; }
    .scdesk-acct-avatar { width: 18px; height: 18px; border-radius: 50%; flex: none; object-fit: cover;
      background: var(--highlight-color, #303030); }
    .scdesk-acct-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .scdesk-acct-check { flex: none; color: var(--special-color, #f50); font-weight: 600; }
    .scdesk-acct-plus { width: 18px; text-align: center; flex: none; font-weight: 600; }
    .scdesk-acct-btn { float: left; height: 46px; display: flex; align-items: center; margin-right: 8px; }
    .scdesk-acct-btn button { font: inherit; font-size: 14px; color: var(--font-primary-color, #fff); background: none;
      border: 1px solid var(--highlight-color, #303030); border-radius: 4px; padding: 4px 10px; cursor: pointer; }
    .scdesk-acct-own { position: fixed; z-index: 2147483000; min-width: 200px; padding: 6px 0; list-style: none; margin: 0;
      background: var(--surface-color, #121212); border: 1px solid var(--highlight-color, #303030); border-radius: 4px;
      box-shadow: 0 4px 16px rgba(0, 0, 0, .4); font-size: 14px; }
    .scdesk-acct-own a { display: flex; align-items: center; gap: 8px; padding: 6px 12px; color: var(--font-primary-color, #fff);
      text-decoration: none; }
    .scdesk-acct-own a:hover { background: var(--highlight-color, #303030); }`;

  const ask = (r) => {
    request = r;
    closeOwn();
    // Close SoundCloud's dropdown too.
    document.body.click();
  };

  const link = (cls, onClick, ...children) => {
    const a = document.createElement('a');
    a.href = '#';
    a.className = cls;
    a.append(...children);
    a.addEventListener('click', (e) => { e.preventDefault(); e.stopPropagation(); onClick(); });
    return a;
  };
  const span = (cls, text) => {
    const s = document.createElement('span');
    s.className = cls;
    s.textContent = text;
    return s;
  };

  /// The menu entries, as <li>s. `linkClass` matches the surrounding menu's links.
  const entries = (linkClass) => {
    const li = (child, cls = '') => {
      const el = document.createElement('li');
      el.className = 'scdesk-acct ' + cls;
      if (child) el.append(child);
      return el;
    };
    const out = [li(null, 'scdesk-acct-sep')];
    for (const acc of accounts) {
      const avatar = acc.avatar ? Object.assign(document.createElement('img'), { src: acc.avatar, alt: '' }) : document.createElement('span');
      avatar.className = 'scdesk-acct-avatar';
      out.push(li(link(`${linkClass} scdesk-acct-link`, () => ask({ action: 'switch', id: acc.id }),
        avatar, span('scdesk-acct-name', acc.name), span('scdesk-acct-check', acc.id === active ? '✓' : ''))));
    }
    out.push(li(link(`${linkClass} scdesk-acct-link`, () => ask({ action: 'add' }),
      span('scdesk-acct-plus', '+'), span('scdesk-acct-name', 'Add account'))));
    if (active && active !== 'main') {
      out.push(li(link(`${linkClass} scdesk-acct-link`, () => ask({ action: 'remove' }),
        span('scdesk-acct-plus', '−'), span('scdesk-acct-name', 'Remove this account'))));
    }
    return out;
  };

  // Signed in: add to SoundCloud's profile dropdown each time it opens.
  const fillProfileMenu = () => {
    const ul = document.querySelector('.profileMenu .headerMenu__list');
    if (!ul || !accounts.length || ul.querySelector('.scdesk-acct')) return;
    ul.append(...entries('headerMenu__link'));
  };

  // Signed out: our own button and dropdown next to "Sign in".
  let own = null;
  const closeOwn = () => { own?.remove(); own = null; };
  const openOwn = (anchor) => {
    closeOwn();
    own = document.createElement('ul');
    own.className = 'scdesk-acct-own';
    own.append(...entries('').slice(1)); // no divider at the top of its own menu
    const r = anchor.getBoundingClientRect();
    own.style.top = `${r.bottom + 4}px`;
    own.style.left = `${Math.max(8, Math.min(r.left, innerWidth - 220))}px`;
    document.documentElement.appendChild(own);
  };
  addEventListener('click', (e) => { if (own && !own.contains(e.target) && !e.target.closest('.scdesk-acct-btn')) closeOwn(); }, true);

  const signedOutButton = () => {
    const login = document.querySelector('.header__right .loginButton');
    const existing = document.querySelector('.scdesk-acct-btn');
    if (!login || accounts.length < 2) { existing?.remove(); return; }
    if (existing) return;
    const wrap = document.createElement('div');
    wrap.className = 'scdesk-acct-btn';
    const b = document.createElement('button');
    b.textContent = 'Accounts ▾';
    b.addEventListener('click', () => (own ? closeOwn() : openOwn(b)));
    wrap.append(b);
    login.closest('.header__right').prepend(wrap);
  };

  const render = () => {
    if (!style.isConnected) document.head?.appendChild(style);
    document.querySelectorAll('.profileMenu .scdesk-acct').forEach((e) => e.remove());
    document.querySelector('.scdesk-acct-btn')?.remove();
    fillProfileMenu();
    signedOutButton();
  };

  let queued = false;
  const watch = () => new MutationObserver(() => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => { queued = false; fillProfileMenu(); signedOutButton(); });
  }).observe(document.body, { childList: true, subtree: true });
  if (document.body) watch();
  else document.addEventListener('DOMContentLoaded', watch, { once: true });

  Object.defineProperty(window, '__scdeskSetAccounts', {
    value: (list, activeId) => { accounts = list; active = activeId; render(); },
  });
  Object.defineProperty(window, '__scdeskTakeAccountRequest', {
    value: () => { const r = request; request = null; return r; },
  });
})();
