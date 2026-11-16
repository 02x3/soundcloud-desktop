#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod blocklist;
mod presence;
#[cfg(windows)]
mod session;
#[cfg(windows)]
mod webview2;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::Duration;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::{Color, NewWindowResponse},
    AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

const START_URL: &str = "https://soundcloud.com/discover";
const THEME_JS: &str = include_str!("../inject/theme.js");
const CLEANUP_JS: &str = include_str!("../inject/cleanup.js");
const NAV_JS: &str = include_str!("../inject/nav.js");
const PRESENCE_JS: &str = include_str!("../inject/presence.js");
// Tauri's default WebView2 flags (no SmartScreen lookup on every navigation), plus media keys,
// and no Cast discovery, autofill, sync or telemetry pings.
// Component updates stay on: WebView2 fetches its DRM module that way.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,\
    AutofillServerCommunication,MediaRouter \
    --enable-features=HardwareMediaKeyHandling \
    --disable-domain-reliability --disable-sync --no-pings";

static BACKGROUNDED: AtomicBool = AtomicBool::new(false);
static DISCORD_ENABLED: AtomicBool = AtomicBool::new(true);

/// Present when the user switched Discord status off in the tray menu.
fn discord_off_marker(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("discord-status-off"))
}

fn set_discord_enabled(app: &AppHandle, on: bool) {
    DISCORD_ENABLED.store(on, Ordering::Relaxed);
    if let Some(marker) = discord_off_marker(app) {
        if on {
            let _ = std::fs::remove_file(&marker);
        } else if let Some(dir) = marker.parent() {
            let _ = std::fs::create_dir_all(dir).and_then(|_| std::fs::write(&marker, b""));
        }
    }
    if let Some(tx) = app.try_state::<Sender<presence::Update>>() {
        let _ = tx.send(presence::Update::Enabled(on));
    }
}

/// Starts the Discord thread and polls the page once a second for what's playing.
fn start_presence(app: &AppHandle) {
    let enabled = !discord_off_marker(app).is_some_and(|m| m.exists());
    DISCORD_ENABLED.store(enabled, Ordering::Relaxed);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || presence::run(rx, enabled));

    let poll_tx = tx.clone();
    let handle = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        if !DISCORD_ENABLED.load(Ordering::Relaxed) {
            continue;
        }
        let Some(window) = handle.get_webview_window("main") else { continue };
        let tx = poll_tx.clone();
        #[cfg(windows)]
        let _ = window.with_webview(move |wv| unsafe {
            let _ = webview2::eval_json(&wv.controller(), "window.__scdeskNowPlaying?.() ?? null", move |json| {
                let track = serde_json::from_str::<Option<presence::Track>>(&json).unwrap_or(None);
                let _ = tx.send(presence::Update::NowPlaying(track));
            });
        });
    });
    app.manage(tx);
}

/// Hidden or minimized: stop painting and trim memory. Shown again: back to normal.
fn set_backgrounded(window: &WebviewWindow, backgrounded: bool) {
    if BACKGROUNDED.swap(backgrounded, Ordering::Relaxed) == backgrounded {
        return;
    }
    #[cfg(windows)]
    let _ = window.with_webview(move |wv| unsafe {
        let _ = webview2::set_backgrounded(&wv.controller(), backgrounded);
    });
}

fn window_state_flags() -> StateFlags {
    // Never restore "hidden", or quitting from the tray would make the next launch start invisible.
    StateFlags::all() - StateFlags::VISIBLE
}

fn is_soundcloud(url: &Url) -> bool {
    url.host_str().is_some_and(|h| {
        h == "soundcloud.com" || h.ends_with(".soundcloud.com") || h.ends_with(".sndcdn.com")
    })
}

/// Third-party sign-in pages SoundCloud opens as popups; they must stay in-app for login to complete.
fn is_auth(url: &Url) -> bool {
    url.host_str().is_some_and(|h| {
        matches!(
            h,
            "accounts.google.com" | "appleid.apple.com" | "facebook.com" | "www.facebook.com" | "m.facebook.com"
        )
    })
}

fn allowed_in_app(url: &Url) -> bool {
    matches!(url.scheme(), "about" | "blob" | "data")
        || (url.scheme() == "https" && (is_soundcloud(url) || is_auth(url)))
}

fn open_external(app: &AppHandle, url: &Url) {
    let _ = app.opener().open_url(url.as_str(), None::<&str>);
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        set_backgrounded(&w, false);
    }
}

fn hide_window(w: &WebviewWindow) {
    let _ = w.hide();
    set_backgrounded(w, true);
}

