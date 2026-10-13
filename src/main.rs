#![allow(non_upper_case_globals)]
mod editor;
mod term;
mod tree;
mod x11;

use editor::*;
use std::ffi::{c_int, c_uchar, c_ulong};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use term::*;
use tree::*;
use x11::*;

const BG: u32 = 0x1e1e1e;
const FG: u32 = 0xd4d4d4;
const GUTTER_FG: u32 = 0x6e7681;
const GUTTER_FG_CUR: u32 = 0xc6c6c6;
const CURLINE: u32 = 0x282828;
const SEL_BG: u32 = 0x264f78;
const FIND_BG: u32 = 0x5a4a1e;
const CURSOR_COL: u32 = 0xe0e0e0;
const CURSOR_DIM: u32 = 0x6e6e6e;
const STATUS_BG: u32 = 0x252526;
const STATUS_FG: u32 = 0xcccccc;
const ACCENT: u32 = 0x0e639c;
const WARN: u32 = 0xe5c07b;
const SPLIT_COL: u32 = 0x2f2f2f;
const TERM_BG: u32 = 0x181818;
const TERM_FG: u32 = 0xcccccc;
const TREE_BG: u32 = 0x252526;
const TREE_SEL: u32 = 0x37373d;
const TREE_SEL_FOCUS: u32 = 0x094771;
const TREE_DIR: u32 = 0x7fb4ff;
const SPLIT_H: i32 = 5;
const PAD: i32 = 6;

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Editor,
    Term,
    Tree,
}

#[derive(Clone, PartialEq)]
enum PromptKind {
    Open,
    SaveAs,
    Find,
    Goto,
    ReplaceFind,
    ReplaceWith(String),
    NewFile(PathBuf),
    NewDir(PathBuf),
    Rename(PathBuf),
    Delete(PathBuf),
    DeleteMarked,
}

struct PromptState {
    kind: PromptKind,
    text: String,
    origin: Pos,
}

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    None,
    Editor,
    Term,
    Split,
}

#[derive(Clone, Copy)]
enum PasteTarget {
    Editor,
    Term,
    Prompt,
}

#[derive(Clone, Copy)]
struct Layout {
    status_h: i32,
    tab_h: i32,
    ed_h: i32,
    split_y: i32,
    term_y: i32,
    term_h: i32,
    tcols: usize,
    trows: usize,
}

fn compute_layout(w: i32, h: i32, cw: i32, ch: i32, visible: bool, frac: f32) -> Layout {
    let status_h = ch + 8;
    let avail = (h - status_h).max(ch * 2);
    if !visible {
        return Layout {
            status_h,
            tab_h: ch + 8,
            ed_h: avail,
            split_y: avail,
            term_y: avail,
            term_h: 0,
            tcols: 80,
            trows: 24,
        };
    }
    let min_rows = 3;
    let want = (((avail as f32 * frac) as i32 - 2 * PAD).max(0)) / ch;
    let max_rows = ((avail - SPLIT_H - 4 * (ch + 2) - 2 * PAD) / ch).max(min_rows);
    let trows = want.clamp(min_rows, max_rows);
    let term_h = trows * ch + 2 * PAD;
    let split_y = (avail - term_h - SPLIT_H).max(0);
    Layout {
        status_h,
        tab_h: ch + 8,
        ed_h: split_y,
        split_y,
        term_y: split_y + SPLIT_H,
        term_h,
        tcols: (((w - 2 * PAD) / cw).max(10)) as usize,
        trows: trows as usize,
    }
}

struct EdGeo {
    ox: i32,
    top: i32,
    lh: i32,
    rows: usize,
    vis_cols: usize,
    gutter_w: i32,
    text_x: i32,
}

fn is_plain(c: char) -> bool {
    (0x20..0x7f).contains(&(c as u32))
}

fn normalize(ks: c_ulong) -> c_ulong {
    match ks {
        0xffb0..=0xffb9 => ks - 0xffb0 + 0x30,
        0xffab => 0x2b,
        0xffad => 0x2d,
        0xffaa => 0x2a,
        0xffaf => 0x2f,
        0xffae => 0x2e,
        0xff8d => XK_Return,
        0xff95 => XK_Home,
        0xff96 => XK_Left,
        0xff97 => XK_Up,
        0xff98 => XK_Right,
        0xff99 => XK_Down,
        0xff9a => XK_Prior,
        0xff9b => XK_Next,
        0xff9c => XK_End,
        0xff9e => XK_Insert,
        0xff9f => XK_Delete,
        0x41..=0x5a => ks + 32,
        _ => ks,
    }
}

fn ctrl_byte(ks: c_ulong) -> Option<u8> {
    match ks {
        0x61..=0x7a => Some((ks - 0x60) as u8),
        0x40 | 0x20 | 0x32 => Some(0),
        0x5b | 0x33 => Some(27),
        0x5c | 0x34 => Some(28),
        0x5d | 0x35 => Some(29),
        0x5e | 0x36 => Some(30),
        0x5f | 0x2f | 0x37 => Some(31),
        0x3f | 0x38 => Some(127),
        _ => None,
    }
}

fn encode_key(ks: c_ulong, text: &str, shift: bool, ctrl: bool, alt: bool, app: bool) -> Vec<u8> {
    let m = 1 + shift as u32 + 2 * (alt as u32) + 4 * (ctrl as u32);
    let arrow = |c: char| -> Vec<u8> {
        (if m == 1 {
            if app {
                format!("\x1bO{}", c)
            } else {
                format!("\x1b[{}", c)
            }
        } else {
            format!("\x1b[1;{}{}", m, c)
        })
        .into_bytes()
    };
    let tilde = |n: u32| -> Vec<u8> {
        (if m == 1 {
            format!("\x1b[{}~", n)
        } else {
            format!("\x1b[{};{}~", n, m)
        })
        .into_bytes()
    };
    match ks {
        XK_Up => arrow('A'),
        XK_Down => arrow('B'),
        XK_Right => arrow('C'),
        XK_Left => arrow('D'),
        XK_Home => arrow('H'),
        XK_End => arrow('F'),
        XK_Insert => tilde(2),
        XK_Delete => tilde(3),
        XK_Prior => tilde(5),
        XK_Next => tilde(6),
        XK_F1..=XK_F4 => {
            let c = (b'P' + (ks - XK_F1) as u8) as char;
            (if m == 1 {
                format!("\x1bO{}", c)
            } else {
                format!("\x1b[1;{}{}", m, c)
            })
            .into_bytes()
        }
        XK_F5..=XK_F12 => {
            let codes = [15, 17, 18, 19, 20, 21, 23, 24];
            tilde(codes[(ks - XK_F5) as usize])
        }
        XK_Return => {
            if alt {
                b"\x1b\r".to_vec()
            } else {
                b"\r".to_vec()
            }
        }
        XK_Tab => b"\t".to_vec(),
        XK_ISO_Left_Tab => b"\x1b[Z".to_vec(),
        XK_BackSpace => {
            let b = if ctrl { 0x08 } else { 0x7f };
            if alt {
                vec![0x1b, b]
            } else {
                vec![b]
            }
        }
        XK_Escape => vec![0x1b],
        _ => {
            if ctrl {
                if let Some(b) = ctrl_byte(ks) {
                    return if alt { vec![0x1b, b] } else { vec![b] };
                }
            }
            if let Some(c) = text.chars().next() {
                if (c as u32) >= 0x20 && c != '\x7f' {
                    let mut v = Vec::new();
                    if alt {
                        v.push(0x1b);
                    }
                    v.extend_from_slice(text.as_bytes());
                    return v;
                }
            }
            Vec::new()
        }
    }
}

fn complete_path(s: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let expanded = if let Some(r) = s.strip_prefix("~/") {
        format!("{}/{}", home, r)
    } else {
        s.to_string()
    };
    let (dir, prefix) = match expanded.rfind('/') {
        Some(i) => (expanded[..=i].to_string(), expanded[i + 1..].to_string()),
        None => (String::new(), expanded.clone()),
    };
    let read = std::fs::read_dir(if dir.is_empty() { "." } else { &dir });
    let Ok(rd) = read else { return s.to_string() };
    let mut names: Vec<(String, bool)> = rd
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with(&prefix) {
                Some((n, e.path().is_dir()))
            } else {
                None
            }
        })
        .collect();
    if names.is_empty() {
        return s.to_string();
    }
    names.sort();
    let mut lcp = names[0].0.clone();
    for (n, _) in &names {
        while !n.starts_with(&lcp) {
            lcp.pop();
        }
    }
    let mut out = format!("{}{}", dir, lcp);
    if names.len() == 1 && names[0].1 {
        out.push('/');
    }
    out
}

struct App {
    x: Xw,
    ed: Editor,
    term: Term,
    pty: Pty,
    pty_dead: bool,
    focus: Focus,
    term_visible: bool,
    tree: Tree,
    tree_visible: bool,
    tree_prefix: Option<char>,
    tabs: Vec<Editor>,
    active: usize,
    discard_tab: Option<usize>,
    last_root: PathBuf,
    pending_cd: Option<PathBuf>,
    term_frac: f32,
    base_px: i32,
    px: i32,
    family: String,
    lay: Layout,
    prompt: Option<PromptState>,
    msg: Option<(String, Instant)>,
    drag: Drag,
    last_click: Instant,
    click_n: u32,
    last_click_pos: (i32, i32),
    term_anchor: (usize, usize),
    clip: String,
    primary: String,
    own_clip: bool,
    own_primary: bool,
    pending_paste: Option<PasteTarget>,
    has_focus: bool,
    blink: bool,
    last_blink: Instant,
    discard_ok: bool,
    find_hl: bool,
    quit: bool,
    redraw: bool,
    title: String,
}

