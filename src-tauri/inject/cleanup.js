// Collapses the slots left behind by blocked ads, the consent banner and the (unusable) Cast button,
// and stops SoundCloud's invisible spinners from repainting the page at full refresh rate.
(() => {
  if (window.top !== window || !/(^|\.)soundcloud\.com$/.test(location.hostname)) return;

  const css = `
    [id^="div-gpt-ad"], [id^="google_ads_iframe"], iframe[src*="doubleclick.net"],
    .sidebarModule:has([id^="div-gpt-ad"]), #onetrust-consent-sdk, .playControls__castControl
    { display: none !important; }`;
  const style = document.createElement('style');
  style.textContent = css;
  const mount = () => document.documentElement && (document.documentElement.appendChild(style), true);
  if (!mount()) {
    const mo = new MutationObserver(() => mount() && mo.disconnect());
    mo.observe(document, { childList: true });
  }

  // Every tile's play button carries a "buffering" SVG spinner (SMIL, repeatCount=indefinite) that stays
  // animating while the button is invisible, forcing style + layout + paint every frame (~13% of a core).
  // Pause SVG animations nobody can see; resume them when they become visible (e.g. on tile hover).
  const syncSpinners = () => {
    if (document.hidden) return;
    for (const a of document.querySelectorAll('animate, animateTransform, animateMotion')) {
      const svg = a.ownerSVGElement;
      if (!svg) continue;
      const visible = svg.checkVisibility({ opacityProperty: true, visibilityProperty: true });
      if (visible === svg.animationsPaused()) visible ? svg.unpauseAnimations() : svg.pauseAnimations();
    }
  };
  setInterval(syncSpinners, 1000);
  let hoverTimer = 0;
  addEventListener('pointerover', () => {
    clearTimeout(hoverTimer);
    // Tile buttons fade in, so check once the transition has had a moment to make them visible.
    hoverTimer = setTimeout(syncSpinners, 200);
  }, { passive: true });
})();