fn toggle_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let shown = w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false);
        if shown {
            hide_window(&w);
        } else {
            show_window(app);
        }
    }
}

fn run_js(app: &AppHandle, js: &str) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(js);
    }
}

fn quit(app: &AppHandle) {
    let _ = app.save_window_state(window_state_flags());
    app.exit(0);
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let discord = CheckMenuItem::with_id(
        app,
        "discord",
        "Show on Discord",
        true,
        DISCORD_ENABLED.load(Ordering::Relaxed),
        None::<&str>,
    )?;
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "show", "Show SoundCloud", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "playpause", "Play / Pause", true, None::<&str>)?,
            &MenuItem::with_id(app, "next", "Next track", true, None::<&str>)?,
            &MenuItem::with_id(app, "prev", "Previous track", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "theme", "Customize colors…", true, None::<&str>)?,
            &discord,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundle has an icon"))
        .tooltip("SoundCloud")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => show_window(app),
            "playpause" => run_js(app, "document.querySelector('.playControls__play')?.click()"),
            "next" => run_js(app, "document.querySelector('.skipControl__next')?.click()"),
            "prev" => run_js(app, "document.querySelector('.skipControl__previous')?.click()"),
            "theme" => {
                show_window(app);
                run_js(app, "window.__scTheme?.open()");
            }
            // The menu flips the checkmark itself; follow it.
            "discord" => set_discord_enabled(app, discord.is_checked().unwrap_or(true)),
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        // Must be registered first: a second launch focuses the running window instead.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_window(app)))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(window_state_flags())
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let nav_handle = handle.clone();
            let popup_handle = handle.clone();

            // Start blank; the blocker navigates to SoundCloud once its filters are in place.
            #[cfg(windows)]
            let first_url = "about:blank";
            #[cfg(not(windows))]
            let first_url = START_URL;

            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(first_url.parse().unwrap()))
                .title("SoundCloud")
                .inner_size(1280.0, 800.0)
                .min_inner_size(820.0, 560.0)
                .background_color(Color(18, 18, 18, 255))
                .visible(false)
                .initialization_script(CLEANUP_JS)
                .initialization_script(NAV_JS)
                .initialization_script(PRESENCE_JS)
                .initialization_script(THEME_JS)
                .additional_browser_args(BROWSER_ARGS)
                // WebView2's native drag & drop, so SoundCloud's upload page works.
                .disable_drag_drop_handler()
                .on_navigation(move |url| {
                    if allowed_in_app(url) {
                        return true;
                    }
                    open_external(&nav_handle, url);
                    false
                })
                .on_new_window(move |url, _features| {
                    if is_auth(&url) || url.scheme() == "about" {
                        NewWindowResponse::Allow
                    } else if is_soundcloud(&url) {
                        // Keep SoundCloud links in the one main window.
                        let h = popup_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Some(w) = h.get_webview_window("main") {
                                let _ = w.navigate(url);
                            }
                        });
                        NewWindowResponse::Deny
                    } else {
                        open_external(&popup_handle, &url);
                        NewWindowResponse::Deny
                    }
                })
                .on_document_title_changed(|window, title| {
                    let title: String = title.chars().take(120).collect();
                    let _ = window.set_title(&title);
                    if let Some(tray) = window.app_handle().tray_by_id("main") {
                        let _ = tray.set_tooltip(Some(&title));
                    }
                })
                .build()?;

            #[cfg(windows)]
            window.with_webview(|wv| unsafe {
                if let Err(e) = webview2::install_blocker(&wv.controller(), &wv.environment(), START_URL) {
                    eprintln!("ad blocker unavailable: {e}");
                    let _ = wv.controller().CoreWebView2().and_then(|c| c.Navigate(&windows_core::HSTRING::from(START_URL)));
                }
            })?;
            #[cfg(windows)]
            session::install(&window, START_URL)?;

            start_presence(&handle);
            build_tray(&handle)?;
            window.show()?;
            Ok(())
        })
        .on_window_event(|window, event| {
            let Some(w) = window.app_handle().get_webview_window("main").filter(|w| w.label() == window.label()) else {
                return;
            };
            match event {
                // Closing the window keeps the music going in the tray, like Discord.
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    hide_window(&w);
                }
                WindowEvent::Resized(_) => {
                    let minimized = w.is_minimized().unwrap_or(false);
                    if minimized || w.is_visible().unwrap_or(false) {
                        set_backgrounded(&w, minimized);
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running SoundCloud");
}
