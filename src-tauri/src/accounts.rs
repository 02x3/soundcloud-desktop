//! Several SoundCloud accounts, each in its own WebView2 profile (separate cookies, login and
//! storage). Switching swaps which profile the window uses; the app never reads anyone's login data.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// The original profile, in WebView2's default folder, so the login from before accounts existed stays.
pub const MAIN: &str = "main";

#[derive(Serialize, Deserialize, Clone)]
pub struct Account {
    pub id: String,
    pub name: String,
    /// SoundCloud avatar image URL, shown in the in-page account switcher.
    #[serde(default)]
    pub avatar: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Accounts {
    pub active: String,
    pub list: Vec<Account>,
}

impl Default for Accounts {
    fn default() -> Self {
        Accounts {
            active: MAIN.into(),
            list: vec![Account { id: MAIN.into(), name: "Account 1".into(), avatar: String::new() }],
        }
    }
}

impl Accounts {
    pub fn active_name(&self) -> &str {
        self.list.iter().find(|a| a.id == self.active).map_or("", |a| a.name.as_str())
    }
}

pub type Store = Mutex<Accounts>;

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("accounts.json"))
}

fn profiles_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_local_data_dir().ok().map(|d| d.join("accounts"))
}

pub fn load(app: &AppHandle) -> Accounts {
    let mut a: Accounts = file(app)
        .and_then(|f| std::fs::read(f).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if !a.list.iter().any(|x| x.id == MAIN) {
        a.list.insert(0, Accounts::default().list.remove(0));
    }
    if !a.list.iter().any(|x| x.id == a.active) {
        a.active = MAIN.into();
    }
    a
}

pub fn save(app: &AppHandle, a: &Accounts) {
    if let Some(f) = file(app) {
        if let Some(dir) = f.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(a) {
            let _ = std::fs::write(f, json);
        }
    }
}

/// WebView2 data folder for an account; `None` means WebView2's default (the main account).
pub fn data_dir(app: &AppHandle, id: &str) -> Option<PathBuf> {
    if id == MAIN {
        None
    } else {
        profiles_dir(app).map(|d| d.join(id))
    }
}

pub fn new_id() -> String {
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    format!("a{ms}")
}

/// Deletes profile folders of removed accounts. A folder still in use (its browser process
/// hasn't exited yet) is left for the next launch.
pub fn delete_unused_profiles(app: &AppHandle, a: &Accounts) {
    let Some(dir) = profiles_dir(app) else { return };
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !a.list.iter().any(|x| x.id == name) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// Labels the active account with the SoundCloud user signed in to it. True if anything changed.
pub fn update_active(app: &AppHandle, name: &str, avatar: &str) -> bool {
    let name: String = name.trim().chars().take(40).collect();
    if name.is_empty() {
        return false;
    }
    let store = app.state::<Store>();
    let mut a = store.lock().unwrap();
    let active = a.active.clone();
    let Some(acc) = a.list.iter_mut().find(|x| x.id == active) else { return false };
    // Only SoundCloud's image host, so the switcher never loads anything else.
    let avatar = if avatar.starts_with("https://") && avatar.contains(".sndcdn.com/") { avatar } else { "" };
    if acc.name == name && acc.avatar == avatar {
        return false;
    }
    acc.name = name;
    acc.avatar = avatar.to_string();
    save(app, &a);
    true
}
