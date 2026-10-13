use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Clone, Copy, PartialEq)]
pub enum SortBy {
    Name,
    Time,
    Size,
}

pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
    pub is_parent: bool,
    pub size: u64,
    pub mtime: u64,
}

pub struct Tree {
    pub root: PathBuf,
    pub items: Vec<Node>,
    pub sel: usize,
    pub scroll: usize,
    pub show_hidden: bool,
    pub sort: SortBy,
    pub reverse: bool,
    pub long: bool,
    pub marks: Vec<PathBuf>,
    pub target: Option<PathBuf>,
    hist: Vec<PathBuf>,
    hist_i: usize,
    expanded: HashSet<PathBuf>,
}

impl Tree {
    pub fn new(root: PathBuf) -> Tree {
        let mut t = Tree {
            hist: vec![root.clone()],
            hist_i: 0,
            root,
            items: Vec::new(),
            sel: 0,
            scroll: 0,
            show_hidden: false,
            sort: SortBy::Name,
            reverse: false,
            long: false,
            marks: Vec::new(),
            target: None,
            expanded: HashSet::new(),
        };
        t.rebuild();
        t
    }

    pub fn rebuild(&mut self) {
        let keep = self.items.get(self.sel).filter(|n| !n.is_parent).map(|n| n.path.clone());
        self.items.clear();
        if let Some(p) = self.root.parent() {
            self.items.push(Node {
                name: "..".into(),
                path: p.to_path_buf(),
                is_dir: true,
                depth: 0,
                expanded: false,
                is_parent: true,
                size: 0,
                mtime: 0,
            });
        }
        let root = self.root.clone();
        self.walk(&root, 0);
        self.sel = match keep.and_then(|k| self.items.iter().position(|n| !n.is_parent && n.path == k)) {
            Some(i) => i,
            None => self.sel.min(self.items.len().saturating_sub(1)),
        };
    }

