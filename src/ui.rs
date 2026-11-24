//! The SquadTalk window, built from plain Win32 controls so it uses very
//! little memory and no GPU.

use crate::audio::{self, AudioEngine, Mixer};
use crate::client::{self, Event, Net};
use crate::config::{self, cfg};
use crate::protocol::{self, lk, UserInfo, C2S};
use crate::server::{self, ServerHandle};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontIndirectW, GetDC, GetDeviceCaps, GetSysColorBrush, InvalidateRect, ReleaseDC, HFONT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{InitCommonControlsEx, INITCOMMONCONTROLSEX};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetAsyncKeyState, GetKeyNameTextW, MapVirtualKeyW, SetFocus,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, GetWindowTextLengthW,
    GetWindowTextW, KillTimer, LoadCursorW, MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassW,
    SendMessageW, SetProcessDPIAware, SetTimer, SetWindowTextW, ShowWindow, SystemParametersInfoW,
    TranslateMessage, IDC_ARROW, MSG, NONCLIENTMETRICSW, WNDCLASSW,
};

// ----- Win32 numbers (written out to stay independent of binding types) -----
const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const WS_GROUP: u32 = 0x0002_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const WS_CLIPCHILDREN: u32 = 0x0200_0000;
const WS_MAIN: u32 = 0x00CA_0000; // overlapped + caption + system menu + minimize
const WS_DIALOG: u32 = 0x00C8_0000; // caption + system menu
const WS_EX_CLIENTEDGE: u32 = 0x200;
const ES_PASSWORD: u32 = 0x20;
const ES_AUTOVSCROLL: u32 = 0x40;
const ES_AUTOHSCROLL: u32 = 0x80;
const ES_MULTILINE: u32 = 0x4;
const ES_READONLY: u32 = 0x800;
const ES_NUMBER: u32 = 0x2000;
const BS_AUTOCHECKBOX: u32 = 3;
const BS_AUTORADIOBUTTON: u32 = 9;
const BM_GETCHECK: u32 = 0xF0;
const BM_SETCHECK: u32 = 0xF1;
const LBS_NOTIFY: u32 = 1;
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
const TBM_GETPOS: u32 = 0x400;
const TBM_SETPOS: u32 = 0x405;
const TBM_SETRANGE: u32 = 0x406;
const TBS_NOTICKS: u32 = 0x10;
const TB_ENDTRACK: u32 = 8;
const WM_DESTROY: u32 = 0x02;
const WM_SETREDRAW: u32 = 0x0B;
const WM_CLOSE: u32 = 0x10;
const WM_SETFONT: u32 = 0x30;
const WM_KEYDOWN: u32 = 0x100;
const WM_COMMAND: u32 = 0x111;
const WM_TIMER: u32 = 0x113;
const WM_HSCROLL: u32 = 0x114;
const WM_APP_NOTIFY: u32 = 0x8001;
const VK_RETURN: usize = 0x0D;
const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const CW_USEDEFAULT: i32 = i32::MIN;
const MB_ICONWARNING: u32 = 0x30;

// ----- Control IDs -----
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
const ID_S_KEY: u16 = 305;
const ID_S_SAVE: u16 = 309;

const TIMER_TICK: usize = 1;
const TIMER_CAPTURE: usize = 2;

