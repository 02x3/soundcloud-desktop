// Reads what SoundCloud's player bar is playing. The app polls this for Discord Rich Presence
// and the taskbar preview buttons.
(() => {
  if (window.top !== window || !/(^|\.)soundcloud\.com$/.test(location.hostname)) return;

  const pageUrl = (href) => {
    if (!href) return '';
    const u = new URL(href, location.origin);
    return u.origin + u.pathname;
  };

  Object.defineProperty(window, '__scdeskNowPlaying', {
    value: () => {
      const link = document.querySelector('.playbackSoundBadge__titleLink');
      if (!link) return null;
      const artist = document.querySelector('.playbackSoundBadge__lightLink');
      const timeline = document.querySelector('.playbackTimeline__progressWrapper');
      const art = document.querySelector('.playbackSoundBadge__avatar .image__full')
        ?.style.backgroundImage.match(/url\("?(.*?)"?\)/)?.[1] || '';
      return {
        title: (link.title || link.textContent).trim(),
        artist: (artist?.title || '').trim(),
        url: pageUrl(link.getAttribute('href')),
        artist_url: pageUrl(artist?.getAttribute('href')),
        // The player bar loads 50x50 artwork; Discord gets the 500x500 version of the same image.
        artwork: art.startsWith('https://') ? art.replace(/-t\d+x\d+(\.\w+)$/, '-t500x500$1') : '',
        duration: Number(timeline?.getAttribute('aria-valuemax')) || 0,
        position: Number(timeline?.getAttribute('aria-valuenow')) || 0,
        playing: !!document.querySelector('.playControls__play.playing'),
        liked: !!document.querySelector('.playbackSoundBadge__like.sc-button-selected'),
      };
    },
  });
})();