impl App {
    fn new(file: Option<String>) -> Result<App, String> {
        let mut x = Xw::new(1180, 760, "branna")?;
        let family = std::env::var("BRANNA_FONT").unwrap_or_else(|_| "monospace".to_string());
        let base_px: i32 = std::env::var("BRANNA_FONT_PX")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(17);
        if !x.set_font(&family, base_px) {
            return Err(format!("cannot load font '{}'", family));
        }
        let mut open_dir = false;
        let ed = match file {
            Some(f) => {
                let pb = PathBuf::from(&f);
                if pb.is_dir() {
                    std::env::set_current_dir(&pb)
                        .map_err(|e| format!("cannot enter {}: {}", f, e))?;
                    open_dir = true;
                    Editor::new()
                } else {
                    Editor::load(pb)?
                }
            }
            None => Editor::new(),
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut tree = Tree::new(cwd);
        let last_root = tree.root.clone();
        if let Some(p) = ed.path.as_ref().and_then(|p| std::fs::canonicalize(p).ok()) {
            tree.select_path(&p);
        }
        // pty is sized as if the terminal were visible, so showing it later needs no re-layout
        let vis_lay = compute_layout(x.w, x.h, x.cw, x.ch, true, 0.30);
        let term = Term::new(vis_lay.tcols, vis_lay.trows);
        let pty = Pty::spawn(vis_lay.tcols, vis_lay.trows)
            .map_err(|e| format!("cannot start shell: {}", e))?;
        pty.resize(vis_lay.tcols, vis_lay.trows, x.w, vis_lay.term_h);
        let lay = compute_layout(x.w, x.h, x.cw, x.ch, false, 0.30);
        Ok(App {
            x,
            ed,
            term,
            pty,
            pty_dead: false,
            focus: if open_dir { Focus::Tree } else { Focus::Editor },
            term_visible: false,
            tree,
            tree_visible: open_dir,
            tree_prefix: None,
            tabs: vec![Editor::new()],
            active: 0,
            discard_tab: None,
            last_root,
            pending_cd: None,
            term_frac: 0.30,
            base_px,
            px: base_px,
            family,
            lay,
            prompt: None,
            msg: None,
            drag: Drag::None,
            last_click: Instant::now(),
            click_n: 0,
            last_click_pos: (0, 0),
            term_anchor: (0, 0),
            clip: String::new(),
            primary: String::new(),
            own_clip: false,
            own_primary: false,
            pending_paste: None,
            has_focus: true,
            blink: true,
            last_blink: Instant::now(),
            discard_ok: false,
            find_hl: false,
            quit: false,
            redraw: true,
            title: String::new(),
        })
    }

    fn msg(&mut self, s: impl Into<String>) {
        self.msg = Some((s.into(), Instant::now()));
        self.redraw = true;
    }

    fn apply_layout(&mut self) {
        self.lay = compute_layout(
            self.x.w,
            self.x.h,
            self.x.cw,
            self.x.ch,
            self.term_visible,
            self.term_frac,
        );
        if self.term_visible {
            self.term.resize(self.lay.tcols, self.lay.trows);
            self.pty
                .resize(self.lay.tcols, self.lay.trows, self.x.w, self.lay.term_h);
        }
        self.redraw = true;
    }

    fn tree_w(&self) -> i32 {
        if self.tree_visible {
            (30 * self.x.cw).min(self.x.w / 2)
        } else {
            0
        }
    }

    fn toggle_tree(&mut self) {
        if !self.tree_visible {
            self.tree_visible = true;
            self.focus = Focus::Tree;
        } else if self.focus == Focus::Tree {
            self.tree_visible = false;
            self.focus = Focus::Editor;
        } else {
            self.focus = Focus::Tree;
        }
        self.ed.follow = true;
        self.redraw = true;
    }

    fn ed_geo(&self) -> EdGeo {
        let lh = self.x.ch + 2;
        let digits = self.ed.lines.len().to_string().len().max(3) as i32;
        let gutter_w = (digits + 2) * self.x.cw;
        let ox = self.tree_w();
        let text_x = ox + gutter_w + 6;
        EdGeo {
            ox,
            top: self.lay.tab_h,
            lh,
            rows: ((self.lay.ed_h - self.lay.tab_h) / lh).max(1) as usize,
            vis_cols: ((self.x.w - text_x) / self.x.cw).max(1) as usize,
            gutter_w,
            text_x,
        }
    }

    fn zoom(&mut self, dir: i32) {
        let new = if dir == 0 {
            self.base_px
        } else {
            let step = (self.px / 10).max(1);
            self.px + dir * step
        }
        .clamp(6, 96);
        if new == self.px {
            return;
        }
        if !self.x.set_font(&self.family, new) {
            self.msg("could not change font size");
            return;
        }
        self.px = new;
        self.apply_layout();
        self.msg(format!("Zoom {}%", new * 100 / self.base_px));
    }

    fn set_clipboard(&mut self, s: String) {
        self.clip = s;
        unsafe { XSetSelectionOwner(self.x.dpy, self.x.atoms.clipboard, self.x.win, 0) };
        self.own_clip = true;
    }

    fn set_primary(&mut self, s: String) {
        if s.is_empty() {
            return;
        }
        self.primary = s;
        unsafe { XSetSelectionOwner(self.x.dpy, XA_PRIMARY, self.x.win, 0) };
        self.own_primary = true;
    }

    fn request_paste(&mut self, sel: Atom, target: PasteTarget) {
        let is_clip = sel == self.x.atoms.clipboard;
        if (is_clip && self.own_clip) || (!is_clip && self.own_primary) {
            let s = if is_clip {
                self.clip.clone()
            } else {
                self.primary.clone()
            };
            self.paste_text(target, &s);
            return;
        }
        self.pending_paste = Some(target);
        unsafe {
            XConvertSelection(
                self.x.dpy,
                sel,
                self.x.atoms.utf8,
                self.x.atoms.prop,
                self.x.win,
                0,
            );
            XFlush(self.x.dpy);
        }
    }

    fn paste_text(&mut self, target: PasteTarget, s: &str) {
        if s.is_empty() {
            return;
        }
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        match target {
            PasteTarget::Editor => {
                self.ed.insert_str(&s);
            }
            PasteTarget::Term => {
                if self.pty_dead {
                    return;
                }
                let body = s.replace('\n', "\r");
                self.term.view_offset = 0;
                if self.term.bracketed {
                    self.pty.send(b"\x1b[200~");
                    self.pty.send(body.as_bytes());
                    self.pty.send(b"\x1b[201~");
                } else {
                    self.pty.send(body.as_bytes());
                }
            }
            PasteTarget::Prompt => {
                if let Some(p) = self.prompt.as_mut() {
                    p.text.push_str(&s.replace('\n', " "));
                }
                self.find_update();
            }
        }
        self.redraw = true;
    }

    fn on_selection_notify(&mut self, e: XSelectionEvent) {
        let target = self.pending_paste.take();
        if e.property == 0 {
            return;
        }
        let mut atype: Atom = 0;
        let mut fmt: c_int = 0;
        let mut n: c_ulong = 0;
        let mut after: c_ulong = 0;
        let mut data: *mut c_uchar = std::ptr::null_mut();
        let ok = unsafe {
            XGetWindowProperty(
                self.x.dpy,
                self.x.win,
                e.property,
                0,
                1 << 22,
                1,
                0,
                &mut atype,
                &mut fmt,
                &mut n,
                &mut after,
                &mut data,
            )
        };
        if ok == 0 && !data.is_null() {
            let s = if atype != self.x.atoms.incr && fmt == 8 {
                let bytes = unsafe { std::slice::from_raw_parts(data, n as usize) };
                String::from_utf8_lossy(bytes).into_owned()
            } else {
                String::new()
            };
            unsafe { XFree(data as *mut _) };
            if let Some(t) = target {
                self.paste_text(t, &s);
            }
        }
    }

    fn on_sel_request(&mut self, r: XSelectionRequestEvent) {
        let a = &self.x.atoms;
        let text: Option<&String> = if r.selection == a.clipboard {
            Some(&self.clip)
        } else if r.selection == XA_PRIMARY {
            Some(&self.primary)
        } else {
            None
        };
        let prop = if r.property == 0 {
            r.target
        } else {
            r.property
        };
        let mut ok = false;
        unsafe {
            if let Some(text) = text {
                if r.target == a.targets {
                    let list: [Atom; 4] = [a.targets, a.utf8, a.text, XA_STRING];
                    XChangeProperty(
                        self.x.dpy,
                        r.requestor,
                        prop,
                        XA_ATOM,
                        32,
                        0,
                        list.as_ptr() as *const c_uchar,
                        4,
                    );
                    ok = true;
                } else if r.target == a.utf8 || r.target == XA_STRING || r.target == a.text {
                    let typ = if r.target == a.text { a.utf8 } else { r.target };
                    XChangeProperty(
                        self.x.dpy,
                        r.requestor,
                        prop,
                        typ,
                        8,
                        0,
                        text.as_ptr(),
                        text.len() as c_int,
                    );
                    ok = true;
                }
            }
            let mut ev = XEvent::zeroed();
            ev.selection = XSelectionEvent {
                type_: SelectionNotify,
                serial: 0,
                send_event: 1,
                display: self.x.dpy,
                requestor: r.requestor,
                selection: r.selection,
                target: r.target,
                property: if ok { prop } else { 0 },
                time: r.time,
            };
            XSendEvent(self.x.dpy, r.requestor, 0, 0, &mut ev);
            XFlush(self.x.dpy);
        }
    }

    fn respawn(&mut self) {
        match Pty::spawn(self.term.cols, self.term.rows) {
            Ok(p) => {
                self.pty = p;
                self.pty
                    .resize(self.term.cols, self.term.rows, self.x.w, self.lay.term_h);
                self.pty_dead = false;
                self.term.feed(b"\x1b[2J\x1b[H");
            }
            Err(e) => self.msg(format!("cannot start shell: {}", e)),
        }
        self.redraw = true;
    }

    fn pump_pty(&mut self) {
        if self.pty_dead {
            return;
        }
        let mut buf = [0u8; 65536];
        for _ in 0..8 {
            match self.pty.read(&mut buf) {
                Ok(0) => {
                    self.on_shell_exit();
                    break;
                }
                Ok(n) => {
                    self.term.feed(&buf[..n]);
                    self.redraw = true;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    self.on_shell_exit();
                    break;
                }
            }
        }
        if !self.term.reply.is_empty() {
            let r = std::mem::take(&mut self.term.reply);
            self.pty.send(&r);
        }
    }

    fn sync_term_dir(&mut self) {
        if self.tree.root != self.last_root {
            self.last_root = self.tree.root.clone();
            let _ = std::env::set_current_dir(&self.last_root);
            self.pending_cd = Some(self.last_root.clone());
        }
        if let Some(p) = self.pending_cd.clone() {
            if self.pty_dead {
                self.pending_cd = None;
                return;
            }
            if self.pty.is_idle() && !self.term.alt && !self.pty.wants_write() {
                let q = p.to_string_lossy().replace('\'', "'\\''");
                let cmd = format!("\x15 cd -- '{}'\r", q);
                self.pty.send(cmd.as_bytes());
                self.pending_cd = None;
            }
        }
    }

    fn on_shell_exit(&mut self) {
        self.pty.reap();
        self.pty_dead = true;
        self.term.feed(
            b"\r\n\x1b[2m[shell exited - press any key in the terminal to restart]\x1b[0m\r\n",
        );
        self.redraw = true;
    }

    fn handle_x_events(&mut self) {
        unsafe {
            while XPending(self.x.dpy) > 0 {
                let mut ev = XEvent::zeroed();
                XNextEvent(self.x.dpy, &mut ev);
                if XFilterEvent(&mut ev, 0) != 0 {
                    continue;
                }
                match ev.type_ {
                    KeyPress => self.on_key(ev.key),
                    ButtonPress => self.on_button_press(ev.button),
                    ButtonRelease => self.on_button_release(ev.button),
                    MotionNotify => self.on_motion(ev.motion),
                    Expose => {
                        if ev.expose.count == 0 {
                            self.redraw = true;
                        }
                    }
                    ConfigureNotify => {
                        let (w, h) = (ev.configure.width, ev.configure.height);
                        if w != self.x.w || h != self.x.h {
                            self.x.resize_buffers(w, h);
                            self.apply_layout();
                        }
                    }
                    FocusIn => {
                        self.has_focus = true;
                        if !self.x.xic.is_null() {
                            XSetICFocus(self.x.xic);
                        }
                        self.redraw = true;
                    }
                    FocusOut => {
                        self.has_focus = false;
                        if !self.x.xic.is_null() {
                            XUnsetICFocus(self.x.xic);
                        }
                        self.redraw = true;
                    }
                    ClientMessage => {
                        if ev.client.data[0] as Atom == self.x.atoms.wm_delete {
                            self.try_quit();
                        }
                    }
                    SelectionClear => {
                        let s = ev.sel_clear.selection;
                        if s == self.x.atoms.clipboard {
                            self.own_clip = false;
                        } else if s == XA_PRIMARY {
                            self.own_primary = false;
                        }
                    }
                    SelectionRequest => self.on_sel_request(ev.sel_req),
                    SelectionNotify => self.on_selection_notify(ev.selection),
                    _ => {}
                }
            }
        }
    }

    fn try_quit(&mut self) {
        if self.any_modified() && !self.discard_ok {
            self.discard_ok = true;
            self.msg("Unsaved changes. Repeat the quit command to discard them.");
        } else {
            self.quit = true;
        }
    }

    fn on_key(&mut self, mut ev: XKeyEvent) {
        let (ks, text) = self.x.lookup(&mut ev);
        let ks = normalize(ks);
        self.blink = true;
        self.last_blink = Instant::now();
        self.redraw = true;
        let was_discard = self.discard_ok;
        self.discard_ok = false;
        let st = ev.state;
        let (shift, ctrl, alt) = (st & 1 != 0, st & 4 != 0, st & 8 != 0);
        if !(ctrl && ks == 0x77) {
            self.discard_tab = None;
        }

        if ctrl && !alt {
            match ks {
                0x2b | 0x3d => return self.zoom(1),
                0x2d | 0x5f => return self.zoom(-1),
                0x30 => return self.zoom(0),
                0x60 | 0x7e => {
                    self.toggle_focus();
                    return;
                }
                0x62 | 0x65 if self.focus != Focus::Term => {
                    self.toggle_tree();
                    return;
                }
                0x6c if self.focus == Focus::Tree && self.prompt.is_none() => {
                    self.tree.rebuild();
                    return;
                }
                _ => {}
            }
        }
        if ks == XK_F12 {
            self.toggle_focus_term_visibility();
            return;
        }
        if self.prompt.is_none() && self.focus != Focus::Term {
            if ctrl && !alt {
                match ks {
                    XK_Tab | XK_Next => return self.cycle_tab(1),
                    XK_ISO_Left_Tab | XK_Prior => return self.cycle_tab(-1),
                    _ => {}
                }
            }
            if alt && !ctrl && (0x31..=0x39).contains(&ks) {
                let i = (ks - 0x31) as usize;
                return self.switch_tab(i.min(self.tabs.len() - 1));
            }
        }
        if self.prompt.is_some() {
            return self.on_key_prompt(ks, &text, ctrl, shift);
        }
        match self.focus {
            Focus::Editor => self.on_key_editor(ks, &text, ctrl, shift, alt, was_discard),
            Focus::Term => self.on_key_term(ks, &text, ctrl, shift, alt),
            Focus::Tree => {
                if ctrl && !alt {
                    self.on_key_editor(ks, &text, ctrl, shift, alt, was_discard)
                } else {
                    self.on_key_tree(ks, &text, ctrl, shift)
                }
            }
        }
    }

    fn toggle_focus_term_visibility(&mut self) {
        self.term_visible = !self.term_visible;
        if self.term_visible {
            self.focus = Focus::Term;
        } else if self.focus == Focus::Term {
            self.focus = Focus::Editor;
        }
        self.apply_layout();
    }

    fn toggle_focus(&mut self) {
        if !self.term_visible {
            self.term_visible = true;
            self.apply_layout();
            self.focus = Focus::Term;
        } else if self.focus == Focus::Term {
            self.term_visible = false;
            self.focus = Focus::Editor;
            self.apply_layout();
        } else {
            self.focus = Focus::Term;
        }
    }

    fn any_modified(&self) -> bool {
        self.ed.modified
            || self
                .tabs
                .iter()
                .enumerate()
                .any(|(i, t)| i != self.active && t.modified)
    }

    fn tab_label(&self, i: usize) -> (String, bool) {
        let e = if i == self.active {
            &self.ed
        } else {
            &self.tabs[i]
        };
        (e.display_name(), e.modified)
    }

    fn new_tab(&mut self, e: Editor) {
        let blank = self.tabs.len() == 1
            && self.ed.path.is_none()
            && !self.ed.modified
            && self.ed.lines.len() == 1
            && self.ed.lines[0].is_empty();
        if blank {
            self.ed = e;
        } else {
            let old = std::mem::replace(&mut self.ed, e);
            self.tabs[self.active] = old;
            self.tabs.insert(self.active + 1, Editor::new());
            self.active += 1;
        }
        self.ed.follow = true;
        self.find_hl = false;
        self.redraw = true;
    }

    fn switch_tab(&mut self, i: usize) {
        if i == self.active || i >= self.tabs.len() {
            return;
        }
        let new = std::mem::replace(&mut self.tabs[i], Editor::new());
        let old = std::mem::replace(&mut self.ed, new);
        self.tabs[self.active] = old;
        self.active = i;
        self.ed.follow = true;
        self.find_hl = false;
        if let Some(p) = self.ed.path.clone() {
            self.tree.select_path(&p);
        }
        self.redraw = true;
    }

    fn cycle_tab(&mut self, d: isize) {
        let n = self.tabs.len() as isize;
        self.switch_tab((self.active as isize + d).rem_euclid(n) as usize);
    }

    fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        let modified = if i == self.active {
            self.ed.modified
        } else {
            self.tabs[i].modified
        };
        if modified && self.discard_tab != Some(i) {
            self.discard_tab = Some(i);
            let name = self.tab_label(i).0;
            self.msg(format!(
                "{} has unsaved changes. Close it again to discard them.",
                name
            ));
            return;
        }
        self.discard_tab = None;
        if i == self.active {
            self.tabs.remove(i);
            if self.tabs.is_empty() {
                self.tabs.push(Editor::new());
            }
            let na = i.min(self.tabs.len() - 1);
            self.ed = std::mem::replace(&mut self.tabs[na], Editor::new());
            self.active = na;
            self.ed.follow = true;
            self.find_hl = false;
            if let Some(p) = self.ed.path.clone() {
                self.tree.select_path(&p);
            }
        } else {
            self.tabs.remove(i);
            if i < self.active {
                self.active -= 1;
            }
        }
        self.redraw = true;
    }

