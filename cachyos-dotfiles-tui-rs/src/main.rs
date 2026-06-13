use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io::{self, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DotfileEntry {
    path: String, enabled: bool, category: String, description: String,
    #[serde(default)] sudo: bool, #[serde(default, rename = "is_dir")] is_dir: bool,
}
fn home() -> PathBuf { PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())) }
fn manifest() -> PathBuf { home().join(".config/cachyos-dotfiles/manifest.json") }
fn config_file() -> PathBuf { home().join(".config/cachyos-dotfiles/config.json") }
fn cli_bin() -> PathBuf {
    if let Ok(e) = std::env::current_exe() { if let Some(d) = e.parent() { let c = d.join("cachyos-dotfiles"); if c.exists() { return c; } } }
    let p = home().join("CW-Projects/CachyOS.f/cachyos-dotfiles"); if p.exists() { return p; }
    PathBuf::from("./cachyos-dotfiles")
}
fn repo_dir() -> PathBuf { home().join(".local/share/cachyos-dotfiles/repo") }
fn load() -> Vec<DotfileEntry> { manifest().exists().then(|| fs::read_to_string(manifest()).ok()).flatten().and_then(|d| serde_json::from_str(&d).ok()).unwrap_or_default() }
fn save(e: &[DotfileEntry]) { let p = manifest(); let _ = std::fs::create_dir_all(p.parent().unwrap()); serde_json::to_string_pretty(e).ok().map(|j| fs::write(&p, j).ok()); }
fn cli_blocking(args: &[&str]) -> (bool, String) { match Command::new(cli_bin()).args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() { Ok(c) => { let o = c.wait_with_output().unwrap(); (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))) } Err(e) => (false, format!("CLI: {}", e)) } }
fn git_status() -> String { let d = repo_dir(); if !d.join(".git").exists() { return "no repo".into(); } match Command::new("git").args(["-C"]).arg(d).args(["status","--porcelain"]).output() { Ok(o) if o.status.success() => { let n = String::from_utf8_lossy(&o.stdout).lines().count(); if n == 0 { "clean".into() } else { format!("{}Δ", n) } } _ => "?".into() } }
fn file_preview(e: &DotfileEntry) -> Vec<Line<'static>> { let src = if e.path.starts_with("~/") { home().join(&e.path[2..]) } else { PathBuf::from(&e.path) }; if !src.exists() || e.is_dir { return vec![]; } let data = fs::read_to_string(&src).unwrap_or_default(); let max = 12; let lns: Vec<&str> = data.lines().take(max).collect(); let trunc = data.lines().count() > max; let mut out = vec![Line::from(Span::styled("── preview ──", Style::default().fg(DM)))]; for l in &lns { out.push(Line::from(Span::styled(format!(" {}", l), Style::default().fg(FG)))); } if trunc { out.push(Line::from(Span::styled(" ...", Style::default().fg(DM)))); } out }

