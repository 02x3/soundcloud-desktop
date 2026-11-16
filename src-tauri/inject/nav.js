// Back / forward buttons in SoundCloud's header, plus the mouse side buttons.
// (Alt+Left / Alt+Right already work through WebView2's built-in shortcuts.)
(() => {
  if (window.top !== window || !/(^|\.)soundcloud\.com$/.test(location.hostname)) return;

  const nav = window.navigation;

  const style = document.createElement('style');
  style.textContent = `
    .scdesk-nav{float:left;display:flex;align-items:center;gap:2px;height:46px;margin-right:8px}
    .scdesk-nav button{width:30px;height:30px;padding:0;display:flex;align-items:center;justify-content:center;
      background:none;border:0;border-radius:50%;cursor:pointer;color:var(--font-primary-color,#fff)}
    .scdesk-nav button:hover:not(:disabled){background:var(--highlight-color,#303030)}
    .scdesk-nav button:disabled{color:var(--font-secondary-color,#999);opacity:.45;cursor:default}`;

  const button = (title, path, onClick) => {
    const b = document.createElement('button');
    b.title = title;
    b.innerHTML = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="${path}"/></svg>`;
    b.addEventListener('click', onClick);
    return b;
  };
  const back = button('Back (Alt+←)', 'M15 18l-6-6 6-6', () => history.back());
  const forward = button('Forward (Alt+→)', 'M9 18l6-6-6-6', () => history.forward());
  const wrap = document.createElement('div');
  wrap.className = 'scdesk-nav';
  wrap.append(back, forward);

  const update = () => {
    back.disabled = nav ? !nav.canGoBack : history.length < 2;
    forward.disabled = nav ? !nav.canGoForward : false;
  };
  nav?.addEventListener('currententrychange', update);
  addEventListener('popstate', update);

  // The header is re-rendered on some navigations, so put the buttons back when they go missing.
  const mount = () => {
    const left = document.querySelector('.header__left');
    if (!left || left.contains(wrap)) return;
    if (!style.isConnected) document.head.appendChild(style);
    left.prepend(wrap);
    update();
  };
  let queued = false;
  const watch = () => {
    new MutationObserver(() => {
      if (queued) return;
      queued = true;
      requestAnimationFrame(() => { queued = false; mount(); });
    }).observe(document.body, { childList: true, subtree: true });
    mount();
  };
  if (document.body) watch();
  else document.addEventListener('DOMContentLoaded', watch, { once: true });

  // Mouse side buttons. Cancelling the event also stops any built-in handling, so it navigates exactly once.
  addEventListener('mouseup', (e) => {
    if (e.button !== 3 && e.button !== 4) return;
    e.preventDefault();
    e.button === 3 ? history.back() : history.forward();
  });
})();