// ----- Small helpers -----
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
fn set_text_if_changed(h: HWND, s: &str) {
    if get_text(h) != s {
        set_text(h, s);
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
fn is_checked(h: HWND) -> bool {
    sendm(h, BM_GETCHECK, 0, 0) == 1
}
fn set_checked(h: HWND, on: bool) {
    sendm(h, BM_SETCHECK, on as usize, 0);
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
        0x01 => return "Left mouse button".into(),
        0x02 => return "Right mouse button".into(),
        0x04 => return "Middle mouse button".into(),
        0x05 => return "Mouse button 4 (side, back)".into(),
        0x06 => return "Mouse button 5 (side, forward)".into(),
        0xC0 => return "` ~ (left of 1)".into(),
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

// ----- The app -----

#[derive(Clone, Copy, PartialEq)]
enum Entry {
    Room(u32),
    User(u32),
}

struct SettingsCtrls {
    input: HWND,
    output: HWND,
    ptt: HWND,
    va: HWND,
    key_label: HWND,
    thr_label: HWND,
    thr: HWND,
    level: HWND,
    beep: HWND,
    join: HWND,
}

impl Default for SettingsCtrls {
    fn default() -> Self {
        let n = null_mut();
        SettingsCtrls {
            input: n,
            output: n,
            ptt: n,
            va: n,
            key_label: n,
            thr_label: n,
            thr: n,
            level: n,
            beep: n,
            join: n,
        }
    }
}

struct App {
    hwnd: HWND,
    font: HFONT,
    dpi: i32,
    // connect page
    c_ctrls: Vec<HWND>,
    ed_name: HWND,
    ed_addr: HWND,
    ed_port: HWND,
    ed_pass: HWND,
    c_status: HWND,
    // main page
    m_ctrls: Vec<HWND>,
    admin: Vec<HWND>,
    list: HWND,
    vol_label: HWND,
    vol_bar: HWND,
    chat_log: HWND,
    chat_in: HWND,
    btn_mute: HWND,
    btn_deaf: HWND,
    status: HWND,
    move_combo: HWND,
    ed_room: HWND,
    // settings window
    settings: HWND,
    s: SettingsCtrls,
    in_names: Vec<String>,
    out_names: Vec<String>,
    pending_key: i32,
    capturing: bool,
    ignore: [bool; 256],
    // session
    net: Option<Arc<Net>>,
    server: Option<ServerHandle>,
    audio: Option<AudioEngine>,
    mixer: Arc<Mutex<Mixer>>,
    connected: bool,
    hosting: bool,
    entries: Vec<Entry>,
    last_lines: Vec<String>,
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

impl App {
    fn ctl(&self, parent: HWND, class: &str, text: &str, style: u32, ex: u32, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
        let sc = |v: i32| v * self.dpi / 96;
        let cls = wide(class);
        let txt = wide(text);
        unsafe {
            let hwnd = CreateWindowExW(
                ex,
                cls.as_ptr(),
                txt.as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                sc(x),
                sc(y),
                sc(w),
                sc(h),
                parent,
                id as usize as _,
                GetModuleHandleW(null()),
                null(),
            );
            SendMessageW(hwnd, WM_SETFONT, self.font as usize, 1);
            hwnd
        }
    }
    fn label(&self, parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32) -> HWND {
        self.ctl(parent, "STATIC", text, 0, 0, x, y, w, h, 0)
    }
    fn button(&self, parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
        self.ctl(parent, "BUTTON", text, WS_TABSTOP, 0, x, y, w, h, id)
    }
    fn edit(&self, parent: HWND, text: &str, style: u32, x: i32, y: i32, w: i32, h: i32, id: u16) -> HWND {
        self.ctl(parent, "EDIT", text, WS_TABSTOP | ES_AUTOHSCROLL | style, WS_EX_CLIENTEDGE, x, y, w, h, id)
    }

    fn new(hwnd: HWND, font: HFONT, dpi: i32) -> App {
        let mut a = App {
            hwnd,
            font,
            dpi,
            c_ctrls: vec![],
            ed_name: null_mut(),
            ed_addr: null_mut(),
            ed_port: null_mut(),
            ed_pass: null_mut(),
            c_status: null_mut(),
            m_ctrls: vec![],
            admin: vec![],
            list: null_mut(),
            vol_label: null_mut(),
            vol_bar: null_mut(),
            chat_log: null_mut(),
            chat_in: null_mut(),
            btn_mute: null_mut(),
            btn_deaf: null_mut(),
            status: null_mut(),
            move_combo: null_mut(),
            ed_room: null_mut(),
            settings: null_mut(),
            s: SettingsCtrls::default(),
            in_names: vec![],
            out_names: vec![],
            pending_key: 0,
            capturing: false,
            ignore: [false; 256],
            net: None,
            server: None,
            audio: None,
            mixer: Arc::new(Mutex::new(Mixer::new())),
            connected: false,
            hosting: false,
            entries: vec![],
            last_lines: vec![],
            combo_rooms: vec![],
            my_room: None,
            members: HashMap::new(),
        };
        a.build_connect_page();
        a.build_main_page();
        a.show_page(false);
        a
    }

    fn build_connect_page(&mut self) {
        let p = self.hwnd;
        let (name, addr, port, pass) = {
            let c = cfg();
            (c.name.clone(), c.address.clone(), c.port.to_string(), c.password.clone())
        };
        let x = 40;
        let mut v = vec![
            self.label(p, "SquadTalk — lightweight voice chat for gaming", x, 18, 600, 22),
            self.label(p, "Your name (what your friends will see)", x, 58, 400, 20),
        ];
        self.ed_name = self.edit(p, &name, 0, x, 78, 320, 26, 0);
        v.push(self.ed_name);
        v.push(self.label(p, "Server address (ask the server owner, looks like 140.238.12.34)", x, 116, 600, 20));
        self.ed_addr = self.edit(p, &addr, 0, x, 136, 320, 26, 0);
        v.push(self.ed_addr);
        v.push(self.label(p, "Port (leave as 9987 unless the owner changed it)", x, 174, 400, 20));
        self.ed_port = self.edit(p, &port, ES_NUMBER, x, 194, 100, 26, 0);
        v.push(self.ed_port);
        v.push(self.label(p, "Server password (leave empty if there is none)", x, 232, 400, 20));
        self.ed_pass = self.edit(p, &pass, ES_PASSWORD, x, 252, 320, 26, 0);
        v.push(self.ed_pass);
        v.push(self.button(p, "Join server", x, 296, 320, 34, ID_JOIN));
        v.push(self.button(p, "Audio and push-to-talk settings", x, 344, 320, 30, ID_SETTINGS_C));
        self.c_status = self.label(p, "", x, 384, 700, 20);
        v.push(self.c_status);
        v.push(self.label(
            p,
            "Advanced: run a temporary server on this PC, for people on the same Wi-Fi.",
            x,
            470,
            700,
            20,
        ));
        v.push(self.button(p, "Host on this PC (same Wi-Fi only)", x, 492, 320, 28, ID_HOST));
        unsafe {
            EnableWindow(self.ed_name, 1);
            SendMessageW(self.ed_name, EM_SETLIMITTEXT, 24, 0);
        }
        self.c_ctrls = v;
    }

    fn build_main_page(&mut self) {
        let p = self.hwnd;
        let mut v = vec![self.label(p, "Rooms and people — double-click a room to join it", 10, 10, 330, 20)];
        self.list = self.ctl(
            p,
            "LISTBOX",
            "",
            WS_TABSTOP | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT,
            WS_EX_CLIENTEDGE,
            10,
            30,
            330,
            300,
            ID_LIST,
        );
        v.push(self.list);
        self.vol_label = self.label(p, "Select a person in the list to change their volume", 10, 338, 330, 20);
        v.push(self.vol_label);
        self.vol_bar = self.ctl(p, "msctls_trackbar32", "", WS_TABSTOP | TBS_NOTICKS, 0, 10, 358, 330, 30, 0);
        sendm(self.vol_bar, TBM_SETRANGE, 1, (200 << 16) as isize);
        sendm(self.vol_bar, TBM_SETPOS, 1, 100);
        enable(self.vol_bar, false);
        v.push(self.vol_bar);
        v.push(self.button(p, "Join selected room", 10, 394, 160, 28, ID_JOINROOM));

        // Owner tools
        let mut ad = vec![self.label(p, "Owner tools (select a person or room in the list first):", 10, 432, 330, 20)];
        ad.push(self.button(p, "Kick", 10, 454, 100, 28, ID_KICK));
        ad.push(self.button(p, "Server-mute", 115, 454, 110, 28, ID_SMUTE));
        ad.push(self.button(p, "Delete room", 230, 454, 110, 28, ID_DELROOM));
        self.move_combo = self.ctl(p, "COMBOBOX", "", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 0, 10, 490, 215, 300, 0);
        ad.push(self.move_combo);
        ad.push(self.button(p, "Move person here", 230, 488, 110, 28, ID_MOVE));
        self.ed_room = self.edit(p, "", 0, 10, 524, 215, 26, 0);
        sendm(self.ed_room, EM_SETLIMITTEXT, 40, 0);
        ad.push(self.ed_room);
        ad.push(self.button(p, "Add room", 230, 523, 110, 28, ID_ADDROOM));
        self.admin = ad;

        // Chat
        v.push(self.label(p, "Room chat", 355, 10, 300, 20));
        self.chat_log = self.ctl(
            p,
            "EDIT",
            "",
            ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | WS_VSCROLL,
            WS_EX_CLIENTEDGE,
            355,
            30,
            415,
            400,
            0,
        );
        v.push(self.chat_log);
        self.chat_in = self.edit(p, "", 0, 355, 438, 330, 26, 0);
        sendm(self.chat_in, EM_SETLIMITTEXT, 500, 0);
        v.push(self.chat_in);
        v.push(self.button(p, "Send", 690, 437, 80, 28, ID_SEND));

        // Controls
        self.btn_mute = self.button(p, "Mute mic", 355, 478, 100, 32, ID_MUTE);
        self.btn_deaf = self.button(p, "Deafen", 460, 478, 100, 32, ID_DEAF);
        v.push(self.btn_mute);
        v.push(self.btn_deaf);
        v.push(self.button(p, "Settings", 565, 478, 100, 32, ID_SETTINGS));
        v.push(self.button(p, "Disconnect", 670, 478, 100, 32, ID_DISCONNECT));
        self.status = self.label(p, "", 355, 522, 415, 40);
        v.push(self.status);
        self.m_ctrls = v;
    }

    fn show_page(&self, connected: bool) {
        for &h in &self.c_ctrls {
            show(h, !connected);
        }
        for &h in &self.m_ctrls {
            show(h, connected);
        }
        let owner = connected && self.is_owner();
        for &h in &self.admin {
            show(h, owner);
        }
    }

    fn is_owner(&self) -> bool {
        self.net.as_ref().map(|n| lk(&n.shared).owner).unwrap_or(false)
    }

    fn log(&self, text: &str) {
        unsafe {
            let mut len = GetWindowTextLengthW(self.chat_log);
            if len > 30000 {
                set_text(self.chat_log, "");
                len = 0;
            }
            let line = wide(&format!("{text}\r\n"));
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
            addr = "127.0.0.1".into();
        }
        self.hosting = host;
        let secret = cfg().secret.clone();
        let hwnd_val = self.hwnd as usize;
        let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(move || unsafe {
            PostMessageW(hwnd_val as HWND, WM_APP_NOTIFY, 0, 0);
        });
        self.net = Some(client::connect(addr, port, name, secret, password, notify, self.mixer.clone()));
        set_text(self.c_status, if host { "Starting your server…" } else { "Connecting…" });
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
        set_text(self.c_status, "");
        set_text(self.chat_log, "");
        set_text(self.btn_mute, "Mute mic");
        set_text(self.btn_deaf, "Deafen");
        lk(&self.mixer).clear();
        self.show_page(true);
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
            self.log("Connected to the server.");
        }
        let (va, key) = {
            let c = cfg();
            (c.voice_activation, c.ptt_key)
        };
        if va {
            self.log("Voice activation is on: just talk.");
        } else {
            self.log(&format!("Hold [{}] to talk. Change it in Settings.", key_name(key)));
        }
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
        self.last_lines.clear();
        self.combo_rooms.clear();
        self.my_room = None;
        self.members.clear();
        sendm(self.list, LB_RESETCONTENT, 0, 0);
        self.show_page(false);
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
                Event::Chat { name, text } => self.log(&format!("{name}: {text}")),
                Event::Failed(r) => {
                    popups.push(format!("Could not connect.\n\n{r}"));
                    self.teardown();
                    set_text(self.c_status, "");
                    break;
                }
                Event::Denied(r) => {
                    popups.push(format!("The server did not let you in:\n\n{r}"));
                    self.teardown();
                    set_text(self.c_status, "");
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
                self.log(&format!("— You are in room \"{rname}\" —"));
            } else {
                let mut joined = false;
                let mut left = false;
                for (s, n) in &members {
                    if !self.members.contains_key(s) {
                        self.log(&format!("{n} joined the room."));
                        joined = true;
                    }
                }
                for (s, n) in &self.members {
                    if !members.contains_key(s) {
                        self.log(&format!("{n} left the room."));
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
        self.show_page(true);
        self.render_list();
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
            Some(Entry::User(s)) => self.users().0.into_iter().find(|u| u.session == s),
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
        let mut lines = Vec::new();
        let mut entries = Vec::new();
        for r in &rooms {
            let mut people: Vec<&UserInfo> = users.iter().filter(|u| u.room == r.id).collect();
            people.sort_by_key(|u| u.name.to_lowercase());
            lines.push(format!("▸ {}   ({})", r.name, people.len()));
            entries.push(Entry::Room(r.id));
            for u in people {
                let mut s = format!(
                    "        {}  {}",
                    if speaking.contains(&u.session) { "●" } else { "○" },
                    u.name
                );
                if u.session == me {
                    s.push_str("  (you)");
                }
                if u.owner {
                    s.push_str("  [owner]");
                }
                if u.server_mute {
                    s.push_str("  [server-muted]");
                } else if u.mute {
                    s.push_str("  [mic off]");
                }
                if u.deaf {
                    s.push_str("  [deafened]");
                }
                lines.push(s);
                entries.push(Entry::User(u.session));
            }
        }
        if lines == self.last_lines {
            return;
        }
        let sel = self.selected();
        let top = sendm(self.list, LB_GETTOPINDEX, 0, 0).max(0) as usize;
        sendm(self.list, WM_SETREDRAW, 0, 0);
        sendm(self.list, LB_RESETCONTENT, 0, 0);
        for l in &lines {
            let w = wide(l);
            sendm(self.list, LB_ADDSTRING, 0, w.as_ptr() as isize);
        }
        if let Some(e) = sel {
            if let Some(i) = entries.iter().position(|x| *x == e) {
                sendm(self.list, LB_SETCURSEL, i, 0);
            }
        }
        sendm(self.list, LB_SETTOPINDEX, top, 0);
        sendm(self.list, WM_SETREDRAW, 1, 0);
        unsafe {
            InvalidateRect(self.list, null(), 1);
        }
        self.entries = entries;
        self.last_lines = lines;
        self.update_volume_ui();
    }

    fn update_volume_ui(&self) {
        let me = self.users().1;
        if let Some(u) = self.selected_user() {
            if u.session != me {
                let v = cfg().volumes.get(&u.uid).copied().unwrap_or(100);
                enable(self.vol_bar, true);
                if sendm(self.vol_bar, TBM_GETPOS, 0, 0) != v as isize {
                    sendm(self.vol_bar, TBM_SETPOS, 1, v as isize);
                }
                set_text_if_changed(self.vol_label, &format!("Volume for {}: {}%", u.name, v));
                return;
            }
        }
        enable(self.vol_bar, false);
        set_text_if_changed(self.vol_label, "Select a person in the list to change their volume");
    }

    fn on_volume(&mut self, code: u32) {
        let pos = sendm(self.vol_bar, TBM_GETPOS, 0, 0).clamp(0, 200) as u32;
        if let Some(u) = self.selected_user() {
            cfg().volumes.insert(u.uid.clone(), pos);
            lk(&self.mixer).volumes.insert(u.session, pos as f32 / 100.0);
            set_text_if_changed(self.vol_label, &format!("Volume for {}: {}%", u.name, pos));
            if code == TB_ENDTRACK {
                config::save();
            }
        }
    }

    fn update_status(&self) {
        let net = match &self.net {
            Some(n) => n,
            None => return,
        };
        let ping = lk(&net.shared).ping_ms;
        let (va, key) = {
            let c = cfg();
            (c.voice_activation, c.ptt_key)
        };
        let mut s = String::from(if self.hosting { "Hosting" } else { "Connected" });
        if let Some(p) = ping {
            s.push_str(&format!("  ·  ping {p} ms"));
        }
        if va {
            s.push_str("  ·  Voice activation");
        } else {
            s.push_str(&format!("  ·  Talk key: {}", key_name(key)));
        }
        if net.self_speaking.load(Ordering::Relaxed) {
            s.push_str("\r\n● You are talking");
        }
        set_text_if_changed(self.status, &s);
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

    fn toggle_mute(&self) {
        if let Some(n) = &self.net {
            let m = !n.self_mute.load(Ordering::Relaxed);
            n.self_mute.store(m, Ordering::Relaxed);
            n.send(&C2S::SetState { mute: m, deaf: n.deaf.load(Ordering::Relaxed) });
            set_text(self.btn_mute, if m { "Unmute mic" } else { "Mute mic" });
            n.notify();
        }
    }

    fn toggle_deaf(&self) {
        if let Some(n) = &self.net {
            let d = !n.deaf.load(Ordering::Relaxed);
            n.deaf.store(d, Ordering::Relaxed);
            lk(&self.mixer).deaf = d;
            n.send(&C2S::SetState { mute: n.self_mute.load(Ordering::Relaxed), deaf: d });
            set_text(self.btn_deaf, if d { "Undeafen" } else { "Deafen" });
        }
    }

    fn on_command(&mut self, id: u16, code: u32) -> Option<String> {
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
                _ => return Some("Select a room in the list first.".into()),
            },
            ID_S_KEY => self.start_capture(),
            ID_S_SAVE => self.save_settings(),
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
        let sc = |v: i32| v * self.dpi / 96;
        let cls = wide("SquadTalkSettings");
        let title = wide("SquadTalk settings");
        let h = unsafe {
            CreateWindowExW(
                0,
                cls.as_ptr(),
                title.as_ptr(),
                WS_DIALOG | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                sc(500),
                sc(470),
                self.hwnd,
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            )
        };
        if h.is_null() {
            return;
        }
        self.settings = h;
        let c = cfg().clone();
        self.pending_key = c.ptt_key;
        let x = 20;
        self.label(h, "Microphone", x, 14, 440, 20);
        self.s.input = self.ctl(h, "COMBOBOX", "", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 0, x, 34, 440, 300, 0);
        self.label(h, "Speakers / headphones", x, 68, 440, 20);
        self.s.output = self.ctl(h, "COMBOBOX", "", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 0, x, 88, 440, 300, 0);
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
        self.s.ptt = self.ctl(
            h,
            "BUTTON",
            "Push-to-talk — hold a key or mouse button to talk",
            WS_TABSTOP | WS_GROUP | BS_AUTORADIOBUTTON,
            0,
            x,
            128,
            440,
            22,
            0,
        );
        self.s.va = self.ctl(
            h,
            "BUTTON",
            "Voice activation — your mic turns on when you speak",
            BS_AUTORADIOBUTTON,
            0,
            x,
            152,
            440,
            22,
            0,
        );
        set_checked(if c.voice_activation { self.s.va } else { self.s.ptt }, true);
        self.s.key_label = self.label(h, &format!("Push-to-talk key: {}", key_name(c.ptt_key)), x, 192, 290, 40);
        self.button(h, "Change key…", 320, 186, 140, 30, ID_S_KEY);
        self.s.thr_label = self.label(h, "", x, 236, 440, 20);
        self.s.thr = self.ctl(h, "msctls_trackbar32", "", WS_TABSTOP | TBS_NOTICKS, 0, x, 256, 440, 30, 0);
        sendm(self.s.thr, TBM_SETRANGE, 1, (60 << 16) as isize);
        sendm(self.s.thr, TBM_SETPOS, 1, (c.va_threshold_db + 70).clamp(0, 60) as isize);
        self.update_threshold_label();
        self.s.level = self.label(h, "", x, 290, 440, 20);
        self.s.beep = self.ctl(h, "BUTTON", "Beep when push-to-talk starts and stops", WS_TABSTOP | BS_AUTOCHECKBOX, 0, x, 322, 440, 22, 0);
        set_checked(self.s.beep, c.ptt_beeps);
        self.s.join = self.ctl(h, "BUTTON", "Play a sound when people join or leave your room", WS_TABSTOP | BS_AUTOCHECKBOX, 0, x, 346, 440, 22, 0);
        set_checked(self.s.join, c.join_sounds);
        self.button(h, "Save", x, 384, 140, 32, ID_S_SAVE);
        self.label(h, "Close this window without saving to cancel.", 175, 392, 300, 20);
        self.update_level_label();
        unsafe {
            ShowWindow(h, SW_SHOW);
        }
    }

    fn update_threshold_label(&self) {
        let db = sendm(self.s.thr, TBM_GETPOS, 0, 0) as i32 - 70;
        set_text_if_changed(
            self.s.thr_label,
            &format!("Voice activation level: {db} dB  (move left = picks up quieter speech)"),
        );
    }

    fn update_level_label(&self) {
        if self.settings.is_null() {
            return;
        }
        let text = match &self.net {
            Some(n) if self.connected => {
                let db = n.mic_level.load(Ordering::Relaxed) / 10;
                format!("Your mic right now: {db} dB  (speak to see it rise)")
            }
            _ => "Your mic level shows here while you're connected to a server.".to_string(),
        };
        set_text_if_changed(self.s.level, &text);
    }

    fn start_capture(&mut self) {
        if self.settings.is_null() {
            return;
        }
        for vk in 0..256usize {
            self.ignore[vk] = key_down(vk as i32);
        }
        self.capturing = true;
        set_text(self.s.key_label, "Press the key or mouse button you want…\r\n(Esc cancels)");
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
                set_text(self.s.key_label, &format!("Push-to-talk key: {}", key_name(self.pending_key)));
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
            c.voice_activation = is_checked(self.s.va);
            c.ptt_key = self.pending_key;
            c.va_threshold_db = sendm(self.s.thr, TBM_GETPOS, 0, 0) as i32 - 70;
            c.ptt_beeps = is_checked(self.s.beep);
            c.join_sounds = is_checked(self.s.join);
            changed
        };
        config::save();
        self.close_settings();
        if restart && self.connected {
            self.start_audio();
            self.log("Audio devices changed.");
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
            self.s = SettingsCtrls::default();
            unsafe {
                DestroyWindow(h);
                SetFocus(self.hwnd);
            }
        }
    }

    fn on_hscroll(&mut self, bar: HWND, code: u32) {
        if bar == self.vol_bar {
            self.on_volume(code);
        } else if !self.settings.is_null() && bar == self.s.thr {
            self.update_threshold_label();
        }
    }

    fn tick(&mut self) -> Vec<String> {
        let popups = self.process_events();
        if self.connected {
            self.render_list();
            self.update_status();
        }
        self.update_level_label();
        popups
    }
}

// ----- Window procedures -----

fn show_popups(hwnd: HWND, popups: Option<Vec<String>>) {
    if let Some(list) = popups {
        for p in list {
            popup(hwnd, &p);
        }
    }
}

unsafe extern "system" fn main_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wp & 0xFFFF) as u16;
            let code = ((wp >> 16) & 0xFFFF) as u32;
            if let Some(Some(text)) = with_app(|a| a.on_command(id, code)) {
                popup(hwnd, &text);
            }
            0
        }
        WM_HSCROLL => {
            let code = (wp & 0xFFFF) as u32;
            with_app(|a| a.on_hscroll(lp as HWND, code));
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
    match msg {
        WM_COMMAND => {
            let id = (wp & 0xFFFF) as u16;
            let code = ((wp >> 16) & 0xFFFF) as u32;
            if let Some(Some(text)) = with_app(|a| a.on_command(id, code)) {
                popup(hwnd, &text);
            }
            0
        }
        WM_HSCROLL => {
            let code = (wp & 0xFFFF) as u32;
            with_app(|a| a.on_hscroll(lp as HWND, code));
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

pub fn run() {
    unsafe {
        SetProcessDPIAware();
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: 0x4 | 0x4000, // trackbars + standard controls
        };
        InitCommonControlsEx(&icc);

        let hinst = GetModuleHandleW(null());
        let bg = GetSysColorBrush(15); // COLOR_BTNFACE
        let cursor = LoadCursorW(null_mut(), IDC_ARROW);
        let main_cls = wide("SquadTalkMain");
        let settings_cls = wide("SquadTalkSettings");
        for (cls, proc_) in [
            (&main_cls, main_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT),
            (&settings_cls, settings_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT),
        ] {
            let wc = WNDCLASSW {
                style: 0x3, // CS_HREDRAW | CS_VREDRAW
                lpfnWndProc: Some(proc_),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: null_mut(),
                hCursor: cursor,
                hbrBackground: bg,
                lpszMenuName: null(),
                lpszClassName: cls.as_ptr(),
            };
            RegisterClassW(&wc);
        }

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
        let mut ncm: NONCLIENTMETRICSW = std::mem::zeroed();
        ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        SystemParametersInfoW(0x29, ncm.cbSize, &mut ncm as *mut _ as *mut c_void, 0);
        let font = CreateFontIndirectW(&ncm.lfMessageFont);

        let sc = |v: i32| v * dpi / 96;
        let title = wide("SquadTalk");
        let hwnd = CreateWindowExW(
            0,
            main_cls.as_ptr(),
            title.as_ptr(),
            WS_MAIN | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            sc(800),
            sc(610),
            null_mut(),
            null_mut(),
            hinst,
            null(),
        );
        if hwnd.is_null() {
            return;
        }
        let app = App::new(hwnd, font, dpi);
        APP.with(|a| *a.borrow_mut() = Some(app));
        ShowWindow(hwnd, SW_SHOW);
        SetTimer(hwnd, TIMER_TICK, 150, None);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if msg.message == WM_KEYDOWN && msg.wParam == VK_RETURN {
                let target = msg.hwnd;
                if let Some(result) = with_app(|a| a.on_enter(target)).flatten() {
                    if let Some(text) = result {
                        popup(hwnd, &text);
                    }
                    continue;
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