// ── Wizard (untouched) ──────────────────────────────────────────────────
fn wizard() -> io::Result<bool> { println!("\x1b[1;36m═══ cachyos-dotfiles Setup Wizard ═══\x1b[0m\n"); print!("  Checking GitHub authentication... "); io::stdout().flush()?; let gh = Command::new("gh").args(["auth","status"]).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false); if gh { println!("\x1b[32m✓ Authenticated\x1b[0m"); } else { println!("\x1b[33m✗ Not authenticated\x1b[0m"); println!("\n  Run: \x1b[36mgh auth login\x1b[0m\n  (HTTPS — no SSH key needed)\n"); return Ok(false); } println!("\n  What would you like to do?\n"); println!("    \x1b[36m[b]\x1b[0m  Backup my dotfiles to GitHub"); println!("    \x1b[36m[r]\x1b[0m  Restore dotfiles from GitHub"); println!("    \x1b[36m[q]\x1b[0m  Quit (launch TUI)"); print!("\n  Choice: "); io::stdout().flush()?; match read_char()? { 'b'|'B' => wiz_backup()?, 'r'|'R' => wiz_restore()?, _ => { println!("  Launching TUI...\n"); return Ok(true); } } Ok(true) }
fn wiz_backup() -> io::Result<()> { println!("\n  Do you have a GitHub repo for dotfiles?\n    \x1b[36m[y]\x1b[0m Yes  \x1b[36m[n]\x1b[0m No — create one"); print!("  Choice: "); io::stdout().flush()?; let has = read_char()?; println!(); let url: String = if has == 'y' || has == 'Y' { print!("  Repo URL: "); io::stdout().flush()?; let mut u = String::new(); io::stdin().read_line(&mut u)?; u.trim().to_string() } else { let user = String::from_utf8_lossy(&Command::new("gh").args(["api","user","--jq",".login"]).output().map(|o| o.stdout).unwrap_or_default()).trim().to_string(); print!("  Repo name [cachyos-dotfiles]: "); io::stdout().flush()?; let mut nn = String::new(); io::stdin().read_line(&mut nn)?; let name = if nn.trim().is_empty() { "cachyos-dotfiles" } else { nn.trim() }; print!("  Private? [Y/n]: "); io::stdout().flush()?; let is_priv = read_char()? != 'n'; println!(); let full = format!("{}/{}", user, name); let mut args = vec!["repo","create",&full,"--description","CachyOS dotfiles backup"]; if is_priv { args.push("--private"); } else { args.push("--public"); } print!("  Creating {}... ", full); io::stdout().flush()?; if Command::new("gh").args(&args).status().map(|s| s.success()).unwrap_or(false) { println!("\x1b[32m✓ Created\x1b[0m"); format!("https://github.com/{}/{}.git", user, name) } else { println!("\x1b[33mFailed\x1b[0m"); print!("  Enter URL manually: "); io::stdout().flush()?; let mut u = String::new(); io::stdin().read_line(&mut u)?; u.trim().to_string() } }; if url.is_empty() { println!("  Aborting.\n"); return Ok(()); } println!("  Initializing with {}...", url); let (_, _) = cli_blocking(&["init","--repo",&url]); let e = load(); println!("\n  Loaded {} files ({} enabled)", e.len(), e.iter().filter(|x| x.enabled).count()); print!("  Run backup now? [Y/n]: "); io::stdout().flush()?; if read_char()?.to_ascii_lowercase() != 'n' { let (_, o) = cli_blocking(&["backup"]); println!("{}", o); } println!("\n  \x1b[36m═══ Done! ═══\x1b[0m\n"); Ok(()) }
fn wiz_restore() -> io::Result<()> { print!("  GitHub repo URL: "); io::stdout().flush()?; let mut url = String::new(); io::stdin().read_line(&mut url)?; let url = url.trim().to_string(); if url.is_empty() { println!("  Aborting.\n"); return Ok(()); } cli_blocking(&["init","--repo",&url]); print!("\n  Run restore now? [Y/n]: "); io::stdout().flush()?; if read_char()?.to_ascii_lowercase() != 'n' { let (_, dry) = cli_blocking(&["restore","--dry-run","--yes"]); println!("{}", dry.chars().take(600).collect::<String>()); print!("\n  Proceed? [y/N]: "); io::stdout().flush()?; if read_char()?.to_ascii_lowercase() == 'y' { let (_, o) = cli_blocking(&["restore","--yes"]); println!("{}", o); } } println!("\n  \x1b[36m═══ Done! ═══\x1b[0m\n"); Ok(()) }
fn read_char() -> io::Result<char> { let mut l = String::new(); io::stdin().read_line(&mut l)?; Ok(l.chars().next().unwrap_or('\n')) }

// ── App ───────────────────────────────────────────────────────────────

struct App {
    entries: Vec<DotfileEntry>, category: usize, filter: String, sel: usize, scroll: usize,
    mode: Mode, dialog: Option<DialogKind>, status: String, repo_status: String,
    list_state: ListState,
    // Async work
    rx: Option<mpsc::Receiver<(bool, String)>>, label: String,
    sp: usize, sp_tick: Instant, result: Option<(bool, String)>, deadline: Instant,
}
#[derive(PartialEq)] enum Mode { Normal, Filter, Help, Dialog, Working }
enum DialogKind { ConfirmRestore(String), Output(String, String) }
const SPINNER: &[&str] = &["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"];
const CATS: &[&str] = &["All","System","KDE","CachyOS","User"];
const MAP: &[&str] = &["all","system","kde","cachyos","user"];