    fn tab_rects(&self) -> Vec<(usize, i32, i32)> {
        let cw = self.x.cw;
        let widths: Vec<i32> = (0..self.tabs.len())
            .map(|i| (self.tab_label(i).0.chars().count().min(22) as i32 + 5) * cw)
            .collect();
        let mut start = 0;
        while start < self.active {
            let right: i32 = widths[start..=self.active].iter().sum();
            if right <= self.x.w {
                break;
            }
            start += 1;
        }
        let mut out = Vec::new();
        let mut xx = 0;
        for i in start..widths.len() {
            if xx + widths[i] > self.x.w && !out.is_empty() {
                break;
            }
            out.push((i, xx, widths[i]));
            xx += widths[i];
        }
        out
    }

    fn tab_click(&mut self, mx: i32, middle: bool) {
        let cw = self.x.cw;
        for (i, tx, tw) in self.tab_rects() {
            if mx >= tx && mx < tx + tw {
                if middle || mx >= tx + tw - 3 * cw {
                    self.close_tab(i);
                } else {
                    self.switch_tab(i);
                    self.focus = Focus::Editor;
                }
                return;
            }
        }
    }

    fn draw_tabs(&self) {
        let x = &self.x;
        let th = self.lay.tab_h;
        x.fill(0, 0, x.w, th, STATUS_BG);
        x.fill(0, th - 1, x.w, 1, SPLIT_COL);
        for (i, tx, tw) in self.tab_rects() {
            let active = i == self.active;
            let (name, modified) = self.tab_label(i);
            if active {
                x.fill(tx, 0, tw, th, BG);
                x.fill(tx, 0, tw, 2, ACCENT);
            }
            x.fill(tx + tw - 1, 2, 1, th - 4, SPLIT_COL);
            let mut label: Vec<char> = name.chars().collect();
            if label.len() > 22 {
                label.truncate(21);
                label.push('\u{2026}');
            }
            let col = if active { FG } else { GUTTER_FG_CUR };
            self.text_cells(tx + x.cw, 2, th - 2, &label, col, false);
            let glyph = [if modified { '\u{25cf}' } else { '\u{00d7}' }];
            let gcol = if modified { WARN } else { GUTTER_FG };
            self.text_cells(tx + tw - 2 * x.cw, 2, th - 2, &glyph, gcol, false);
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        let same = |a: &std::path::Path, b: &std::path::Path| {
            a == b
                || matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(p), Ok(q)) if p == q)
        };
        if self.ed.path.as_ref().map_or(false, |p| same(p, &path)) {
            self.focus = Focus::Editor;
            return;
        }
        let existing = (0..self.tabs.len()).find(|&i| {
            i != self.active && self.tabs[i].path.as_ref().map_or(false, |p| same(p, &path))
        });
        if let Some(i) = existing {
            self.switch_tab(i);
            self.focus = Focus::Editor;
            return;
        }
        match Editor::load(path.clone()) {
            Ok(e) => {
                self.new_tab(e);
                self.focus = Focus::Editor;
                self.tree.select_path(&path);
            }
            Err(e) => self.msg(format!("Cannot open {}: {}", path.display(), e)),
        }
    }

    fn tree_activate(&mut self) {
        let Some(n) = self.tree.current() else { return };
        let (is_dir, is_parent, expanded, path) =
            (n.is_dir, n.is_parent, n.expanded, n.path.clone());
        if is_parent {
            self.tree.go_up();
        } else if is_dir {
            let i = self.tree.sel;
            self.tree.set_expanded(i, !expanded);
        } else {
            self.open_path(path);
        }
    }

    fn tree_click(&mut self, my: i32) {
        let lh = self.x.ch + 2;
        let row = (my - self.lay.tab_h).div_euclid(lh);
        if row < 1 {
            return;
        }
        let idx = self.tree.scroll + (row - 1) as usize;
        if idx < self.tree.items.len() {
            self.tree.sel = idx;
            if self.click_n >= 2 {
                self.tree_activate();
            }
        }
    }

    fn tree_marked_or_current(&self) -> Vec<PathBuf> {
        if self.tree.marks.is_empty() {
            self.tree
                .current()
                .filter(|n| !n.is_parent)
                .map(|n| vec![n.path.clone()])
                .unwrap_or_default()
        } else {
            self.tree.marks.clone()
        }
    }

    fn tree_copy_move(&mut self, mv: bool) {
        let Some(target) = self.tree.target.clone() else {
            self.msg("No target: put the cursor on a folder and press mt");
            return;
        };
        let items = self.tree.marks.clone();
        if items.is_empty() {
            self.msg("Nothing marked (mf marks a file)");
            return;
        }
        let (mut ok, mut bad) = (0, 0);
        for src in items {
            let Some(name) = src.file_name() else {
                continue;
            };
            let dst = target.join(name);
            if dst.exists() || src == dst || (src.is_dir() && dst.starts_with(&src)) {
                bad += 1;
                continue;
            }
            let r = if mv {
                std::fs::rename(&src, &dst)
            } else {
                copy_recursive(&src, &dst)
            };
            if r.is_ok() {
                ok += 1;
                if mv && self.ed.path.as_ref() == Some(&src) {
                    self.ed.path = Some(dst.clone());
                }
            } else {
                bad += 1;
            }
        }
        self.tree.marks.clear();
        self.tree.reveal(&target);
        self.msg(format!(
            "{} {} item(s){}",
            if mv { "Moved" } else { "Copied" },
            ok,
            if bad > 0 {
                format!(", {} skipped/failed", bad)
            } else {
                String::new()
            }
        ));
    }

    fn tree_file_info(&mut self) {
        let Some(n) = self.tree.current().filter(|n| !n.is_parent) else {
            return;
        };
        let age = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().saturating_sub(n.mtime))
            .unwrap_or(0);
        let ago = if age < 90 {
            format!("{}s", age)
        } else if age < 5400 {
            format!("{}m", age / 60)
        } else if age < 129600 {
            format!("{}h", age / 3600)
        } else {
            format!("{}d", age / 86400)
        };
        let info = format!(
            "{}  {}  modified {} ago",
            n.name,
            if n.is_dir {
                "directory".to_string()
            } else {
                format!("{} bytes", n.size)
            },
            ago
        );
        self.msg(info);
    }

    fn on_key_tree(&mut self, ks: c_ulong, _text: &str, ctrl: bool, shift: bool) {
        if ctrl {
            return;
        }
        let rows = (((self.lay.ed_h - self.lay.tab_h) / (self.x.ch + 2)) as isize - 1).max(1);
        let ch = if ks < 128 { ks as u8 as char } else { '\0' };

        if let Some(pf) = self.tree_prefix.take() {
            match (pf, ch, shift) {
                ('g', 'g', _) => self.tree.sel = 0,
                ('g', 'h', _) => {
                    self.tree.show_hidden = !self.tree.show_hidden;
                    self.tree.rebuild();
                }
                ('g', 'n', _) => {
                    if let Some(n) = self.tree.current() {
                        if n.is_dir && !n.is_parent {
                            let p = n.path.clone();
                            self.tree.set_root(p);
                        }
                    }
                }
                ('m', 'f', _) => {
                    self.tree.toggle_mark();
                    self.tree.move_sel(1);
                }
                ('m', 'u', _) => {
                    self.tree.marks.clear();
                    self.msg("Marks cleared");
                }
                ('m', 't', _) => {
                    let d = self.tree.target_dir();
                    self.msg(format!("Target: {}", d.display()));
                    self.tree.target = Some(d);
                }
                ('m', 'c', _) => self.tree_copy_move(false),
                ('m', 'm', _) => self.tree_copy_move(true),
                ('m', 'd', true) => {
                    if self.tree.marks.is_empty() {
                        self.msg("Nothing marked");
                    } else {
                        self.open_prompt(PromptKind::DeleteMarked, String::new());
                    }
                }
                ('q', 'f', _) => self.tree_file_info(),
                _ => {}
            }
            return;
        }

        match ks {
            XK_Up => self.tree.move_sel(-1),
            XK_Down => self.tree.move_sel(1),
            XK_Prior => self.tree.move_sel(-rows),
            XK_Next => self.tree.move_sel(rows),
            XK_Home => self.tree.sel = 0,
            XK_End => self.tree.sel = self.tree.items.len().saturating_sub(1),
            XK_Left => self.tree.collapse_or_parent(),
            XK_Right => {
                let i = self.tree.sel;
                match self
                    .tree
                    .current()
                    .map(|n| (n.is_dir && !n.is_parent, n.expanded))
                {
                    Some((true, false)) => self.tree.set_expanded(i, true),
                    Some((true, true)) => self.tree.move_sel(1),
                    _ => {}
                }
            }
            XK_Return => self.tree_activate(),
            XK_Escape | XK_Tab => self.focus = Focus::Editor,
            XK_Delete => self.tree_delete_prompt(),
            _ => match (ch, shift) {
                ('g', false) | ('m', false) | ('q', false) => self.tree_prefix = Some(ch),
                ('g', true) => self.tree.sel = self.tree.items.len().saturating_sub(1),
                ('k', false) => self.tree.move_sel(-1),
                ('j', false) => self.tree.move_sel(1),
                ('h', false) => self.tree.collapse_or_parent(),
                ('l', false) | ('o', false) | ('v', false) | ('t', false) | ('p', false) => {
                    self.tree_activate()
                }
                ('-', _) => self.tree.go_up(),
                ('u', false) => {
                    if !self.tree.history(true) {
                        self.msg("At oldest directory in history");
                    }
                }
                ('u', true) => {
                    if !self.tree.history(false) {
                        self.msg("At newest directory in history");
                    }
                }
                ('a', _) => {
                    self.tree.show_hidden = !self.tree.show_hidden;
                    self.tree.rebuild();
                    self.msg(if self.tree.show_hidden {
                        "Showing hidden files"
                    } else {
                        "Hiding dot files"
                    });
                }
                ('s', false) => {
                    self.tree.cycle_sort();
                    self.msg(match self.tree.sort {
                        SortBy::Name => "Sorted by name",
                        SortBy::Time => "Sorted by time (newest first)",
                        SortBy::Size => "Sorted by size (largest first)",
                    });
                }
                ('r', false) => self.tree.toggle_reverse(),
                ('i', false) => self.tree.long = !self.tree.long,
                ('c', false) => {
                    // :cd - make the browsing directory the shell's / process's current directory
                    self.last_root = PathBuf::new();
                    self.msg(format!("cd {}", self.tree.root.display()));
                }
                ('x', false) => {
                    if let Some(n) = self.tree.current().filter(|n| !n.is_parent) {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(&n.path)
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .spawn();
                    }
                }
                ('%', _) => {
                    let d = self.tree.target_dir();
                    self.open_prompt(PromptKind::NewFile(d), String::new());
                }
                ('d', false) => {
                    let d = self.tree.target_dir();
                    self.open_prompt(PromptKind::NewDir(d), String::new());
                }
                ('d', true) => self.tree_delete_prompt(),
                ('r', true) => {
                    if let Some(n) = self.tree.current() {
                        if !n.is_parent {
                            let (p, name) = (n.path.clone(), n.name.clone());
                            self.open_prompt(PromptKind::Rename(p), name);
                        }
                    }
                }
                _ => {}
            },
        }
    }

    fn tree_delete_prompt(&mut self) {
        if let Some(n) = self.tree.current() {
            if !n.is_parent {
                let p = n.path.clone();
                self.open_prompt(PromptKind::Delete(p), String::new());
            }
        }
    }

    fn draw_tree(&self, g: &EdGeo) {
        let x = &self.x;
        let tw = g.ox;
        let cw = x.cw;
        let focused = self.focus == Focus::Tree && self.has_focus;
        x.fill(0, g.top, tw, self.lay.ed_h - g.top, TREE_BG);
        x.fill(tw - 1, g.top, 1, self.lay.ed_h - g.top, SPLIT_COL);
        let maxc = ((tw - 2 * cw) / cw).max(1) as usize;
        let root_name = self
            .tree
            .root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.tree.root.display().to_string());
        let sort = match self.tree.sort {
            SortBy::Name => "",
            SortBy::Time => " [time]",
            SortBy::Size => " [size]",
        };
        let head: Vec<char> = format!(
            "{}{}{}",
            root_name.to_uppercase(),
            sort,
            if self.tree.reverse { " [rev]" } else { "" }
        )
        .chars()
        .take(maxc)
        .collect();
        self.text_cells(cw, g.top, g.lh, &head, 0xbbbbbb, true);
        for r in 1..g.rows {
            let idx = self.tree.scroll + r - 1;
            let Some(n) = self.tree.items.get(idx) else {
                break;
            };
            let y = g.top + r as i32 * g.lh;
            if idx == self.tree.sel {
                x.fill(
                    0,
                    y,
                    tw - 1,
                    g.lh,
                    if focused { TREE_SEL_FOCUS } else { TREE_SEL },
                );
            }
            let marked = self.tree.is_marked(&n.path);
            let mark = if n.is_parent {
                "\u{25b4} "
            } else if n.is_dir {
                if n.expanded {
                    "\u{25be} "
                } else {
                    "\u{25b8} "
                }
            } else {
                "  "
            };
            let size_txt: String = if self.tree.long && !n.is_parent {
                if n.is_dir {
                    "-".to_string()
                } else {
                    human_size(n.size)
                }
            } else {
                String::new()
            };
            let size_w = if self.tree.long { 7 } else { 0 };
            let name_max = maxc.saturating_sub(size_w);
            let label: Vec<char> = format!(
                "{}{}{}{}",
                if marked { "*" } else { "" },
                " ".repeat(n.depth * 2),
                mark,
                n.name
            )
            .chars()
            .take(name_max)
            .collect();
            let color = if marked {
                WARN
            } else if n.is_dir {
                TREE_DIR
            } else {
                FG
            };
            self.text_cells(cw, y, g.lh, &label, color, false);
            if !size_txt.is_empty() {
                let sc: Vec<char> = size_txt.chars().collect();
                let sx = tw - cw - sc.len() as i32 * cw;
                self.text_cells(sx, y, g.lh, &sc, GUTTER_FG_CUR, false);
            }
        }
    }

    fn open_prompt(&mut self, kind: PromptKind, text: String) {
        self.prompt = Some(PromptState {
            kind,
            text,
            origin: self.ed.cur,
        });
    }

    fn do_save(&mut self) {
        match self.ed.save() {
            Ok(()) => {
                let p = self
                    .ed
                    .path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                self.msg(format!("Saved {}", p));
            }
            Err(e) => self.msg(format!("Save failed: {}", e)),
        }
    }

    fn copy_selection(&mut self, cut: bool) {
        let text = match self.ed.selected_text() {
            Some(t) => t,
            None => {
                let l = self.ed.cur.line;
                self.ed.select_line(l);
                let t = self.ed.selected_text().unwrap_or_default();
                let t = if t.ends_with('\n') {
                    t
                } else {
                    format!("{}\n", t)
                };
                if !cut {
                    self.ed.anchor = None;
                }
                t
            }
        };
        if text.is_empty() || text == "\n" && self.ed.lines.iter().all(|l| l.is_empty()) {
            self.ed.anchor = None;
        }
        self.set_clipboard(text);
        if cut {
            self.ed.delete_selection();
        }
    }

    fn on_key_editor(
        &mut self,
        ks: c_ulong,
        text: &str,
        ctrl: bool,
        shift: bool,
        alt: bool,
        was_discard: bool,
    ) {
        let rows = self.ed_geo().rows as isize;
        let ch = if ks < 128 { ks as u8 as char } else { '\0' };
        if ctrl && !alt {
            match ch {
                's' => {
                    if shift || self.ed.path.is_none() {
                        let pre = self
                            .ed
                            .path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        self.open_prompt(PromptKind::SaveAs, pre);
                    } else {
                        self.do_save();
                    }
                }
                'o' => self.open_prompt(PromptKind::Open, String::new()),
                'n' => self.new_tab(Editor::new()),
                'w' => self.close_tab(self.active),
                'q' => {
                    if self.any_modified() && !was_discard {
                        self.discard_ok = true;
                        self.msg("Unsaved changes. Press Ctrl+Q again to quit without saving.");
                    } else {
                        self.quit = true;
                    }
                }
                'z' => {
                    if shift {
                        self.ed.redo();
                    } else {
                        self.ed.undo();
                    }
                }
                'y' => {
                    self.ed.redo();
                }
                'x' => self.copy_selection(true),
                'c' => self.copy_selection(false),
                'v' => self.request_paste(self.x.atoms.clipboard, PasteTarget::Editor),
                'a' => self.ed.select_all(),
                'f' => {
                    let pre = match self.ed.selected_text() {
                        Some(t) if !t.contains('\n') => t,
                        _ => self.ed.find.clone(),
                    };
                    self.open_prompt(PromptKind::Find, pre);
                    self.find_update();
                }
                'h' => self.open_prompt(PromptKind::ReplaceFind, self.ed.find.clone()),
                'g' => self.open_prompt(PromptKind::Goto, String::new()),
                'd' => self.ed.duplicate_line(),
                'k' if shift => self.ed.delete_line(),
                'l' => {
                    let l = self.ed.cur.line;
                    self.ed.select_line(l);
                }
                _ => match ks {
                    XK_BackSpace => self.ed.delete_word_back(),
                    XK_Delete => self.ed.delete_word_fwd(),
                    XK_Left => self.ed.move_left(shift, true),
                    XK_Right => self.ed.move_right(shift, true),
                    XK_Home => self.ed.doc_start(shift),
                    XK_End => self.ed.doc_finish(shift),
                    XK_Up => self.ed.scroll_y = self.ed.scroll_y.saturating_sub(1),
                    XK_Down => self.ed.scroll_y += 1,
                    XK_Insert => self.copy_selection(false),
                    _ => {}
                },
            }
            return;
        }
        if alt {
            match ks {
                XK_Up => self.ed.move_lines(true),
                XK_Down => self.ed.move_lines(false),
                _ => {}
            }
            return;
        }
        match ks {
            XK_Left => self.ed.move_left(shift, false),
            XK_Right => self.ed.move_right(shift, false),
            XK_Up => self.ed.move_vert(-1, shift),
            XK_Down => self.ed.move_vert(1, shift),
            XK_Home => self.ed.home(shift),
            XK_End => self.ed.end(shift),
            XK_Prior => self.ed.move_vert(-(rows - 1).max(1), shift),
            XK_Next => self.ed.move_vert((rows - 1).max(1), shift),
            XK_BackSpace => self.ed.backspace(),
            XK_Delete => {
                if shift {
                    self.copy_selection(true)
                } else {
                    self.ed.delete()
                }
            }
            XK_Return => self.ed.newline(),
            XK_Tab => self.ed.tab(),
            XK_ISO_Left_Tab => self.ed.shift_tab(),
            XK_Escape => {
                self.ed.anchor = None;
                self.find_hl = false;
            }
            XK_Insert => {
                if shift {
                    self.request_paste(self.x.atoms.clipboard, PasteTarget::Editor)
                }
            }
            XK_F3 => self.find_step(!shift),
            _ => {
                if let Some(c) = text.chars().next() {
                    if (c as u32) >= 0x20 && c != '\x7f' {
                        self.ed.insert_str(text);
                    }
                }
            }
        }
    }

    fn on_key_term(&mut self, ks: c_ulong, text: &str, ctrl: bool, shift: bool, alt: bool) {
        if self.pty_dead {
            self.respawn();
            return;
        }
        let ch = if ks < 128 { ks as u8 as char } else { '\0' };
        if ctrl && shift {
            match ch {
                'c' => {
                    if let Some(t) = self.term.selected_text() {
                        self.set_clipboard(t);
                    }
                    return;
                }
                'v' => {
                    self.request_paste(self.x.atoms.clipboard, PasteTarget::Term);
                    return;
                }
                _ => {}
            }
        }
        if shift && !ctrl {
            let page = (self.term.rows as isize - 1).max(1);
            match ks {
                XK_Prior => return self.term.scroll_view(page),
                XK_Next => return self.term.scroll_view(-page),
                XK_Insert => {
                    self.request_paste(XA_PRIMARY, PasteTarget::Term);
                    return;
                }
                _ => {}
            }
        }
        self.term.view_offset = 0;
        self.term.sel_a = None;
        let bytes = encode_key(ks, text, shift, ctrl, alt, self.term.app_cursor);
        if !bytes.is_empty() {
            self.pty.send(&bytes);
        }
    }

    fn on_key_prompt(&mut self, ks: c_ulong, text: &str, ctrl: bool, shift: bool) {
        let ch = if ks < 128 { ks as u8 as char } else { '\0' };
        match ks {
            XK_Escape => {
                self.prompt = None;
                return;
            }
            XK_Return => return self.prompt_submit(shift),
            XK_BackSpace => {
                if let Some(p) = self.prompt.as_mut() {
                    p.text.pop();
                }
                self.find_update();
                return;
            }
            XK_Tab => {
                if let Some(p) = self.prompt.as_mut() {
                    if matches!(p.kind, PromptKind::Open | PromptKind::SaveAs) {
                        p.text = complete_path(&p.text);
                    }
                }
                return;
            }
            _ => {}
        }
        if ctrl {
            match ch {
                'v' => self.request_paste(self.x.atoms.clipboard, PasteTarget::Prompt),
                'u' => {
                    if let Some(p) = self.prompt.as_mut() {
                        p.text.clear();
                    }
                    self.find_update();
                }
                _ => {}
            }
            return;
        }
        if let Some(c) = text.chars().next() {
            if (c as u32) >= 0x20 && c != '\x7f' {
                if let Some(p) = self.prompt.as_mut() {
                    p.text.push_str(text);
                }
                self.find_update();
            }
        }
    }

    fn find_update(&mut self) {
        let Some(p) = self.prompt.as_ref() else {
            return;
        };
        if p.kind != PromptKind::Find {
            return;
        }
        let (t, origin) = (p.text.clone(), p.origin);
        self.ed.find = t.clone();
        self.find_hl = true;
        if t.is_empty() {
            self.ed.anchor = None;
            self.ed.cur = origin;
            return;
        }
        match self.ed.find_from(&t, origin, true) {
            Some((a, b)) => self.ed.select_range(a, b),
            None => {
                self.ed.anchor = None;
                self.ed.cur = origin;
            }
        }
    }

    fn find_step(&mut self, forward: bool) {
        if self.ed.find.is_empty() {
            return;
        }
        self.find_hl = true;
        let from = match self.ed.selection() {
            Some((a, b)) => {
                if forward {
                    b
                } else {
                    a
                }
            }
            None => self.ed.cur,
        };
        let t = self.ed.find.clone();
        match self.ed.find_from(&t, from, forward) {
            Some((a, b)) => self.ed.select_range(a, b),
            None => self.msg(format!("No match for '{}'", t)),
        }
    }

    fn prompt_submit(&mut self, shift: bool) {
        let Some(p) = self.prompt.take() else { return };
        let text = p.text.trim().to_string();
        match p.kind {
            PromptKind::Find => {
                self.prompt = Some(p);
                self.find_step(!shift);
            }
            PromptKind::Goto => {
                if let Ok(n) = text.parse::<usize>() {
                    self.ed.goto_line(n);
                } else {
                    self.msg("Not a line number");
                }
            }
            PromptKind::Open | PromptKind::SaveAs => {
                if text.is_empty() {
                    return;
                }
                let home = std::env::var("HOME").unwrap_or_default();
                let path = if let Some(r) = text.strip_prefix("~/") {
                    format!("{}/{}", home, r)
                } else {
                    text
                };
                if p.kind == PromptKind::Open && std::path::Path::new(&path).is_dir() {
                    match std::fs::canonicalize(&path) {
                        Ok(d) => {
                            self.tree.set_root(d);
                            self.tree_visible = true;
                            self.focus = Focus::Tree;
                            self.ed.follow = true;
                        }
                        Err(e) => self.msg(format!("Cannot open {}: {}", path, e)),
                    }
                } else if p.kind == PromptKind::Open {
                    self.open_path(PathBuf::from(&path));
                } else {
                    self.ed.path = Some(PathBuf::from(path));
                    self.do_save();
                }
            }
            PromptKind::NewFile(dir) => {
                if text.is_empty() {
                    return;
                }
                let path = dir.join(&text);
                if path.exists() {
                    self.msg("Already exists");
                    return;
                }
                if let Some(par) = path.parent() {
                    let _ = std::fs::create_dir_all(par);
                }
                match std::fs::File::create(&path) {
                    Ok(_) => {
                        self.tree.reveal(&path);
                        self.open_path(path);
                    }
                    Err(e) => self.msg(format!("Cannot create: {}", e)),
                }
            }
            PromptKind::NewDir(dir) => {
                if text.is_empty() {
                    return;
                }
                let path = dir.join(&text);
                match std::fs::create_dir_all(&path) {
                    Ok(()) => {
                        self.tree.reveal(&path);
                        self.msg(format!("Created {}", path.display()));
                    }
                    Err(e) => self.msg(format!("Cannot create: {}", e)),
                }
            }
            PromptKind::Rename(old) => {
                if text.is_empty() {
                    return;
                }
                let new = old
                    .parent()
                    .map(|d| d.join(&text))
                    .unwrap_or_else(|| PathBuf::from(&text));
                match std::fs::rename(&old, &new) {
                    Ok(()) => {
                        if self.ed.path.as_ref() == Some(&old) {
                            self.ed.path = Some(new.clone());
                        }
                        for t in self.tabs.iter_mut() {
                            if t.path.as_ref() == Some(&old) {
                                t.path = Some(new.clone());
                            }
                        }
                        self.tree.reveal(&new);
                        self.msg(format!("Renamed to {}", text));
                    }
                    Err(e) => self.msg(format!("Rename failed: {}", e)),
                }
            }
            PromptKind::DeleteMarked => {
                if text.eq_ignore_ascii_case("y") || text.eq_ignore_ascii_case("yes") {
                    let (mut ok, mut bad) = (0, 0);
                    for p in std::mem::take(&mut self.tree.marks) {
                        let r = if p.is_dir() {
                            std::fs::remove_dir(&p)
                        } else {
                            std::fs::remove_file(&p)
                        };
                        if r.is_ok() {
                            ok += 1;
                        } else {
                            bad += 1;
                        }
                    }
                    self.tree.rebuild();
                    self.msg(format!(
                        "Deleted {}, failed {} (folders only when empty)",
                        ok, bad
                    ));
                } else {
                    self.msg("Delete cancelled");
                }
            }
            PromptKind::Delete(path) => {
                if text.eq_ignore_ascii_case("y") || text.eq_ignore_ascii_case("yes") {
                    let r = if path.is_dir() {
                        std::fs::remove_dir(&path)
                    } else {
                        std::fs::remove_file(&path)
                    };
                    match r {
                        Ok(()) => {
                            self.tree.rebuild();
                            self.msg("Deleted (folders are only removed when empty)");
                        }
                        Err(e) => self.msg(format!("Delete failed: {}", e)),
                    }
                } else {
                    self.msg("Delete cancelled");
                }
            }
            PromptKind::ReplaceFind => {
                if !p.text.is_empty() {
                    self.prompt = Some(PromptState {
                        kind: PromptKind::ReplaceWith(p.text),
                        text: String::new(),
                        origin: p.origin,
                    });
                }
            }
            PromptKind::ReplaceWith(find) => {
                let n = self.ed.replace_all(&find, &p.text);
                self.ed.find = find;
                self.find_hl = true;
                self.msg(format!(
                    "Replaced {} occurrence{}",
                    n,
                    if n == 1 { "" } else { "s" }
                ));
            }
        }
    }

    fn ed_pos_at(&self, mx: i32, my: i32) -> Pos {
        let g = self.ed_geo();
        let row = (my - g.top).div_euclid(g.lh).max(0) as usize;
        let line = (self.ed.scroll_y + row).min(self.ed.lines.len() - 1);
        let vc = ((mx - g.text_x + self.x.cw / 2).div_euclid(self.x.cw)).max(0) as usize
            + self.ed.scroll_x;
        pos(line, col_from_visual(&self.ed.lines[line], vc))
    }

    fn term_cell_at(&self, mx: i32, my: i32) -> (usize, usize) {
        let r = ((my - self.lay.term_y - PAD).div_euclid(self.x.ch))
            .clamp(0, self.term.rows as i32 - 1) as usize;
        let c = ((mx - PAD).div_euclid(self.x.cw)).clamp(0, self.term.cols as i32 - 1) as usize;
        (self.term.view_abs(r), c)
    }

    fn region(&self, x: i32, y: i32) -> u8 {
        if y >= self.x.h - self.lay.status_h {
            3
        } else if y < self.lay.tab_h {
            5
        } else if self.term_visible && y >= self.lay.split_y && y < self.lay.term_y {
            1
        } else if self.term_visible && y >= self.lay.term_y {
            2
        } else if self.tree_visible && x < self.tree_w() {
            4
        } else {
            0
        }
    }

    fn on_button_press(&mut self, e: XButtonEvent) {
        self.redraw = true;
        self.blink = true;
        let (mx, my) = (e.x, e.y);
        let shift = e.state & 1 != 0;
        let ctrl = e.state & 4 != 0;
        let reg = self.region(mx, my);
        if reg != 5 {
            self.discard_tab = None;
        }
        match e.button {
            1 => {
                let now = Instant::now();
                let near = (mx - self.last_click_pos.0).abs() < 5
                    && (my - self.last_click_pos.1).abs() < 5;
                if near && now.duration_since(self.last_click) < Duration::from_millis(400) {
                    self.click_n = self.click_n % 3 + 1;
                } else {
                    self.click_n = 1;
                }
                self.last_click = now;
                self.last_click_pos = (mx, my);
                match reg {
                    1 => self.drag = Drag::Split,
                    5 => self.tab_click(mx, false),
                    4 => {
                        self.focus = Focus::Tree;
                        self.tree_click(my);
                    }
                    0 => {
                        self.focus = Focus::Editor;
                        let p = self.ed_pos_at(mx, my);
                        match self.click_n {
                            1 => {
                                if shift {
                                    if self.ed.anchor.is_none() {
                                        self.ed.anchor = Some(self.ed.cur);
                                    }
                                } else {
                                    self.ed.anchor = Some(p);
                                }
                                self.ed.cur = p;
                                self.ed.want_vcol = None;
                            }
                            2 => self.ed.select_word_at(p),
                            _ => self.ed.select_line(p.line),
                        }
                        self.drag = Drag::Editor;
                    }
                    2 => {
                        self.focus = Focus::Term;
                        let cell = self.term_cell_at(mx, my);
                        self.term_anchor = cell;
                        match self.click_n {
                            1 => self.term.sel_a = None,
                            2 => {
                                let (s, en) = self.term.word_at(cell.0, cell.1);
                                self.term.sel_a = Some(s);
                                self.term.sel_b = en;
                            }
                            _ => {
                                self.term.sel_a = Some((cell.0, 0));
                                self.term.sel_b = (cell.0, self.term.cols - 1);
                            }
                        }
                        self.drag = Drag::Term;
                    }
                    _ => {}
                }
            }
            2 => match reg {
                0 => {
                    self.focus = Focus::Editor;
                    let p = self.ed_pos_at(mx, my);
                    self.ed.anchor = None;
                    self.ed.cur = p;
                    self.request_paste(XA_PRIMARY, PasteTarget::Editor);
                }
                2 => {
                    self.focus = Focus::Term;
                    self.request_paste(XA_PRIMARY, PasteTarget::Term);
                }
                5 => self.tab_click(mx, true),
                _ => {}
            },
            4 | 5 => {
                let up = e.button == 4;
                if ctrl {
                    self.zoom(if up { 1 } else { -1 });
                } else if reg == 2 {
                    if self.term.alt {
                        let seq: &[u8] = match (up, self.term.app_cursor) {
                            (true, true) => b"\x1bOA",
                            (true, false) => b"\x1b[A",
                            (false, true) => b"\x1bOB",
                            (false, false) => b"\x1b[B",
                        };
                        for _ in 0..3 {
                            self.pty.send(seq);
                        }
                    } else {
                        self.term.scroll_view(if up { 3 } else { -3 });
                    }
                } else if reg == 4 {
                    let max = self.tree.items.len().saturating_sub(1);
                    self.tree.scroll = if up {
                        self.tree.scroll.saturating_sub(3)
                    } else {
                        (self.tree.scroll + 3).min(max)
                    };
                } else if reg == 0 {
                    if shift {
                        let d = 6usize;
                        self.ed.scroll_x = if up {
                            self.ed.scroll_x.saturating_sub(d)
                        } else {
                            self.ed.scroll_x + d
                        };
                    } else if up {
                        self.ed.scroll_y = self.ed.scroll_y.saturating_sub(3);
                    } else {
                        self.ed.scroll_y = (self.ed.scroll_y + 3).min(self.ed.lines.len() - 1);
                    }
                }
            }
            6 | 7 => {
                if reg == 0 {
                    let d = 6usize;
                    self.ed.scroll_x = if e.button == 6 {
                        self.ed.scroll_x.saturating_sub(d)
                    } else {
                        self.ed.scroll_x + d
                    };
                }
            }
            _ => {}
        }
    }

    fn on_button_release(&mut self, e: XButtonEvent) {
        if e.button != 1 {
            return;
        }
        match self.drag {
            Drag::Editor => {
                if let Some(t) = self.ed.selected_text() {
                    self.set_primary(t);
                }
            }
            Drag::Term => {
                if let Some(t) = self.term.selected_text() {
                    self.set_primary(t);
                }
            }
            _ => {}
        }
        self.drag = Drag::None;
        self.redraw = true;
    }

    fn on_motion(&mut self, e: XMotionEvent) {
        let (mx, my) = (e.x, e.y);
        match self.drag {
            Drag::Split => {
                let avail = (self.x.h - self.lay.status_h).max(1) as f32;
                let t = avail - my as f32 + (SPLIT_H as f32) / 2.0;
                self.term_frac = (t / avail).clamp(0.08, 0.85);
                self.apply_layout();
            }
            Drag::Editor => {
                let g = self.ed_geo();
                if my < g.top {
                    self.ed.scroll_y = self.ed.scroll_y.saturating_sub(1);
                } else if my >= self.lay.ed_h {
                    self.ed.scroll_y = (self.ed.scroll_y + 1).min(self.ed.lines.len() - 1);
                }
                if mx < g.text_x && self.ed.scroll_x > 0 {
                    self.ed.scroll_x -= 1;
                } else if mx > self.x.w - 4 {
                    self.ed.scroll_x += 1;
                }
                let p = self.ed_pos_at(mx, my.clamp(g.top, self.lay.ed_h - 1));
                self.ed.cur = p;
                self.ed.want_vcol = None;
                self.redraw = true;
            }
            Drag::Term => {
                if my < self.lay.term_y + PAD {
                    self.term.scroll_view(1);
                } else if my > self.x.h - self.lay.status_h {
                    self.term.scroll_view(-1);
                }
                let cell = self.term_cell_at(mx, my);
                if cell != self.term_anchor || self.term.sel_a.is_some() {
                    self.term.sel_a = Some(self.term_anchor);
                    self.term.sel_b = cell;
                }
                self.redraw = true;
            }
            Drag::None => {
                let shape = match self.region(mx, my) {
                    0 | 2 => 1,
                    1 => 2,
                    _ => 0,
                };
                self.x.set_cursor(shape);
            }
        }
    }

    fn text_cells(&self, x: i32, ytop: i32, cell_h: i32, chars: &[char], rgb: u32, bold: bool) {
        let base = ytop + (cell_h - self.x.ch) / 2 + self.x.ascent;
        let cw = self.x.cw;
        let mut i = 0;
        while i < chars.len() {
            if is_plain(chars[i]) {
                let j = i + chars[i..].iter().take_while(|c| is_plain(**c)).count();
                if chars[i..j].iter().any(|&c| c != ' ') {
                    let s: String = chars[i..j].iter().collect();
                    self.x.text(x + i as i32 * cw, base, &s, rgb, bold);
                }
                i = j;
            } else {
                let c = chars[i];
                if c != '\0' && c != ' ' && !c.is_control() {
                    let mut b = [0u8; 4];
                    self.x
                        .text(x + i as i32 * cw, base, c.encode_utf8(&mut b), rgb, bold);
                }
                i += 1;
            }
        }
    }

    fn fill_cols(&self, g: &EdGeo, y: i32, vs: usize, ve: usize, color: u32) {
        let sx = self.ed.scroll_x;
        let lo = vs.max(sx);
        let hi = ve.min(sx + g.vis_cols + 1);
        if hi <= lo {
            return;
        }
        let cw = self.x.cw;
        self.x.fill(
            g.text_x + (lo - sx) as i32 * cw,
            y,
            (hi - lo) as i32 * cw,
            g.lh,
            color,
        );
    }

    fn render(&mut self) {
        let g = self.ed_geo();
        if self.ed.follow {
            self.ed.ensure_visible(g.rows, g.vis_cols);
            self.ed.follow = false;
        }
        self.ed.clamp_scroll();
        let g = self.ed_geo();
        self.x.fill(0, 0, self.x.w, self.x.h, BG);
        self.draw_editor(&g);
        self.draw_tabs();
        if self.tree_visible {
            self.tree.ensure_visible(g.rows.saturating_sub(1));
            self.draw_tree(&g);
        }
        if self.term_visible {
            let active = self.drag == Drag::Split;
            self.x.fill(
                0,
                self.lay.split_y,
                self.x.w,
                SPLIT_H,
                if active { ACCENT } else { SPLIT_COL },
            );
            self.draw_term();
        }
        self.draw_status();
        self.x.present();
    }

    fn draw_editor(&self, g: &EdGeo) {
        let x = &self.x;
        let ed = &self.ed;
        let w = x.w;
        x.fill(g.ox, g.top, w - g.ox, self.lay.ed_h - g.top, BG);
        let sel = ed.selection();
        let find_chars: Vec<char> = ed.find.chars().collect();
        let ci = smart_ci(&ed.find);
        let focused = self.focus == Focus::Editor;
        for i in 0..g.rows {
            let li = ed.scroll_y + i;
            if li >= ed.lines.len() {
                break;
            }
            let y = g.top + i as i32 * g.lh;
            let line = &ed.lines[li];
            let vis = expand_tabs(line);
            if li == ed.cur.line && sel.is_none() && focused {
                x.fill(g.ox + g.gutter_w, y, w - g.ox - g.gutter_w, g.lh, CURLINE);
            }
            if self.find_hl && !find_chars.is_empty() {
                let mut c = 0;
                while let Some(m) = find_in_line(line, &find_chars, c, ci) {
                    self.fill_cols(
                        g,
                        y,
                        visual_col(line, m),
                        visual_col(line, m + find_chars.len()),
                        FIND_BG,
                    );
                    c = m + find_chars.len();
                }
            }
            if let Some((a, b)) = sel {
                if li >= a.line && li <= b.line {
                    let vs = if li == a.line {
                        visual_col(line, a.col)
                    } else {
                        0
                    };
                    let ve = if li == b.line {
                        visual_col(line, b.col)
                    } else {
                        vis.len() + 1
                    };
                    self.fill_cols(g, y, vs, ve, SEL_BG);
                }
            }
            let start = ed.scroll_x.min(vis.len());
            let end = (ed.scroll_x + g.vis_cols).min(vis.len());
            if start < end {
                self.text_cells(g.text_x, y, g.lh, &vis[start..end], FG, false);
            }
            x.fill(g.ox, y, g.gutter_w, g.lh, BG);
            let num: Vec<char> = (li + 1).to_string().chars().collect();
            let nx = g.ox + g.gutter_w - x.cw - num.len() as i32 * x.cw;
            let col = if li == ed.cur.line {
                GUTTER_FG_CUR
            } else {
                GUTTER_FG
            };
            self.text_cells(nx, y, g.lh, &num, col, false);
        }
        let cl = ed.cur.line;
        if cl >= ed.scroll_y && cl < ed.scroll_y + g.rows {
            let vc = visual_col(&ed.lines[cl], ed.cur.col);
            if vc >= ed.scroll_x && vc <= ed.scroll_x + g.vis_cols {
                let cx = g.text_x + (vc - ed.scroll_x) as i32 * x.cw;
                let cy = g.top + (cl - ed.scroll_y) as i32 * g.lh;
                if focused && self.has_focus {
                    if self.blink {
                        x.fill(cx, cy, 2, g.lh, CURSOR_COL);
                    }
                } else {
                    x.fill(cx, cy, 2, g.lh, CURSOR_DIM);
                }
            }
        }
        x.fill(0, self.lay.ed_h, w, 0, BG);
    }

    fn resolve(&self, c: &Cell, selected: bool) -> (u32, u32) {
        let mut fg = if c.fg == DEFAULT_FG { TERM_FG } else { c.fg };
        let mut bg = if c.bg == DEFAULT_BG { TERM_BG } else { c.bg };
        if c.attr & REVERSE != 0 {
            std::mem::swap(&mut fg, &mut bg);
        }
        if c.attr & DIM != 0 {
            let f = |v: u32| (v * 6 / 10) & 0xff;
            fg = (f(fg >> 16) << 16) | (f(fg >> 8) << 8) | f(fg);
        }
        if selected {
            bg = SEL_BG;
        }
        (fg, bg)
    }

    fn draw_term(&self) {
        let t = &self.term;
        let x = &self.x;
        let (cw, ch) = (x.cw, x.ch);
        let ox = PAD;
        let oy = self.lay.term_y + PAD;
        x.fill(0, self.lay.term_y, x.w, self.lay.term_h + 40, TERM_BG);
        x.fill(0, x.h - self.lay.status_h - 0, 0, 0, TERM_BG);
        for r in 0..t.rows {
            let row = t.view_row(r);
            let abs = t.view_abs(r);
            let y = oy + r as i32 * ch;
            let cells: Vec<(char, u32, u32, u8)> = (0..t.cols)
                .map(|c| {
                    let cell = row.get(c).copied().unwrap_or(BLANK);
                    let (fg, bg) = self.resolve(&cell, t.is_sel(abs, c));
                    (cell.ch, fg, bg, cell.attr)
                })
                .collect();
            let mut c = 0;
            while c < cells.len() {
                let bg = cells[c].2;
                let mut e = c + 1;
                while e < cells.len() && cells[e].2 == bg {
                    e += 1;
                }
                if bg != TERM_BG {
                    x.fill(ox + c as i32 * cw, y, (e - c) as i32 * cw, ch, bg);
                }
                c = e;
            }
            let mut c = 0;
            while c < cells.len() {
                let (fg, attr) = (cells[c].1, cells[c].3 & (BOLD | UNDERLINE));
                let mut e = c + 1;
                while e < cells.len()
                    && cells[e].1 == fg
                    && (cells[e].3 & (BOLD | UNDERLINE)) == attr
                {
                    e += 1;
                }
                let chars: Vec<char> = cells[c..e].iter().map(|k| k.0).collect();
                self.text_cells(ox + c as i32 * cw, y, ch, &chars, fg, attr & BOLD != 0);
                if attr & UNDERLINE != 0 {
                    x.fill(ox + c as i32 * cw, y + ch - 2, (e - c) as i32 * cw, 1, fg);
                }
                c = e;
            }
        }
        if t.view_offset == 0 && t.cursor_visible {
            let (cx, cy) = (t.cx.min(t.cols - 1), t.cy.min(t.rows - 1));
            let px = ox + cx as i32 * cw;
            let py = oy + cy as i32 * ch;
            if self.focus == Focus::Term && self.has_focus {
                if self.blink {
                    x.fill(px, py, cw, ch, CURSOR_COL);
                    let cell = t.view_row(cy).get(cx).copied().unwrap_or(BLANK);
                    if cell.ch != ' ' && cell.ch != '\0' {
                        self.text_cells(px, py, ch, &[cell.ch], TERM_BG, cell.attr & BOLD != 0);
                    }
                }
            } else {
                x.fill(px, py, cw, 1, CURSOR_DIM);
                x.fill(px, py + ch - 1, cw, 1, CURSOR_DIM);
                x.fill(px, py, 1, ch, CURSOR_DIM);
                x.fill(px + cw - 1, py, 1, ch, CURSOR_DIM);
            }
        }
        if t.view_offset > 0 {
            let label: Vec<char> = format!(" scrolled back {} lines ", t.view_offset)
                .chars()
                .collect();
            let w = label.len() as i32 * cw;
            x.fill(x.w - w - PAD, oy, w, ch, 0x444444);
            self.text_cells(x.w - w - PAD, oy, ch, &label, 0xffffff, false);
        }
    }

    fn draw_status(&self) {
        let x = &self.x;
        let y0 = x.h - self.lay.status_h;
        let cw = x.cw;
        x.fill(0, y0, x.w, self.lay.status_h, STATUS_BG);
        x.fill(0, y0, x.w, 1, SPLIT_COL);
        let pill: Vec<char> = (match self.focus {
            Focus::Editor => " EDITOR ",
            Focus::Term => " TERMINAL ",
            Focus::Tree => " FILES ",
        })
        .chars()
        .collect();
        let pw = pill.len() as i32 * cw;
        x.fill(0, y0 + 1, pw, self.lay.status_h - 1, ACCENT);
        self.text_cells(0, y0 + 1, self.lay.status_h - 1, &pill, 0xffffff, true);
        let tx = pw + cw;
        let ty = y0 + 1;
        let th = self.lay.status_h - 1;
        let msg_active = self
            .msg
            .as_ref()
            .map_or(false, |(_, t)| t.elapsed() < Duration::from_secs(4));
        if let Some(p) = &self.prompt {
            let label = match &p.kind {
                PromptKind::Open => "Open file: ".to_string(),
                PromptKind::SaveAs => "Save as: ".to_string(),
                PromptKind::Find => "Find (Enter next, Shift+Enter prev, Esc close): ".to_string(),
                PromptKind::Goto => "Go to line: ".to_string(),
                PromptKind::ReplaceFind => "Replace all - find: ".to_string(),
                PromptKind::ReplaceWith(f) => format!("Replace '{}' with: ", f),
                PromptKind::NewFile(d) => format!("New file in {}/: ", d.display()),
                PromptKind::NewDir(d) => format!("New folder in {}/: ", d.display()),
                PromptKind::Rename(o) => format!(
                    "Rename {} to: ",
                    o.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
                PromptKind::DeleteMarked => format!(
                    "Delete {} marked item(s)? type y to confirm: ",
                    self.tree.marks.len()
                ),
                PromptKind::Delete(o) => format!(
                    "Delete '{}'? type y to confirm: ",
                    o.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
            };
            let full: Vec<char> = format!("{}{}", label, p.text).chars().collect();
            let max = ((x.w - tx) / cw - 2).max(1) as usize;
            let start = full.len().saturating_sub(max);
            let shown = &full[start..];
            self.text_cells(tx, ty, th, shown, STATUS_FG, false);
            let cx = tx + shown.len() as i32 * cw;
            x.fill(cx, ty + 2, 2, th - 4, CURSOR_COL);
            return;
        }
        let left = if msg_active {
            (self.msg.as_ref().unwrap().0.clone(), WARN)
        } else {
            let name = self.ed.display_name();
            (
                format!(
                    "{}{}",
                    name,
                    if self.ed.modified { " \u{25cf}" } else { "" }
                ),
                STATUS_FG,
            )
        };
        let lc: Vec<char> = left.0.chars().collect();
        self.text_cells(tx, ty, th, &lc, left.1, false);
        let zoom = self.px * 100 / self.base_px;
        let right = if self.focus == Focus::Tree {
            "CR open  - up  % file  d dir  R ren  D del  mf/mt/mc/mm marks  gh hidden  s sort  Esc editor".to_string()
        } else if self.focus == Focus::Editor {
            format!(
                "Ln {}, Col {}   Spaces: {}   UTF-8   {}   {}%",
                self.ed.cur.line + 1,
                visual_col(&self.ed.lines[self.ed.cur.line], self.ed.cur.col) + 1,
                TAB,
                if self.ed.crlf { "CRLF" } else { "LF" },
                zoom
            )
        } else {
            format!("Ctrl+` editor   Ctrl+Shift+C/V copy/paste   {}%", zoom)
        };
        let rc: Vec<char> = right.chars().collect();
        let rx = x.w - (rc.len() as i32 + 1) * cw;
        if rx > tx + lc.len() as i32 * cw + 2 * cw {
            self.text_cells(rx, ty, th, &rc, STATUS_FG, false);
        }
    }

    fn update_title(&mut self) {
        let t = format!(
            "{}{} - branna",
            if self.ed.modified { "\u{25cf} " } else { "" },
            self.ed.display_name()
        );
        if t != self.title {
            self.x.set_title(&t);
            self.title = t;
        }
    }

    fn run(&mut self) {
        loop {
            self.handle_x_events();
            if self.quit {
                break;
            }
            self.pump_pty();
            self.sync_term_dir();
            if self.last_blink.elapsed() >= Duration::from_millis(530) {
                self.blink = !self.blink;
                self.last_blink = Instant::now();
                self.redraw = true;
            }
            if let Some((_, t)) = &self.msg {
                if t.elapsed() >= Duration::from_secs(4) {
                    self.msg = None;
                    self.redraw = true;
                }
            }
            if self.redraw {
                self.redraw = false;
                self.render();
            }
            self.update_title();
            let pfd = if self.pty_dead { -1 } else { self.pty.fd() };
            let mut fds = [
                PollFd {
                    fd: self.x.conn_fd(),
                    events: POLLIN,
                    revents: 0,
                },
                PollFd {
                    fd: pfd,
                    events: POLLIN | if self.pty.wants_write() { POLLOUT } else { 0 },
                    revents: 0,
                },
            ];
            unsafe { poll(fds.as_mut_ptr(), 2, 100) };
            if fds[1].revents & POLLOUT != 0 {
                self.pty.flush();
            }
        }
    }
}

fn main() {
    let arg = std::env::args().nth(1);
    if matches!(arg.as_deref(), Some("-h") | Some("--help")) {
        println!("usage: branna [file|folder]\n\nCtrl+S save  Ctrl+O open  Ctrl+F find  Ctrl+H replace  Ctrl+G goto\nCtrl++ / Ctrl+- / Ctrl+0 zoom   Ctrl+` show/hide terminal   Ctrl+B / Ctrl+E file tree   Ctrl+Tab next tab   Ctrl+W close tab");
        return;
    }
    match App::new(arg) {
        Ok(mut app) => app.run(),
        Err(e) => {
            eprintln!("branna: {}", e);
            std::process::exit(1);
        }
    }
}
