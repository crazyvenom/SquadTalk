//! The SquadTalk window.
//!
//! Plain Win32 controls with custom painting for a dark gaming look.
//! No browser engine and no GPU, so it stays tiny while you play.

use crate::audio::{self, AudioEngine, Mixer};
use crate::client::{self, Event, Net};
use crate::config::{self, cfg};
use crate::protocol::{self, lk, UserInfo, C2S};
use crate::server::{self, ServerHandle};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontIndirectW, CreatePen,
    CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect, GetDC, GetDeviceCaps,
    InvalidateRect, ReleaseDC, RoundRect, SelectObject, SetBkColor, SetBkMode, SetTextColor, HDC,
    LOGFONTW, PAINTSTRUCT,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::UI::Controls::{
    InitCommonControlsEx, SetWindowTheme, DRAWITEMSTRUCT, INITCOMMONCONTROLSEX, MEASUREITEMSTRUCT,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetAsyncKeyState, GetKeyNameTextW, MapVirtualKeyW, ReleaseCapture, SetCapture, SetFocus,
    TrackMouseEvent, TRACKMOUSEEVENT,
};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, DrawIconEx,
    GetClientRect, GetMessageW, GetParent, GetWindowTextLengthW, GetWindowTextW, KillTimer, LoadCursorW,
    LoadImageW, MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassW, SendMessageW, SetProcessDPIAware,
    SetTimer, SetWindowTextW, ShowWindow, TranslateMessage, HICON, IDC_ARROW, MSG, WNDCLASSW,
};

// ===================== Win32 numbers =====================
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const WS_CLIPCHILDREN: u32 = 0x0200_0000;
const WS_MAIN: u32 = 0x00CA_0000; // overlapped + caption + system menu + minimize
const WS_DIALOG: u32 = 0x00C8_0000; // caption + system menu
const ES_PASSWORD: u32 = 0x20;
const ES_AUTOVSCROLL: u32 = 0x40;
const ES_AUTOHSCROLL: u32 = 0x80;
const ES_MULTILINE: u32 = 0x4;
const ES_READONLY: u32 = 0x800;
const ES_NUMBER: u32 = 0x2000;
const BS_OWNERDRAW: u32 = 0xB;
const LBS_NOTIFY: u32 = 1;
const LBS_OWNERDRAWFIXED: u32 = 0x10;
const LBS_NOINTEGRALHEIGHT: u32 = 0x100;
const LB_ADDSTRING: u32 = 0x180;
const LB_RESETCONTENT: u32 = 0x184;
const LB_SETCURSEL: u32 = 0x186;
const LB_GETCURSEL: u32 = 0x188;
const LB_GETTOPINDEX: u32 = 0x18E;
const LB_SETTOPINDEX: u32 = 0x197;
const LBN_SELCHANGE: u32 = 1;
const LBN_DBLCLK: u32 = 2;
const CBS_DROPDOWNLIST: u32 = 3;
const CB_ADDSTRING: u32 = 0x143;
const CB_GETCURSEL: u32 = 0x147;
const CB_RESETCONTENT: u32 = 0x14B;
const CB_SETCURSEL: u32 = 0x14E;
const EM_SETSEL: u32 = 0xB1;
const EM_SCROLLCARET: u32 = 0xB7;
const EM_REPLACESEL: u32 = 0xC2;
const EM_SETLIMITTEXT: u32 = 0xC5;
const EM_SETMARGINS: u32 = 0xD3;
const EN_SETFOCUS: u32 = 0x100;
const EN_KILLFOCUS: u32 = 0x200;
const WM_DESTROY: u32 = 0x02;
const WM_PAINT: u32 = 0x0F;
const WM_CLOSE: u32 = 0x10;
const WM_ERASEBKGND: u32 = 0x14;
const WM_SETREDRAW: u32 = 0x0B;
const WM_DRAWITEM: u32 = 0x2B;
const WM_MEASUREITEM: u32 = 0x2C;
const WM_SETFONT: u32 = 0x30;
const WM_SETICON: u32 = 0x80;
const WM_KEYDOWN: u32 = 0x100;
const WM_COMMAND: u32 = 0x111;
const WM_TIMER: u32 = 0x113;
const WM_CTLCOLOREDIT: u32 = 0x133;
const WM_CTLCOLORLISTBOX: u32 = 0x134;
const WM_CTLCOLORSTATIC: u32 = 0x138;
const WM_MOUSEMOVE: u32 = 0x200;
const WM_LBUTTONDOWN: u32 = 0x201;
const WM_LBUTTONUP: u32 = 0x202;
const WM_MOUSELEAVE: u32 = 0x2A3;
const WM_APP_NOTIFY: u32 = 0x8001;
const WM_APP_SLIDER: u32 = 0x8002;
const WM_APP_SLIDER_END: u32 = 0x8003;
const VK_RETURN: usize = 0x0D;
const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const CW_USEDEFAULT: i32 = i32::MIN;
const MB_ICONWARNING: u32 = 0x30;
const ODT_LISTBOX: u32 = 2;
const ODT_BUTTON: u32 = 4;
const ODS_SELECTED: u32 = 1;
const ODS_DISABLED: u32 = 4;
const DT_CENTER: u32 = 0x1;
const DT_RIGHT: u32 = 0x2;
const DT_VCENTER: u32 = 0x4;
const DT_WORDBREAK: u32 = 0x10;
const DT_SINGLELINE: u32 = 0x20;
const DT_CALCRECT: u32 = 0x400;
const DT_NOPREFIX: u32 = 0x800;
const DT_END_ELLIPSIS: u32 = 0x8000;
const LINE: u32 = DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_END_ELLIPSIS;

// ===================== Control IDs =====================
const ID_JOIN: u16 = 101;
const ID_HOST: u16 = 102;
const ID_SETTINGS_C: u16 = 103;
const ID_LIST: u16 = 201;
const ID_JOINROOM: u16 = 202;
const ID_SEND: u16 = 203;
const ID_MUTE: u16 = 204;
const ID_DEAF: u16 = 205;
const ID_SETTINGS: u16 = 206;
const ID_DISCONNECT: u16 = 207;
const ID_KICK: u16 = 208;
const ID_SMUTE: u16 = 209;
const ID_DELROOM: u16 = 210;
const ID_MOVE: u16 = 211;
const ID_ADDROOM: u16 = 212;
const ID_S_PTT: u16 = 303;
const ID_S_VA: u16 = 304;
const ID_S_KEY: u16 = 305;
const ID_S_BEEP: u16 = 307;
const ID_S_JOIN: u16 = 308;
const ID_S_SAVE: u16 = 309;
const ID_S_CANCEL: u16 = 310;

const TIMER_TICK: usize = 1;
const TIMER_CAPTURE: usize = 2;

// ===================== Icons (Segoe MDL2 Assets) =====================
const IC_MIC: u32 = 0xE720;
const IC_MIC_OFF: u32 = 0xEC54;
const IC_HEADPHONES: u32 = 0xE7F6;
const IC_SETTINGS: u32 = 0xE713;
const IC_HANGUP: u32 = 0xE778;
const IC_PHONE: u32 = 0xE717;
const IC_SEND: u32 = 0xE724;
const IC_PEOPLE: u32 = 0xE716;
const IC_CHAT: u32 = 0xE8BD;
const IC_STAR: u32 = 0xE735;
const IC_SPEAKER_OFF: u32 = 0xE74F;
const IC_SHIELD: u32 = 0xEA18;
const IC_ADD: u32 = 0xE710;
const IC_DELETE: u32 = 0xE74D;
const IC_FORWARD: u32 = 0xE72A;
const IC_CANCEL: u32 = 0xE711;
const IC_KEYBOARD: u32 = 0xE765;
const IC_SAVE: u32 = 0xE74E;
const IC_PC: u32 = 0xE7F4;
const IC_ENTER: u32 = 0xE8AF;

// ===================== Colours =====================
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}
const C_BG: u32 = rgb(11, 15, 26);
const C_BG2: u32 = rgb(17, 23, 40);
const C_HEADER: u32 = rgb(7, 10, 18);
const C_PANEL: u32 = rgb(20, 25, 42);
const C_PANEL_BORDER: u32 = rgb(34, 41, 66);
const C_FIELD: u32 = rgb(27, 33, 54);
const C_FIELD_HOVER: u32 = rgb(36, 44, 70);
const C_FIELD_BORDER: u32 = rgb(46, 54, 86);
const C_TEXT: u32 = rgb(232, 236, 245);
const C_MUTED: u32 = rgb(140, 149, 176);
const C_DIM: u32 = rgb(94, 102, 130);
const C_ACCENT: u32 = rgb(34, 146, 208);
const C_ACCENT2: u32 = rgb(42, 171, 225);
const C_ACCENT_DARK: u32 = rgb(24, 108, 158);
const C_GREEN: u32 = rgb(61, 220, 132);
const C_GREEN_DARK: u32 = rgb(16, 56, 40);
const C_RED: u32 = rgb(229, 72, 77);
const C_RED_SOFT: u32 = rgb(255, 110, 110);
const C_RED_DARK: u32 = rgb(58, 26, 34);
const C_AMBER: u32 = rgb(255, 190, 80);
const C_SEL: u32 = rgb(30, 46, 82);
const C_ROOM: u32 = rgb(26, 32, 52);
const C_ROOM_CUR: u32 = rgb(20, 46, 76);
const C_WHITE: u32 = rgb(255, 255, 255);
const AVATAR: [u32; 8] = [
    rgb(124, 92, 255),
    rgb(255, 122, 89),
    rgb(46, 196, 182),
    rgb(240, 160, 40),
    rgb(239, 71, 111),
    rgb(76, 175, 80),
    rgb(33, 150, 243),
    rgb(171, 71, 188),
];

fn lerp(a: u32, b: u32, t: f32) -> u32 {
    let ch = |s: u32| -> (f32, f32) { (((a >> s) & 0xFF) as f32, ((b >> s) & 0xFF) as f32) };
    let mut out = 0u32;
    for s in [0u32, 8, 16] {
        let (x, y) = ch(s);
        out |= ((x + (y - x) * t) as u32 & 0xFF) << s;
    }
    out
}

// ===================== Theme: DPI, fonts, brushes =====================
static DPI: AtomicI32 = AtomicI32::new(96);
static ROOM_LIST: AtomicUsize = AtomicUsize::new(0);

fn s(v: i32) -> i32 {
    v * DPI.load(Ordering::Relaxed) / 96
}
fn rc(x: i32, y: i32, w: i32, h: i32) -> RECT {
    RECT { left: s(x), top: s(y), right: s(x + w), bottom: s(y + h) }
}

struct Theme {
    panel_brush: usize,
    field_brush: usize,
    body: usize,
    small: usize,
    bold: usize,
    title: usize,
    head: usize,
    key: usize,
    icon: usize,
    icon_big: usize,
    icon_small: usize,
    app_icon: usize,
}
static THEME: OnceLock<Theme> = OnceLock::new();
fn th() -> &'static Theme {
    THEME.get().expect("theme is created at startup")
}

unsafe fn make_font(face: &str, tenth_pt: i32, weight: i32) -> usize {
    let mut lf: LOGFONTW = std::mem::zeroed();
    lf.lfHeight = -(tenth_pt * DPI.load(Ordering::Relaxed) / 720);
    lf.lfWeight = weight;
    lf.lfCharSet = 1; // DEFAULT_CHARSET
    lf.lfQuality = 5; // CLEARTYPE_QUALITY
    for (i, c) in face.encode_utf16().take(31).enumerate() {
        lf.lfFaceName[i] = c;
    }
    CreateFontIndirectW(&lf) as usize
}

