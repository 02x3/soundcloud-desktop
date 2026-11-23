//! Discord Rich Presence: "Listening to SoundCloud" with the current track, like Spotify's.
//!
//! The page script (inject/presence.js) is polled once a second; this thread owns the Discord
//! connection and only talks to Discord when what should be shown actually changes.

use discord_rich_presence::{
    activity::{Activity, ActivityType, Assets, StatusDisplayType, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use serde::Deserialize;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const APP_ID: &str = "1566829792844709918";
/// Discord allows about 5 activity updates per 20 seconds.
const MIN_GAP: Duration = Duration::from_secs(4);
const RECONNECT_EVERY: Duration = Duration::from_secs(15);
/// How far the playback position may drift from what Discord shows before it's treated as a seek.
const SEEK_TOLERANCE_MS: i64 = 3000;
/// SoundCloud reports "paused" for a moment between tracks; only clear the status after a real pause.
const CLEAR_AFTER: Duration = Duration::from_secs(3);

#[derive(Deserialize, Clone, Debug)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub url: String,
    pub artist_url: String,
    pub artwork: String,
    pub duration: f64,
    pub position: f64,
    pub playing: bool,
    #[serde(default)]
    pub liked: bool,
}

pub enum Update {
    NowPlaying(Option<Track>),
    Enabled(bool),
}

/// What Discord currently shows, to decide whether a new update is needed.
struct Shown {
    url: String,
    start_ms: i64,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn start_ms(t: &Track) -> i64 {
    now_ms() - (t.position * 1000.0) as i64
}

/// Discord rejects text fields shorter than 2 or longer than 128 characters.
fn text(s: &str) -> String {
    let mut s: String = s.chars().take(128).collect();
    if s.chars().count() < 2 {
        s.push('\u{2800}');
    }
    s
}

fn activity(t: &Track) -> Activity<'_> {
    let start = start_ms(t);
    let end = start + (t.duration * 1000.0) as i64;

    let mut assets = Assets::new().large_text(text(&t.title)).large_url(t.url.as_str());
    if !t.artwork.is_empty() {
        assets = assets.large_image(t.artwork.as_str());
    }

    // Member list shows "Listening to <artist>" instead of "Listening to SoundCloud".
    // Discord rejects the State display type without a state, so fall back to the song title.
    let display = if t.artist.is_empty() { StatusDisplayType::Details } else { StatusDisplayType::State };
    let mut a = Activity::new()
        .activity_type(ActivityType::Listening)
        .status_display_type(display)
        .details(text(&t.title))
        .details_url(t.url.as_str())
        .timestamps(Timestamps::new().start(start).end(end))
        .assets(assets);
    if !t.artist.is_empty() {
        a = a.state(text(&t.artist));
        if !t.artist_url.is_empty() {
            a = a.state_url(t.artist_url.as_str());
        }
    }
    a
}

fn needs_update(shown: &Option<Shown>, want: Option<&Track>) -> bool {
    match (shown, want) {
        (None, None) => false,
        (Some(_), None) | (None, Some(_)) => true,
        (Some(s), Some(t)) => s.url != t.url || (s.start_ms - start_ms(t)).abs() > SEEK_TOLERANCE_MS,
    }
}

/// Sends one command and reads Discord's reply, so replies never pile up in the pipe.
fn send(client: &mut DiscordIpcClient, want: Option<&Track>) -> Result<(), discord_rich_presence::error::Error> {
    match want {
        Some(t) => client.set_activity(activity(t))?,
        None => client.clear_activity()?,
    }
    let (_, reply) = client.recv()?;
    if reply["evt"] == "ERROR" {
        eprintln!("discord rejected the activity: {}", reply["data"]);
    } else if cfg!(debug_assertions) {
        eprintln!("discord: {}", reply["data"]);
    }
    Ok(())
}

pub fn run(rx: Receiver<Update>, mut enabled: bool) {
    let mut client: Option<DiscordIpcClient> = None;
    let mut next_connect = Instant::now();
    let mut last_send: Option<Instant> = None;
    let mut latest: Option<Track> = None;
    let mut shown: Option<Shown> = None;
    let mut idle_since: Option<Instant> = None;

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Update::NowPlaying(t)) => latest = t,
            Ok(Update::Enabled(e)) => enabled = e,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        // Paused or stopped clears the status, like Spotify.
        let want = latest.as_ref().filter(|t| enabled && t.playing && t.duration > 0.0);
        idle_since = if want.is_some() { None } else { idle_since.or(Some(Instant::now())) };
        // Switching it off in the tray clears right away; a pause waits out track changes.
        let settling = enabled && idle_since.is_some_and(|t| t.elapsed() < CLEAR_AFTER);
        if !needs_update(&shown, want) || settling || last_send.is_some_and(|t| t.elapsed() < MIN_GAP) {
            continue;
        }

        if client.is_none() {
            if want.is_none() || Instant::now() < next_connect {
                continue;
            }
            let mut c = DiscordIpcClient::new(APP_ID);
            if c.connect().is_err() {
                // Discord isn't running (yet); try again later.
                next_connect = Instant::now() + RECONNECT_EVERY;
                continue;
            }
            client = Some(c);
        }

        let c = client.as_mut().unwrap();
        last_send = Some(Instant::now());
        match send(c, want) {
            Ok(()) => shown = want.map(|t| Shown { url: t.url.clone(), start_ms: start_ms(t) }),
            Err(_) => {
                // Discord was closed or restarted. Whatever it showed is gone with the connection.
                let _ = c.close();
                client = None;
                shown = None;
                next_connect = Instant::now() + RECONNECT_EVERY;
            }
        }
    }
}
