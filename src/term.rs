use std::collections::VecDeque;
use std::env;
use std::ffi::{c_char, c_int, c_ulong, CStr};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};

extern "C" {
    fn posix_openpt(flags: c_int) -> c_int;
    fn grantpt(fd: c_int) -> c_int;
    fn unlockpt(fd: c_int) -> c_int;
    fn ptsname_r(fd: c_int, buf: *mut c_char, len: usize) -> c_int;
    fn ioctl(fd: c_int, req: c_ulong, ...) -> c_int;
    fn setsid() -> i32;
    fn kill(pid: i32, sig: c_int) -> c_int;
    fn tcgetpgrp(fd: c_int) -> i32;
    pub fn poll(fds: *mut PollFd, n: c_ulong, timeout: c_int) -> c_int;
}

#[repr(C)]
pub struct PollFd {
    pub fd: c_int,
    pub events: i16,
    pub revents: i16,
}
pub const POLLIN: i16 = 1;
pub const POLLOUT: i16 = 4;

#[repr(C)]
struct Winsize {
    row: u16,
    col: u16,
    xpix: u16,
    ypix: u16,
}

const O_RDWR: c_int = 2;
const O_NOCTTY: c_int = 0o400;
const O_NONBLOCK: c_int = 0o4000;
const O_CLOEXEC: c_int = 0o2000000;
const TIOCSCTTY: c_ulong = 0x540E;
const TIOCSWINSZ: c_ulong = 0x5414;

pub struct Pty {
    master: File,
    child: Child,
    out: Vec<u8>,
}

impl Pty {
    pub fn spawn(cols: usize, rows: usize) -> io::Result<Pty> {
        unsafe {
            let fd = posix_openpt(O_RDWR | O_NOCTTY | O_CLOEXEC | O_NONBLOCK);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            let master = File::from_raw_fd(fd);
            if grantpt(fd) != 0 || unlockpt(fd) != 0 {
                return Err(io::Error::last_os_error());
            }
            let mut buf = [0 as c_char; 128];
            let r = ptsname_r(fd, buf.as_mut_ptr(), 128);
            if r != 0 {
                return Err(io::Error::from_raw_os_error(r));
            }
            let path = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
            let ws = Winsize { row: rows as u16, col: cols as u16, xpix: 0, ypix: 0 };
            ioctl(fd, TIOCSWINSZ, &ws as *const Winsize);
            let slave = OpenOptions::new().read(true).write(true).open(&path)?;
            let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
            let mut cmd = Command::new(&shell);
            cmd.env("TERM", "xterm-256color")
                .env("COLORTERM", "truecolor")
                .env_remove("LINES")
                .env_remove("COLUMNS")
                .stdin(Stdio::from(slave.try_clone()?))
                .stdout(Stdio::from(slave.try_clone()?))
                .stderr(Stdio::from(slave));
            cmd.pre_exec(|| {
                setsid();
                ioctl(0, TIOCSCTTY, 0 as c_int);
                Ok(())
            });
            let child = cmd.spawn()?;
            Ok(Pty { master, child, out: Vec::new() })
        }
    }

    pub fn fd(&self) -> c_int {
        self.master.as_raw_fd()
    }

    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.master.read(buf)
    }

    pub fn send(&mut self, data: &[u8]) {
        self.out.extend_from_slice(data);
        self.flush();
    }

    pub fn flush(&mut self) {
        while !self.out.is_empty() {
            match self.master.write(&self.out) {
                Ok(0) => break,
                Ok(n) => {
                    self.out.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    self.out.clear();
                    break;
                }
            }
        }
    }

    pub fn wants_write(&self) -> bool {
        !self.out.is_empty()
    }

    pub fn resize(&self, cols: usize, rows: usize, w: i32, h: i32) {
        let ws = Winsize { row: rows as u16, col: cols as u16, xpix: w.max(0) as u16, ypix: h.max(0) as u16 };
        unsafe {
            ioctl(self.fd(), TIOCSWINSZ, &ws as *const Winsize);
        }
    }

    pub fn is_idle(&self) -> bool {
        unsafe { tcgetpgrp(self.fd()) == self.child.id() as i32 }
    }

    pub fn reap(&mut self) {
        let _ = self.child.try_wait();
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            unsafe {
                kill(self.child.id() as i32, 1);
            }
        }
    }
}