unsafe fn init_theme(hinst: *mut c_void) {
    let icon = LoadImageW(hinst, 1usize as *const u16, 1, s(32), s(32), 0);
    let _ = THEME.set(Theme {
        panel_brush: CreateSolidBrush(C_PANEL) as usize,
        field_brush: CreateSolidBrush(C_FIELD) as usize,
        body: make_font("Segoe UI", 100, 400),
        small: make_font("Segoe UI", 90, 400),
        bold: make_font("Segoe UI Semibold", 100, 600),
        title: make_font("Segoe UI Semibold", 150, 600),
        head: make_font("Segoe UI", 85, 700),
        key: make_font("Segoe UI Semibold", 105, 600),
        icon: make_font("Segoe MDL2 Assets", 120, 400),
        icon_big: make_font("Segoe MDL2 Assets", 150, 400),
        icon_small: make_font("Segoe MDL2 Assets", 95, 400),
        app_icon: icon as usize,
    });
}

/// Dark title bar and dark scrollbars (Windows 10 1903+ / Windows 11).
unsafe fn enable_dark_mode() {
    let lib = LoadLibraryW(wide("uxtheme.dll").as_ptr());
    if lib.is_null() {
        return;
    }
    // Ordinal 135 = SetPreferredAppMode(ForceDark).
    if let Some(f) = GetProcAddress(lib, 135usize as *const u8) {
        let set_mode: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(f);
        set_mode(2);
    }
}

unsafe fn dark_title_bar(hwnd: HWND) {
    let on: i32 = 1;
    DwmSetWindowAttribute(hwnd, 20, &on as *const i32 as *const c_void, 4);
    let caption = C_HEADER;
    DwmSetWindowAttribute(hwnd, 35, &caption as *const u32 as *const c_void, 4);
    let text = C_TEXT;
    DwmSetWindowAttribute(hwnd, 36, &text as *const u32 as *const c_void, 4);
}

unsafe fn dark_control(h: HWND, combo: bool) {
    let theme = wide(if combo { "DarkMode_CFD" } else { "DarkMode_Explorer" });
    SetWindowTheme(h, theme.as_ptr(), null());
}

// ===================== Small helpers =====================
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
fn sendm(h: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    unsafe { SendMessageW(h, msg, wp, lp) }
}
fn set_text(h: HWND, s: &str) {
    let w = wide(s);
    unsafe {
        SetWindowTextW(h, w.as_ptr());
    }
}
fn get_text(h: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(h);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(h, buf.as_mut_ptr(), len + 1);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
}
fn show(h: HWND, visible: bool) {
    unsafe {
        ShowWindow(h, if visible { SW_SHOW } else { SW_HIDE });
    }
}
fn enable(h: HWND, on: bool) {
    unsafe {
        EnableWindow(h, on as i32);
    }
}
fn invalidate(h: HWND, r: Option<RECT>) {
    unsafe {
        match r {
            Some(r) => InvalidateRect(h, &r, 0),
            None => InvalidateRect(h, null(), 0),
        };
    }
}
fn popup(owner: HWND, text: &str) {
    let t = wide(text);
    let c = wide("SquadTalk");
    unsafe {
        MessageBoxW(owner, t.as_ptr(), c.as_ptr(), MB_ICONWARNING);
    }
}

pub fn key_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

pub fn key_name(vk: i32) -> String {
    match vk {
        0x01 => return "Left mouse".into(),
        0x02 => return "Right mouse".into(),
        0x04 => return "Middle mouse".into(),
        0x05 => return "Mouse 4".into(),
        0x06 => return "Mouse 5".into(),
        0xC0 => return "`".into(),
        0xA0 => return "Left Shift".into(),
        0xA1 => return "Right Shift".into(),
        0xA2 => return "Left Ctrl".into(),
        0xA3 => return "Right Ctrl".into(),
        0xA4 => return "Left Alt".into(),
        0xA5 => return "Right Alt".into(),
        _ => {}
    }
    unsafe {
        let scan = MapVirtualKeyW(vk as u32, 0);
        let extended = matches!(vk, 0x21..=0x2E | 0x5B | 0x5C | 0x6F | 0x90);
        let lparam = ((scan as i32) << 16) | if extended { 1 << 24 } else { 0 };
        let mut buf = [0u16; 64];
        let n = GetKeyNameTextW(lparam, buf.as_mut_ptr(), 64);
        if n > 0 {
            String::from_utf16_lossy(&buf[..n as usize])
        } else {
            format!("Key #{vk}")
        }
    }
}

/// This PC's address on the home network (Wi-Fi / LAN).
fn lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:53").ok()?; // no data is sent; this just picks the right network card
    match s.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_unspecified() => Some(v4.to_string()),
        _ => None,
    }
}

// ===================== Drawing helpers =====================
unsafe fn fill(hdc: HDC, r: RECT, color: u32) {
    let b = CreateSolidBrush(color);
    FillRect(hdc, &r, b);
    DeleteObject(b);
}

unsafe fn rrect(hdc: HDC, r: RECT, radius: i32, fill_color: u32, border: Option<u32>) {
    let b = CreateSolidBrush(fill_color);
    let p = match border {
        Some(c) => CreatePen(0, 1, c),
        None => CreatePen(5, 0, 0), // PS_NULL
    };
    let ob = SelectObject(hdc, b);
    let op = SelectObject(hdc, p);
    let extra = if border.is_none() { 1 } else { 0 };
    RoundRect(hdc, r.left, r.top, r.right + extra, r.bottom + extra, radius * 2, radius * 2);
    SelectObject(hdc, ob);
    SelectObject(hdc, op);
    DeleteObject(b);
    DeleteObject(p);
}

unsafe fn circle(hdc: HDC, cx: i32, cy: i32, radius: i32, color: u32) {
    rrect(hdc, RECT { left: cx - radius, top: cy - radius, right: cx + radius, bottom: cy + radius }, radius, color, None);
}

unsafe fn text(hdc: HDC, s: &str, r: RECT, font: usize, color: u32, flags: u32) {
    let w: Vec<u16> = s.encode_utf16().collect();
    let mut r = r;
    SelectObject(hdc, font as _);
    SetTextColor(hdc, color);
    SetBkMode(hdc, 1);
    DrawTextW(hdc, w.as_ptr() as _, w.len() as i32, &mut r, flags);
}

unsafe fn text_width(hdc: HDC, s: &str, font: usize) -> i32 {
    let w: Vec<u16> = s.encode_utf16().collect();
    let mut r = RECT { left: 0, top: 0, right: 10000, bottom: 100 };
    SelectObject(hdc, font as _);
    DrawTextW(hdc, w.as_ptr() as _, w.len() as i32, &mut r, DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX);
    r.right - r.left
}

fn glyph_str(code: u32) -> String {
    char::from_u32(code).map(|c| c.to_string()).unwrap_or_default()
}

unsafe fn glyph(hdc: HDC, code: u32, r: RECT, font: usize, color: u32) {
    text(hdc, &glyph_str(code), r, font, color, DT_CENTER | LINE);
}

unsafe fn section_title(hdc: HDC, icon: u32, title: &str, x: i32, y: i32) {
    glyph(hdc, icon, rc(x, y, 20, 20), th().icon_small, C_ACCENT2);
    text(hdc, title, rc(x + 24, y, 260, 20), th().head, C_MUTED, LINE);
}

unsafe fn divider(hdc: HDC, x: i32, y: i32, w: i32) {
    fill(hdc, RECT { left: s(x), top: s(y), right: s(x + w), bottom: s(y) + 1 }, C_PANEL_BORDER);
}

/// Background with a subtle vertical gradient.
unsafe fn backdrop(hdc: HDC, top: i32, w: i32, h: i32) {
    let bands = 32;
    let span = (h - top).max(1);
    for i in 0..bands {
        let y0 = top + span * i / bands;
        let y1 = top + span * (i + 1) / bands;
        let c = lerp(C_BG, C_BG2, i as f32 / (bands - 1) as f32);
        fill(hdc, RECT { left: 0, top: y0, right: w, bottom: y1 }, c);
    }
}

/// Paints into an off-screen bitmap first, so nothing flickers.
unsafe fn buffered_paint(hwnd: HWND, paint: impl FnOnce(HDC, i32, i32) -> bool) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut r: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut r);
    let (w, h) = (r.right.max(1), r.bottom.max(1));
    let mem = CreateCompatibleDC(hdc);
    let bmp = CreateCompatibleBitmap(hdc, w, h);
    let old = SelectObject(mem, bmp);
    if !paint(mem, w, h) {
        fill(mem, r, C_BG);
    }
    BitBlt(hdc, 0, 0, w, h, mem, 0, 0, 0x00CC_0020);
    SelectObject(mem, old);
    DeleteObject(bmp);
    DeleteDC(mem);
    EndPaint(hwnd, &ps);
}

// ===================== Custom buttons =====================
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Primary,
    Secondary,
    Danger,
    Ghost,
    Round,
    RoundDanger,
    Segment,
    Toggle,
    Square,
}

#[derive(Clone, Copy)]
struct Btn {
    kind: Kind,
    icon: Option<u32>,
    active: bool,
    bg: u32,
}

thread_local! {
    static BTNS: RefCell<HashMap<usize, Btn>> = RefCell::new(HashMap::new());
    static HOVER: Cell<usize> = const { Cell::new(0) };
    static ROWS: RefCell<Vec<Row>> = const { RefCell::new(Vec::new()) };
    static SLIDERS: RefCell<HashMap<usize, Slider>> = RefCell::new(HashMap::new());
}

fn set_active(h: HWND, on: bool) {
    let changed = BTNS.with(|b| match b.borrow_mut().get_mut(&(h as usize)) {
        Some(x) if x.active != on => {
            x.active = on;
            true
        }
        _ => false,
    });
    if changed {
        invalidate(h, None);
    }
}
fn is_active(h: HWND) -> bool {
    BTNS.with(|b| b.borrow().get(&(h as usize)).map(|x| x.active).unwrap_or(false))
}

unsafe extern "system" fn hover_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
    match msg {
        WM_MOUSEMOVE => {
            if HOVER.with(|h| h.get()) != hwnd as usize {
                let old = HOVER.with(|h| h.replace(hwnd as usize));
                if old != 0 {
                    InvalidateRect(old as HWND, null(), 0);
                }
                InvalidateRect(hwnd, null(), 0);
                let mut t = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: 2, // TME_LEAVE
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut t);
            }
        }
        WM_MOUSELEAVE => {
            if HOVER.with(|h| h.get()) == hwnd as usize {
                HOVER.with(|h| h.set(0));
                InvalidateRect(hwnd, null(), 0);
            }
        }
        WM_ERASEBKGND => return 1,
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

