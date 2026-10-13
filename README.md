# branna

Dark text editor with an embedded terminal. Rust `std` only (no crates); talks to the
system's libX11 and libXft directly. Needs an X11 session or XWayland (default on Ubuntu).

## Build
    cargo build --release        # needs only rustc/cargo; libx11-6 and libxft2 are normally preinstalled
    ./target/release/branna [file|folder]     # `branna .` opens the folder with the file tree
If linking fails: `sudo apt install libxft2 libx11-6`.

Font: `BRANNA_FONT="JetBrains Mono" BRANNA_FONT_PX=18 ./branna` (defaults: monospace, 17px).

## Keys
Zoom: Ctrl++  Ctrl+-  Ctrl+0 (also Ctrl+wheel)
Terminal (hidden by default): Ctrl+` shows+focuses it, Ctrl+` again hides it (F12 also toggles); from the editor Ctrl+` just focuses it. Drag the divider to resize.
File tree (netrw-style): Ctrl+B or Ctrl+E shows/focuses/hides it (Ctrl+B/E again from the tree hides it)
  j/k or arrows move, gg / G top / bottom, CR (or l/o/v/t, double-click) open file or expand/collapse folder,
  - parent dir, gn make folder the tree root, u / U back / forward in root history, gh or a toggle dot files,
  s cycle sort (name/time/size), r reverse, i long listing (sizes), Ctrl+L refresh, qf file info, x xdg-open
  % new file, d new folder, R rename, D or Delete delete (empty folders only)
  marks: mf mark file, mu clear marks, mt set target folder, mc copy marked there, mm move marked there, mD delete marked
  c re-sync the terminal to the tree root, Esc/Tab back to the editor
Terminal follows the tree: whenever the root changes the shell gets `cd` (when it is idle at a prompt;
  otherwise as soon as it becomes idle). Restarted shells start in the root as well.
Tabs: every opened file gets a tab (opening a file that is already open just switches to it).
  Ctrl+Tab / Ctrl+PgDn next, Ctrl+Shift+Tab / Ctrl+PgUp previous, Alt+1..9 jump to tab, Ctrl+N new tab,
  Ctrl+W close tab (asks to repeat if unsaved). Mouse: click a tab to switch, click its x / dot or middle-click to close.
  Ctrl+Q refuses to quit while any tab has unsaved changes (repeat to discard).
File: Ctrl+S save, Ctrl+Shift+S save as, Ctrl+O open (Tab completes paths), Ctrl+N new, Ctrl+Q quit
Edit: Ctrl+Z / Ctrl+Y (Ctrl+Shift+Z) undo/redo, Ctrl+X/C/V, Ctrl+A, Ctrl+D duplicate line,
      Ctrl+Shift+K delete line, Ctrl+L select line, Alt+Up/Down move lines, Tab / Shift+Tab indent
Move: arrows, Home/End, Ctrl+arrows (words), Ctrl+Home/End, PgUp/PgDn, Shift extends selection
Search: Ctrl+F (Enter/Shift+Enter, F3), Ctrl+H replace all, Ctrl+G go to line
Terminal: Ctrl+Shift+C / Ctrl+Shift+V, middle-click pastes, Shift+PgUp/PgDn scrollback, mouse wheel
