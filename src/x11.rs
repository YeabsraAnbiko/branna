#![allow(
    non_camel_case_types,
    non_snake_case,
    dead_code,
    non_upper_case_globals
)]
//! Hand-written FFI to libX11 and libXft (system libraries, no crates).
//! Linked by soname so no -dev packages are needed at build time.

use std::ffi::{c_char, c_int, c_long, c_uchar, c_uint, c_ulong, c_void, CString};
use std::ptr;

pub type Window = c_ulong;
pub type Atom = c_ulong;
pub type Pixmap = c_ulong;
pub type Colormap = c_ulong;
pub type Cursor = c_ulong;
pub type KeySym = c_ulong;
pub type Time = c_ulong;
pub type Display = c_void;
pub type Visual = c_void;
pub type GC = *mut c_void;

pub const KeyPress: c_int = 2;
pub const ButtonPress: c_int = 4;
pub const ButtonRelease: c_int = 5;
pub const MotionNotify: c_int = 6;
pub const FocusIn: c_int = 9;
pub const FocusOut: c_int = 10;
pub const Expose: c_int = 12;
pub const ConfigureNotify: c_int = 22;
pub const SelectionClear: c_int = 29;
pub const SelectionRequest: c_int = 30;
pub const SelectionNotify: c_int = 31;
pub const ClientMessage: c_int = 33;

pub const KeyPressMask: c_long = 1;
pub const ButtonPressMask: c_long = 1 << 2;
pub const ButtonReleaseMask: c_long = 1 << 3;
pub const PointerMotionMask: c_long = 1 << 6;
pub const ButtonMotionMask: c_long = 1 << 13;
pub const ExposureMask: c_long = 1 << 15;
pub const StructureNotifyMask: c_long = 1 << 17;
pub const FocusChangeMask: c_long = 1 << 21;

pub const XA_PRIMARY: Atom = 1;
pub const XA_ATOM: Atom = 4;
pub const XA_STRING: Atom = 31;