pub const DEFAULT_FG: u32 = 0x0100_0000;
pub const DEFAULT_BG: u32 = 0x0200_0000;
pub const BOLD: u8 = 1;
pub const DIM: u8 = 2;
pub const ITALIC: u8 = 4;
pub const UNDERLINE: u8 = 8;
pub const REVERSE: u8 = 16;

pub const ANSI: [u32; 16] = [
    0x000000, 0xcd3131, 0x0dbc79, 0xe5e510, 0x2472c8, 0xbc3fbc, 0x11a8cd, 0xe5e5e5, 0x666666, 0xf14c4c, 0x23d18b,
    0xf5f543, 0x3b8eea, 0xd670d6, 0x29b8db, 0xffffff,
];

#[derive(Clone, Copy, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub fg: u32,
    pub bg: u32,
    pub attr: u8,
}

pub const BLANK: Cell = Cell { ch: ' ', fg: DEFAULT_FG, bg: DEFAULT_BG, attr: 0 };

fn palette256(n: i64) -> u32 {
    let n = n.clamp(0, 255) as usize;
    if n < 16 {
        ANSI[n]
    } else if n < 232 {
        let c = n - 16;
        let lv = [0u32, 95, 135, 175, 215, 255];
        (lv[c / 36] << 16) | (lv[(c / 6) % 6] << 8) | lv[c % 6]
    } else {
        let g = 8 + 10 * (n as u32 - 232);
        (g << 16) | (g << 8) | g
    }
}

fn char_width(c: char) -> usize {
    let u = c as u32;
    if (0x300..=0x36f).contains(&u) || (0x200b..=0x200f).contains(&u) || u == 0xfe0f || u == 0xfe0e {
        return 0;
    }
    let wide = (0x1100..=0x115f).contains(&u)
        || (0x2e80..=0xa4cf).contains(&u)
        || (0xac00..=0xd7a3).contains(&u)
        || (0xf900..=0xfaff).contains(&u)
        || (0xfe30..=0xfe6f).contains(&u)
        || (0xff00..=0xff60).contains(&u)
        || (0xffe0..=0xffe6).contains(&u)
        || (0x1f300..=0x1faff).contains(&u)
        || (0x20000..=0x3fffd).contains(&u);
    if wide {
        2
    } else {
        1
    }
}

#[derive(Clone, Copy, PartialEq)]
enum St {
    Ground,
    Esc,
    EscSkip,
    Csi,
    Str,
    StrEsc,
}

const SB_MAX: usize = 10000;

pub struct Term {
    pub cols: usize,
    pub rows: usize,
    grid: Vec<Vec<Cell>>,
    other: Vec<Vec<Cell>>,
    scrollback: VecDeque<Vec<Cell>>,
    pub cx: usize,
    pub cy: usize,
    pen: Cell,
    top: usize,
    bottom: usize,
    saved: (usize, usize, Cell),
    wrap_pending: bool,
    pub autowrap: bool,
    pub cursor_visible: bool,
    pub app_cursor: bool,
    pub bracketed: bool,
    pub alt: bool,
    pub view_offset: usize,
    pub sel_a: Option<(usize, usize)>,
    pub sel_b: (usize, usize),
    pub reply: Vec<u8>,
    st: St,
    utf8_buf: Vec<u8>,
    utf8_need: usize,
    csi_buf: Vec<u8>,
    csi_inter: Vec<u8>,
}

fn blank_row(cols: usize, bg: u32) -> Vec<Cell> {
    vec![Cell { ch: ' ', fg: DEFAULT_FG, bg, attr: 0 }; cols]
}

impl Term {
    pub fn new(cols: usize, rows: usize) -> Term {
        let (cols, rows) = (cols.max(2), rows.max(2));
        Term {
            cols,
            rows,
            grid: vec![blank_row(cols, DEFAULT_BG); rows],
            other: vec![blank_row(cols, DEFAULT_BG); rows],
            scrollback: VecDeque::new(),
            cx: 0,
            cy: 0,
            pen: BLANK,
            top: 0,
            bottom: rows - 1,
            saved: (0, 0, BLANK),
            wrap_pending: false,
            autowrap: true,
            cursor_visible: true,
            app_cursor: false,
            bracketed: false,
            alt: false,
            view_offset: 0,
            sel_a: None,
            sel_b: (0, 0),
            reply: Vec::new(),
            st: St::Ground,
            utf8_buf: Vec::new(),
            utf8_need: 0,
            csi_buf: Vec::new(),
            csi_inter: Vec::new(),
        }
    }


    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    pub fn abs_row(&self, abs: usize) -> &[Cell] {
        let sb = self.scrollback.len();
        if abs < sb {
            &self.scrollback[abs]
        } else {
            &self.grid[(abs - sb).min(self.rows - 1)]
        }
    }

