#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;
mod blocklist;
mod presence;
#[cfg(windows)]
mod session;
#[cfg(windows)]
mod thumbbar;
#[cfg(windows)]
mod webview2;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::{Color, NewWindowResponse},
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

const START_URL: &str = "https://soundcloud.com/discover";
const THEME_JS: &str = include_str!("../inject/theme.js");
const CLEANUP_JS: &str = include_str!("../inject/cleanup.js");
const NAV_JS: &str = include_str!("../inject/nav.js");
const PRESENCE_JS: &str = include_str!("../inject/presence.js");
const ACCOUNTS_JS: &str = include_str!("../inject/accounts.js");
/// Polled once a second: what's playing, and any click in the in-page account switcher.
const POLL_JS: &str = "({ track: window.__scdeskNowPlaying?.() ?? null, \
    account: window.__scdeskTakeAccountRequest?.() ?? null })";

#[derive(serde::Deserialize)]
struct Poll {
    track: Option<presence::Track>,
    account: Option<AccountRequest>,
}

#[derive(serde::Deserialize)]
struct AccountRequest {
    action: String,
    #[serde(default)]
    id: String,
}
// Tauri's default WebView2 flags (no SmartScreen lookup on every navigation), plus media keys,
// and no Cast discovery, autofill, sync or telemetry pings.
// Component updates stay on: WebView2 fetches its DRM module that way.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,\
    AutofillServerCommunication,MediaRouter \
    --enable-features=HardwareMediaKeyHandling \
    --disable-domain-reliability --disable-sync --no-pings";

static BACKGROUNDED: AtomicBool = AtomicBool::new(false);
static DISCORD_ENABLED: AtomicBool = AtomicBool::new(true);
static APP: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();
/// Label of the window showing the active account. Each account's window has its own label,
/// so the window-state plugin remembers its size and position separately.
static CURRENT_LABEL: Mutex<String> = Mutex::new(String::new());

const LIKE_JS: &str = "document.querySelector('.playbackSoundBadge__like')?.click()";
const PREV_JS: &str = "document.querySelector('.skipControl__previous')?.click()";
const PLAY_PAUSE_JS: &str = "document.querySelector('.playControls__play')?.click()";
const NEXT_JS: &str = "document.querySelector('.skipControl__next')?.click()";

fn label_for(account: &str) -> String {
    if account == accounts::MAIN {
        "main".into()
    } else {
        format!("acct-{account}")
    }
}

/// The window of the active account.
fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    let label = CURRENT_LABEL.lock().unwrap().clone();
    app.get_webview_window(&label)
}

/// A button under the taskbar hover preview was clicked.
#[cfg(windows)]
fn thumb_clicked(id: u32) {
    let Some(app) = APP.get() else { return };
    let js = match id {
        thumbbar::LIKE => LIKE_JS,
        thumbbar::PREV => PREV_JS,
        thumbbar::PLAY_PAUSE => PLAY_PAUSE_JS,
        thumbbar::NEXT => NEXT_JS,
        _ => return,
    };
    run_js(app, js);
}

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