impl App {
    fn new() -> Self { let mut s = ListState::default(); s.select(Some(0)); let gs = git_status(); Self { entries: load(), category: 0, filter: String::new(), sel: 0, scroll: 0, mode: Mode::Normal, dialog: None, status: format!("[{}]  Space:toggle  b:backup  r:restore  d:diff  h:help  q:quit", gs), repo_status: gs, list_state: s, rx: None, label: String::new(), sp: 0, sp_tick: Instant::now(), result: None, deadline: Instant::now() } }
    fn filtered(&self) -> Vec<usize> { let ft = self.filter.to_lowercase(); self.entries.iter().enumerate().filter(|(_, e)| (self.category == 0 || MAP.get(self.category) == Some(&e.category.as_str())) && (ft.is_empty() || e.path.to_lowercase().contains(&ft) || e.description.to_lowercase().contains(&ft))).map(|(i, _)| i).collect() }
    fn real_idx(&self) -> Option<usize> { self.filtered().get(self.sel).copied() }
    fn count(&self) -> usize { self.filtered().len() }
    fn sync(&mut self, vis: u16) { let n = self.count(); let v = vis.saturating_sub(2) as usize; if n == 0 || v == 0 { self.list_state.select(None); return; } if self.sel >= n { self.sel = n.saturating_sub(1); } if self.sel < self.scroll { self.scroll = self.sel; } else if self.sel >= self.scroll.saturating_add(v) { self.scroll = self.sel.saturating_sub(v.saturating_sub(1)); } if n > v { self.scroll = self.scroll.min(n.saturating_sub(v)); } else { self.scroll = 0; } self.list_state.select(Some((self.sel.saturating_sub(self.scroll)).min(v))); }
    fn toggle(&mut self) { if self.count() == 0 { return; } if let Some(i) = self.real_idx() { self.entries[i].enabled = !self.entries[i].enabled; self.status = format!("[{}]  {} {}", self.repo_status, if self.entries[i].enabled { "✓" } else { "✗" }, self.entries[i].path); save(&self.entries); } }
    fn set_status(&mut self, msg: &str) { self.status = format!("[{}]  {}", self.repo_status, msg); }
    fn toggle_category(&mut self, cat: usize) { let ena = !self.entries.iter().any(|e| e.category == MAP[cat] && e.enabled); for e in &mut self.entries { if e.category == MAP[cat] { e.enabled = ena; } } save(&self.entries); self.status = format!("[{}]  {} all {}", self.repo_status, if ena { "✓" } else { "✗" }, CATS[cat]); }
    fn update_repo(&mut self) { self.repo_status = git_status(); }
    fn launch(&mut self, label: &str, args: Vec<String>) {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || { let ca: Vec<&str> = args.iter().map(|s| s.as_str()).collect(); let r = cli_blocking(&ca); let _ = tx.send(r); });
        self.rx = Some(rx); self.label = label.to_string();
        self.mode = Mode::Working; self.sp = 0; self.sp_tick = Instant::now();
        self.result = None; self.deadline = Instant::now() + Duration::from_millis(500);
    }
    fn poll(&mut self) -> Option<(bool, String)> {
        if self.sp_tick.elapsed().as_millis() >= 80 { self.sp = (self.sp + 1) % SPINNER.len(); self.sp_tick = Instant::now(); }
        if let Some(ref rx) = self.rx { if let Ok(r) = rx.try_recv() { self.rx = None; self.result = Some(r); } }
        if Instant::now() >= self.deadline { return self.result.take(); }
        None
    }
}

// ── Styling ───────────────────────────────────────────────────────────

const BG: Color = Color::Rgb(40,42,54); const FG: Color = Color::Rgb(248,248,242); const CY: Color = Color::Rgb(139,233,253);
const GN: Color = Color::Rgb(80,250,123); const RD: Color = Color::Rgb(255,85,85); const YW: Color = Color::Rgb(241,250,140);
const PK: Color = Color::Rgb(255,121,198); const DM: Color = Color::Rgb(98,114,164); const HL: Color = Color::Rgb(68,71,90);