unsafe fn draw_button(d: &DRAWITEMSTRUCT) {
    let h = d.hwndItem;
    let info = BTNS.with(|b| b.borrow().get(&(h as usize)).copied()).unwrap_or(Btn {
        kind: Kind::Secondary,
        icon: None,
        active: false,
        bg: C_BG,
    });
    let hdc = d.hDC;
    let r = d.rcItem;
    let pressed = d.itemState & ODS_SELECTED != 0;
    let disabled = d.itemState & ODS_DISABLED != 0;
    let hover = HOVER.with(|x| x.get()) == h as usize && !disabled;
    let label = get_text(h);
    let t = th();
    fill(hdc, r, info.bg);

    let (w, hgt) = (r.right - r.left, r.bottom - r.top);
    let (fill_c, border, fg, font) = match info.kind {
        Kind::Primary => (
            if disabled { C_ACCENT_DARK } else if pressed { C_ACCENT_DARK } else if hover { C_ACCENT2 } else { C_ACCENT },
            None,
            if disabled { C_MUTED } else { C_WHITE },
            t.bold,
        ),
        Kind::Secondary => (
            if pressed { C_PANEL } else if hover { C_FIELD_HOVER } else { C_FIELD },
            Some(if hover { C_ACCENT_DARK } else { C_FIELD_BORDER }),
            if disabled { C_DIM } else { C_TEXT },
            t.body,
        ),
        Kind::Danger => (
            if pressed { rgb(48, 22, 28) } else if hover { rgb(84, 34, 44) } else { C_RED_DARK },
            Some(rgb(96, 40, 52)),
            if disabled { C_DIM } else { C_RED_SOFT },
            t.body,
        ),
        Kind::Ghost => (info.bg, None, if hover { C_ACCENT2 } else { C_MUTED }, t.small),
        Kind::Segment => (
            if info.active { C_ACCENT } else if hover { C_FIELD_HOVER } else { C_FIELD },
            if info.active { None } else { Some(C_FIELD_BORDER) },
            if info.active { C_WHITE } else { C_MUTED },
            t.bold,
        ),
        Kind::Square => (
            if pressed { C_ACCENT_DARK } else if hover { C_ACCENT2 } else { C_ACCENT },
            None,
            C_WHITE,
            t.icon,
        ),
        _ => (C_FIELD, None, C_TEXT, t.body),
    };

    match info.kind {
        Kind::Round | Kind::RoundDanger => {
            let (c, ic) = match (info.kind, info.active) {
                (Kind::RoundDanger, _) => (if pressed { rgb(170, 40, 46) } else if hover { rgb(245, 95, 100) } else { C_RED }, C_WHITE),
                (_, true) => (if hover { rgb(110, 38, 50) } else { rgb(90, 31, 40) }, C_RED_SOFT),
                _ => (if pressed { C_PANEL } else if hover { C_FIELD_HOVER } else { C_FIELD }, C_TEXT),
            };
            let rad = w.min(hgt) / 2;
            circle(hdc, r.left + w / 2, r.top + hgt / 2, rad, c);
            if let Some(code) = info.icon {
                glyph(hdc, code, r, t.icon_big, ic);
            }
            return;
        }
        Kind::Toggle => {
            let track = if info.active { C_ACCENT } else { rgb(44, 52, 84) };
            rrect(hdc, r, hgt / 2, track, None);
            let knob_r = hgt / 2 - s(4);
            let cx = if info.active { r.right - hgt / 2 } else { r.left + hgt / 2 };
            circle(hdc, cx, r.top + hgt / 2, knob_r, C_WHITE);
            return;
        }
        Kind::Ghost => {}
        _ => rrect(hdc, r, s(8), fill_c, border),
    }

    // Icon + label, centred together.
    let gap = s(8);
    let icon_w = if info.icon.is_some() { s(18) } else { 0 };
    let label_w = if label.is_empty() { 0 } else { text_width(hdc, &label, font) };
    let total = icon_w + if icon_w > 0 && label_w > 0 { gap } else { 0 } + label_w;
    let mut x = r.left + (w - total).max(0) / 2;
    if let Some(code) = info.icon {
        let icon_font = if info.kind == Kind::Square { t.icon } else { t.icon_small };
        glyph(hdc, code, RECT { left: x, top: r.top, right: x + icon_w, bottom: r.bottom }, icon_font, fg);
        x += icon_w + gap;
    }
    if label_w > 0 {
        text(hdc, &label, RECT { left: x, top: r.top, right: r.right, bottom: r.bottom }, font, fg, LINE);
    }
}

// ===================== Custom slider =====================
struct Slider {
    min: i32,
    max: i32,
    value: i32,
    enabled: bool,
    drag: bool,
    bg: u32,
}

fn slider_set(h: HWND, v: i32) {
    SLIDERS.with(|m| {
        if let Some(sl) = m.borrow_mut().get_mut(&(h as usize)) {
            sl.value = v.clamp(sl.min, sl.max);
        }
    });
    invalidate(h, None);
}
fn slider_get(h: HWND) -> i32 {
    SLIDERS.with(|m| m.borrow().get(&(h as usize)).map(|s| s.value).unwrap_or(0))
}
fn slider_enable(h: HWND, on: bool) {
    let changed = SLIDERS.with(|m| match m.borrow_mut().get_mut(&(h as usize)) {
        Some(sl) if sl.enabled != on => {
            sl.enabled = on;
            true
        }
        _ => false,
    });
    if changed {
        invalidate(h, None);
    }
}

