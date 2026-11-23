# SoundCloud Desktop

A lightweight, unofficial SoundCloud app for Windows built with Tauri, using the WebView2 runtime that ships with Windows.

> **Unofficial.** This project is not affiliated with, endorsed by or connected to SoundCloud. "SoundCloud" and its logo are trademarks of SoundCloud Global Limited & Co. KG. The app only loads the soundcloud.com website.

## Install

Download the latest `SoundCloud_<version>_x64-setup.exe` from the [Releases](../../releases) page and run it.
The installer isn't code-signed, so Windows SmartScreen may warn you: click **More info → Run anyway**.

For the Discord status, keep the Discord desktop app running and turn on
**Discord Settings → Activity Privacy → Share your detected activities with others**.

## Features

- Only SoundCloud loads in the app. Other links open in your default browser.
- Closing the window hides it to the tray, and the music keeps playing. Left-click the tray icon to show or hide the window; right-click it for play/pause, next, previous, colors and quit.
- Launching it a second time focuses the window that's already running.
- Remembers its window size and position.
- **Stays signed in:** SoundCloud renews its 24-hour login token when the page loads, and gives up silently if that fails (for example when Windows reopens the app right after a restart). If the page comes up signed out while your saved refresh token is still there, the app reloads it after 2, 10, 30 and 60 seconds until the renewal goes through. A real sign-out deletes that token, so it never fights you. Page loads that fail for lack of a connection are retried every 10 seconds.
- **Multiple accounts:** open SoundCloud's profile menu (your avatar, top right) to see your accounts, switch between them, add one or remove one. Signed out, an **Accounts** button next to "Sign in" does the same; the tray menu has an Accounts submenu too. Each account lives in its own WebView2 profile, so logins, cookies and themes are fully separate, and accounts are named after the SoundCloud handle signed in to them.
- Back and forward buttons sit at the left of SoundCloud's header. The mouse side buttons and Alt+←/→ also work.
- **Discord Rich Presence:** while a track plays, your Discord status shows "Listening to …" with the song, artist, cover art and a progress bar. It clears 3 seconds after you pause, and the tray menu's "Show on Discord" turns it off (remembered between launches). It needs the Discord desktop app running. The Discord application ID is in `src-tauri/src/presence.rs`.
- **Ad & tracker blocking:** about 65 ad, tracker and consent-banner domains (Google ads and analytics, Prebid exchanges, Amazon, TikTok, Facebook, Reddit, OneTrust, ...) are blocked inside WebView2 for every frame. The list is in `src-tauri/src/blocklist.rs`; run `cargo test` in `src-tauri` after editing it. Don't block `dwt.soundcloud.com`: it's SoundCloud's bot protection (DataDome), and without it follows and other account actions fail.
- **Performance:**
  - SoundCloud's discover page keeps invisible "buffering" spinners animating at full refresh rate. The app pauses them, which takes idle CPU from about 14% to about 3% of a core.
  - In the tray, the page stops painting and WebView2 trims its memory: about 11 MB in RAM and under 1% CPU, and music keeps playing.
  - SmartScreen lookups, Cast discovery, autofill, sync and telemetry pings are turned off.
- **Color customizer:** click the palette button in SoundCloud's top bar, press `Ctrl+,`, or use the tray menu.
  - Presets, six color pickers and an approximate waveform tint.
  - Paste or load any userstyle CSS (for example from userstyles.world). `@-moz-document` wrappers are unwrapped automatically. Userstyles that need a preprocessor (Stylus/LESS `@var`) won't work.
  - "Copy theme code" and "Paste theme code" share a theme between machines.

## Rebuild

Requires Rust and the Visual Studio C++ Build Tools.

```
npm install
npm run build
```

- `src-tauri/src/main.rs`: the window, navigation lock, tray and plugins.
- `src-tauri/src/webview2.rs`: request blocking, the tray-time background mode, page-load and cookie checks, through WebView2's own APIs.
- `src-tauri/src/session.rs`: the sign-in recovery and connection retries.
- `src-tauri/src/accounts.rs` + `src-tauri/inject/accounts.js`: the account list, profiles and the in-page switcher.
- `src-tauri/src/thumbbar.rs`: the like / previous / play-pause / next buttons under the taskbar hover preview.
- `src-tauri/inject/theme.js`: the color customizer, injected into soundcloud.com.
- `src-tauri/inject/cleanup.js`: hides empty ad slots and pauses the invisible spinners.
- `src-tauri/inject/nav.js`: the back/forward buttons and mouse side buttons.
- `src-tauri/inject/presence.js` + `src-tauri/src/presence.rs`: reads the player bar and sends it to Discord.
- `assets/soundcloud-logo-icon.webp`: the source for the icons (`npm run icon` regenerates them; delete the generated `android`/`ios` folders afterwards).
