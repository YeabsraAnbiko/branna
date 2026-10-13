use std::fs;
use std::path::PathBuf;

pub const TAB: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

pub fn pos(line: usize, col: usize) -> Pos {
    Pos { line, col }
}

struct Edit {
    start: Pos,
    removed: String,
    inserted: String,
    before: (Pos, Option<Pos>),
    after: Pos,
    group: u64,
}

pub fn end_pos(start: Pos, s: &str) -> Pos {
    let mut p = start;
    for c in s.chars() {
        if c == '\n' {
            p.line += 1;
            p.col = 0;
        } else {
            p.col += 1;
        }
    }
    p
}

pub fn visual_col(line: &[char], col: usize) -> usize {
    let mut v = 0;
    for &c in line.iter().take(col) {
        v += if c == '\t' { TAB - v % TAB } else { 1 };
    }
    v
}

pub fn col_from_visual(line: &[char], target: usize) -> usize {
    let mut acc = 0;
    for (i, &c) in line.iter().enumerate() {
        let w = if c == '\t' { TAB - acc % TAB } else { 1 };
        if acc + w > target {
            return if target - acc > w / 2 { i + 1 } else { i };
        }
        acc += w;
    }
    line.len()
}

pub fn expand_tabs(line: &[char]) -> Vec<char> {
    let mut out = Vec::with_capacity(line.len());
    for &c in line {
        if c == '\t' {
            let n = TAB - out.len() % TAB;
            for _ in 0..n {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn cls(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

fn eq_ci(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

pub fn smart_ci(term: &str) -> bool {
    !term.chars().any(|c| c.is_uppercase())
}

pub fn find_in_line(line: &[char], t: &[char], start: usize, ci: bool) -> Option<usize> {
    if t.is_empty() || line.len() < t.len() {
        return None;
    }
    let last = line.len() - t.len();
    (start..=last).find(|&i| {
        t.iter().enumerate().all(|(k, &tc)| if ci { eq_ci(line[i + k], tc) } else { line[i + k] == tc })
    })
}

fn rfind_in_line(line: &[char], t: &[char], limit: usize, ci: bool) -> Option<usize> {
    if t.is_empty() || limit < t.len() || line.len() < t.len() {
        return None;
    }
    let max_i = (limit - t.len()).min(line.len() - t.len());
    (0..=max_i).rev().find(|&i| {
        t.iter().enumerate().all(|(k, &tc)| if ci { eq_ci(line[i + k], tc) } else { line[i + k] == tc })
    })
}

pub struct Editor {
    pub lines: Vec<Vec<char>>,
    pub cur: Pos,
    pub anchor: Option<Pos>,
    pub want_vcol: Option<usize>,
    pub path: Option<PathBuf>,
    pub modified: bool,
    pub crlf: bool,
    pub scroll_y: usize,
    pub scroll_x: usize,
    pub follow: bool,
    pub find: String,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    break_merge: bool,
    next_group: u64,
    open_group: Option<u64>,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            lines: vec![Vec::new()],
            cur: Pos::default(),
            anchor: None,
            want_vcol: None,
            path: None,
            modified: false,
            crlf: false,
            scroll_y: 0,
            scroll_x: 0,
            follow: true,
            find: String::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            break_merge: false,
            next_group: 0,
            open_group: None,
        }
    }

    pub fn load(path: PathBuf) -> Result<Editor, String> {
        let mut ed = Editor::new();
        match fs::read(&path) {
            Ok(bytes) => {
                let s = String::from_utf8(bytes).map_err(|_| "file is not valid UTF-8".to_string())?;
                ed.crlf = s.contains("\r\n");
                let s = if ed.crlf { s.replace("\r\n", "\n") } else { s };
                ed.lines = s.split('\n').map(|l| l.chars().collect()).collect();
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        ed.path = Some(path);
        Ok(ed)
    }

    pub fn save(&mut self) -> Result<(), String> {
        let path = self.path.clone().ok_or("no file name")?;
        let sep = if self.crlf { "\r\n" } else { "\n" };
        let mut out = String::new();
        for (i, l) in self.lines.iter().enumerate() {
            if i > 0 {
                out.push_str(sep);
            }
            out.extend(l.iter());
        }
        fs::write(&path, out).map_err(|e| e.to_string())?;
        self.modified = false;
        Ok(())
    }

    pub fn display_name(&self) -> String {
        match &self.path {
            Some(p) => p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string()),
            None => "untitled".to_string(),
        }
    }

    pub fn text_between(&self, a: Pos, b: Pos) -> String {
        let mut s = String::new();
        if a.line == b.line {
            s.extend(self.lines[a.line][a.col..b.col].iter());
            return s;
        }
        s.extend(self.lines[a.line][a.col..].iter());
        for l in a.line + 1..b.line {
            s.push('\n');
            s.extend(self.lines[l].iter());
        }
        s.push('\n');
        s.extend(self.lines[b.line][..b.col].iter());
        s
    }

    pub fn selection(&self) -> Option<(Pos, Pos)> {
        let a = self.anchor?;
        if a == self.cur {
            None
        } else if a < self.cur {
            Some((a, self.cur))
        } else {
            Some((self.cur, a))
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selection().map(|(a, b)| self.text_between(a, b))
    }

    pub fn line_text(&self, l: usize) -> String {
        self.lines[l].iter().collect()
    }

    pub fn doc_end(&self) -> Pos {
        let l = self.lines.len() - 1;
        pos(l, self.lines[l].len())
    }

    fn raw_replace(&mut self, a: Pos, b: Pos, text: &str) {
        let mut head: Option<Vec<char>> = Some(self.lines[a.line][..a.col].to_vec());
        let tail: Vec<char> = self.lines[b.line][b.col..].to_vec();
        let parts: Vec<Vec<char>> = text.split('\n').map(|s| s.chars().collect()).collect();
        let n = parts.len();
        let mut new_lines = Vec::with_capacity(n);
        for (i, mut p) in parts.into_iter().enumerate() {
            if i == 0 {
                let mut h = head.take().unwrap();
                h.extend(p);
                p = h;
            }
            if i == n - 1 {
                p.extend(tail.iter());
            }
            new_lines.push(p);
        }
        self.lines.splice(a.line..=b.line, new_lines);
    }

    fn group_id(&mut self) -> u64 {
        if let Some(g) = self.open_group {
            g
        } else {
            self.next_group += 1;
            self.next_group
        }
    }

    fn begin_group(&mut self) {
        self.next_group += 1;
        self.open_group = Some(self.next_group);
    }

    fn end_group(&mut self) {
        self.open_group = None;
        self.break_merge = true;
    }

    pub fn replace_range(&mut self, a: Pos, b: Pos, text: &str) {
        if a == b && text.is_empty() {
            return;
        }
        let removed = self.text_between(a, b);
        let before = (self.cur, self.anchor);
        self.raw_replace(a, b, text);
        let after = end_pos(a, text);
        self.cur = after;
        self.anchor = None;
        self.want_vcol = None;
        self.follow = true;
        self.modified = true;
        let group = self.group_id();
        self.push_undo(Edit { start: a, removed, inserted: text.to_string(), before, after, group });
    }

    fn push_undo(&mut self, e: Edit) {
        self.redo.clear();
        if !self.break_merge && self.open_group.is_none() {
            if let Some(l) = self.undo.last_mut() {
                let simple = |s: &str| !s.contains('\n');
                if e.removed.is_empty()
                    && l.removed.is_empty()
                    && simple(&e.inserted)
                    && simple(&l.inserted)
                    && !l.inserted.is_empty()
                    && end_pos(l.start, &l.inserted) == e.start
                {
                    let ws_new = e.inserted.chars().next().map_or(false, |c| c.is_whitespace());
                    let ws_old = l.inserted.chars().last().map_or(true, |c| c.is_whitespace());
                    if !(ws_new && !ws_old) {
                        l.inserted.push_str(&e.inserted);
                        l.after = e.after;
                        return;
                    }
                } else if e.inserted.is_empty()
                    && l.inserted.is_empty()
                    && simple(&e.removed)
                    && simple(&l.removed)
                    && !l.removed.is_empty()
                {
                    let e_end = pos(e.start.line, e.start.col + e.removed.chars().count());
                    if e_end == l.start {
                        l.start = e.start;
                        l.removed = format!("{}{}", e.removed, l.removed);
                        l.after = e.after;
                        return;
                    }
                    if e.start == l.start {
                        l.removed.push_str(&e.removed);
                        l.after = e.after;
                        return;
                    }
                }
            }
        }
        self.break_merge = false;
        self.undo.push(e);
    }

    pub fn undo(&mut self) -> bool {
        let Some(last) = self.undo.pop() else { return false };
        let g = last.group;
        let mut items = vec![last];
        while self.undo.last().map_or(false, |e| e.group == g) {
            items.push(self.undo.pop().unwrap());
        }
        for e in items {
            let end = end_pos(e.start, &e.inserted);
            self.raw_replace(e.start, end, &e.removed);
            self.cur = e.before.0;
            self.anchor = e.before.1;
            self.redo.push(e);
        }
        self.after_history();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(first) = self.redo.pop() else { return false };
        let g = first.group;
        let mut items = vec![first];
        while self.redo.last().map_or(false, |e| e.group == g) {
            items.push(self.redo.pop().unwrap());
        }
        for e in items {
            let end = end_pos(e.start, &e.removed);
            self.raw_replace(e.start, end, &e.inserted);
            self.cur = e.after;
            self.anchor = None;
            self.undo.push(e);
        }
        self.after_history();
        true
    }

    fn after_history(&mut self) {
        self.modified = true;
        self.break_merge = true;
        self.want_vcol = None;
        self.follow = true;
        let l = self.cur.line.min(self.lines.len() - 1);
        self.cur = pos(l, self.cur.col.min(self.lines[l].len()));
        if let Some(a) = self.anchor {
            let al = a.line.min(self.lines.len() - 1);
            self.anchor = Some(pos(al, a.col.min(self.lines[al].len())));
        }
    }

    fn begin_move(&mut self, extend: bool) {
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(self.cur);
            }
        } else {
            self.anchor = None;
        }
        self.break_merge = true;
        self.follow = true;
    }

    pub fn step_left(&self, p: Pos) -> Pos {
        if p.col > 0 {
            pos(p.line, p.col - 1)
        } else if p.line > 0 {
            pos(p.line - 1, self.lines[p.line - 1].len())
        } else {
            p
        }
    }

    pub fn step_right(&self, p: Pos) -> Pos {
        if p.col < self.lines[p.line].len() {
            pos(p.line, p.col + 1)
        } else if p.line + 1 < self.lines.len() {
            pos(p.line + 1, 0)
        } else {
            p
        }
    }

    pub fn next_word(&self, p: Pos) -> Pos {
        let line = &self.lines[p.line];
        if p.col >= line.len() {
            return if p.line + 1 < self.lines.len() { pos(p.line + 1, 0) } else { p };
        }
        let mut i = p.col;
        while i < line.len() && line[i].is_whitespace() {
            i += 1;
        }
        if i < line.len() {
            let k = cls(line[i]);
            while i < line.len() && cls(line[i]) == k {
                i += 1;
            }
        }
        pos(p.line, i)
    }

    pub fn prev_word(&self, p: Pos) -> Pos {
        if p.col == 0 {
            return if p.line > 0 { pos(p.line - 1, self.lines[p.line - 1].len()) } else { p };
        }
        let line = &self.lines[p.line];
        let mut i = p.col;
        while i > 0 && line[i - 1].is_whitespace() {
            i -= 1;
        }
        if i > 0 {
            let k = cls(line[i - 1]);
            while i > 0 && cls(line[i - 1]) == k {
                i -= 1;
            }
        }
        pos(p.line, i)
    }

    pub fn move_left(&mut self, extend: bool, word: bool) {
        if !extend && !word {
            if let Some((a, _)) = self.selection() {
                self.cur = a;
                self.anchor = None;
                self.want_vcol = None;
                self.follow = true;
                self.break_merge = true;
                return;
            }
        }
        self.begin_move(extend);
        self.want_vcol = None;
        self.cur = if word { self.prev_word(self.cur) } else { self.step_left(self.cur) };
    }

    pub fn move_right(&mut self, extend: bool, word: bool) {
        if !extend && !word {
            if let Some((_, b)) = self.selection() {
                self.cur = b;
                self.anchor = None;
                self.want_vcol = None;
                self.follow = true;
                self.break_merge = true;
                return;
            }
        }
        self.begin_move(extend);
        self.want_vcol = None;
        self.cur = if word { self.next_word(self.cur) } else { self.step_right(self.cur) };
    }

    pub fn move_vert(&mut self, dy: isize, extend: bool) {
        let vc = self.want_vcol.unwrap_or_else(|| visual_col(&self.lines[self.cur.line], self.cur.col));
        self.begin_move(extend);
        let n = self.lines.len() as isize;
        let target = self.cur.line as isize + dy;
        if target < 0 {
            self.cur = pos(0, 0);
        } else if target >= n {
            self.cur = self.doc_end();
        } else {
            let l = target as usize;
            self.cur = pos(l, col_from_visual(&self.lines[l], vc));
        }
        self.want_vcol = Some(vc);
    }

    pub fn home(&mut self, extend: bool) {
        self.begin_move(extend);
        self.want_vcol = None;
        let line = &self.lines[self.cur.line];
        let fnw = line.iter().position(|c| !c.is_whitespace()).unwrap_or(line.len());
        let col = if self.cur.col == fnw { 0 } else { fnw };
        self.cur.col = col;
    }

    pub fn end(&mut self, extend: bool) {
        self.begin_move(extend);
        self.want_vcol = None;
        self.cur.col = self.lines[self.cur.line].len();
    }

    pub fn doc_start(&mut self, extend: bool) {
        self.begin_move(extend);
        self.want_vcol = None;
        self.cur = pos(0, 0);
    }

    pub fn doc_finish(&mut self, extend: bool) {
        self.begin_move(extend);
        self.want_vcol = None;
        self.cur = self.doc_end();
    }

    pub fn select_all(&mut self) {
        self.anchor = Some(pos(0, 0));
        self.cur = self.doc_end();
        self.break_merge = true;
    }

    pub fn select_range(&mut self, a: Pos, b: Pos) {
        self.anchor = Some(a);
        self.cur = b;
        self.follow = true;
        self.want_vcol = None;
        self.break_merge = true;
    }

    pub fn select_line(&mut self, l: usize) {
        self.anchor = Some(pos(l, 0));
        self.cur = if l + 1 < self.lines.len() { pos(l + 1, 0) } else { pos(l, self.lines[l].len()) };
        self.break_merge = true;
    }

    pub fn select_word_at(&mut self, p: Pos) {
        let line = &self.lines[p.line];
        if line.is_empty() {
            self.cur = p;
            self.anchor = None;
            return;
        }
        let i = p.col.min(line.len() - 1);
        let k = cls(line[i]);
        let mut s = i;
        while s > 0 && cls(line[s - 1]) == k {
            s -= 1;
        }
        let mut e = i + 1;
        while e < line.len() && cls(line[e]) == k {
            e += 1;
        }
        self.anchor = Some(pos(p.line, s));
        self.cur = pos(p.line, e);
        self.break_merge = true;
    }

    pub fn insert_str(&mut self, s: &str) {
        let (a, b) = self.selection().unwrap_or((self.cur, self.cur));
        self.replace_range(a, b, s);
    }

    pub fn newline(&mut self) {
        let (a, b) = self.selection().unwrap_or((self.cur, self.cur));
        let line = &self.lines[a.line];
        let mut ins = String::from("\n");
        for &c in line.iter().take(a.col) {
            if c == ' ' || c == '\t' {
                ins.push(c);
            } else {
                break;
            }
        }
        if a.col > 0 && matches!(line[a.col - 1], '{' | '(' | '[') {
            ins.push_str("    ");
        }
        self.replace_range(a, b, &ins);
    }

    pub fn backspace(&mut self) {
        if let Some((a, b)) = self.selection() {
            self.replace_range(a, b, "");
            return;
        }
        let prev = self.step_left(self.cur);
        self.replace_range(prev, self.cur, "");
    }

    pub fn delete(&mut self) {
        if let Some((a, b)) = self.selection() {
            self.replace_range(a, b, "");
            return;
        }
        let next = self.step_right(self.cur);
        self.replace_range(self.cur, next, "");
    }

    pub fn delete_word_back(&mut self) {
        if let Some((a, b)) = self.selection() {
            self.replace_range(a, b, "");
            return;
        }
        let p = self.prev_word(self.cur);
        self.replace_range(p, self.cur, "");
    }

    pub fn delete_word_fwd(&mut self) {
        if let Some((a, b)) = self.selection() {
            self.replace_range(a, b, "");
            return;
        }
        let p = self.next_word(self.cur);
        self.replace_range(self.cur, p, "");
    }

    pub fn delete_selection(&mut self) {
        if let Some((a, b)) = self.selection() {
            self.replace_range(a, b, "");
        }
    }

    fn sel_lines(&self) -> (usize, usize) {
        match self.selection() {
            Some((a, b)) => (a.line, if b.col == 0 && b.line > a.line { b.line - 1 } else { b.line }),
            None => (self.cur.line, self.cur.line),
        }
    }

    pub fn tab(&mut self) {
        if let Some((a, b)) = self.selection() {
            if a.line != b.line {
                let (l0, l1) = self.sel_lines();
                self.indent_lines(l0, l1, true);
                return;
            }
        }
        let (a, b) = self.selection().unwrap_or((self.cur, self.cur));
        let vc = visual_col(&self.lines[a.line], a.col);
        let n = TAB - vc % TAB;
        self.replace_range(a, b, &" ".repeat(n));
    }

    pub fn shift_tab(&mut self) {
        let (l0, l1) = self.sel_lines();
        self.indent_lines(l0, l1, false);
    }

    fn indent_lines(&mut self, l0: usize, l1: usize, add: bool) {
        let (c0, a0) = (self.cur, self.anchor);
        let mut deltas = vec![0isize; l1 - l0 + 1];
        self.begin_group();
        for l in l0..=l1 {
            if add {
                if !self.lines[l].is_empty() {
                    self.replace_range(pos(l, 0), pos(l, 0), "    ");
                    deltas[l - l0] = 4;
                }
            } else {
                let line = &self.lines[l];
                let n = if line.first() == Some(&'\t') {
                    1
                } else {
                    line.iter().take(TAB).take_while(|&&c| c == ' ').count()
                };
                if n > 0 {
                    self.replace_range(pos(l, 0), pos(l, n), "");
                    deltas[l - l0] = -(n as isize);
                }
            }
        }
        self.end_group();
        let adj = |p: Pos| -> Pos {
            if p.line >= l0 && p.line <= l1 {
                let d = deltas[p.line - l0];
                pos(p.line, (p.col as isize + d).max(0) as usize)
            } else {
                p
            }
        };
        self.cur = adj(c0);
        self.anchor = a0.map(adj);
        self.follow = true;
    }

    pub fn delete_line(&mut self) {
        let l = self.cur.line;
        let n = self.lines.len();
        let (a, b) = if l + 1 < n {
            (pos(l, 0), pos(l + 1, 0))
        } else if l > 0 {
            (pos(l - 1, self.lines[l - 1].len()), pos(l, self.lines[l].len()))
        } else {
            (pos(0, 0), pos(0, self.lines[0].len()))
        };
        self.replace_range(a, b, "");
    }

    pub fn duplicate_line(&mut self) {
        let l = self.cur.line;
        let text = self.line_text(l);
        let col = self.cur.col;
        let end = pos(l, self.lines[l].len());
        self.replace_range(end, end, &format!("\n{}", text));
        self.cur = pos(l + 1, col);
    }

    pub fn move_lines(&mut self, up: bool) {
        let (l0, l1) = self.sel_lines();
        let n = self.lines.len();
        let (c0, a0) = (self.cur, self.anchor);
        let block: Vec<String> = (l0..=l1).map(|l| self.line_text(l)).collect();
        let block = block.join("\n");
        let shift: isize;
        if up {
            if l0 == 0 {
                return;
            }
            let prev = self.line_text(l0 - 1);
            let end = pos(l1, self.lines[l1].len());
            self.replace_range(pos(l0 - 1, 0), end, &format!("{}\n{}", block, prev));
            shift = -1;
        } else {
            if l1 + 1 >= n {
                return;
            }
            let next = self.line_text(l1 + 1);
            let end = pos(l1 + 1, self.lines[l1 + 1].len());
            self.replace_range(pos(l0, 0), end, &format!("{}\n{}", next, block));
            shift = 1;
        }
        let mv = |p: Pos| pos((p.line as isize + shift) as usize, p.col);
        self.cur = mv(c0);
        self.anchor = a0.map(mv);
        self.follow = true;
    }

    pub fn find_from(&self, term: &str, from: Pos, forward: bool) -> Option<(Pos, Pos)> {
        if term.is_empty() {
            return None;
        }
        let t: Vec<char> = term.chars().collect();
        let ci = smart_ci(term);
        let n = self.lines.len();
        if forward {
            for k in 0..=n {
                let li = (from.line + k) % n;
                let start = if k == 0 { from.col } else { 0 };
                if let Some(c) = find_in_line(&self.lines[li], &t, start, ci) {
                    return Some((pos(li, c), pos(li, c + t.len())));
                }
            }
        } else {
            for k in 0..=n {
                let li = (from.line + n - k % n) % n;
                let limit = if k == 0 { from.col } else { self.lines[li].len() };
                if let Some(c) = rfind_in_line(&self.lines[li], &t, limit, ci) {
                    return Some((pos(li, c), pos(li, c + t.len())));
                }
            }
        }
        None
    }

    pub fn replace_all(&mut self, find: &str, with: &str) -> usize {
        if find.is_empty() {
            return 0;
        }
        let t: Vec<char> = find.chars().collect();
        let ci = smart_ci(find);
        let wl = with.chars().count();
        let mut count = 0;
        self.begin_group();
        for li in 0..self.lines.len() {
            let mut col = 0;
            while let Some(c) = find_in_line(&self.lines[li], &t, col, ci) {
                self.replace_range(pos(li, c), pos(li, c + t.len()), with);
                col = c + wl;
                count += 1;
            }
        }
        self.end_group();
        count
    }

    pub fn goto_line(&mut self, n: usize) {
        let l = n.saturating_sub(1).min(self.lines.len() - 1);
        self.anchor = None;
        self.cur = pos(l, 0);
        self.follow = true;
        self.break_merge = true;
    }

    pub fn ensure_visible(&mut self, rows: usize, vis_cols: usize) {
        if self.cur.line < self.scroll_y {
            self.scroll_y = self.cur.line;
        } else if self.cur.line >= self.scroll_y + rows {
            self.scroll_y = self.cur.line + 1 - rows;
        }
        let vc = visual_col(&self.lines[self.cur.line], self.cur.col);
        if vc < self.scroll_x {
            self.scroll_x = vc;
        } else if vc >= self.scroll_x + vis_cols {
            self.scroll_x = vc + 1 - vis_cols;
        }
    }

    pub fn clamp_scroll(&mut self) {
        self.scroll_y = self.scroll_y.min(self.lines.len() - 1);
    }
}