/// Starts the Discord thread and polls the page once a second for what's playing,
/// which also keeps the taskbar preview buttons in step.
fn start_presence(app: &AppHandle) {
    let enabled = !discord_off_marker(app).is_some_and(|m| m.exists());
    DISCORD_ENABLED.store(enabled, Ordering::Relaxed);

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || presence::run(rx, enabled));

    let poll_tx = tx.clone();
    let handle = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let Some(window) = main_window(&handle) else { continue };
        let tx = poll_tx.clone();
        let app = handle.clone();
        #[cfg(windows)]
        let _ = window.with_webview(move |wv| unsafe {
            let _ = webview2::eval_json(&wv.controller(), POLL_JS, move |json| {
                let Ok(poll) = serde_json::from_str::<Poll>(&json) else { return };
                if let Some(t) = &poll.track {
                    thumbbar::set_state(t.playing, t.liked);
                }
                // The Discord thread ignores this while "Show on Discord" is off.
                let _ = tx.send(presence::Update::NowPlaying(poll.track));
                if let Some(req) = poll.account {
                    off_ui_thread(&app, move |app| match req.action.as_str() {
                        "switch" => {
                            let _ = switch_account(app, &req.id);
                        }
                        "add" => add_account(app),
                        "remove" => remove_active_account(app),
                        _ => {}
                    });
                }
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
    if let Some(w) = main_window(app) {
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
    if let Some(w) = main_window(app) {
        let shown = w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false);
        if shown {
            hide_window(&w);
        } else {
            show_window(app);
        }
    }
}

fn run_js(app: &AppHandle, js: &str) {
    if let Some(w) = main_window(app) {
        let _ = w.eval(js);
    }
}

fn quit(app: &AppHandle) {
    let _ = app.save_window_state(window_state_flags());
    app.exit(0);
}

/// Builds the window for an account, hidden. `place` reuses the previous window's spot when switching.
fn create_window(
    app: &AppHandle,
    account: &str,
    place: Option<(PhysicalPosition<i32>, PhysicalSize<u32>)>,
) -> tauri::Result<WebviewWindow> {
    let nav_handle = app.clone();
    let popup_handle = app.clone();

    // Start blank; the blocker navigates to SoundCloud once its filters are in place.
    #[cfg(windows)]
    let first_url = "about:blank";
    #[cfg(not(windows))]
    let first_url = START_URL;

    let label = label_for(account);
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(first_url.parse().unwrap()))
        .title("SoundCloud")
        .inner_size(1280.0, 800.0)
        .min_inner_size(820.0, 560.0)
        .background_color(Color(18, 18, 18, 255))
        .visible(false)
        .initialization_script(CLEANUP_JS)
        .initialization_script(NAV_JS)
        .initialization_script(PRESENCE_JS)
        .initialization_script(ACCOUNTS_JS)
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
                    if let Some(w) = main_window(&h) {
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
        });
    if let Some(dir) = accounts::data_dir(app, account) {
        builder = builder.data_directory(dir);
    }
    if let Some((pos, size)) = place {
        builder = builder
            .position(pos.x as f64, pos.y as f64)
            .inner_size(size.width as f64, size.height as f64);
    }
    let window = builder.build()?;
    if let Some((pos, size)) = place {
        // Physical pixels are exact across DPI settings; the builder takes logical ones.
        let _ = window.set_position(pos);
        let _ = window.set_size(size);
    }

    #[cfg(windows)]
    {
        window.with_webview(|wv| unsafe {
            if let Err(e) = webview2::install_blocker(&wv.controller(), &wv.environment(), START_URL) {
                eprintln!("ad blocker unavailable: {e}");
                let _ = wv.controller().CoreWebView2().and_then(|c| c.Navigate(&windows_core::HSTRING::from(START_URL)));
            }
        })?;
        session::install(&window, START_URL)?;
        // On the UI thread that owns the window, before it's shown, so the buttons go on as
        // soon as the taskbar button exists.
        let hwnd = window.hwnd()?.0 as isize;
        app.run_on_main_thread(move || {
            let hwnd = windows::Win32::Foundation::HWND(hwnd as *mut _);
            if let Err(e) = thumbbar::install(hwnd, thumb_clicked) {
                eprintln!("taskbar buttons unavailable: {e}");
            }
        })?;
    }
    Ok(window)
}

/// Runs account switching away from the UI thread. Building a window from a menu or webview
/// callback on the UI thread deadlocks on Windows: the build waits for the event loop it's blocking.
fn off_ui_thread(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let app = app.clone();
    std::thread::spawn(move || f(&app));
}

/// Swaps the window over to another account's profile, in the same spot on screen.
/// Call it off the UI thread (see `off_ui_thread`).
fn switch_account(app: &AppHandle, id: &str) -> tauri::Result<()> {
    let old = main_window(app);
    let place = old.as_ref().and_then(|w| Some((w.outer_position().ok()?, w.inner_size().ok()?)));
    {
        let store = app.state::<accounts::Store>();
        let mut a = store.lock().unwrap();
        if (a.active == id && old.is_some()) || !a.list.iter().any(|x| x.id == id) {
            return Ok(());
        }
        a.active = id.to_string();
        accounts::save(app, &a);
    }
    let _ = app.save_window_state(window_state_flags());
    // New window first, then close the old one: no blank gap, and never zero windows.
    let window = create_window(app, id, place)?;
    *CURRENT_LABEL.lock().unwrap() = label_for(id);
    BACKGROUNDED.store(false, Ordering::Relaxed);
    window.show()?;
    let _ = window.set_focus();
    if let Some(old) = old {
        old.destroy()?;
    }
    refresh_tray(app);
    Ok(())
}

fn add_account(app: &AppHandle) {
    let id = accounts::new_id();
    {
        let store = app.state::<accounts::Store>();
        let mut a = store.lock().unwrap();
        let name = format!("Account {}", a.list.len() + 1);
        a.list.push(accounts::Account { id: id.clone(), name, avatar: String::new() });
        accounts::save(app, &a);
    }
    let _ = switch_account(app, &id);
}

/// Removes the active account (never the main one): back to the main account, then its
/// profile folder, and with it that account's login, is deleted.
fn remove_active_account(app: &AppHandle) {
    let removed = {
        let store = app.state::<accounts::Store>();
        let a = store.lock().unwrap();
        if a.active == accounts::MAIN {
            return;
        }
        a.active.clone()
    };
    let _ = switch_account(app, accounts::MAIN);
    {
        let store = app.state::<accounts::Store>();
        let mut a = store.lock().unwrap();
        a.list.retain(|x| x.id != removed);
        accounts::save(app, &a);
    }
    accounts_changed(app);
    // Give the old profile's browser processes a moment to exit and release their files.
    let h = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        let a = h.state::<accounts::Store>().lock().unwrap().clone();
        accounts::delete_unused_profiles(&h, &a);
    });
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let a = app.state::<accounts::Store>().lock().unwrap().clone();
    let account_items = a
        .list
        .iter()
        .map(|acc| CheckMenuItem::with_id(app, format!("account:{}", acc.id), &acc.name, true, acc.id == a.active, None::<&str>))
        .collect::<tauri::Result<Vec<_>>>()?;
    let accounts_menu = Submenu::with_id(app, "accounts", format!("Account: {}", a.active_name()), true)?;
    for item in &account_items {
        accounts_menu.append(item)?;
    }
    accounts_menu.append_items(&[
        &PredefinedMenuItem::separator(app)?,
        &MenuItem::with_id(app, "account-add", "Add account…", true, None::<&str>)?,
        &MenuItem::with_id(
            app,
            "account-remove",
            "Remove this account (signs it out)",
            a.active != accounts::MAIN,
            None::<&str>,
        )?,
    ])?;

    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "show", "Show SoundCloud", true, None::<&str>)?,
            &accounts_menu,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "playpause", "Play / Pause", true, None::<&str>)?,
            &MenuItem::with_id(app, "next", "Next track", true, None::<&str>)?,
            &MenuItem::with_id(app, "prev", "Previous track", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "theme", "Customize colors…", true, None::<&str>)?,
            &CheckMenuItem::with_id(
                app,
                "discord",
                "Show on Discord",
                true,
                DISCORD_ENABLED.load(Ordering::Relaxed),
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )
}

