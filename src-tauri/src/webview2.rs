//! Direct WebView2 tweaks that Tauri doesn't expose: request blocking and hidden-window resource use.

use crate::blocklist;
use webview2_com::{
    take_pwstr, ExecuteScriptCompletedHandler, GetCookiesCompletedHandler,
    Microsoft::Web::WebView2::Win32::*, NavigationCompletedEventHandler,
    WebResourceRequestedEventHandler,
};
use windows_core::{Interface, BOOL, HSTRING, PWSTR};

/// How a top-level page load ended.
pub enum Load {
    Done,
    /// No connection, DNS failure, timeout... worth retrying.
    NetworkError,
    /// Cancelled (e.g. an external link we sent to the browser) or anything else not worth retrying.
    Other,
}

/// Blocks ad/tracker requests from every frame and worker, then loads `start_url`.
/// The webview is created on about:blank so nothing slips through before the filters exist.
pub unsafe fn install_blocker(
    controller: &ICoreWebView2Controller,
    env: &ICoreWebView2Environment,
    start_url: &str,
) -> windows_core::Result<()> {
    let core = controller.CoreWebView2()?;
    let core22 = core.cast::<ICoreWebView2_22>().ok();
    for f in blocklist::filters() {
        let f = HSTRING::from(f);
        match &core22 {
            Some(c) => c.AddWebResourceRequestedFilterWithRequestSourceKinds(
                &f,
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
            )?,
            None => core.AddWebResourceRequestedFilter(&f, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL)?,
        }
    }

    let env = env.clone();
    let mut token = 0i64;
    core.add_WebResourceRequested(
        &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let uri = {
                let mut uri = PWSTR::null();
                args.Request()?.Uri(&mut uri)?;
                take_pwstr(uri)
            };
            // Wry's own handler (Tauri's custom protocols) sees these events too; only touch our matches.
            if blocklist::is_blocked(&uri) {
                let response = env.CreateWebResourceResponse(None, 403, &HSTRING::from("Blocked"), &HSTRING::new())?;
                args.SetResponse(&response)?;
            }
            Ok(())
        })),
        &mut token,
    )?;

    core.Navigate(&HSTRING::from(start_url))
}

/// Runs `script` in the page and hands its JSON-encoded result to `done`.
/// Unlike Tauri's IPC, this needs no bridge the page itself could call.
pub unsafe fn eval_json(
    controller: &ICoreWebView2Controller,
    script: &str,
    done: impl FnOnce(String) + 'static,
) -> windows_core::Result<()> {
    controller.CoreWebView2()?.ExecuteScript(
        &HSTRING::from(script),
        &ExecuteScriptCompletedHandler::create(Box::new(move |result, json| {
            if result.is_ok() {
                done(json);
            }
            Ok(())
        })),
    )
}

/// Calls `on_load` after every full page load (SoundCloud's in-app navigation doesn't count).
pub unsafe fn on_page_load(
    controller: &ICoreWebView2Controller,
    on_load: impl Fn(Load) + 'static,
) -> windows_core::Result<()> {
    let mut token = 0i64;
    controller.CoreWebView2()?.add_NavigationCompleted(
        &NavigationCompletedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut ok = BOOL::default();
            args.IsSuccess(&mut ok)?;
            let load = if ok.as_bool() {
                Load::Done
            } else {
                let mut status = COREWEBVIEW2_WEB_ERROR_STATUS::default();
                args.WebErrorStatus(&mut status)?;
                match status {
                    COREWEBVIEW2_WEB_ERROR_STATUS_CANNOT_CONNECT
                    | COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED
                    | COREWEBVIEW2_WEB_ERROR_STATUS_DISCONNECTED
                    | COREWEBVIEW2_WEB_ERROR_STATUS_TIMEOUT
                    | COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_ABORTED
                    | COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_RESET
                    | COREWEBVIEW2_WEB_ERROR_STATUS_SERVER_UNREACHABLE => Load::NetworkError,
                    _ => Load::Other,
                }
            };
            on_load(load);
            Ok(())
        })),
        &mut token,
    )
}

/// Whether a cookie called `name` is stored for `uri`, including httpOnly ones the page can't see.
pub unsafe fn has_cookie(
    controller: &ICoreWebView2Controller,
    uri: &str,
    name: &'static str,
    done: impl FnOnce(bool) + 'static,
) -> windows_core::Result<()> {
    let cookies = controller.CoreWebView2()?.cast::<ICoreWebView2_2>()?.CookieManager()?;
    cookies.GetCookies(
        &HSTRING::from(uri),
        &GetCookiesCompletedHandler::create(Box::new(move |result, list| {
            let mut found = false;
            if let (Ok(()), Some(list)) = (result, list) {
                let mut count = 0u32;
                list.Count(&mut count)?;
                for i in 0..count {
                    let mut n = PWSTR::null();
                    list.GetValueAtIndex(i)?.Name(&mut n)?;
                    if take_pwstr(n) == name {
                        found = true;
                        break;
                    }
                }
            }
            done(found);
            Ok(())
        })),
    )
}

/// While the window sits in the tray: stop rendering and let WebView2 trim memory.
/// Audio keeps playing either way.
pub unsafe fn set_backgrounded(controller: &ICoreWebView2Controller, backgrounded: bool) -> windows_core::Result<()> {
    controller.SetIsVisible(!backgrounded)?;
    if let Ok(core19) = controller.CoreWebView2()?.cast::<ICoreWebView2_19>() {
        core19.SetMemoryUsageTargetLevel(if backgrounded {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
        } else {
            COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
        })?;
    }
    Ok(())
}