fn entry_item(e: &DotfileEntry) -> ListItem<'static> { ListItem::new(Line::from(Span::styled(format!("{} {}{}{}", if e.enabled { "✓" } else { "✗" }, e.path, if e.is_dir { "/" } else { "" }, if e.sudo { " [sudo]" } else { "" }), Style::default().fg(if !e.enabled { RD } else if e.sudo { YW } else { GN })))) }
fn detail_lines(e: &DotfileEntry) -> Vec<Line<'static>> { let p = e.path.clone(); let c = e.category.clone(); let d = e.description.clone(); let mut lines = vec![Line::from(Span::styled(format!("Path:       {}", p), Style::default().fg(CY))),Line::from(Span::styled(format!("Category:   {}", c), Style::default().fg(DM))),Line::from(Span::styled(format!("Status:     {}", if e.enabled { "✓ Enabled" } else { "✗ Disabled" }), Style::default().fg(GN))),Line::from(Span::styled(format!("Needs sudo: {}", if e.sudo { "Yes" } else { "No" }), Style::default().fg(YW))),Line::default(),Line::from(Span::styled(d, Style::default().fg(DM)))]; if p.contains(".ssh/") { lines.push(Line::default()); lines.push(Line::from(Span::styled("⚠ SECURITY", Style::default().fg(PK)))); } if p.contains("kwalletrc") { lines.push(Line::default()); lines.push(Line::from(Span::styled("⚠ Wallet config", Style::default().fg(YW)))); } if p.contains("kdeconnect") { lines.push(Line::default()); lines.push(Line::from(Span::styled("⚠ Device IDs", Style::default().fg(YW)))); } lines.push(Line::default()); lines.extend(file_preview(e)); lines }

// ── Render ────────────────────────────────────────────────────────────

fn ui(f: &mut Frame, a: &mut App) {
    let area = f.area();
    let [tab, body, stat] = Layout::vertical([Constraint::Length(2), Constraint::Min(3), Constraint::Length(2)]).areas(area);
    let tabs = Tabs::new(CATS.iter().copied().collect::<Vec<_>>()).select(a.category).style(Style::default().fg(DM)).highlight_style(Style::default().fg(CY).add_modifier(Modifier::BOLD)).divider(Span::raw(" │ "));
    let tb = if a.filter.is_empty() { Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(DM)) } else { Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(DM)).title(Span::styled(format!("🔍 {}", a.filter), Style::default().fg(YW))) };
    f.render_widget(tabs.block(tb), tab);
    let [left, right] = Layout::horizontal([Constraint::Length(52), Constraint::Min(20)]).areas(body);
    let idx = a.filtered(); let en = idx.iter().filter(|&&i| a.entries[i].enabled).count();
    a.sync(left.height);
    let vis = (left.height.saturating_sub(2) as usize).max(1);
    let vis_items: Vec<ListItem> = idx.iter().skip(a.scroll).take(vis).map(|&i| entry_item(&a.entries[i])).collect();
    f.render_stateful_widget(List::new(vis_items).block(Block::default().borders(Borders::ALL).title(format!(" Files ({}/{}) ", en, idx.len())).border_style(Style::default().fg(DM))).highlight_style(Style::default().bg(HL).add_modifier(Modifier::BOLD)).highlight_symbol("▶ "), left, &mut a.list_state);

    if a.mode == Mode::Working {
        let lines = vec![
            Line::from(Span::styled(format!("{} {}", SPINNER[a.sp], a.label), Style::default().fg(CY).add_modifier(Modifier::BOLD))),
            Line::default(),
            Line::from(Span::styled("Running... (please wait)", Style::default().fg(DM))),
        ];
        f.render_widget(Paragraph::new(Text::from(lines)).block(Block::default().borders(Borders::ALL).title(" Working ").border_style(Style::default().fg(CY))).style(Style::default().fg(FG)), right);
    } else {
        let dl = a.real_idx().map(|i| detail_lines(&a.entries[i])).unwrap_or_else(|| vec![Line::from(Span::styled("Select a file", Style::default().fg(DM).add_modifier(Modifier::ITALIC)))]);
        f.render_widget(Paragraph::new(Text::from(dl)).block(Block::default().borders(Borders::ALL).title(" Details ").border_style(Style::default().fg(DM))).style(Style::default().fg(FG)), right);
    }

    f.render_widget(Paragraph::new(Line::from(Span::styled(&a.status, Style::default().fg(CY)))).block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(DM))), stat);
    if a.mode == Mode::Help { help_pop(f, area); }
    if let Some(ref dk) = a.dialog { dlg_pop(f, area, dk); }
}