    fn walk(&mut self, dir: &Path, depth: usize) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        let hidden = self.show_hidden;
        let mut entries: Vec<Node> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if !hidden && name.starts_with('.') {
                    return None;
                }
                let path = e.path();
                let meta = fs::metadata(&path).ok();
                let is_dir = meta.as_ref().map_or(false, |m| m.is_dir());
                let size = meta.as_ref().map_or(0, |m| m.len());
                let mtime = meta
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                Some(Node { name, path, is_dir, depth, expanded: false, is_parent: false, size, mtime })
            })
            .collect();
        let (sort, rev) = (self.sort, self.reverse);
        entries.sort_by(|a, b| {
            let ord = match sort {
                SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortBy::Time => b.mtime.cmp(&a.mtime),
                SortBy::Size => b.size.cmp(&a.size),
            };
            let ord = if rev { ord.reverse() } else { ord };
            b.is_dir.cmp(&a.is_dir).then(ord)
        });
        for mut n in entries {
            n.expanded = n.is_dir && self.expanded.contains(&n.path);
            let (expanded, path) = (n.expanded, n.path.clone());
            self.items.push(n);
            if expanded {
                self.walk(&path, depth + 1);
            }
        }
    }

    pub fn current(&self) -> Option<&Node> {
        self.items.get(self.sel)
    }

    pub fn move_sel(&mut self, d: isize) {
        if self.items.is_empty() {
            return;
        }
        self.sel = (self.sel as isize + d).clamp(0, self.items.len() as isize - 1) as usize;
    }

    pub fn set_expanded(&mut self, i: usize, on: bool) {
        if let Some(n) = self.items.get(i) {
            if n.is_dir && !n.is_parent {
                let p = n.path.clone();
                if on {
                    self.expanded.insert(p);
                } else {
                    self.expanded.remove(&p);
                }
                self.sel = i;
                self.rebuild();
            }
        }
    }

    pub fn collapse_or_parent(&mut self) {
        let Some(n) = self.items.get(self.sel) else { return };
        if n.is_dir && n.expanded {
            self.set_expanded(self.sel, false);
            return;
        }
        if n.depth == 0 {
            return;
        }
        if let Some(w) = n.path.parent().map(|p| p.to_path_buf()) {
            if let Some(i) = self.items.iter().position(|x| !x.is_parent && x.path == w) {
                self.sel = i;
            }
        }
    }

    fn change_root(&mut self, p: PathBuf, record: bool) {
        if record {
            self.hist.truncate(self.hist_i + 1);
            self.hist.push(p.clone());
            self.hist_i = self.hist.len() - 1;
        }
        self.root = p;
        self.items.clear();
        self.sel = 0;
        self.scroll = 0;
        self.rebuild();
        if self.items.len() > 1 && self.items[0].is_parent {
            self.sel = 1;
        }
    }

    pub fn go_up(&mut self) {
        if let Some(p) = self.root.parent().map(|p| p.to_path_buf()) {
            let old = self.root.clone();
            self.change_root(p, true);
            self.select_path(&old);
        }
    }

    pub fn set_root(&mut self, p: PathBuf) {
        self.change_root(p, true);
    }

    pub fn history(&mut self, back: bool) -> bool {
        let ni = if back {
            if self.hist_i == 0 {
                return false;
            }
            self.hist_i - 1
        } else {
            if self.hist_i + 1 >= self.hist.len() {
                return false;
            }
            self.hist_i + 1
        };
        self.hist_i = ni;
        let p = self.hist[ni].clone();
        self.change_root(p, false);
        true
    }

    pub fn target_dir(&self) -> PathBuf {
        match self.current() {
            Some(n) if n.is_dir && !n.is_parent => n.path.clone(),
            Some(n) if !n.is_parent => n.path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| self.root.clone()),
            _ => self.root.clone(),
        }
    }

    pub fn select_path(&mut self, p: &Path) {
        if let Some(i) = self.items.iter().position(|n| !n.is_parent && n.path == p) {
            self.sel = i;
        }
    }

    pub fn ensure_visible(&mut self, rows: usize) {
        let rows = rows.max(1);
        if self.sel < self.scroll {
            self.scroll = self.sel;
        } else if self.sel >= self.scroll + rows {
            self.scroll = self.sel + 1 - rows;
        }
        self.scroll = self.scroll.min(self.items.len().saturating_sub(1));
    }

    pub fn reveal(&mut self, p: &Path) {
        let mut cur = p.parent();
        while let Some(d) = cur {
            if d == self.root || !d.starts_with(&self.root) {
                break;
            }
            self.expanded.insert(d.to_path_buf());
            cur = d.parent();
        }
        self.rebuild();
        self.select_path(p);
    }


    pub fn cycle_sort(&mut self) {
        self.sort = match self.sort {
            SortBy::Name => SortBy::Time,
            SortBy::Time => SortBy::Size,
            SortBy::Size => SortBy::Name,
        };
        self.rebuild();
    }

    pub fn toggle_reverse(&mut self) {
        self.reverse = !self.reverse;
        self.rebuild();
    }

    pub fn toggle_mark(&mut self) {
        if let Some(n) = self.items.get(self.sel) {
            if n.is_parent {
                return;
            }
            let p = n.path.clone();
            if let Some(i) = self.marks.iter().position(|m| *m == p) {
                self.marks.remove(i);
            } else {
                self.marks.push(p);
            }
        }
    }

    pub fn is_marked(&self, p: &Path) -> bool {
        self.marks.iter().any(|m| m == p)
    }
}

pub fn copy_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)?;
        for e in fs::read_dir(src)? {
            let e = e?;
            copy_recursive(&e.path(), &dst.join(e.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(src, dst).map(|_| ())
    }
}

pub fn human_size(n: u64) -> String {
    const U: [&str; 4] = ["K", "M", "G", "T"];
    if n < 1024 {
        return format!("{}B", n);
    }
    let mut v = n as f64 / 1024.0;
    let mut i = 0;
    while v >= 1024.0 && i < 3 {
        v /= 1024.0;
        i += 1;
    }
    format!("{:.1}{}", v, U[i])
}