    pub fn view_abs(&self, r: usize) -> usize {
        let sb = self.scrollback.len();
        sb - self.view_offset.min(sb) + r
    }

    pub fn view_row(&self, r: usize) -> &[Cell] {
        self.abs_row(self.view_abs(r))
    }

    pub fn scroll_view(&mut self, delta: isize) {
        let max = self.scrollback.len() as isize;
        self.view_offset = (self.view_offset as isize + delta).clamp(0, max) as usize;
    }

    fn sel_ordered(&self) -> Option<((usize, usize), (usize, usize))> {
        let a = self.sel_a?;
        let b = self.sel_b;
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    pub fn is_sel(&self, abs: usize, col: usize) -> bool {
        match self.sel_ordered() {
            Some((s, e)) => (abs, col) >= s && (abs, col) <= e,
            None => false,
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        let (s, e) = self.sel_ordered()?;
        let total = self.scrollback.len() + self.rows;
        let mut out = String::new();
        for abs in s.0..=e.0.min(total - 1) {
            let row = self.abs_row(abs);
            let c0 = if abs == s.0 { s.1 } else { 0 };
            let c1 = if abs == e.0 { e.1 + 1 } else { row.len() };
            let mut line: String = row
                .iter()
                .enumerate()
                .filter(|(i, c)| *i >= c0 && *i < c1 && c.ch != '\0')
                .map(|(_, c)| c.ch)
                .collect();
            while line.ends_with(' ') {
                line.pop();
            }
            out.push_str(&line);
            if abs < e.0 {
                out.push('\n');
            }
        }
        Some(out)
    }

    pub fn word_at(&self, abs: usize, col: usize) -> ((usize, usize), (usize, usize)) {
        let row = self.abs_row(abs);
        let col = col.min(row.len().saturating_sub(1));
        let cls = |c: char| -> u8 {
            if c == ' ' || c == '\0' {
                0
            } else if c.is_alphanumeric() || c == '_' || c == '/' || c == '.' || c == '-' {
                1
            } else {
                2
            }
        };
        let k = cls(row[col].ch);
        let mut s = col;
        while s > 0 && cls(row[s - 1].ch) == k {
            s -= 1;
        }
        let mut e = col;
        while e + 1 < row.len() && cls(row[e + 1].ch) == k {
            e += 1;
        }
        ((abs, s), (abs, e))
    }


    pub fn resize(&mut self, cols: usize, rows: usize) {
        let (cols, rows) = (cols.max(2), rows.max(2));
        if cols == self.cols && rows == self.rows {
            return;
        }
        fn adjust(
            grid: &mut Vec<Vec<Cell>>,
            cols: usize,
            rows: usize,
            cy: &mut usize,
            sb: Option<&mut VecDeque<Vec<Cell>>>,
        ) {
            if grid.len() > rows {
                let excess = grid.len() - rows;
                let from_top = if *cy >= rows { (*cy + 1 - rows).min(excess) } else { 0 };
                let removed: Vec<Vec<Cell>> = grid.drain(0..from_top).collect();
                *cy = cy.saturating_sub(from_top);
                if let Some(sb) = sb {
                    for r in removed {
                        sb.push_back(r);
                        while sb.len() > SB_MAX {
                            sb.pop_front();
                        }
                    }
                }
                grid.truncate(rows);
            }
            while grid.len() < rows {
                grid.push(blank_row(cols, DEFAULT_BG));
            }
            for r in grid.iter_mut() {
                r.resize(cols, BLANK);
            }
        }
        let mut cy = self.cy;
        if self.alt {
            adjust(&mut self.grid, cols, rows, &mut cy, None);
            let mut dummy = 0;
            adjust(&mut self.other, cols, rows, &mut dummy, Some(&mut self.scrollback));
        } else {
            adjust(&mut self.grid, cols, rows, &mut cy, Some(&mut self.scrollback));
            let mut dummy = 0;
            adjust(&mut self.other, cols, rows, &mut dummy, None);
        }
        self.cy = cy;
        self.cols = cols;
        self.rows = rows;
        self.top = 0;
        self.bottom = rows - 1;
        self.cx = self.cx.min(cols - 1);
        self.cy = self.cy.min(rows - 1);
        self.wrap_pending = false;
        self.view_offset = 0;
        self.sel_a = None;
    }


    pub fn feed(&mut self, data: &[u8]) {
        for &b in data {
            self.byte(b);
        }
    }

    fn byte(&mut self, b: u8) {
        match self.st {
            St::Ground => self.ground(b),
            St::Esc => self.esc(b),
            St::EscSkip => self.st = St::Ground,
            St::Csi => self.csi_byte(b),
            St::Str => {
                if b == 0x07 || b == 0x18 || b == 0x1a {
                    self.st = St::Ground;
                } else if b == 0x1b {
                    self.st = St::StrEsc;
                }
            }
            St::StrEsc => {
                self.st = St::Ground;
                if b != b'\\' {
                    self.esc(b);
                }
            }
        }
    }

    fn ground(&mut self, b: u8) {
        if self.utf8_need > 0 {
            if b & 0xC0 == 0x80 {
                self.utf8_buf.push(b);
                self.utf8_need -= 1;
                if self.utf8_need == 0 {
                    let c = std::str::from_utf8(&self.utf8_buf)
                        .ok()
                        .and_then(|s| s.chars().next())
                        .unwrap_or('\u{fffd}');
                    self.utf8_buf.clear();
                    self.put(c);
                }
                return;
            }
            self.utf8_need = 0;
            self.utf8_buf.clear();
            self.put('\u{fffd}');
        }
        match b {
            0x1b => self.st = St::Esc,
            0x00..=0x1f => self.control(b),
            0x20..=0x7e => self.put(b as char),
            0x7f => {}
            0xC2..=0xDF => {
                self.utf8_need = 1;
                self.utf8_buf = vec![b];
            }
            0xE0..=0xEF => {
                self.utf8_need = 2;
                self.utf8_buf = vec![b];
            }
            0xF0..=0xF4 => {
                self.utf8_need = 3;
                self.utf8_buf = vec![b];
            }
            _ => self.put('\u{fffd}'),
        }
    }

    fn control(&mut self, b: u8) {
        match b {
            0x08 => {
                self.cx = self.cx.saturating_sub(1);
                self.wrap_pending = false;
            }
            0x09 => {
                self.cx = ((self.cx / 8 + 1) * 8).min(self.cols - 1);
                self.wrap_pending = false;
            }
            0x0a | 0x0b | 0x0c => {
                self.wrap_pending = false;
                self.linefeed();
            }
            0x0d => {
                self.cx = 0;
                self.wrap_pending = false;
            }
            _ => {}
        }
    }

    fn esc(&mut self, b: u8) {
        self.st = St::Ground;
        match b {
            b'[' => {
                self.st = St::Csi;
                self.csi_buf.clear();
                self.csi_inter.clear();
            }
            b']' | b'P' | b'X' | b'^' | b'_' => self.st = St::Str,
            b'(' | b')' | b'*' | b'+' | b'-' | b'.' | b'/' | b'#' | b'%' => self.st = St::EscSkip,
            b'7' => self.saved = (self.cx, self.cy, self.pen),
            b'8' => self.restore_cursor(),
            b'D' => self.linefeed(),
            b'E' => {
                self.cx = 0;
                self.linefeed();
            }
            b'M' => self.reverse_index(),
            b'c' => self.reset(),
            b'Z' => self.reply.extend_from_slice(b"\x1b[?1;2c"),
            0x1b => self.st = St::Esc,
            _ => {}
        }
    }

    fn csi_byte(&mut self, b: u8) {
        match b {
            0x1b => self.st = St::Esc,
            0x18 | 0x1a => self.st = St::Ground,
            0x00..=0x1f => self.control(b),
            0x20..=0x2f => self.csi_inter.push(b),
            0x30..=0x3f => self.csi_buf.push(b),
            0x40..=0x7e => {
                self.st = St::Ground;
                self.csi_dispatch(b);
            }
            _ => {}
        }
    }

    fn restore_cursor(&mut self) {
        let (x, y, pen) = self.saved;
        self.cx = x.min(self.cols - 1);
        self.cy = y.min(self.rows - 1);
        self.pen = pen;
        self.wrap_pending = false;
    }

    fn reset(&mut self) {
        self.grid = vec![blank_row(self.cols, DEFAULT_BG); self.rows];
        self.other = vec![blank_row(self.cols, DEFAULT_BG); self.rows];
        self.alt = false;
        self.pen = BLANK;
        self.cx = 0;
        self.cy = 0;
        self.top = 0;
        self.bottom = self.rows - 1;
        self.autowrap = true;
        self.cursor_visible = true;
        self.app_cursor = false;
        self.bracketed = false;
        self.wrap_pending = false;
    }

    fn goto(&mut self, x: usize, y: usize) {
        self.cx = x.min(self.cols - 1);
        self.cy = y.min(self.rows - 1);
        self.wrap_pending = false;
    }

    fn blank_cell(&self) -> Cell {
        Cell { ch: ' ', fg: DEFAULT_FG, bg: self.pen.bg, attr: 0 }
    }

    fn blank_row(&self) -> Vec<Cell> {
        vec![self.blank_cell(); self.cols]
    }

    fn put(&mut self, ch: char) {
        let w = char_width(ch);
        if w == 0 {
            return;
        }
        if self.wrap_pending {
            if self.autowrap {
                self.cx = 0;
                self.linefeed();
            }
            self.wrap_pending = false;
        }
        if w == 2 && self.cx + 1 >= self.cols && self.autowrap {
            self.cx = 0;
            self.linefeed();
        }
        let cell = Cell { ch, fg: self.pen.fg, bg: self.pen.bg, attr: self.pen.attr };
        self.grid[self.cy][self.cx] = cell;
        if w == 2 && self.cx + 1 < self.cols {
            self.grid[self.cy][self.cx + 1] = Cell { ch: '\0', ..cell };
        }
        self.cx += w;
        if self.cx >= self.cols {
            self.cx = self.cols - 1;
            self.wrap_pending = true;
        }
    }

    fn linefeed(&mut self) {
        if self.cy == self.bottom {
            self.scroll_up(1);
        } else if self.cy < self.rows - 1 {
            self.cy += 1;
        }
    }

    fn reverse_index(&mut self) {
        if self.cy == self.top {
            self.scroll_down(1);
        } else if self.cy > 0 {
            self.cy -= 1;
        }
    }

    fn push_sb(&mut self, row: Vec<Cell>) {
        self.scrollback.push_back(row);
        if self.scrollback.len() > SB_MAX {
            self.scrollback.pop_front();
            if let Some(a) = self.sel_a.as_mut() {
                a.0 = a.0.saturating_sub(1);
            }
            self.sel_b.0 = self.sel_b.0.saturating_sub(1);
        }
        if self.view_offset > 0 {
            self.view_offset = (self.view_offset + 1).min(self.scrollback.len());
        }
    }

    fn scroll_up(&mut self, n: usize) {
        let n = n.min(self.bottom - self.top + 1);
        for _ in 0..n {
            let row = self.grid.remove(self.top);
            if !self.alt && self.top == 0 {
                self.push_sb(row);
            }
            let blank = self.blank_row();
            self.grid.insert(self.bottom, blank);
        }
    }

    fn scroll_down(&mut self, n: usize) {
        let n = n.min(self.bottom - self.top + 1);
        for _ in 0..n {
            self.grid.remove(self.bottom);
            let blank = self.blank_row();
            self.grid.insert(self.top, blank);
        }
    }

    fn erase(&mut self, row: usize, c0: usize, c1: usize) {
        let blank = self.blank_cell();
        for c in c0..c1.min(self.cols) {
            self.grid[row][c] = blank;
        }
    }

    fn set_alt(&mut self, on: bool) {
        if on == self.alt {
            return;
        }
        std::mem::swap(&mut self.grid, &mut self.other);
        self.alt = on;
        self.view_offset = 0;
        self.sel_a = None;
        self.top = 0;
        self.bottom = self.rows - 1;
        if on {
            self.grid = vec![blank_row(self.cols, DEFAULT_BG); self.rows];
        }
    }

    fn set_mode(&mut self, v: i64, on: bool) {
        match v {
            1 => self.app_cursor = on,
            7 => self.autowrap = on,
            25 => self.cursor_visible = on,
            47 | 1047 => self.set_alt(on),
            1049 => {
                if on {
                    self.saved = (self.cx, self.cy, self.pen);
                    self.set_alt(true);
                } else {
                    self.set_alt(false);
                    self.restore_cursor();
                }
            }
            2004 => self.bracketed = on,
            _ => {}
        }
    }

    fn sgr(&mut self, params: &[i64]) {
        if params.is_empty() {
            self.pen = BLANK;
            return;
        }
        let mut i = 0;
        while i < params.len() {
            let v = params[i].max(0);
            match v {
                0 => self.pen = BLANK,
                1 => self.pen.attr |= BOLD,
                2 => self.pen.attr |= DIM,
                3 => self.pen.attr |= ITALIC,
                4 => self.pen.attr |= UNDERLINE,
                7 => self.pen.attr |= REVERSE,
                22 => self.pen.attr &= !(BOLD | DIM),
                23 => self.pen.attr &= !ITALIC,
                24 => self.pen.attr &= !UNDERLINE,
                27 => self.pen.attr &= !REVERSE,
                30..=37 => self.pen.fg = ANSI[(v - 30) as usize],
                39 => self.pen.fg = DEFAULT_FG,
                40..=47 => self.pen.bg = ANSI[(v - 40) as usize],
                49 => self.pen.bg = DEFAULT_BG,
                90..=97 => self.pen.fg = ANSI[(v - 90 + 8) as usize],
                100..=107 => self.pen.bg = ANSI[(v - 100 + 8) as usize],
                38 | 48 => {
                    let mut color = None;
                    match params.get(i + 1).copied() {
                        Some(5) => {
                            if let Some(&n) = params.get(i + 2) {
                                color = Some(palette256(n));
                                i += 2;
                            }
                        }
                        Some(2) => {
                            let mut j = i + 2;
                            if params.get(j) == Some(&-1) && params.len() >= i + 6 {
                                j += 1;
                            }
                            if j + 2 < params.len() {
                                let c = |k: usize| params[k].clamp(0, 255) as u32;
                                color = Some((c(j) << 16) | (c(j + 1) << 8) | c(j + 2));
                                i = j + 2;
                            }
                        }
                        _ => {}
                    }
                    if let Some(c) = color {
                        if v == 38 {
                            self.pen.fg = c;
                        } else {
                            self.pen.bg = c;
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn csi_dispatch(&mut self, fin: u8) {
        let buf = std::mem::take(&mut self.csi_buf);
        let inter = std::mem::take(&mut self.csi_inter);
        let (private, rest): (u8, &[u8]) = match buf.first() {
            Some(&c) if matches!(c, b'<' | b'=' | b'>' | b'?') => (c, &buf[1..]),
            _ => (0, &buf[..]),
        };
        let mut params: Vec<i64> = Vec::new();
        if !rest.is_empty() {
            for part in rest.split(|&c| c == b';' || c == b':') {
                params.push(if part.is_empty() {
                    -1
                } else {
                    std::str::from_utf8(part).ok().and_then(|s| s.parse().ok()).unwrap_or(-1)
                });
            }
        }
        let p = |i: usize, d: i64| -> usize {
            match params.get(i) {
                Some(&v) if v > 0 => v as usize,
                _ => d as usize,
            }
        };
        let raw0 = params.first().copied().unwrap_or(0).max(0);
        let (cols, rows) = (self.cols, self.rows);

        if !inter.is_empty() {
            return;
        }
        if private == b'?' {
            match fin {
                b'h' => params.iter().for_each(|&v| self.set_mode(v, true)),
                b'l' => params.iter().for_each(|&v| self.set_mode(v, false)),
                _ => {}
            }
            return;
        }
        if private == b'>' {
            if fin == b'c' {
                self.reply.extend_from_slice(b"\x1b[>0;10;1c");
            }
            return;
        }
        if private != 0 {
            return;
        }

        match fin {
            b'A' => {
                let n = p(0, 1);
                let top = if self.cy >= self.top { self.top } else { 0 };
                self.goto(self.cx, self.cy.saturating_sub(n).max(top));
            }
            b'B' | b'e' => {
                let n = p(0, 1);
                let bot = if self.cy <= self.bottom { self.bottom } else { rows - 1 };
                self.goto(self.cx, (self.cy + n).min(bot));
            }
            b'C' | b'a' => self.goto(self.cx + p(0, 1), self.cy),
            b'D' => self.goto(self.cx.saturating_sub(p(0, 1)), self.cy),
            b'E' => self.goto(0, (self.cy + p(0, 1)).min(rows - 1)),
            b'F' => self.goto(0, self.cy.saturating_sub(p(0, 1))),
            b'G' | b'`' => self.goto(p(0, 1) - 1, self.cy),
            b'H' | b'f' => self.goto(p(1, 1) - 1, p(0, 1) - 1),
            b'd' => self.goto(self.cx, p(0, 1) - 1),
            b'J' => {
                let (cx, cy) = (self.cx, self.cy);
                match raw0 {
                    0 => {
                        self.erase(cy, cx, cols);
                        for r in cy + 1..rows {
                            self.erase(r, 0, cols);
                        }
                    }
                    1 => {
                        for r in 0..cy {
                            self.erase(r, 0, cols);
                        }
                        self.erase(cy, 0, cx + 1);
                    }
                    2 => {
                        for r in 0..rows {
                            self.erase(r, 0, cols);
                        }
                    }
                    3 => {
                        self.scrollback.clear();
                        self.view_offset = 0;
                        self.sel_a = None;
                    }
                    _ => {}
                }
            }
            b'K' => {
                let (cx, cy) = (self.cx, self.cy);
                match raw0 {
                    0 => self.erase(cy, cx, cols),
                    1 => self.erase(cy, 0, cx + 1),
                    2 => self.erase(cy, 0, cols),
                    _ => {}
                }
            }
            b'L' => {
                if self.cy >= self.top && self.cy <= self.bottom {
                    let n = p(0, 1).min(self.bottom - self.cy + 1);
                    for _ in 0..n {
                        self.grid.remove(self.bottom);
                        let blank = self.blank_row();
                        self.grid.insert(self.cy, blank);
                    }
                    self.cx = 0;
                    self.wrap_pending = false;
                }
            }
            b'M' => {
                if self.cy >= self.top && self.cy <= self.bottom {
                    let n = p(0, 1).min(self.bottom - self.cy + 1);
                    for _ in 0..n {
                        self.grid.remove(self.cy);
                        let blank = self.blank_row();
                        self.grid.insert(self.bottom, blank);
                    }
                    self.cx = 0;
                    self.wrap_pending = false;
                }
            }
            b'P' => {
                let n = p(0, 1).min(cols - self.cx);
                let blank = self.blank_cell();
                let row = &mut self.grid[self.cy];
                row.drain(self.cx..self.cx + n);
                row.extend(std::iter::repeat(blank).take(n));
            }
            b'@' => {
                let n = p(0, 1).min(cols - self.cx);
                let blank = self.blank_cell();
                let row = &mut self.grid[self.cy];
                for _ in 0..n {
                    row.insert(self.cx, blank);
                }
                row.truncate(cols);
            }
            b'X' => {
                let n = p(0, 1);
                self.erase(self.cy, self.cx, self.cx + n);
            }
            b'S' => self.scroll_up(p(0, 1)),
            b'T' => self.scroll_down(p(0, 1)),
            b'm' => self.sgr(&params),
            b'r' => {
                let t = p(0, 1) - 1;
                let b = p(1, rows as i64) - 1;
                if t < b && b < rows {
                    self.top = t;
                    self.bottom = b;
                    self.goto(0, 0);
                }
            }
            b's' => self.saved = (self.cx, self.cy, self.pen),
            b'u' => self.restore_cursor(),
            b'n' => match raw0 {
                5 => self.reply.extend_from_slice(b"\x1b[0n"),
                6 => {
                    let s = format!("\x1b[{};{}R", self.cy + 1, self.cx + 1);
                    self.reply.extend_from_slice(s.as_bytes());
                }
                _ => {}
            },
            b'c' => self.reply.extend_from_slice(b"\x1b[?1;2c"),
            _ => {}
        }
    }
}
