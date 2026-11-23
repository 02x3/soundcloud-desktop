//! Taskbar thumbnail toolbar: like / previous / play-pause / next buttons under the window preview
//! that appears when hovering the app's taskbar button, like Spotify's.

use std::cell::RefCell;
use std::sync::OnceLock;
use windows::core::{w, Result};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::{
    DefSubclassProc, ITaskbarList3, SetWindowSubclass, TaskbarList, THBF_ENABLED, THBN_CLICKED, THB_FLAGS,
    THB_ICON, THB_TOOLTIP, THUMBBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, GetSystemMetrics, RegisterWindowMessageW, HICON, ICONINFO, SM_CXSMICON, WM_COMMAND,
};

/// Button ids, in the order they appear (Spotify's layout).
pub const LIKE: u32 = 0;
pub const PREV: u32 = 1;
pub const PLAY_PAUSE: u32 = 2;
pub const NEXT: u32 = 3;

struct Icons {
    heart: HICON,
    heart_filled: HICON,
    prev: HICON,
    play: HICON,
    pause: HICON,
    next: HICON,
}

struct Bar {
    taskbar: ITaskbarList3,
    hwnd: HWND,
    icons: Icons,
    playing: bool,
    liked: bool,
}

// Everything here runs on the UI thread, which owns the window.
thread_local! {
    static BAR: RefCell<Option<Bar>> = const { RefCell::new(None) };
}
static ON_CLICK: OnceLock<fn(u32)> = OnceLock::new();
static BUTTON_CREATED_MSG: OnceLock<u32> = OnceLock::new();

#[derive(Clone, Copy)]
enum Shape {
    Heart,
    HeartFilled,
    Prev,
    Play,
    Pause,
    Next,
}

/// Call on the UI thread before the window is first shown: the buttons are added when
/// Windows creates the taskbar button (and again if Explorer restarts).
pub fn install(hwnd: HWND, on_click: fn(u32)) -> Result<()> {
    let _ = ON_CLICK.set(on_click);
    unsafe {
        let _ = BUTTON_CREATED_MSG.set(RegisterWindowMessageW(w!("TaskbarButtonCreated")));
        let taskbar: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)?;
        taskbar.HrInit()?;
        let size = GetSystemMetrics(SM_CXSMICON).max(16);
        let icons = Icons {
            heart: make_icon(size, Shape::Heart)?,
            heart_filled: make_icon(size, Shape::HeartFilled)?,
            prev: make_icon(size, Shape::Prev)?,
            play: make_icon(size, Shape::Play)?,
            pause: make_icon(size, Shape::Pause)?,
            next: make_icon(size, Shape::Next)?,
        };
        BAR.with(|b| *b.borrow_mut() = Some(Bar { taskbar, hwnd, icons, playing: false, liked: false }));
        let _ = SetWindowSubclass(hwnd, Some(subclass_proc), 1, 0);
    }
    Ok(())
}

/// Keeps the play/pause and heart icons in step with the player.
pub fn set_state(playing: bool, liked: bool) {
    BAR.with(|b| {
        if let Some(bar) = b.borrow_mut().as_mut() {
            if bar.playing != playing || bar.liked != liked {
                bar.playing = playing;
                bar.liked = liked;
                let _ = unsafe { bar.taskbar.ThumbBarUpdateButtons(bar.hwnd, &buttons(bar)) };
            }
        }
    });
}

fn add_buttons() {
    BAR.with(|b| {
        if let Some(bar) = b.borrow().as_ref() {
            let _ = unsafe { bar.taskbar.ThumbBarAddButtons(bar.hwnd, &buttons(bar)) };
        }
    });
}

fn buttons(bar: &Bar) -> [THUMBBUTTON; 4] {
    let i = &bar.icons;
    [
        button(LIKE, if bar.liked { i.heart_filled } else { i.heart }, if bar.liked { "Unlike" } else { "Like" }),
        button(PREV, i.prev, "Previous"),
        button(PLAY_PAUSE, if bar.playing { i.pause } else { i.play }, if bar.playing { "Pause" } else { "Play" }),
        button(NEXT, i.next, "Next"),
    ]
}

fn button(id: u32, icon: HICON, tip: &str) -> THUMBBUTTON {
    let mut b = THUMBBUTTON {
        dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
        iId: id,
        hIcon: icon,
        dwFlags: THBF_ENABLED,
        ..Default::default()
    };
    for (slot, c) in b.szTip.iter_mut().zip(tip.encode_utf16().take(259)) {
        *slot = c;
    }
    b
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if Some(&msg) == BUTTON_CREATED_MSG.get() {
        add_buttons();
    } else if msg == WM_COMMAND && (wparam.0 >> 16) as u32 & 0xffff == THBN_CLICKED {
        if let Some(on_click) = ON_CLICK.get() {
            on_click((wparam.0 & 0xffff) as u32);
        }
        return LRESULT(0);
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

/// Draws a white glyph with 4x4 supersampled edges into a 32-bit icon.
unsafe fn make_icon(size: i32, shape: Shape) -> Result<HICON> {
    const SS: i32 = 4;
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size, // top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let color = unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)? };
    let pixels = unsafe { std::slice::from_raw_parts_mut(bits as *mut u32, (size * size) as usize) };
    for py in 0..size {
        for px in 0..size {
            let mut hits = 0;
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32) / size as f32;
                    hits += covered(shape, x, y) as i32;
                }
            }
            let alpha = (hits * 255 / (SS * SS)) as u32;
            pixels[(py * size + px) as usize] = (alpha << 24) | 0x00FF_FFFF;
        }
    }
    let mask = unsafe { CreateBitmap(size, size, 1, 1, None) };
    let icon = unsafe {
        CreateIconIndirect(&ICONINFO { fIcon: true.into(), hbmMask: mask, hbmColor: color, ..Default::default() })
    };
    unsafe {
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
    }
    icon
}

/// Whether point (x, y) in the unit square is inside the glyph.
fn covered(shape: Shape, x: f32, y: f32) -> bool {
    let bar = |x0: f32, x1: f32| x >= x0 && x <= x1 && (0.2..=0.8).contains(&y);
    match shape {
        Shape::Play => triangle((x, y), (0.3, 0.18), (0.3, 0.82), (0.82, 0.5)),
        Shape::Pause => bar(0.27, 0.43) || bar(0.57, 0.73),
        Shape::Next => triangle((x, y), (0.2, 0.2), (0.2, 0.8), (0.62, 0.5)) || bar(0.65, 0.79),
        Shape::Prev => covered(Shape::Next, 1.0 - x, y),
        Shape::HeartFilled => heart(x, y, 1.0),
        Shape::Heart => heart(x, y, 1.0) && !heart(x, y, 0.6),
    }
}

fn triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let side = |p: (f32, f32), q: (f32, f32), r: (f32, f32)| (p.0 - r.0) * (q.1 - r.1) - (q.0 - r.0) * (p.1 - r.1);
    let (d1, d2, d3) = (side(p, a, b), side(p, b, c), side(p, c, a));
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

/// The classic heart curve (x² + y² − 1)³ − x²y³ ≤ 0, fitted to the square; `scale` shrinks it
/// around its middle for the outline's inner edge.
fn heart(x: f32, y: f32, scale: f32) -> bool {
    let hx = (x - 0.5) * 2.6 / scale;
    let hy = ((0.55 - y) * 2.6 - 0.12) / scale + 0.12;
    let r = hx * hx + hy * hy - 1.0;
    r * r * r - hx * hx * hy * hy * hy <= 0.0
}
