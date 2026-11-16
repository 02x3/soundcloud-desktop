//! Ad, tracker and consent-banner hosts that SoundCloud's web app pulls in.
//! None of them are needed for browsing or playback.
//!
//! Never add dwt.soundcloud.com: it serves DataDome's bot-protection tag, and without it
//! SoundCloud's API rejects account actions like following.

/// Blocked along with all of their subdomains.
pub const HOSTS: &[&str] = &[
    // Google ads & analytics
    "doubleclick.net",
    "googlesyndication.com",
    "googletagservices.com",
    "googletagmanager.com",
    "googleadservices.com",
    "google-analytics.com",
    "analytics.google.com",
    "adtrafficquality.google",
    // Header bidding / ad exchanges
    "amazon-adsystem.com",
    "aditude.io",
    "aditude.cloud",
    "htlbid.com",
    "dn0qt3r0xannq.cloudfront.net",
    "prodregistryv2.org",
    "criteo.com",
    "criteo.net",
    "lijit.com",
    "33across.com",
    "tremorhub.com",
    "smartadserver.com",
    "taboola.com",
    "adnxs.com",
    "rubiconproject.com",
    "pubmatic.com",
    "openx.net",
    "casalemedia.com",
    "indexww.com",
    "sharethrough.com",
    "triplelift.com",
    "3lift.com",
    "yieldmo.com",
    "sonobi.com",
    "media.net",
    "teads.tv",
    "adsrvr.org",
    "id5-sync.com",
    "rlcdn.com",
    "bidswitch.net",
    "demdex.net",
    // Audio / video ad servers
    "fwmrm.net",
    "adswizz.com",
    "spotxchange.com",
    "springserve.com",
    // Ad verification
    "clean.gg",
    "script.ac",
    "moatads.com",
    "doubleverify.com",
    "adsafeprotected.com",
    // Tracking pixels
    "analytics.tiktok.com",
    "connect.facebook.net",
    "alb.reddit.com",
    "redditstatic.com",
    "quantserve.com",
    "quantcount.com",
    "scorecardresearch.com",
    "analytics.yahoo.com",
    "appsflyer.com",
    "appsflyersdk.com",
    "s10h.io",
    "ams-pageview-public.s3.amazonaws.com",
    "mpc-prod-25-s6uit34pua-wl.a.run.app",
    // Cookie banner (OneTrust)
    "cookielaw.org",
    "onetrust.com",
];

/// (host, path prefix) pairs. A host ending in '.' matches any host starting with it,
/// for Google's per-country domains (www.google.com.br, ...).
pub const PATHS: &[(&str, &str)] = &[
    ("www.google.", "/ads/"),
    ("www.google.", "/pagead/"),
    ("www.google.", "/ccm/"),
    ("www.gstatic.com", "/cv/"), // Chromecast sender SDK, unusable in WebView2
    ("www.gstatic.com", "/cast/"),
    ("www.gstatic.com", "/eureka/"),
    ("cdn.jsdelivr.net", "/npm/meta-capi-param-builder"),
];

/// Coarse WebView2 filters. Only matching requests reach `is_blocked`, so everything else
/// (audio streams, artwork, the app itself) never leaves the browser's network stack.
pub fn filters() -> Vec<String> {
    let mut out = Vec::with_capacity(HOSTS.len() * 2 + PATHS.len());
    for h in HOSTS {
        out.push(format!("*://{h}/*"));
        out.push(format!("*://*.{h}/*"));
    }
    for (h, p) in PATHS {
        let h = if h.ends_with('.') { format!("{h}*") } else { h.to_string() };
        out.push(format!("*://{h}{p}*"));
    }
    out
}

/// Exact check: the filters are wildcards, which can also match things like query strings.
pub fn is_blocked(uri: &str) -> bool {
    let Some(rest) = uri.split_once("://").map(|(_, r)| r) else {
        return false;
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(end);
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let host = host.split(':').next().unwrap_or(host).to_ascii_lowercase();

    let host_matches = |d: &str| host == d || (host.len() > d.len() && host.ends_with(d) && host.as_bytes()[host.len() - d.len() - 1] == b'.');
    if HOSTS.iter().any(|d| host_matches(d)) {
        return true;
    }
    PATHS.iter().any(|(h, p)| {
        let host_ok = if h.ends_with('.') { host.starts_with(h) } else { host == *h };
        host_ok && path.starts_with(p)
    })
}

#[cfg(test)]
mod tests {
    use super::is_blocked;

    #[test]
    fn blocks_ads_and_trackers() {
        assert!(is_blocked("https://securepubads.g.doubleclick.net/tag/js/gpt.js"));
        assert!(is_blocked("https://doubleclick.net/x"));
        assert!(is_blocked("https://www.google.com.br/ads/ga-audiences?v=1"));
        assert!(is_blocked("https://www.gstatic.com/cv/js/sender/v1/cast_sender.js"));
        assert!(is_blocked("https://cdn.cookielaw.org/consent/abc/otSDKStub.js"));
    }

    #[test]
    fn keeps_soundcloud_and_lookalikes() {
        assert!(!is_blocked("https://soundcloud.com/discover"));
        assert!(!is_blocked("https://api-v2.soundcloud.com/me"));
        assert!(!is_blocked("https://cf-hls-media.sndcdn.com/playlist/x.m3u8"));
        assert!(!is_blocked("https://www.google.com/recaptcha/api2/aframe"));
        assert!(!is_blocked("https://accounts.google.com/gsi/client"));
        assert!(!is_blocked("https://notdoubleclick.net/"));
        assert!(!is_blocked("https://soundcloud.com/search?q=doubleclick.net/"));
        assert!(!is_blocked("https://cdn.jsdelivr.net/npm/other-lib"));
        assert!(!is_blocked("https://dwt.soundcloud.com/tags.js"));
    }
}