fn help_pop(f: &mut Frame, a: Rect) {
    let e: &[(&str,&str)] = &[("","cachyos-dotfiles TUI"),("",""),("↑↓jk","Move"),("Space","Toggle"),("!@#$","Toggle all (Sys|KDE|Cach|Usr)"),("1-4","Category"),("a","All"),("R","Restore selected"),("Tab","Next"),("/","Filter (Esc)"),("",""),("b","Backup"),("r","Restore all"),("d","Diff"),("",""),("q","Quit  h/Esc  Close")];
    let text: Vec<Line> = e.iter().map(|(k,d)| if k.is_empty()&&d.is_empty(){Line::default()} else if k.is_empty(){Line::from(Span::styled(*d,Style::default().fg(FG)))} else{Line::from(vec![Span::styled(format!("{} ",k),Style::default().fg(CY).add_modifier(Modifier::BOLD)),Span::styled(*d,Style::default().fg(FG))])}).collect();
    let pop = centered(a, 44, (text.len()+2) as u16);
    f.render_widget(Clear, pop); f.render_widget(Paragraph::new(Text::from(text)).block(Block::default().borders(Borders::ALL).title(" Help ").border_style(Style::default().fg(CY)).style(Style::default().bg(BG))), pop);
}
fn dlg_pop(f: &mut Frame, a: Rect, dk: &DialogKind) { match dk { DialogKind::ConfirmRestore(dry) => { let lines = vec![Line::from(Span::styled("Pull from GitHub and overwrite system files.",Style::default().fg(YW))),Line::from(Span::styled("/etc/* uses sudo. Existing files backed up.",Style::default().fg(DM))),Line::default(),Line::from(Span::styled(dry.clone(),Style::default().fg(DM))),Line::default(),Line::from(Span::styled("[y] Yes    [n] No",Style::default().fg(CY).add_modifier(Modifier::BOLD)))]; let pop = centered(a, 70, (lines.len()+2) as u16); f.render_widget(Clear, pop); f.render_widget(Paragraph::new(Text::from(lines)).block(Block::default().borders(Borders::ALL).title(" Restore? ").border_style(Style::default().fg(YW)).style(Style::default().bg(BG))), pop); } DialogKind::Output(t, text) => { let d: String = text.chars().take(2000).collect(); let lines: Vec<Line> = d.lines().map(|l| Line::from(Span::styled(l.to_string(),Style::default().fg(FG)))).collect(); let pop = centered(a, 80, (lines.len() as u16 + 3).min(24)); f.render_widget(Clear, pop); f.render_widget(Paragraph::new(Text::from(lines)).block(Block::default().borders(Borders::ALL).title(format!(" {} ",t)).border_style(Style::default().fg(CY)).style(Style::default().bg(BG))), pop); } } }
fn centered(r: Rect, w: u16, h: u16) -> Rect { Rect { x: r.x+(r.width.saturating_sub(w)/2), y: r.y+(r.height.saturating_sub(h)/2), width: w.min(r.width), height: h.min(r.height) } }

// ── Input ─────────────────────────────────────────────────────────────