/// Rebuilds the tray menu after the accounts or their names change.
fn refresh_tray(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("main"), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

/// Hands the account list to the page's switcher (inject/accounts.js).
pub fn push_accounts(app: &AppHandle) {
    let a = app.state::<accounts::Store>().lock().unwrap().clone();
    if let (Ok(list), Ok(active)) = (serde_json::to_string(&a.list), serde_json::to_string(&a.active)) {
        run_js(app, &format!("window.__scdeskSetAccounts?.({list}, {active})"));
    }
}

/// Updates the tray menu and the in-page switcher after accounts are added, removed or renamed.
pub fn accounts_changed(app: &AppHandle) {
    refresh_tray(app);
    push_accounts(app);
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundle has an icon"))
        .tooltip("SoundCloud")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_window(app),
            "playpause" => run_js(app, PLAY_PAUSE_JS),
            "next" => run_js(app, NEXT_JS),
            "prev" => run_js(app, PREV_JS),
            "theme" => {
                show_window(app);
                run_js(app, "window.__scTheme?.open()");
            }
            // The menu flips the checkmark itself; follow it.
            "discord" => set_discord_enabled(app, !DISCORD_ENABLED.load(Ordering::Relaxed)),
            "account-add" => off_ui_thread(app, add_account),
            "account-remove" => off_ui_thread(app, remove_active_account),
            "quit" => quit(app),
            id => {
                if let Some(account) = id.strip_prefix("account:") {
                    let account = account.to_string();
                    off_ui_thread(app, move |app| {
                        let _ = switch_account(app, &account);
                        // Clicking the current account unticks it; put the tick back.
                        refresh_tray(app);
                    });
                }
            }
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
            let _ = APP.set(handle.clone());

            let accounts = accounts::load(&handle);
            accounts::delete_unused_profiles(&handle, &accounts);
            let active = accounts.active.clone();
            app.manage::<accounts::Store>(Mutex::new(accounts));
            *CURRENT_LABEL.lock().unwrap() = label_for(&active);

            let window = create_window(&handle, &active, None)?;
            start_presence(&handle);
            build_tray(&handle)?;
            window.show()?;
            Ok(())
        })
        .on_window_event(|window, event| {
            let Some(w) = main_window(window.app_handle()).filter(|w| w.label() == window.label()) else {
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
        .build(tauri::generate_context!())
        .expect("error while building SoundCloud")
        .run(|_app, event| {
            // Closing a window (e.g. the old one when switching accounts) must not end the app;
            // only Quit (which exits with a code) does.
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}