pub const XK_BackSpace: c_ulong = 0xff08;
pub const XK_Tab: c_ulong = 0xff09;
pub const XK_Return: c_ulong = 0xff0d;
pub const XK_Escape: c_ulong = 0xff1b;
pub const XK_Delete: c_ulong = 0xffff;
pub const XK_Home: c_ulong = 0xff50;
pub const XK_Left: c_ulong = 0xff51;
pub const XK_Up: c_ulong = 0xff52;
pub const XK_Right: c_ulong = 0xff53;
pub const XK_Down: c_ulong = 0xff54;
pub const XK_Prior: c_ulong = 0xff55;
pub const XK_Next: c_ulong = 0xff56;
pub const XK_End: c_ulong = 0xff57;
pub const XK_Insert: c_ulong = 0xff63;
pub const XK_ISO_Left_Tab: c_ulong = 0xfe20;
pub const XK_F1: c_ulong = 0xffbe;
pub const XK_F3: c_ulong = 0xffc0;
pub const XK_F4: c_ulong = 0xffc1;
pub const XK_F5: c_ulong = 0xffc2;
pub const XK_F12: c_ulong = 0xffc9;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XKeyEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: Time,
    pub x: c_int,
    pub y: c_int,
    pub x_root: c_int,
    pub y_root: c_int,
    pub state: c_uint,
    pub keycode: c_uint,
    pub same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XButtonEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: Time,
    pub x: c_int,
    pub y: c_int,
    pub x_root: c_int,
    pub y_root: c_int,
    pub state: c_uint,
    pub button: c_uint,
    pub same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XMotionEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub window: Window,
    pub root: Window,
    pub subwindow: Window,
    pub time: Time,
    pub x: c_int,
    pub y: c_int,
    pub x_root: c_int,
    pub y_root: c_int,
    pub state: c_uint,
    pub is_hint: c_char,
    pub same_screen: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XConfigureEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub event: Window,
    pub window: Window,
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
    pub border_width: c_int,
    pub above: Window,
    pub override_redirect: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XExposeEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub window: Window,
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
    pub count: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XClientMessageEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub window: Window,
    pub message_type: Atom,
    pub format: c_int,
    pub data: [c_long; 5],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XSelectionRequestEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub owner: Window,
    pub requestor: Window,
    pub selection: Atom,
    pub target: Atom,
    pub property: Atom,
    pub time: Time,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XSelectionClearEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub window: Window,
    pub selection: Atom,
    pub time: Time,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XSelectionEvent {
    pub type_: c_int,
    pub serial: c_ulong,
    pub send_event: c_int,
    pub display: *mut Display,
    pub requestor: Window,
    pub selection: Atom,
    pub target: Atom,
    pub property: Atom,
    pub time: Time,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union XEvent {
    pub type_: c_int,
    pub key: XKeyEvent,
    pub button: XButtonEvent,
    pub motion: XMotionEvent,
    pub configure: XConfigureEvent,
    pub expose: XExposeEvent,
    pub client: XClientMessageEvent,
    pub sel_req: XSelectionRequestEvent,
    pub sel_clear: XSelectionClearEvent,
    pub selection: XSelectionEvent,
    pad: [c_long; 24],
}

impl XEvent {
    pub fn zeroed() -> XEvent {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C)]
pub struct XClassHint {
    pub res_name: *mut c_char,
    pub res_class: *mut c_char,
}

#[repr(C)]
pub struct XftFont {
    pub ascent: c_int,
    pub descent: c_int,
    pub height: c_int,
    pub max_advance_width: c_int,
    charset: *mut c_void,
    pattern: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XRenderColor {
    pub red: u16,
    pub green: u16,
    pub blue: u16,
    pub alpha: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct XftColor {
    pub pixel: c_ulong,
    pub color: XRenderColor,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct XGlyphInfo {
    pub width: u16,
    pub height: u16,
    pub x: i16,
    pub y: i16,
    pub xOff: i16,
    pub yOff: i16,
}

pub enum XftDraw {}

#[link(name = "libX11.so.6", kind = "dylib", modifiers = "+verbatim")]
extern "C" {
    pub fn XOpenDisplay(name: *const c_char) -> *mut Display;
    pub fn XDefaultScreen(d: *mut Display) -> c_int;
    pub fn XRootWindow(d: *mut Display, s: c_int) -> Window;
    pub fn XDefaultVisual(d: *mut Display, s: c_int) -> *mut Visual;
    pub fn XDefaultColormap(d: *mut Display, s: c_int) -> Colormap;
    pub fn XDefaultDepth(d: *mut Display, s: c_int) -> c_int;
    pub fn XCreateSimpleWindow(
        d: *mut Display,
        parent: Window,
        x: c_int,
        y: c_int,
        w: c_uint,
        h: c_uint,
        border_w: c_uint,
        border: c_ulong,
        background: c_ulong,
    ) -> Window;
    pub fn XSelectInput(d: *mut Display, w: Window, mask: c_long) -> c_int;
    pub fn XMapWindow(d: *mut Display, w: Window) -> c_int;
    pub fn XStoreName(d: *mut Display, w: Window, name: *const c_char) -> c_int;
    pub fn XSetWMProtocols(d: *mut Display, w: Window, protos: *mut Atom, n: c_int) -> c_int;
    pub fn XSetClassHint(d: *mut Display, w: Window, hint: *mut XClassHint) -> c_int;
    pub fn XInternAtom(d: *mut Display, name: *const c_char, only_if_exists: c_int) -> Atom;
    pub fn XCreateGC(d: *mut Display, drawable: c_ulong, mask: c_ulong, values: *mut c_void) -> GC;
    pub fn XCreatePixmap(
        d: *mut Display,
        drawable: c_ulong,
        w: c_uint,
        h: c_uint,
        depth: c_uint,
    ) -> Pixmap;
    pub fn XFreePixmap(d: *mut Display, p: Pixmap) -> c_int;
    pub fn XCopyArea(
        d: *mut Display,
        src: c_ulong,
        dst: c_ulong,
        gc: GC,
        sx: c_int,
        sy: c_int,
        w: c_uint,
        h: c_uint,
        dx: c_int,
        dy: c_int,
    ) -> c_int;
    pub fn XFlush(d: *mut Display) -> c_int;
    pub fn XPending(d: *mut Display) -> c_int;
    pub fn XNextEvent(d: *mut Display, ev: *mut XEvent) -> c_int;
    pub fn XConnectionNumber(d: *mut Display) -> c_int;
    pub fn XFilterEvent(ev: *mut XEvent, w: Window) -> c_int;
    pub fn XSetLocaleModifiers(m: *const c_char) -> *mut c_char;
    pub fn XOpenIM(
        d: *mut Display,
        db: *mut c_void,
        name: *mut c_char,
        class: *mut c_char,
    ) -> *mut c_void;
    pub fn XCreateIC(im: *mut c_void, ...) -> *mut c_void;
    pub fn XSetICFocus(ic: *mut c_void);
    pub fn XUnsetICFocus(ic: *mut c_void);
    pub fn Xutf8LookupString(
        ic: *mut c_void,
        ev: *mut XKeyEvent,
        buf: *mut c_char,
        n: c_int,
        ks: *mut KeySym,
        status: *mut c_int,
    ) -> c_int;
    pub fn XLookupString(
        ev: *mut XKeyEvent,
        buf: *mut c_char,
        n: c_int,
        ks: *mut KeySym,
        status: *mut c_void,
    ) -> c_int;
    pub fn XSetSelectionOwner(d: *mut Display, sel: Atom, owner: Window, time: Time) -> c_int;
    pub fn XConvertSelection(
        d: *mut Display,
        sel: Atom,
        target: Atom,
        prop: Atom,
        requestor: Window,
        time: Time,
    ) -> c_int;
    pub fn XGetWindowProperty(
        d: *mut Display,
        w: Window,
        prop: Atom,
        offset: c_long,
        len: c_long,
        delete: c_int,
        req_type: Atom,
        actual_type: *mut Atom,
        actual_format: *mut c_int,
        nitems: *mut c_ulong,
        bytes_after: *mut c_ulong,
        prop_return: *mut *mut c_uchar,
    ) -> c_int;
    pub fn XChangeProperty(
        d: *mut Display,
        w: Window,
        prop: Atom,
        typ: Atom,
        format: c_int,
        mode: c_int,
        data: *const c_uchar,
        n: c_int,
    ) -> c_int;
    pub fn XSendEvent(
        d: *mut Display,
        w: Window,
        propagate: c_int,
        mask: c_long,
        ev: *mut XEvent,
    ) -> c_int;
    pub fn XFree(p: *mut c_void) -> c_int;
    pub fn XCreateFontCursor(d: *mut Display, shape: c_uint) -> Cursor;
    pub fn XDefineCursor(d: *mut Display, w: Window, c: Cursor) -> c_int;
}

#[link(name = "libXft.so.2", kind = "dylib", modifiers = "+verbatim")]
extern "C" {
    pub fn XftFontOpenName(d: *mut Display, screen: c_int, name: *const c_char) -> *mut XftFont;
    pub fn XftFontClose(d: *mut Display, f: *mut XftFont);
    pub fn XftDrawCreate(
        d: *mut Display,
        drawable: c_ulong,
        v: *mut Visual,
        cmap: Colormap,
    ) -> *mut XftDraw;
    pub fn XftDrawChange(draw: *mut XftDraw, drawable: c_ulong);
    pub fn XftDrawDestroy(draw: *mut XftDraw);
    pub fn XftDrawRect(
        draw: *mut XftDraw,
        c: *const XftColor,
        x: c_int,
        y: c_int,
        w: c_uint,
        h: c_uint,
    );
    pub fn XftDrawStringUtf8(
        draw: *mut XftDraw,
        c: *const XftColor,
        f: *mut XftFont,
        x: c_int,
        y: c_int,
        s: *const u8,
        len: c_int,
    );
    pub fn XftTextExtentsUtf8(
        d: *mut Display,
        f: *mut XftFont,
        s: *const u8,
        len: c_int,
        e: *mut XGlyphInfo,
    );
}

extern "C" {
    fn setlocale(cat: c_int, loc: *const c_char) -> *mut c_char;
}

pub struct Atoms {
    pub clipboard: Atom,
    pub utf8: Atom,
    pub targets: Atom,
    pub text: Atom,
    pub prop: Atom,
    pub wm_protocols: Atom,
    pub wm_delete: Atom,
    pub net_wm_name: Atom,
    pub incr: Atom,
}

pub struct Xw {
    pub dpy: *mut Display,
    pub screen: c_int,
    pub root: Window,
    pub win: Window,
    pub visual: *mut Visual,
    pub cmap: Colormap,
    pub depth: c_int,
    pub gc: GC,
    pub pixmap: Pixmap,
    pub draw: *mut XftDraw,
    pub font: *mut XftFont,
    pub bold: *mut XftFont,
    pub w: i32,
    pub h: i32,
    pub atoms: Atoms,
    pub xic: *mut c_void,
    pub cw: i32,
    pub ch: i32,
    pub ascent: i32,
    cursors: [Cursor; 3],
    cur_shape: usize,
}

fn intern(dpy: *mut Display, name: &str) -> Atom {
    let c = CString::new(name).unwrap();
    unsafe { XInternAtom(dpy, c.as_ptr(), 0) }
}

fn xc(rgb: u32) -> XftColor {
    let r = ((rgb >> 16) & 255) as u16;
    let g = ((rgb >> 8) & 255) as u16;
    let b = (rgb & 255) as u16;
    XftColor {
        pixel: (rgb & 0xffffff) as c_ulong,
        color: XRenderColor {
            red: r * 257,
            green: g * 257,
            blue: b * 257,
            alpha: 0xffff,
        },
    }
}

impl Xw {
    pub fn new(w: i32, h: i32, title: &str) -> Result<Xw, String> {
        unsafe {
            let empty = CString::new("").unwrap();
            setlocale(6, empty.as_ptr());
            let dpy = XOpenDisplay(ptr::null());
            if dpy.is_null() {
                return Err(
                    "cannot open the X display (DISPLAY not set; on Wayland this needs XWayland)"
                        .into(),
                );
            }
            let screen = XDefaultScreen(dpy);
            let root = XRootWindow(dpy, screen);
            let visual = XDefaultVisual(dpy, screen);
            let cmap = XDefaultColormap(dpy, screen);
            let depth = XDefaultDepth(dpy, screen);
            let win =
                XCreateSimpleWindow(dpy, root, 0, 0, w as c_uint, h as c_uint, 0, 0, 0x1e1e1e);
            let mask = KeyPressMask
                | ButtonPressMask
                | ButtonReleaseMask
                | PointerMotionMask
                | ButtonMotionMask
                | ExposureMask
                | StructureNotifyMask
                | FocusChangeMask;
            XSelectInput(dpy, win, mask);

            let atoms = Atoms {
                clipboard: intern(dpy, "CLIPBOARD"),
                utf8: intern(dpy, "UTF8_STRING"),
                targets: intern(dpy, "TARGETS"),
                text: intern(dpy, "TEXT"),
                prop: intern(dpy, "BRANNA_SEL"),
                wm_protocols: intern(dpy, "WM_PROTOCOLS"),
                wm_delete: intern(dpy, "WM_DELETE_WINDOW"),
                net_wm_name: intern(dpy, "_NET_WM_NAME"),
                incr: intern(dpy, "INCR"),
            };
            let mut del = atoms.wm_delete;
            XSetWMProtocols(dpy, win, &mut del, 1);

            let n1 = CString::new("branna").unwrap();
            let n2 = CString::new("Branna").unwrap();
            let mut hint = XClassHint {
                res_name: n1.as_ptr() as *mut c_char,
                res_class: n2.as_ptr() as *mut c_char,
            };
            XSetClassHint(dpy, win, &mut hint);

            XSetLocaleModifiers(empty.as_ptr());
            let mut xim = XOpenIM(dpy, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
            if xim.is_null() {
                let m = CString::new("@im=none").unwrap();
                XSetLocaleModifiers(m.as_ptr());
                xim = XOpenIM(dpy, ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
            }
            let mut xic = ptr::null_mut();
            if !xim.is_null() {
                let k1 = CString::new("inputStyle").unwrap();
                let k2 = CString::new("clientWindow").unwrap();
                let k3 = CString::new("focusWindow").unwrap();
                xic = XCreateIC(
                    xim,
                    k1.as_ptr(),
                    (0x0008 | 0x0400) as c_ulong,
                    k2.as_ptr(),
                    win,
                    k3.as_ptr(),
                    win,
                    ptr::null::<c_void>(),
                );
            }

            let gc = XCreateGC(dpy, win, 0, ptr::null_mut());
            let pixmap = XCreatePixmap(dpy, win, w as c_uint, h as c_uint, depth as c_uint);
            let draw = XftDrawCreate(dpy, pixmap, visual, cmap);
            let cursors = [
                XCreateFontCursor(dpy, 68),
                XCreateFontCursor(dpy, 152),
                XCreateFontCursor(dpy, 116),
            ];

            let mut xw = Xw {
                dpy,
                screen,
                root,
                win,
                visual,
                cmap,
                depth,
                gc,
                pixmap,
                draw,
                font: ptr::null_mut(),
                bold: ptr::null_mut(),
                w,
                h,
                atoms,
                xic,
                cw: 8,
                ch: 16,
                ascent: 12,
                cursors,
                cur_shape: 0,
            };
            xw.set_title(title);
            XMapWindow(dpy, win);
            XFlush(dpy);
            Ok(xw)
        }
    }

    pub fn conn_fd(&self) -> c_int {
        unsafe { XConnectionNumber(self.dpy) }
    }

    pub fn set_title(&mut self, title: &str) {
        let c = CString::new(title.replace('\0', "")).unwrap();
        unsafe {
            XStoreName(self.dpy, self.win, c.as_ptr());
            XChangeProperty(
                self.dpy,
                self.win,
                self.atoms.net_wm_name,
                self.atoms.utf8,
                8,
                0,
                title.as_ptr(),
                title.len() as c_int,
            );
        }
    }

    pub fn set_font(&mut self, family: &str, px: i32) -> bool {
        unsafe {
            let n1 = CString::new(format!("{}:pixelsize={}", family, px)).unwrap();
            let n2 = CString::new(format!("{}:pixelsize={}:weight=bold", family, px)).unwrap();
            let f = XftFontOpenName(self.dpy, self.screen, n1.as_ptr());
            if f.is_null() {
                return false;
            }
            let b = XftFontOpenName(self.dpy, self.screen, n2.as_ptr());
            if !self.font.is_null() {
                XftFontClose(self.dpy, self.font);
            }
            if !self.bold.is_null() {
                XftFontClose(self.dpy, self.bold);
            }
            self.font = f;
            self.bold = b;
            let mut gi = XGlyphInfo::default();
            XftTextExtentsUtf8(self.dpy, f, b"M".as_ptr(), 1, &mut gi);
            self.cw = (gi.xOff as i32).max(1);
            self.ch = (*f).height.max(1);
            self.ascent = (*f).ascent;
            true
        }
    }

    pub fn resize_buffers(&mut self, w: i32, h: i32) {
        unsafe {
            let p = XCreatePixmap(
                self.dpy,
                self.win,
                w.max(1) as c_uint,
                h.max(1) as c_uint,
                self.depth as c_uint,
            );
            XftDrawChange(self.draw, p);
            XFreePixmap(self.dpy, self.pixmap);
            self.pixmap = p;
            self.w = w;
            self.h = h;
        }
    }

    pub fn fill(&self, x: i32, y: i32, w: i32, h: i32, rgb: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let c = xc(rgb);
        unsafe { XftDrawRect(self.draw, &c, x, y, w as c_uint, h as c_uint) }
    }

    pub fn text(&self, x: i32, base_y: i32, s: &str, rgb: u32, bold: bool) {
        if s.is_empty() {
            return;
        }
        let c = xc(rgb);
        let f = if bold && !self.bold.is_null() {
            self.bold
        } else {
            self.font
        };
        unsafe { XftDrawStringUtf8(self.draw, &c, f, x, base_y, s.as_ptr(), s.len() as c_int) }
    }

    pub fn present(&self) {
        unsafe {
            XCopyArea(
                self.dpy,
                self.pixmap,
                self.win,
                self.gc,
                0,
                0,
                self.w.max(1) as c_uint,
                self.h.max(1) as c_uint,
                0,
                0,
            );
            XFlush(self.dpy);
        }
    }

    pub fn set_cursor(&mut self, shape: usize) {
        if shape != self.cur_shape {
            self.cur_shape = shape;
            unsafe {
                XDefineCursor(self.dpy, self.win, self.cursors[shape]);
                XFlush(self.dpy);
            }
        }
    }

    pub fn lookup(&self, ev: &mut XKeyEvent) -> (KeySym, String) {
        let mut buf = [0u8; 64];
        let mut ks: KeySym = 0;
        unsafe {
            if !self.xic.is_null() {
                let mut status: c_int = 0;
                let n = Xutf8LookupString(
                    self.xic,
                    ev,
                    buf.as_mut_ptr() as *mut c_char,
                    63,
                    &mut ks,
                    &mut status,
                );
                let text = if status == 2 || status == 4 {
                    String::from_utf8_lossy(&buf[..(n.max(0) as usize).min(63)]).into_owned()
                } else {
                    String::new()
                };
                if status != 3 && status != 4 {
                    ks = 0;
                }
                (ks, text)
            } else {
                let n = XLookupString(
                    ev,
                    buf.as_mut_ptr() as *mut c_char,
                    63,
                    &mut ks,
                    ptr::null_mut(),
                );
                let text: String = buf[..(n.max(0) as usize).min(63)]
                    .iter()
                    .map(|&b| b as char)
                    .collect();
                (ks, text)
            }
        }
    }
}