fn handle(k: KeyEvent, a: &mut App) -> bool {
    match a.mode { Mode::Working => true, Mode::Normal => normal(k, a), Mode::Filter => filter(k, a), Mode::Help => { a.mode = Mode::Normal; true }, Mode::Dialog => dlg_in(k, a) }
}
fn normal(k: KeyEvent, a: &mut App) -> bool {
    let n = a.count();
    match k.code {
        KeyCode::Char('q') => false, KeyCode::Esc => true,
        KeyCode::Char('h') => { a.mode = Mode::Help; true },
        KeyCode::Char(' ') | KeyCode::Enter => { a.toggle(); true },
        KeyCode::Char('b') => { a.launch("Backing up...", vec!["backup".into()]); true },
        KeyCode::Char('r') => { a.launch("Dry-run...", vec!["restore".into(),"--dry-run".into(),"--yes".into()]); true },
        KeyCode::Char('R') => { if let Some(i) = a.real_idx() { let e = &a.entries[i]; let (rp, su) = repo_file(e); let src = source_path(e); if rp.exists() { if su { let _ = Command::new("sudo").args(["cp", rp.to_str().unwrap(), src.to_str().unwrap()]).spawn(); } else { let _ = fs::copy(&rp, &src); } a.set_status(&format!("✓ Restored {}", e.path)); } else { a.set_status(&format!("✗ Not in repo: {}", e.path)); } } true },
        KeyCode::Char('d') => { if let Some(i) = a.real_idx() { let p = a.entries[i].path.clone(); let (_, out) = cli_blocking(&["diff", &p]); a.mode = Mode::Dialog; a.dialog = Some(DialogKind::Output(format!("Diff: {}", p), out)); } true },
        KeyCode::Char('1') => { a.category = 1; a.sel = 0; a.scroll = 0; true },
        KeyCode::Char('2') => { a.category = 2; a.sel = 0; a.scroll = 0; true },
        KeyCode::Char('3') => { a.category = 3; a.sel = 0; a.scroll = 0; true },
        KeyCode::Char('4') => { a.category = 4; a.sel = 0; a.scroll = 0; true },
        KeyCode::Char('!') => { a.toggle_category(1); true }, KeyCode::Char('@') => { a.toggle_category(2); true },
        KeyCode::Char('#') => { a.toggle_category(3); true }, KeyCode::Char('$') => { a.toggle_category(4); true },
        KeyCode::Char('a') => { a.category = 0; a.sel = 0; a.scroll = 0; true },
        KeyCode::Char('/') => { a.mode = Mode::Filter; true },
        KeyCode::Tab => { a.category = (a.category + 1) % CATS.len(); a.sel = 0; a.scroll = 0; true },
        KeyCode::BackTab => { a.category = if a.category == 0 { CATS.len() - 1 } else { a.category - 1 }; a.sel = 0; a.scroll = 0; true },
        KeyCode::Up | KeyCode::Char('k') => { if n > 0 { a.sel = if a.sel == 0 { n - 1 } else { a.sel - 1 }; } true },
        KeyCode::Down | KeyCode::Char('j') => { if n > 0 { a.sel = (a.sel + 1) % n; } true },
        _ => true,
    }
}
fn filter(k: KeyEvent, a: &mut App) -> bool { match k.code { KeyCode::Esc => { a.filter.clear(); a.mode = Mode::Normal; a.sel = 0; a.scroll = 0; true }, KeyCode::Enter => { a.mode = Mode::Normal; true }, KeyCode::Backspace => { a.filter.pop(); a.sel = 0; a.scroll = 0; true }, KeyCode::Char(c) => { a.filter.push(c); a.sel = 0; a.scroll = 0; true }, _ => true } }
fn dlg_in(k: KeyEvent, a: &mut App) -> bool {
    if let Some(DialogKind::ConfirmRestore(_)) = a.dialog {
        match k.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => { a.launch("Restoring...", vec!["restore".into(),"--yes".into()]); true },
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => { a.dialog = None; a.mode = Mode::Normal; true },
            _ => true,
        }
    } else { a.dialog = None; a.mode = Mode::Normal; true }
}

fn repo_file(e: &DotfileEntry) -> (PathBuf, bool) { if e.path.starts_with("~/") { (repo_dir().join("home").join(&e.path[2..]), false) } else if e.path.starts_with('/') { (repo_dir().join("root").join(&e.path[1..]), e.sudo) } else { (repo_dir().join("home").join(&e.path), false) } }
fn source_path(e: &DotfileEntry) -> PathBuf { if e.path.starts_with("~/") { home().join(&e.path[2..]) } else { PathBuf::from(&e.path) } }

fn main() -> io::Result<()> {
    let wiz = std::env::args().any(|a| a == "--wizard" || a == "-w") || !manifest().exists() || !config_file().exists();
    if wiz { match wizard() { Ok(false) => return Ok(()), _ => {} } }
    if !manifest().exists() { eprintln!("Run with --wizard first."); std::process::exit(1); }
    let mut t = ratatui::init(); t.clear().unwrap();
    let mut a = App::new();
    let r = run(&mut t, &mut a);
    ratatui::restore(); r
}

fn run(t: &mut ratatui::DefaultTerminal, a: &mut App) -> io::Result<()> {
    loop {
        t.draw(|f| ui(f, a))?;
        if a.mode == Mode::Working {
            if let Some((ok, out)) = a.poll() {
                if a.label.contains("Dry-run") {
                    a.update_repo();
                    a.dialog = Some(DialogKind::ConfirmRestore(out.chars().take(400).collect()));
                    a.mode = Mode::Dialog;
                } else {
                    a.update_repo();
                    a.mode = Mode::Dialog;
                    a.dialog = Some(DialogKind::Output(a.label.split("...").next().unwrap_or("Result").into(), out));
                    a.set_status(if ok { "✓ Done" } else { "✗ Failed" });
                }
                continue;
            }
            while event::poll(Duration::from_millis(0))? { let _ = event::read()?; }
            thread::sleep(Duration::from_millis(16));
            continue;
        }
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(k) = event::read()? { if !handle(k, a) { return Ok(()); } }
        }
    }
}
