//! Keeps you signed in across restarts.
//!
//! SoundCloud's access token lasts 24 hours and is renewed at page load from a long-lived refresh
//! token. If that renewal fails (Windows reopens the app right after sign-in, before the network or
//! SoundCloud's bot check is ready), the page shows you as logged out and never retries, even though
//! the refresh token is still saved. This reloads the page until the renewal goes through, and keeps
//! retrying page loads that failed for lack of a connection.

use crate::webview2::{self, Load};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tauri::{Manager, WebviewWindow};

const LOGIN_STATE_JS: &str = "document.querySelector('.header__userNav') ? 'in' \
    : document.querySelector('.header__right .loginButton') ? 'out' : 'unknown'";
/// The signed-in user's handle (from their profile link) and avatar image.
const USER_JS: &str = "(() => { const b = document.querySelector('.header__userNavUsernameButton'); \
    if (!b) return null; \
    const name = decodeURIComponent((b.getAttribute('href') || '').split('/')[1] || ''); \
    const img = b.querySelector('.header__userNavAvatar .image__full')?.style.backgroundImage.match(/url\\(\"?(.*?)\"?\\)/)?.[1] || ''; \
    return { name, avatar: img } })()";

#[derive(serde::Deserialize)]
struct User {
    name: String,
    avatar: String,
}
const REFRESH_COOKIE: &str = "oauth_refresh_token";
/// Waits before each reload while signed out with a refresh token still saved.
const RELOGIN_DELAYS: [u64; 4] = [2, 10, 30, 60];
const NETWORK_RETRY: Duration = Duration::from_secs(10);
/// SoundCloud draws its header a moment after load; how long to wait, and how often to look again.
const FIRST_CHECK: Duration = Duration::from_secs(6);
const MAX_UNKNOWN_CHECKS: u32 = 5;

static RELOGIN_ATTEMPTS: AtomicU32 = AtomicU32::new(0);

fn after(delay: Duration, f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        f();
    });
}

pub fn install(window: &WebviewWindow, start_url: &'static str) -> tauri::Result<()> {
    let w = window.clone();
    window.with_webview(move |wv| unsafe {
        let _ = webview2::on_page_load(&wv.controller(), move |load| match load {
            Load::Done => {
                crate::push_accounts(w.app_handle());
                let w = w.clone();
                after(FIRST_CHECK, move || check_login(w, 0));
            }
            Load::NetworkError => {
                let w = w.clone();
                after(NETWORK_RETRY, move || {
                    let _ = w.navigate(start_url.parse().unwrap());
                });
            }
            Load::Other => {}
        });
    })
}

fn check_login(window: WebviewWindow, checks: u32) {
    let w = window.clone();
    let _ = window.with_webview(move |wv| unsafe {
        let controller = wv.controller();
        let _ = webview2::eval_json(&controller.clone(), LOGIN_STATE_JS, move |json| {
            match serde_json::from_str::<String>(&json).as_deref() {
                Ok("in") => {
                    RELOGIN_ATTEMPTS.store(0, Ordering::Relaxed);
                    // Label the account after whoever is signed in to it.
                    let _ = webview2::eval_json(&controller, USER_JS, move |json| {
                        if let Ok(user) = serde_json::from_str::<User>(&json) {
                            let app = w.app_handle();
                            if crate::accounts::update_active(app, &user.name, &user.avatar) {
                                crate::accounts_changed(app);
                            }
                        }
                    });
                }
                Ok("out") => {
                    // Only a real sign-out deletes the refresh token; with it still there, the renewal just failed.
                    let _ = webview2::has_cookie(&controller, "https://soundcloud.com", REFRESH_COOKIE, move |saved| {
                        let attempt = RELOGIN_ATTEMPTS.load(Ordering::Relaxed) as usize;
                        if saved && attempt < RELOGIN_DELAYS.len() {
                            RELOGIN_ATTEMPTS.store(attempt as u32 + 1, Ordering::Relaxed);
                            after(Duration::from_secs(RELOGIN_DELAYS[attempt]), move || {
                                let _ = w.eval("location.reload()");
                            });
                        }
                    });
                }
                _ if checks < MAX_UNKNOWN_CHECKS => after(Duration::from_secs(3), move || check_login(w, checks + 1)),
                _ => {}
            }
        });
    });
}