unsafe extern "system" fn slider_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let key = hwnd as usize;
    let pad = s(10);
    let mut client: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut client);
    let width = (client.right - 2 * pad).max(1);
    let x_of = |lp: LPARAM| (lp & 0xFFFF) as u16 as i16 as i32;
    let set_from_x = |x: i32| -> Option<i32> {
        SLIDERS.with(|m| {
            let mut m = m.borrow_mut();
            let sl = m.get_mut(&key)?;
            let v = sl.min + ((x - pad).clamp(0, width) * (sl.max - sl.min) + width / 2) / width;
            if v != sl.value {
                sl.value = v;
                Some(v)
            } else {
                None
            }
        })
    };
    match msg {
        WM_PAINT => {
            let st = SLIDERS.with(|m| m.borrow().get(&key).map(|s| (s.min, s.max, s.value, s.enabled, s.bg)));
            buffered_paint(hwnd, |hdc, w, h| {
                let Some((min, max, value, enabled, bg)) = st else { return false };
                fill(hdc, RECT { left: 0, top: 0, right: w, bottom: h }, bg);
                let cy = h / 2;
                let track = RECT { left: pad, top: cy - s(3), right: w - pad, bottom: cy + s(3) };
                rrect(hdc, track, s(3), C_FIELD_BORDER, None);
                let frac = if max > min { (value - min) as f32 / (max - min) as f32 } else { 0.0 };
                let tx = pad + ((w - 2 * pad) as f32 * frac) as i32;
                if enabled {
                    rrect(hdc, RECT { left: pad, top: cy - s(3), right: tx.max(pad + s(6)), bottom: cy + s(3) }, s(3), C_ACCENT, None);
                    circle(hdc, tx, cy, s(9), C_ACCENT2);
                    circle(hdc, tx, cy, s(6), C_WHITE);
                } else {
                    circle(hdc, tx, cy, s(7), C_DIM);
                }
                true
            });
            0
        }
        WM_ERASEBKGND => 1,
        WM_LBUTTONDOWN => {
            let enabled = SLIDERS.with(|m| m.borrow().get(&key).map(|s| s.enabled).unwrap_or(false));
            if enabled {
                SetCapture(hwnd);
                SLIDERS.with(|m| {
                    if let Some(sl) = m.borrow_mut().get_mut(&key) {
                        sl.drag = true;
                    }
                });
                if let Some(v) = set_from_x(x_of(lp)) {
                    InvalidateRect(hwnd, null(), 0);
                    SendMessageW(GetParent(hwnd), WM_APP_SLIDER, v as isize as usize, hwnd as isize);
                }
            }
            0
        }
        WM_MOUSEMOVE => {
            let dragging = SLIDERS.with(|m| m.borrow().get(&key).map(|s| s.drag).unwrap_or(false));
            if dragging {
                if let Some(v) = set_from_x(x_of(lp)) {
                    InvalidateRect(hwnd, null(), 0);
                    SendMessageW(GetParent(hwnd), WM_APP_SLIDER, v as isize as usize, hwnd as isize);
                }
            }
            0
        }
        WM_LBUTTONUP => {
            let was = SLIDERS.with(|m| {
                m.borrow_mut().get_mut(&key).map(|s| std::mem::replace(&mut s.drag, false)).unwrap_or(false)
            });
            if was {
                ReleaseCapture();
                let v = slider_get(hwnd);
                SendMessageW(GetParent(hwnd), WM_APP_SLIDER_END, v as isize as usize, hwnd as isize);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

// ===================== Room list rows =====================
#[derive(Clone, PartialEq)]
enum Row {
    Room {
        name: String,
        count: usize,
        current: bool,
    },
    User {
        name: String,
        speaking: bool,
        me: bool,
        owner: bool,
        mute: bool,
        deaf: bool,
        server_mute: bool,
    },
}

fn avatar_color(name: &str) -> u32 {
    let h = name.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    AVATAR[(h % AVATAR.len() as u32) as usize]
}

unsafe fn draw_list_item(d: &DRAWITEMSTRUCT) {
    let hdc = d.hDC;
    let r = d.rcItem;
    fill(hdc, r, C_PANEL);
    if d.itemID == u32::MAX {
        return;
    }
    let row = ROWS.with(|x| x.borrow().get(d.itemID as usize).cloned());
    let Some(row) = row else { return };
    let selected = d.itemState & ODS_SELECTED != 0;
    let t = th();
    let inner = RECT { left: r.left + s(2), top: r.top + s(2), right: r.right - s(4), bottom: r.bottom - s(2) };
    match row {
        Row::Room { name, count, current } => {
            let base = if current { C_ROOM_CUR } else { C_ROOM };
            rrect(hdc, inner, s(7), base, Some(if selected { C_ACCENT } else if current { C_ACCENT_DARK } else { C_PANEL_BORDER }));
            if current {
                fill(hdc, RECT { left: inner.left + s(1), top: inner.top + s(6), right: inner.left + s(4), bottom: inner.bottom - s(6) }, C_ACCENT2);
            }
            glyph(hdc, IC_PEOPLE, RECT { left: inner.left + s(8), top: inner.top, right: inner.left + s(30), bottom: inner.bottom }, t.icon_small, if current { C_ACCENT2 } else { C_MUTED });
            text(hdc, &name, RECT { left: inner.left + s(34), top: inner.top, right: inner.right - s(44), bottom: inner.bottom }, t.bold, C_TEXT, LINE);
            let chip = RECT { left: inner.right - s(38), top: inner.top + s(7), right: inner.right - s(8), bottom: inner.bottom - s(7) };
            rrect(hdc, chip, s(8), if count > 0 { C_ACCENT_DARK } else { C_FIELD }, None);
            text(hdc, &count.to_string(), chip, t.small, if count > 0 { C_WHITE } else { C_DIM }, DT_CENTER | LINE);
        }
        Row::User { name, speaking, me, owner, mute, deaf, server_mute } => {
            if selected {
                rrect(hdc, inner, s(7), C_SEL, None);
            }
            let cx = inner.left + s(30);
            let cy = (inner.top + inner.bottom) / 2;
            if speaking {
                circle(hdc, cx, cy, s(13), C_GREEN);
                circle(hdc, cx, cy, s(11), if selected { C_SEL } else { C_PANEL });
            }
            circle(hdc, cx, cy, s(10), avatar_color(&name));
            let initial: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
            text(hdc, &initial, RECT { left: cx - s(10), top: cy - s(10), right: cx + s(10), bottom: cy + s(10) }, t.bold, C_WHITE, DT_CENTER | LINE);

            // Status icons from the right.
            let mut right = inner.right - s(6);
            let mut badge = |code: u32, color: u32| {
                let br = RECT { left: right - s(20), top: inner.top, right, bottom: inner.bottom };
                glyph(hdc, code, br, t.icon_small, color);
                right -= s(22);
            };
            if server_mute {
                badge(IC_SPEAKER_OFF, C_RED);
            } else if mute {
                badge(IC_MIC_OFF, C_RED_SOFT);
            }
            if deaf {
                badge(IC_HEADPHONES, C_RED_SOFT);
            }
            if owner {
                badge(IC_STAR, C_AMBER);
            }
            let label = if me { format!("{name}  (you)") } else { name };
            let color = if speaking { C_GREEN } else { C_TEXT };
            text(hdc, &label, RECT { left: cx + s(18), top: inner.top, right, bottom: inner.bottom }, t.body, color, LINE);
        }
    }
}

/// Text box colours, shared by both windows.
unsafe fn ctl_color(msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
    let hdc = wp as HDC;
    match msg {
        WM_CTLCOLORLISTBOX if lp as usize == ROOM_LIST.load(Ordering::Relaxed) => {
            SetTextColor(hdc, C_TEXT);
            SetBkColor(hdc, C_PANEL);
            Some(th().panel_brush as LRESULT)
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            SetTextColor(hdc, C_TEXT);
            SetBkColor(hdc, C_FIELD);
            Some(th().field_brush as LRESULT)
        }
        WM_CTLCOLORSTATIC => {
            SetTextColor(hdc, C_TEXT);
            SetBkColor(hdc, C_PANEL);
            Some(th().panel_brush as LRESULT)
        }
        _ => None,
    }
}

// ===================== The app =====================
#[derive(Clone, Copy, PartialEq)]
enum Entry {
    Room(u32),
    User(u32),
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Connect,
    Main,
}

#[derive(Clone, PartialEq)]
enum Pill {
    Muted,
    Talking,
    Voice,
    Ptt(String),
}

/// A text box drawn inside a rounded "field" painted by its window.
struct Field {
    page: Page,
    owner_only: bool,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    edit: HWND,
}

struct SettingsCtrls {
    input: HWND,
    output: HWND,
    seg_ptt: HWND,
    seg_va: HWND,
    thr: HWND,
    beep: HWND,
    join: HWND,
}

impl Default for SettingsCtrls {
    fn default() -> Self {
        let n = null_mut();
        SettingsCtrls { input: n, output: n, seg_ptt: n, seg_va: n, thr: n, beep: n, join: n }
    }
}

struct App {
    hwnd: HWND,
    page: Page,
    fields: Vec<Field>,
    focused: HWND,
    // connect page
    c_ctrls: Vec<HWND>,
    ed_name: HWND,
    ed_addr: HWND,
    ed_port: HWND,
    ed_pass: HWND,
    conn_status: String,
    // main page
    m_ctrls: Vec<HWND>,
    admin: Vec<HWND>,
    list: HWND,
    vol_bar: HWND,
    chat_log: HWND,
    chat_in: HWND,
    btn_mute: HWND,
    btn_deaf: HWND,
    move_combo: HWND,
    ed_room: HWND,
    vol_text: String,
    header_sub: String,
    status1: String,
    status2: String,
    pill: Pill,
    server_label: String,
    // settings window
    settings: HWND,
    s: SettingsCtrls,
    in_names: Vec<String>,
    out_names: Vec<String>,
    pending_key: i32,
    capturing: bool,
    ignore: [bool; 256],
    level_db: i32,
    // session
    net: Option<Arc<Net>>,
    server: Option<ServerHandle>,
    audio: Option<AudioEngine>,
    mixer: Arc<Mutex<Mixer>>,
    connected: bool,
    hosting: bool,
    entries: Vec<Entry>,
    last_rows: Vec<Row>,
    combo_rooms: Vec<u32>,
    my_room: Option<u32>,
    members: HashMap<u32, String>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` on the app unless it is already busy (re-entrant window messages).
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| match a.try_borrow_mut() {
        Ok(mut g) => g.as_mut().map(f),
        Err(_) => None,
    })
}

// ----- control creation -----
unsafe fn make(parent: HWND, class: &str, text_: &str, style: u32, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
    let cls = wide(class);
    let txt = wide(text_);
    let hwnd = CreateWindowExW(
        0,
        cls.as_ptr(),
        txt.as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        s(x),
        s(y),
        s(w),
        s(h),
        parent,
        id as usize as _,
        GetModuleHandleW(null()),
        null(),
    );
    SendMessageW(hwnd, WM_SETFONT, th().body, 1);
    hwnd
}

fn button(parent: HWND, label: &str, x: i32, y: i32, w: i32, h: i32, id: u16, kind: Kind, icon: Option<u32>, bg: u32) -> HWND {
    unsafe {
        let hwnd = make(parent, "BUTTON", label, WS_TABSTOP | BS_OWNERDRAW, x, y, w, h, id);
        BTNS.with(|b| b.borrow_mut().insert(hwnd as usize, Btn { kind, icon, active: false, bg }));
        SetWindowSubclass(hwnd, Some(hover_proc), 1, 0);
        hwnd
    }
}

fn slider(parent: HWND, x: i32, y: i32, w: i32, h: i32, min: i32, max: i32, value: i32, bg: u32) -> HWND {
    unsafe {
        let hwnd = make(parent, "SquadSlider", "", 0, x, y, w, h, 0);
        SLIDERS.with(|m| {
            m.borrow_mut().insert(hwnd as usize, Slider { min, max, value, enabled: true, drag: false, bg })
        });
        hwnd
    }
}

impl App {
    fn field(&mut self, parent: HWND, page: Page, owner_only: bool, value: &str, style: u32, x: i32, y: i32, w: i32, h: i32, limit: usize) -> HWND {
        let eh = 20;
        let edit = unsafe {
            make(parent, "EDIT", value, WS_TABSTOP | ES_AUTOHSCROLL | style, x + 12, y + (h - eh) / 2, w - 24, eh, 0)
        };
        sendm(edit, EM_SETLIMITTEXT, limit, 0);
        self.fields.push(Field { page, owner_only, x, y, w, h, edit });
        edit
    }

    fn new(hwnd: HWND) -> App {
        let n = null_mut();
        let mut a = App {
            hwnd,
            page: Page::Connect,
            fields: vec![],
            focused: n,
            c_ctrls: vec![],
            ed_name: n,
            ed_addr: n,
            ed_port: n,
            ed_pass: n,
            conn_status: String::new(),
            m_ctrls: vec![],
            admin: vec![],
            list: n,
            vol_bar: n,
            chat_log: n,
            chat_in: n,
            btn_mute: n,
            btn_deaf: n,
            move_combo: n,
            ed_room: n,
            vol_text: String::new(),
            header_sub: String::new(),
            status1: String::new(),
            status2: String::new(),
            pill: Pill::Voice,
            server_label: String::new(),
            settings: n,
            s: SettingsCtrls::default(),
            in_names: vec![],
            out_names: vec![],
            pending_key: 0,
            capturing: false,
            ignore: [false; 256],
            level_db: -100,
            net: None,
            server: None,
            audio: None,
            mixer: Arc::new(Mutex::new(Mixer::new())),
            connected: false,
            hosting: false,
            entries: vec![],
            last_rows: vec![],
            combo_rooms: vec![],
            my_room: None,
            members: HashMap::new(),
        };
        a.build_connect_page();
        a.build_main_page();
        a.show_page(Page::Connect);
        a
    }

    fn build_connect_page(&mut self) {
        let p = self.hwnd;
        let (name, addr, port, pass) = {
            let c = cfg();
            (c.name.clone(), c.address.clone(), c.port.to_string(), c.password.clone())
        };
        self.ed_name = self.field(p, Page::Connect, false, &name, 0, 278, 186, 404, 42, 24);
        self.ed_addr = self.field(p, Page::Connect, false, &addr, 0, 278, 256, 404, 42, 100);
        self.ed_port = self.field(p, Page::Connect, false, &port, ES_NUMBER, 278, 326, 110, 42, 5);
        self.ed_pass = self.field(p, Page::Connect, false, &pass, ES_PASSWORD, 400, 326, 282, 42, 100);
        let mut v = vec![self.ed_name, self.ed_addr, self.ed_port, self.ed_pass];
        v.push(button(p, "JOIN SERVER", 278, 390, 404, 46, ID_JOIN, Kind::Primary, Some(IC_ENTER), C_PANEL));
        v.push(button(p, "Audio and push-to-talk settings", 278, 446, 404, 40, ID_SETTINGS_C, Kind::Secondary, Some(IC_SETTINGS), C_PANEL));
        v.push(button(p, "Host on this PC (same Wi-Fi only)", 330, 616, 300, 28, ID_HOST, Kind::Ghost, Some(IC_PC), C_BG2));
        self.c_ctrls = v;
    }

    fn build_main_page(&mut self) {
        let p = self.hwnd;
        let mut v = Vec::new();
        unsafe {
            self.list = make(p, "LISTBOX", "", WS_TABSTOP | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT | LBS_OWNERDRAWFIXED, 24, 108, 304, 230, ID_LIST);
            dark_control(self.list, false);
        }
        ROOM_LIST.store(self.list as usize, Ordering::Relaxed);
        v.push(self.list);
        self.vol_bar = slider(p, 16, 370, 320, 28, 0, 200, 100, C_PANEL);
        slider_enable(self.vol_bar, false);
        v.push(self.vol_bar);
        v.push(button(p, "Join selected room", 24, 404, 304, 34, ID_JOINROOM, Kind::Secondary, Some(IC_PEOPLE), C_PANEL));

        // Owner tools
        let mut ad = vec![
            button(p, "Kick", 24, 476, 94, 32, ID_KICK, Kind::Danger, Some(IC_CANCEL), C_PANEL),
            button(p, "Server-mute", 126, 476, 112, 32, ID_SMUTE, Kind::Secondary, Some(IC_SPEAKER_OFF), C_PANEL),
            button(p, "Room", 246, 476, 82, 32, ID_DELROOM, Kind::Danger, Some(IC_DELETE), C_PANEL),
        ];
        unsafe {
            self.move_combo = make(p, "COMBOBOX", "", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 24, 516, 190, 300, 0);
            dark_control(self.move_combo, true);
        }
        ad.push(self.move_combo);
        ad.push(button(p, "Move here", 222, 514, 106, 32, ID_MOVE, Kind::Secondary, Some(IC_FORWARD), C_PANEL));
        self.ed_room = self.field(p, Page::Main, true, "", 0, 24, 552, 190, 32, 40);
        ad.push(self.ed_room);
        ad.push(button(p, "Add room", 222, 552, 106, 32, ID_ADDROOM, Kind::Primary, Some(IC_ADD), C_PANEL));
        self.admin = ad;

        // Chat
        unsafe {
            self.chat_log = make(p, "EDIT", "", ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | WS_VSCROLL, 364, 108, 568, 420, 0);
            dark_control(self.chat_log, false);
        }
        sendm(self.chat_log, EM_SETMARGINS, 3, ((s(8) as isize) << 16) | s(8) as isize);
        v.push(self.chat_log);
        self.chat_in = self.field(p, Page::Main, false, "", 0, 364, 540, 508, 40, 500);
        v.push(self.chat_in);
        v.push(button(p, "", 880, 540, 52, 40, ID_SEND, Kind::Square, Some(IC_SEND), C_PANEL));

        // Bottom bar
        self.btn_mute = button(p, "", 708, 614, 52, 52, ID_MUTE, Kind::Round, Some(IC_MIC), C_HEADER);
        self.btn_deaf = button(p, "", 768, 614, 52, 52, ID_DEAF, Kind::Round, Some(IC_HEADPHONES), C_HEADER);
        v.push(self.btn_mute);
        v.push(self.btn_deaf);
        v.push(button(p, "", 828, 614, 52, 52, ID_SETTINGS, Kind::Round, Some(IC_SETTINGS), C_HEADER));
        v.push(button(p, "", 888, 614, 52, 52, ID_DISCONNECT, Kind::RoundDanger, Some(IC_HANGUP), C_HEADER));
        self.m_ctrls = v;
    }

    fn show_page(&mut self, page: Page) {
        self.page = page;
        let main = page == Page::Main;
        for &h in &self.c_ctrls {
            show(h, !main);
        }
        for &h in &self.m_ctrls {
            show(h, main);
        }
        let owner = main && self.is_owner();
        for &h in &self.admin {
            show(h, owner);
        }
        invalidate(self.hwnd, None);
    }

    fn is_owner(&self) -> bool {
        self.net.as_ref().map(|n| lk(&n.shared).owner).unwrap_or(false)
    }

    fn log(&self, text_: &str) {
        unsafe {
            let mut len = GetWindowTextLengthW(self.chat_log);
            if len > 30000 {
                set_text(self.chat_log, "");
                len = 0;
            }
            let line = wide(&format!("{text_}\r\n"));
            SendMessageW(self.chat_log, EM_SETSEL, len as usize, len as isize);
            SendMessageW(self.chat_log, EM_REPLACESEL, 0, line.as_ptr() as isize);
            SendMessageW(self.chat_log, EM_SCROLLCARET, 0, 0);
        }
    }

    fn users(&self) -> (Vec<UserInfo>, u32) {
        match &self.net {
            Some(n) => {
                let sh = lk(&n.shared);
                (sh.users.clone(), sh.session)
            }
            None => (vec![], 0),
        }
    }

    fn set_conn_status(&mut self, s_: &str) {
        if self.conn_status != s_ {
            self.conn_status = s_.to_string();
            invalidate(self.hwnd, Some(rc(250, 520, 460, 60)));
        }
    }

    // ----- painting -----

    unsafe fn paint_main(&self, hdc: HDC, w: i32, h: i32) {
        let t = th();
        backdrop(hdc, s(56), w, h);

        // Header
        fill(hdc, RECT { left: 0, top: 0, right: w, bottom: s(56) }, C_HEADER);
        fill(hdc, RECT { left: 0, top: s(56) - 1, right: w, bottom: s(56) }, C_PANEL_BORDER);
        if t.app_icon != 0 {
            DrawIconEx(hdc, s(16), s(12), t.app_icon as HICON, s(32), s(32), 0, null_mut(), 3);
        }
        text(hdc, "SquadTalk", rc(58, 8, 300, 24), t.title, C_TEXT, LINE);
        let sub = if self.page == Page::Connect { "Voice chat built for gaming" } else { self.header_sub.as_str() };
        text(hdc, sub, rc(58, 32, 520, 18), t.small, C_MUTED, LINE);

        let fields_for = |page: Page, owner: bool| {
            for f in self.fields.iter().filter(|f| f.page == page && (!f.owner_only || owner)) {
                let border = if f.edit == self.focused { C_ACCENT } else { C_FIELD_BORDER };
                rrect(hdc, rc(f.x, f.y, f.w, f.h), s(8), C_FIELD, Some(border));
            }
        };

        match self.page {
            Page::Connect => {
                rrect(hdc, rc(250, 80, 460, 430), s(16), C_PANEL, Some(C_PANEL_BORDER));
                text(hdc, "Join a server", rc(278, 100, 404, 30), t.title, C_TEXT, LINE);
                text(hdc, "Enter the details your server owner gave you.", rc(278, 130, 404, 20), t.small, C_MUTED, LINE);
                text(hdc, "YOUR NAME", rc(278, 166, 404, 18), t.head, C_MUTED, LINE);
                text(hdc, "SERVER ADDRESS", rc(278, 236, 404, 18), t.head, C_MUTED, LINE);
                text(hdc, "PORT", rc(278, 306, 110, 18), t.head, C_MUTED, LINE);
                text(hdc, "PASSWORD", rc(400, 306, 282, 18), t.head, C_MUTED, LINE);
                fields_for(Page::Connect, false);
                if !self.conn_status.is_empty() {
                    text(hdc, &self.conn_status, rc(250, 524, 460, 22), t.bold, C_AMBER, DT_CENTER | LINE);
                }
                text(
                    hdc,
                    "Your push-to-talk key works even while a game is in front.",
                    rc(250, 560, 460, 20),
                    t.small,
                    C_DIM,
                    DT_CENTER | LINE,
                );
            }
            Page::Main => {
                let owner = self.is_owner();
                // Header right: server + role badge
                let mut right = w - s(16);
                if owner || self.hosting {
                    let label = if self.hosting { "HOSTING" } else { "OWNER" };
                    let bw = s(84);
                    let br = RECT { left: right - bw, top: s(16), right, bottom: s(40) };
                    rrect(hdc, br, s(12), if self.hosting { C_ACCENT_DARK } else { rgb(70, 52, 14) }, None);
                    text(hdc, label, br, t.head, if self.hosting { C_WHITE } else { C_AMBER }, DT_CENTER | LINE);
                    right -= bw + s(10);
                }
                text(hdc, &self.server_label, RECT { left: right - s(320), top: s(16), right, bottom: s(40) }, t.small, C_MUTED, DT_RIGHT | LINE);

                // Panels
                rrect(hdc, rc(16, 72, 320, 520), s(14), C_PANEL, Some(C_PANEL_BORDER));
                rrect(hdc, rc(352, 72, 592, 520), s(14), C_PANEL, Some(C_PANEL_BORDER));
                section_title(hdc, IC_PEOPLE, "ROOMS", 30, 82);
                text(hdc, "double-click to join", rc(150, 82, 172, 20), t.small, C_DIM, DT_RIGHT | LINE);
                divider(hdc, 28, 344, 296);
                text(hdc, &self.vol_text, rc(28, 350, 296, 18), t.small, C_MUTED, LINE);
                if owner {
                    divider(hdc, 28, 446, 296);
                    section_title(hdc, IC_SHIELD, "OWNER TOOLS", 30, 452);
                }
                section_title(hdc, IC_CHAT, "ROOM CHAT", 366, 82);
                fields_for(Page::Main, owner);

                // Bottom bar
                fill(hdc, RECT { left: 0, top: s(604), right: w, bottom: h }, C_HEADER);
                fill(hdc, RECT { left: 0, top: s(604), right: w, bottom: s(604) + 1 }, C_PANEL_BORDER);
                self.paint_pill(hdc);
                text(hdc, &self.status1, rc(452, 618, 240, 22), t.bold, C_TEXT, LINE);
                text(hdc, &self.status2, rc(452, 640, 240, 18), t.small, C_MUTED, LINE);
            }
        }
    }

    unsafe fn paint_pill(&self, hdc: HDC) {
        let t = th();
        let r = rc(16, 614, 420, 52);
        let (bg, border, dot, icon, icon_c) = match &self.pill {
            Pill::Talking => (C_GREEN_DARK, C_GREEN, C_GREEN, IC_MIC, rgb(8, 30, 20)),
            Pill::Muted => (C_RED_DARK, rgb(120, 44, 56), rgb(90, 31, 40), IC_MIC_OFF, C_RED_SOFT),
            Pill::Voice => (C_FIELD, C_FIELD_BORDER, C_ACCENT_DARK, IC_MIC, C_WHITE),
            Pill::Ptt(_) => (C_FIELD, C_FIELD_BORDER, C_ACCENT, IC_MIC, C_WHITE),
        };
        rrect(hdc, r, s(26), bg, Some(border));
        let (cx, cy) = (r.left + s(28), (r.top + r.bottom) / 2);
        circle(hdc, cx, cy, s(19), dot);
        glyph(hdc, icon, RECT { left: cx - s(19), top: cy - s(19), right: cx + s(19), bottom: cy + s(19) }, t.icon, icon_c);
        let x0 = r.left + s(58);
        let area = |x: i32| RECT { left: x, top: r.top, right: r.right - s(16), bottom: r.bottom };
        match &self.pill {
            Pill::Talking => text(hdc, "TALKING", area(x0), t.title, C_GREEN, LINE),
            Pill::Muted => text(hdc, "Microphone off", area(x0), t.bold, C_RED_SOFT, LINE),
            Pill::Voice => text(hdc, "Voice activation: just talk", area(x0), t.bold, C_MUTED, LINE),
            Pill::Ptt(key) => {
                text(hdc, "Hold", area(x0), t.bold, C_TEXT, LINE);
                let kx = x0 + text_width(hdc, "Hold", t.bold) + s(10);
                let kw = text_width(hdc, key, t.key) + s(22);
                let kr = RECT { left: kx, top: cy - s(14), right: kx + kw, bottom: cy + s(14) };
                rrect(hdc, RECT { left: kr.left, top: kr.top + s(2), right: kr.right, bottom: kr.bottom + s(2) }, s(6), rgb(14, 18, 30), None);
                rrect(hdc, kr, s(6), rgb(44, 54, 88), Some(C_ACCENT_DARK));
                text(hdc, key, kr, t.key, C_WHITE, DT_CENTER | LINE);
                text(hdc, "to talk", area(kr.right + s(10)), t.bold, C_TEXT, LINE);
            }
        }
    }

    unsafe fn paint_settings(&self, hdc: HDC, w: i32, h: i32) {
        let t = th();
        fill(hdc, RECT { left: 0, top: 0, right: w, bottom: h }, C_BG);
        fill(hdc, RECT { left: 0, top: 0, right: w, bottom: s(56) }, C_HEADER);
        fill(hdc, RECT { left: 0, top: s(56) - 1, right: w, bottom: s(56) }, C_PANEL_BORDER);
        glyph(hdc, IC_SETTINGS, rc(16, 14, 28, 28), t.icon_big, C_ACCENT2);
        text(hdc, "Settings", rc(52, 12, 300, 32), t.title, C_TEXT, LINE);

        section_title(hdc, IC_HEADPHONES, "AUDIO DEVICES", 22, 70);
        text(hdc, "Microphone", rc(24, 94, 472, 18), t.small, C_MUTED, LINE);
        text(hdc, "Speakers / headphones", rc(24, 150, 472, 18), t.small, C_MUTED, LINE);

        section_title(hdc, IC_MIC, "HOW YOU TALK", 22, 212);
        text(hdc, "Push-to-talk key (keyboard or mouse button)", rc(24, 286, 472, 18), t.small, C_MUTED, LINE);
        let kr = rc(24, 306, 322, 40);
        if self.capturing {
            rrect(hdc, kr, s(8), rgb(56, 44, 16), Some(C_AMBER));
            text(hdc, "Press a key or mouse button…  (Esc cancels)", kr, t.bold, C_AMBER, DT_CENTER | LINE);
        } else {
            rrect(hdc, kr, s(8), rgb(44, 54, 88), Some(C_ACCENT_DARK));
            text(hdc, &key_name(self.pending_key), kr, t.key, C_WHITE, DT_CENTER | LINE);
        }

        let thr = slider_get(self.s.thr) - 70;
        text(hdc, &format!("Voice activation level: {thr} dB"), rc(24, 360, 300, 18), t.small, C_MUTED, LINE);
        text(hdc, "move left = picks up quieter speech", rc(220, 360, 276, 18), t.small, C_DIM, DT_RIGHT | LINE);
        // Live mic meter with the threshold marker
        let m = rc(24, 384, 472, 10);
        rrect(hdc, m, s(5), C_FIELD, None);
        let pos = |db: i32| m.left + ((m.right - m.left) as f32 * ((db.clamp(-70, 0) + 70) as f32 / 70.0)) as i32;
        if self.connected {
            let lx = pos(self.level_db);
            if lx > m.left + s(4) {
                rrect(hdc, RECT { left: m.left, top: m.top, right: lx, bottom: m.bottom }, s(5), if self.level_db > thr { C_GREEN } else { C_ACCENT_DARK }, None);
            }
        }
        let tx = pos(thr);
        fill(hdc, RECT { left: tx - s(1), top: m.top - s(4), right: tx + s(2), bottom: m.bottom + s(4) }, C_AMBER);
        let level_text = if self.connected {
            format!("Your mic right now: {} dB   (green = would be sent)", self.level_db)
        } else {
            "Join a server to see your live mic level here.".to_string()
        };
        text(hdc, &level_text, rc(24, 430, 472, 18), t.small, C_DIM, LINE);

        section_title(hdc, IC_SPEAKER_OFF, "SOUNDS", 22, 462);
        text(hdc, "Beep when push-to-talk starts and stops", rc(24, 484, 410, 30), t.body, C_TEXT, LINE);
        text(hdc, "Sound when people join or leave your room", rc(24, 520, 410, 30), t.body, C_TEXT, LINE);
    }

    // ----- connecting -----

    fn do_join(&mut self, host: bool) -> Option<String> {
        if self.net.is_some() {
            return None;
        }
        let name = get_text(self.ed_name).trim().to_string();
        if name.is_empty() {
            return Some("Please type your name first.".into());
        }
        let mut addr = get_text(self.ed_addr).trim().to_string();
        let port: u16 = match get_text(self.ed_port).trim().parse() {
            Ok(p) if p > 0 => p,
            _ => return Some("The port must be a number like 9987.".into()),
        };
        let password = get_text(self.ed_pass);
        if !host && addr.is_empty() {
            return Some("Type the server address first (ask the server owner, it looks like 140.238.12.34).".into());
        }
        {
            let mut c = cfg();
            c.name = name.clone();
            if !host {
                c.address = addr.clone();
            }
            c.port = port;
            c.password = password.clone();
        }
        config::save();

        if host {
            let (owner_uid, rooms) = {
                let c = cfg();
                (protocol::uid_of(&c.secret), c.host_rooms.clone())
            };
            let opts = server::Options {
                port,
                password: password.clone(),
                owner_code: String::new(),
                owner_uids: vec![owner_uid],
                rooms,
                on_save: Arc::new(|saved: server::Saved| {
                    config::cfg().host_rooms = saved.rooms;
                    config::save();
                }),
                log: false,
            };
            match server::start(opts) {
                Ok(h) => self.server = Some(h),
                Err(e) => {
                    return Some(format!(
                        "Could not start the server on port {port}.\nIs SquadTalk already hosting in another window?\n\n(Details: {e})"
                    ))
                }
            }
            self.server_label = format!("This PC  ·  port {port}");
            addr = "127.0.0.1".into();
        } else {
            self.server_label = format!("{addr}  ·  port {port}");
        }
        self.hosting = host;
        let secret = cfg().secret.clone();
        let hwnd_val = self.hwnd as usize;
        let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(move || unsafe {
            PostMessageW(hwnd_val as HWND, WM_APP_NOTIFY, 0, 0);
        });
        self.net = Some(client::connect(addr, port, name, secret, password, notify, self.mixer.clone()));
        self.set_conn_status(if host { "Starting your server…" } else { "Connecting…" });
        enable(self.c_ctrls[4], false);
        None
    }

    fn start_audio(&mut self) {
        self.audio = None;
        if let Some(net) = self.net.clone() {
            let (engine, warnings) = audio::start(&net, &self.mixer);
            self.audio = Some(engine);
            for w in warnings {
                self.log(&format!("⚠ {w}"));
            }
        }
    }

    fn on_connected(&mut self) {
        self.connected = true;
        self.set_conn_status("");
        enable(self.c_ctrls[4], true);
        set_text(self.chat_log, "");
        set_active(self.btn_mute, false);
        set_active(self.btn_deaf, false);
        lk(&self.mixer).clear();
        self.header_sub = "Connected".into();
        self.show_page(Page::Main);
        self.start_audio();
        if self.hosting {
            let port = cfg().port;
            self.log("A temporary server is running on this PC. You are the owner.");
            if let Some(ip) = lan_ip() {
                self.log(&format!("People on the same Wi-Fi can join with address: {ip}   (port {port})"));
            }
            if !cfg().password.is_empty() {
                self.log("They also need the server password you typed.");
            }
            self.log("For friends in other places, use your 24/7 Oracle server instead.");
        } else {
            self.log("Connected to the server. Welcome!");
        }
        let (va, key) = {
            let c = cfg();
            (c.voice_activation, c.ptt_key)
        };
        if va {
            self.log("Voice activation is on: just talk.");
        } else {
            self.log(&format!("Hold [{}] to talk. You can change the key in Settings.", key_name(key)));
        }
        self.update_status();
    }

    fn teardown(&mut self) {
        if let Some(n) = self.net.take() {
            n.stop();
        }
        self.audio = None;
        self.server = None;
        self.connected = false;
        self.hosting = false;
        lk(&self.mixer).clear();
        self.entries.clear();
        self.last_rows.clear();
        ROWS.with(|r| r.borrow_mut().clear());
        self.combo_rooms.clear();
        self.my_room = None;
        self.members.clear();
        sendm(self.list, LB_RESETCONTENT, 0, 0);
        if !self.c_ctrls.is_empty() {
            enable(self.c_ctrls[4], true);
        }
        self.show_page(Page::Connect);
    }

    fn process_events(&mut self) -> Vec<String> {
        let mut popups = vec![];
        let net = match &self.net {
            Some(n) => n.clone(),
            None => return popups,
        };
        let events: Vec<Event> = lk(&net.shared).events.drain(..).collect();
        for ev in events {
            match ev {
                Event::Connected => self.on_connected(),
                Event::State => self.on_state(),
                Event::Chat { name, text } => self.log(&format!("{name}:  {text}")),
                Event::Failed(r) => {
                    popups.push(format!("Could not connect.\n\n{r}"));
                    self.teardown();
                    self.set_conn_status("");
                    break;
                }
                Event::Denied(r) => {
                    popups.push(format!("The server did not let you in:\n\n{r}"));
                    self.teardown();
                    self.set_conn_status("");
                    break;
                }
                Event::Kicked(r) => {
                    popups.push(r);
                    self.teardown();
                    break;
                }
                Event::Disconnected => {
                    popups.push("Lost the connection to the server.".into());
                    self.teardown();
                    break;
                }
            }
        }
        popups
    }

    fn on_state(&mut self) {
        let net = match &self.net {
            Some(n) => n.clone(),
            None => return,
        };
        let (rooms, users, me) = {
            let sh = lk(&net.shared);
            (sh.rooms.clone(), sh.users.clone(), sh.session)
        };

        // Per-person volumes and cleanup of people who left.
        {
            let vols: HashMap<u32, f32> = {
                let c = cfg();
                users
                    .iter()
                    .map(|u| (u.session, *c.volumes.get(&u.uid).unwrap_or(&100) as f32 / 100.0))
                    .collect()
            };
            let keep: HashSet<u32> = users.iter().map(|u| u.session).collect();
            let mut m = lk(&self.mixer);
            m.volumes = vols;
            m.retain_sessions(&keep);
        }

        // Join / leave notices for the room you're in.
        let my_room = users.iter().find(|u| u.session == me).map(|u| u.room);
        let members: HashMap<u32, String> = users
            .iter()
            .filter(|u| Some(u.room) == my_room && u.session != me)
            .map(|u| (u.session, u.name.clone()))
            .collect();
        if let Some(r) = my_room {
            let sounds = cfg().join_sounds;
            if self.my_room != Some(r) {
                let rname = rooms.iter().find(|x| x.id == r).map(|x| x.name.clone()).unwrap_or_default();
                self.log(&format!("━━  You are in \"{rname}\"  ━━"));
            } else {
                let mut joined = false;
                let mut left = false;
                for (s_, n) in &members {
                    if !self.members.contains_key(s_) {
                        self.log(&format!("→ {n} joined the room"));
                        joined = true;
                    }
                }
                for (s_, n) in &self.members {
                    if !members.contains_key(s_) {
                        self.log(&format!("← {n} left the room"));
                        left = true;
                    }
                }
                if sounds {
                    let mut m = lk(&self.mixer);
                    if joined {
                        m.play_effect(&audio::join_sound());
                    }
                    if left {
                        m.play_effect(&audio::leave_sound());
                    }
                }
            }
        }
        self.my_room = my_room;
        self.members = members;

        // Room list in the "move to" box.
        let ids: Vec<u32> = rooms.iter().map(|r| r.id).collect();
        if ids != self.combo_rooms {
            let old = sendm(self.move_combo, CB_GETCURSEL, 0, 0);
            sendm(self.move_combo, CB_RESETCONTENT, 0, 0);
            for r in &rooms {
                let w = wide(&r.name);
                sendm(self.move_combo, CB_ADDSTRING, 0, w.as_ptr() as isize);
            }
            let sel = if old >= 0 && (old as usize) < rooms.len() { old as usize } else { 0 };
            sendm(self.move_combo, CB_SETCURSEL, sel, 0);
            self.combo_rooms = ids;
        }
        self.show_page(Page::Main);
        self.render_list();
        self.update_status();
    }

    fn selected(&self) -> Option<Entry> {
        let i = sendm(self.list, LB_GETCURSEL, 0, 0);
        if i < 0 {
            None
        } else {
            self.entries.get(i as usize).copied()
        }
    }

    fn selected_user(&self) -> Option<UserInfo> {
        match self.selected() {
            Some(Entry::User(s_)) => self.users().0.into_iter().find(|u| u.session == s_),
            _ => None,
        }
    }

    fn render_list(&mut self) {
        let net = match &self.net {
            Some(n) => n.clone(),
            None => return,
        };
        let (rooms, users, me) = {
            let sh = lk(&net.shared);
            (sh.rooms.clone(), sh.users.clone(), sh.session)
        };
        let mut speaking: HashSet<u32> = lk(&self.mixer).speaking().into_iter().collect();
        if net.self_speaking.load(Ordering::Relaxed) {
            speaking.insert(me);
        }
        let my_room = users.iter().find(|u| u.session == me).map(|u| u.room);
        let mut rows = Vec::new();
        let mut entries = Vec::new();
        for r in &rooms {
            let mut people: Vec<&UserInfo> = users.iter().filter(|u| u.room == r.id).collect();
            people.sort_by_key(|u| u.name.to_lowercase());
            rows.push(Row::Room { name: r.name.clone(), count: people.len(), current: Some(r.id) == my_room });
            entries.push(Entry::Room(r.id));
            for u in people {
                rows.push(Row::User {
                    name: u.name.clone(),
                    speaking: speaking.contains(&u.session),
                    me: u.session == me,
                    owner: u.owner,
                    mute: u.mute,
                    deaf: u.deaf,
                    server_mute: u.server_mute,
                });
                entries.push(Entry::User(u.session));
            }
        }
        if rows == self.last_rows {
            return;
        }
        ROWS.with(|x| *x.borrow_mut() = rows.clone());
        if entries != self.entries {
            let sel = self.selected();
            let top = sendm(self.list, LB_GETTOPINDEX, 0, 0).max(0) as usize;
            sendm(self.list, WM_SETREDRAW, 0, 0);
            sendm(self.list, LB_RESETCONTENT, 0, 0);
            for _ in &rows {
                sendm(self.list, LB_ADDSTRING, 0, 0);
            }
            if let Some(e) = sel {
                if let Some(i) = entries.iter().position(|x| *x == e) {
                    sendm(self.list, LB_SETCURSEL, i, 0);
                }
            }
            sendm(self.list, LB_SETTOPINDEX, top, 0);
            sendm(self.list, WM_SETREDRAW, 1, 0);
            self.entries = entries;
        }
        invalidate(self.list, None);
        self.last_rows = rows;
        self.update_volume_ui();
    }

    fn set_vol_text(&mut self, t: String) {
        if self.vol_text != t {
            self.vol_text = t;
            invalidate(self.hwnd, Some(rc(24, 348, 304, 22)));
        }
    }

    fn update_volume_ui(&mut self) {
        let me = self.users().1;
        if let Some(u) = self.selected_user() {
            if u.session != me {
                let v = cfg().volumes.get(&u.uid).copied().unwrap_or(100);
                slider_enable(self.vol_bar, true);
                if slider_get(self.vol_bar) != v as i32 {
                    slider_set(self.vol_bar, v as i32);
                }
                self.set_vol_text(format!("VOLUME · {}  —  {}%", u.name, v));
                return;
            }
        }
        slider_enable(self.vol_bar, false);
        self.set_vol_text("VOLUME · select a person to change it".into());
    }

    fn on_slider(&mut self, bar: HWND, value: i32, end: bool) {
        if bar == self.vol_bar {
            if let Some(u) = self.selected_user() {
                let pos = value.clamp(0, 200) as u32;
                cfg().volumes.insert(u.uid.clone(), pos);
                lk(&self.mixer).volumes.insert(u.session, pos as f32 / 100.0);
                self.set_vol_text(format!("VOLUME · {}  —  {}%", u.name, pos));
                if end {
                    config::save();
                }
            }
        } else if !self.settings.is_null() && bar == self.s.thr {
            invalidate(self.settings, Some(rc(0, 356, 520, 96)));
        }
    }

    fn update_status(&mut self) {
        let net = match &self.net {
            Some(n) => n.clone(),
            None => return,
        };
        let ping = lk(&net.shared).ping_ms;
        let (va, key) = {
            let c = cfg();
            (c.voice_activation, c.ptt_key)
        };
        let pill = if net.self_mute.load(Ordering::Relaxed) || net.deaf.load(Ordering::Relaxed) {
            Pill::Muted
        } else if net.self_speaking.load(Ordering::Relaxed) {
            Pill::Talking
        } else if va {
            Pill::Voice
        } else {
            Pill::Ptt(key_name(key))
        };
        if pill != self.pill {
            self.pill = pill;
            invalidate(self.hwnd, Some(rc(10, 608, 432, 64)));
        }
        let s1 = match ping {
            Some(p) if p < 80 => format!("●  Ping {p} ms · great"),
            Some(p) if p < 160 => format!("●  Ping {p} ms · ok"),
            Some(p) => format!("●  Ping {p} ms · slow"),
            None => "●  Connected".to_string(),
        };
        let (users, _) = self.users();
        let room_name = {
            let sh = lk(&net.shared);
            self.my_room.and_then(|r| sh.rooms.iter().find(|x| x.id == r).map(|x| x.name.clone()))
        };
        let in_room = users.iter().filter(|u| Some(u.room) == self.my_room).count();
        let s2 = match room_name {
            Some(n) => format!("{n} · {in_room} in room · {} online", users.len()),
            None => format!("{} online", users.len()),
        };
        if s1 != self.status1 || s2 != self.status2 {
            self.status1 = s1;
            self.status2 = s2;
            invalidate(self.hwnd, Some(rc(446, 612, 250, 52)));
        }
        let sub = format!("{} · {} online", if self.hosting { "Hosting on this PC" } else { "Connected" }, users.len());
        if sub != self.header_sub {
            self.header_sub = sub;
            invalidate(self.hwnd, Some(rc(58, 30, 520, 22)));
        }
    }

    fn join_selected_room(&self) {
        if let (Some(Entry::Room(id)), Some(n)) = (self.selected(), &self.net) {
            n.send(&C2S::Join { room: id });
        }
    }

    fn send_chat(&self) {
        let t = get_text(self.chat_in);
        let t = t.trim();
        if t.is_empty() {
            return;
        }
        if let Some(n) = &self.net {
            n.send(&C2S::Chat { text: t.to_string() });
        }
        set_text(self.chat_in, "");
    }

    fn toggle_mute(&mut self) {
        if let Some(n) = self.net.clone() {
            let m = !n.self_mute.load(Ordering::Relaxed);
            n.self_mute.store(m, Ordering::Relaxed);
            n.send(&C2S::SetState { mute: m, deaf: n.deaf.load(Ordering::Relaxed) });
            set_active(self.btn_mute, m);
            BTNS.with(|b| {
                if let Some(x) = b.borrow_mut().get_mut(&(self.btn_mute as usize)) {
                    x.icon = Some(if m { IC_MIC_OFF } else { IC_MIC });
                }
            });
            invalidate(self.btn_mute, None);
            self.update_status();
        }
    }

    fn toggle_deaf(&mut self) {
        if let Some(n) = self.net.clone() {
            let d = !n.deaf.load(Ordering::Relaxed);
            n.deaf.store(d, Ordering::Relaxed);
            lk(&self.mixer).deaf = d;
            n.send(&C2S::SetState { mute: n.self_mute.load(Ordering::Relaxed), deaf: d });
            set_active(self.btn_deaf, d);
            self.update_status();
        }
    }

    fn on_command(&mut self, id: u16, code: u32, ctl: HWND) -> Option<String> {
        // Text boxes: highlight the one you're typing in.
        if code == EN_SETFOCUS || code == EN_KILLFOCUS {
            if let Some(f) = self.fields.iter().find(|f| f.edit == ctl) {
                let r = rc(f.x - 2, f.y - 2, f.w + 4, f.h + 4);
                if code == EN_SETFOCUS {
                    self.focused = ctl;
                } else if self.focused == ctl {
                    self.focused = null_mut();
                }
                invalidate(self.hwnd, Some(r));
            }
            return None;
        }
        match id {
            ID_JOIN => return self.do_join(false),
            ID_HOST => return self.do_join(true),
            ID_SETTINGS_C | ID_SETTINGS => self.open_settings(),
            ID_LIST if code == LBN_DBLCLK => self.join_selected_room(),
            ID_LIST if code == LBN_SELCHANGE => self.update_volume_ui(),
            ID_JOINROOM => self.join_selected_room(),
            ID_SEND => self.send_chat(),
            ID_MUTE => self.toggle_mute(),
            ID_DEAF => self.toggle_deaf(),
            ID_DISCONNECT => self.teardown(),
            ID_KICK => {
                let me = self.users().1;
                match self.selected_user() {
                    Some(u) if u.session != me => {
                        if let Some(n) = &self.net {
                            n.send(&C2S::Kick { session: u.session });
                        }
                    }
                    _ => return Some("Select another person in the list first.".into()),
                }
            }
            ID_SMUTE => match self.selected_user() {
                Some(u) => {
                    if let Some(n) = &self.net {
                        n.send(&C2S::ServerMute { session: u.session, muted: !u.server_mute });
                    }
                }
                None => return Some("Select a person in the list first.".into()),
            },
            ID_MOVE => {
                let idx = sendm(self.move_combo, CB_GETCURSEL, 0, 0);
                let room = if idx >= 0 { self.combo_rooms.get(idx as usize).copied() } else { None };
                match (self.selected_user(), room) {
                    (Some(u), Some(room)) => {
                        if let Some(n) = &self.net {
                            n.send(&C2S::Move { session: u.session, room });
                        }
                    }
                    _ => return Some("Select a person in the list, then pick a room in the box.".into()),
                }
            }
            ID_ADDROOM => {
                let name = get_text(self.ed_room).trim().to_string();
                if name.is_empty() {
                    return Some("Type a name for the new room in the box first.".into());
                }
                if let Some(n) = &self.net {
                    n.send(&C2S::AddRoom { name });
                }
                set_text(self.ed_room, "");
            }
            ID_DELROOM => match self.selected() {
                Some(Entry::Room(id)) => {
                    if self.combo_rooms.first() == Some(&id) {
                        return Some("The first room can't be deleted. Everyone starts there.".into());
                    }
                    if let Some(n) = &self.net {
                        n.send(&C2S::DeleteRoom { room: id });
                    }
                }
                _ => return Some("Select a room in the list first, then click the delete button.".into()),
            },
            ID_S_PTT | ID_S_VA => {
                set_active(self.s.seg_ptt, id == ID_S_PTT);
                set_active(self.s.seg_va, id == ID_S_VA);
            }
            ID_S_BEEP | ID_S_JOIN => set_active(ctl, !is_active(ctl)),
            ID_S_KEY => self.start_capture(),
            ID_S_SAVE => self.save_settings(),
            ID_S_CANCEL => self.close_settings(),
            _ => {}
        }
        None
    }

    /// Enter pressed in a text box.
    fn on_enter(&mut self, h: HWND) -> Option<Option<String>> {
        if h == self.chat_in {
            self.send_chat();
            return Some(None);
        }
        if h == self.ed_name || h == self.ed_addr || h == self.ed_port || h == self.ed_pass {
            return Some(self.do_join(false));
        }
        if h == self.ed_room {
            return Some(self.on_command(ID_ADDROOM, 0, null_mut()));
        }
        None
    }

    // ----- settings window -----

    fn open_settings(&mut self) {
        if !self.settings.is_null() {
            unsafe {
                SetFocus(self.settings);
            }
            return;
        }
        let cls = wide("SquadTalkSettings");
        let title = wide("SquadTalk settings");
        let (ww, wh) = window_size(520, 630, WS_DIALOG);
        let h = unsafe {
            CreateWindowExW(0, cls.as_ptr(), title.as_ptr(), WS_DIALOG | WS_CLIPCHILDREN, CW_USEDEFAULT, CW_USEDEFAULT, ww, wh, self.hwnd, null_mut(), GetModuleHandleW(null()), null())
        };
        if h.is_null() {
            return;
        }
        unsafe {
            dark_title_bar(h);
        }
        self.settings = h;
        let c = cfg().clone();
        self.pending_key = c.ptt_key;
        self.capturing = false;
        unsafe {
            self.s.input = make(h, "COMBOBOX", "", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 24, 112, 472, 300, 0);
            self.s.output = make(h, "COMBOBOX", "", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 24, 168, 472, 300, 0);
            dark_control(self.s.input, true);
            dark_control(self.s.output, true);
        }
        self.in_names = audio::list_devices(true);
        self.out_names = audio::list_devices(false);
        for (combo, names, current) in [
            (self.s.input, &self.in_names, &c.input_device),
            (self.s.output, &self.out_names, &c.output_device),
        ] {
            let w = wide("Windows default");
            sendm(combo, CB_ADDSTRING, 0, w.as_ptr() as isize);
            let mut sel = 0;
            for (i, n) in names.iter().enumerate() {
                let w = wide(n);
                sendm(combo, CB_ADDSTRING, 0, w.as_ptr() as isize);
                if n == current {
                    sel = i + 1;
                }
            }
            sendm(combo, CB_SETCURSEL, sel, 0);
        }
        self.s.seg_ptt = button(h, "Push-to-talk", 24, 236, 230, 38, ID_S_PTT, Kind::Segment, Some(IC_KEYBOARD), C_BG);
        self.s.seg_va = button(h, "Voice activation", 266, 236, 230, 38, ID_S_VA, Kind::Segment, Some(IC_MIC), C_BG);
        set_active(if c.voice_activation { self.s.seg_va } else { self.s.seg_ptt }, true);
        button(h, "Change key", 356, 306, 140, 40, ID_S_KEY, Kind::Secondary, Some(IC_KEYBOARD), C_BG);
        self.s.thr = slider(h, 14, 398, 492, 28, 0, 60, (c.va_threshold_db + 70).clamp(0, 60), C_BG);
        self.s.beep = button(h, "", 450, 487, 46, 24, ID_S_BEEP, Kind::Toggle, None, C_BG);
        self.s.join = button(h, "", 450, 523, 46, 24, ID_S_JOIN, Kind::Toggle, None, C_BG);
        set_active(self.s.beep, c.ptt_beeps);
        set_active(self.s.join, c.join_sounds);
        button(h, "Save", 24, 568, 230, 42, ID_S_SAVE, Kind::Primary, Some(IC_SAVE), C_BG);
        button(h, "Cancel", 266, 568, 230, 42, ID_S_CANCEL, Kind::Secondary, None, C_BG);
        unsafe {
            ShowWindow(h, SW_SHOW);
        }
    }

    fn update_level(&mut self) {
        if self.settings.is_null() {
            return;
        }
        let db = match &self.net {
            Some(n) if self.connected => n.mic_level.load(Ordering::Relaxed) / 10,
            _ => -100,
        };
        if db != self.level_db {
            self.level_db = db;
            invalidate(self.settings, Some(rc(0, 380, 520, 52)));
        }
    }

    fn start_capture(&mut self) {
        if self.settings.is_null() {
            return;
        }
        for vk in 0..256usize {
            self.ignore[vk] = key_down(vk as i32);
        }
        self.capturing = true;
        invalidate(self.settings, Some(rc(20, 302, 330, 48)));
        unsafe {
            SetTimer(self.hwnd, TIMER_CAPTURE, 20, None);
        }
    }

    fn capture_tick(&mut self) {
        if !self.capturing {
            return;
        }
        for vk in 1..255usize {
            let down = key_down(vk as i32);
            if self.ignore[vk] {
                if !down {
                    self.ignore[vk] = false;
                }
                continue;
            }
            // Skip left click and the generic Shift/Ctrl/Alt (left/right versions are used instead).
            if !down || matches!(vk, 0x01 | 0x10 | 0x11 | 0x12) {
                continue;
            }
            self.capturing = false;
            unsafe {
                KillTimer(self.hwnd, TIMER_CAPTURE);
            }
            if vk != 0x1B {
                self.pending_key = vk as i32;
            }
            if !self.settings.is_null() {
                invalidate(self.settings, Some(rc(20, 302, 330, 48)));
            }
            return;
        }
    }

    fn save_settings(&mut self) {
        if self.settings.is_null() {
            return;
        }
        let pick = |combo: HWND, names: &Vec<String>| -> String {
            let i = sendm(combo, CB_GETCURSEL, 0, 0);
            if i <= 0 {
                String::new()
            } else {
                names.get(i as usize - 1).cloned().unwrap_or_default()
            }
        };
        let input = pick(self.s.input, &self.in_names);
        let output = pick(self.s.output, &self.out_names);
        let restart = {
            let mut c = cfg();
            let changed = c.input_device != input || c.output_device != output;
            c.input_device = input;
            c.output_device = output;
            c.voice_activation = is_active(self.s.seg_va);
            c.ptt_key = self.pending_key;
            c.va_threshold_db = slider_get(self.s.thr) - 70;
            c.ptt_beeps = is_active(self.s.beep);
            c.join_sounds = is_active(self.s.join);
            changed
        };
        config::save();
        self.close_settings();
        if self.connected {
            if restart {
                self.start_audio();
                self.log("Audio devices changed.");
            }
            self.update_status();
        }
    }

    fn close_settings(&mut self) {
        if self.capturing {
            self.capturing = false;
            unsafe {
                KillTimer(self.hwnd, TIMER_CAPTURE);
            }
        }
        if !self.settings.is_null() {
            let h = self.settings;
            self.settings = null_mut();
            let s_ = std::mem::take(&mut self.s);
            SLIDERS.with(|m| m.borrow_mut().remove(&(s_.thr as usize)));
            unsafe {
                DestroyWindow(h);
                SetFocus(self.hwnd);
            }
        }
    }

    fn tick(&mut self) -> Vec<String> {
        let popups = self.process_events();
        if self.connected {
            self.render_list();
            self.update_status();
        }
        self.update_level();
        popups
    }
}

fn window_size(client_w: i32, client_h: i32, style: u32) -> (i32, i32) {
    let mut r = RECT { left: 0, top: 0, right: s(client_w), bottom: s(client_h) };
    unsafe {
        AdjustWindowRectEx(&mut r, style, 0, 0);
    }
    (r.right - r.left, r.bottom - r.top)
}

// ===================== Window procedures =====================

fn show_popups(hwnd: HWND, popups: Option<Vec<String>>) {
    if let Some(list) = popups {
        for p in list {
            popup(hwnd, &p);
        }
    }
}

/// Messages both windows handle the same way.
unsafe fn common_msg(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
    if let Some(r) = ctl_color(msg, wp, lp) {
        return Some(r);
    }
    match msg {
        WM_COMMAND => {
            let id = (wp & 0xFFFF) as u16;
            let code = ((wp >> 16) & 0xFFFF) as u32;
            if let Some(Some(text_)) = with_app(|a| a.on_command(id, code, lp as HWND)) {
                popup(hwnd, &text_);
            }
            Some(0)
        }
        WM_DRAWITEM => {
            let d = &*(lp as *const DRAWITEMSTRUCT);
            if d.CtlType == ODT_BUTTON {
                draw_button(d);
            } else if d.CtlType == ODT_LISTBOX {
                draw_list_item(d);
            }
            Some(1)
        }
        WM_MEASUREITEM => {
            let m = &mut *(lp as *mut MEASUREITEMSTRUCT);
            m.itemHeight = s(36) as u32;
            Some(1)
        }
        WM_APP_SLIDER | WM_APP_SLIDER_END => {
            let end = msg == WM_APP_SLIDER_END;
            with_app(|a| a.on_slider(lp as HWND, wp as isize as i32, end));
            Some(0)
        }
        WM_ERASEBKGND => Some(1),
        _ => None,
    }
}

unsafe extern "system" fn main_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if let Some(r) = common_msg(hwnd, msg, wp, lp) {
        return r;
    }
    match msg {
        WM_PAINT => {
            buffered_paint(hwnd, |hdc, w, h| {
                with_app(|a| unsafe { a.paint_main(hdc, w, h) }).is_some()
            });
            0
        }
        WM_APP_NOTIFY => {
            let p = with_app(|a| {
                let p = a.process_events();
                if a.connected {
                    a.render_list();
                    a.update_status();
                }
                p
            });
            show_popups(hwnd, p);
            0
        }
        WM_TIMER => {
            if wp == TIMER_CAPTURE {
                with_app(|a| a.capture_tick());
            } else {
                let p = with_app(|a| a.tick());
                show_popups(hwnd, p);
            }
            0
        }
        WM_CLOSE => {
            with_app(|a| {
                a.close_settings();
                a.teardown();
            });
            config::save();
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe extern "system" fn settings_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if let Some(r) = common_msg(hwnd, msg, wp, lp) {
        return r;
    }
    match msg {
        WM_PAINT => {
            buffered_paint(hwnd, |hdc, w, h| {
                with_app(|a| unsafe { a.paint_settings(hdc, w, h) }).is_some()
            });
            0
        }
        WM_CLOSE => {
            if with_app(|a| a.close_settings()).is_none() {
                DestroyWindow(hwnd);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

pub fn run() {
    unsafe {
        SetProcessDPIAware();
        enable_dark_mode();
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: 0x4 | 0x4000, // bars + standard controls
        };
        InitCommonControlsEx(&icc);

        let dpi = {
            let dc = GetDC(null_mut());
            let d = GetDeviceCaps(dc, 88); // LOGPIXELSX
            ReleaseDC(null_mut(), dc);
            if d > 0 {
                d
            } else {
                96
            }
        };
        DPI.store(dpi, Ordering::Relaxed);

        let hinst = GetModuleHandleW(null());
        init_theme(hinst);
        let big_icon = LoadImageW(hinst, 1usize as *const u16, 1, 0, 0, 0x40); // LR_DEFAULTSIZE
        let small_icon = LoadImageW(hinst, 1usize as *const u16, 1, s(16), s(16), 0);

        let cursor = LoadCursorW(null_mut(), IDC_ARROW);
        let main_cls = wide("SquadTalkMain");
        let settings_cls = wide("SquadTalkSettings");
        let slider_cls = wide("SquadSlider");
        for (cls, proc_, icon) in [
            (&main_cls, main_proc as WndProc, big_icon),
            (&settings_cls, settings_proc as WndProc, big_icon),
            (&slider_cls, slider_proc as WndProc, null_mut()),
        ] {
            let wc = WNDCLASSW {
                style: 0x3, // CS_HREDRAW | CS_VREDRAW
                lpfnWndProc: Some(proc_),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: icon,
                hCursor: cursor,
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: cls.as_ptr(),
            };
            RegisterClassW(&wc);
        }

        let title = wide("SquadTalk");
        let (ww, wh) = window_size(960, 680, WS_MAIN);
        let hwnd = CreateWindowExW(
            0,
            main_cls.as_ptr(),
            title.as_ptr(),
            WS_MAIN | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            ww,
            wh,
            null_mut(),
            null_mut(),
            hinst,
            null(),
        );
        if hwnd.is_null() {
            return;
        }
        dark_title_bar(hwnd);
        if !small_icon.is_null() {
            SendMessageW(hwnd, WM_SETICON, 0, small_icon as isize);
        }
        if !big_icon.is_null() {
            SendMessageW(hwnd, WM_SETICON, 1, big_icon as isize);
        }
        let app = App::new(hwnd);
        APP.with(|a| *a.borrow_mut() = Some(app));
        ShowWindow(hwnd, SW_SHOW);
        SetTimer(hwnd, TIMER_TICK, 150, None);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if msg.message == WM_KEYDOWN && msg.wParam == VK_RETURN {
                let target = msg.hwnd;
                if let Some(result) = with_app(|a| a.on_enter(target)).flatten() {
                    if let Some(text_) = result {
                        popup(hwnd, &text_);
                    }
                    continue;
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
