use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io::{self, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
    let cargo = home().join(".cargo/bin/cachyos-dotfiles"); if cargo.exists() { return cargo; }
    PathBuf::from("./cachyos-dotfiles")
}
fn repo_dir() -> PathBuf { home().join(".local/share/cachyos-dotfiles/repo") }
fn load() -> Vec<DotfileEntry> { manifest().exists().then(|| fs::read_to_string(manifest()).ok()).flatten().and_then(|d| serde_json::from_str(&d).ok()).unwrap_or_default() }
fn save(e: &[DotfileEntry]) { let p = manifest(); let _ = std::fs::create_dir_all(p.parent().unwrap()); serde_json::to_string_pretty(e).ok().map(|j| fs::write(&p, j).ok()); }
fn cli_blocking(args: &[&str]) -> (bool, String) { match Command::new(cli_bin()).args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() { Ok(c) => { let o = c.wait_with_output().unwrap(); (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))) } Err(e) => (false, format!("CLI: {}", e)) } }
fn git_status() -> String { let d = repo_dir(); if !d.join(".git").exists() { return "no repo".into(); } match Command::new("git").args(["-C"]).arg(d).args(["status","--porcelain"]).output() { Ok(o) if o.status.success() => { let n = String::from_utf8_lossy(&o.stdout).lines().count(); if n == 0 { "clean".into() } else { format!("{}Δ", n) } } _ => "?".into() } }
fn last_backup_time() -> String {
    let d = repo_dir(); if !d.join(".git").exists() { return String::new(); }
    match Command::new("git").args(["-C"]).arg(d).args(["log","-1","--format=%ct"]).output() {
        Ok(o) if o.status.success() => {
            let ts: u64 = String::from_utf8_lossy(&o.stdout).trim().parse().unwrap_or(0);
            if ts == 0 { return String::new(); }
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            let ago = now.saturating_sub(ts);
            if ago < 60 { "just now".into() } else if ago < 3600 { format!("{}m ago", ago/60) } else if ago < 86400 { format!("{}h ago", ago/3600) } else { format!("{}d ago", ago/86400) }
        } _ => String::new(),
    }
}
fn file_changed_since_backup(e: &DotfileEntry) -> bool { let src = if e.path.starts_with("~/") { home().join(&e.path[2..]) } else { PathBuf::from(&e.path) }; let (rp, _) = repo_file(e); if !src.exists() || !rp.exists() { return false; } fs::read(&src).ok().zip(fs::read(&rp).ok()).map(|(a,b)| a != b).unwrap_or(false) }
fn unified_diff(e: &DotfileEntry) -> Vec<Line<'static>> { let src = if e.path.starts_with("~/") { home().join(&e.path[2..]) } else { PathBuf::from(&e.path) }; let (rp, _) = repo_file(e); if !src.exists() || !rp.exists() { return vec![Line::from("(file missing)")]; } let out = Command::new("diff").args(["-u", rp.to_str().unwrap_or(""), src.to_str().unwrap_or("")]).output(); match out { Ok(o) => { let raw = String::from_utf8_lossy(&o.stdout); if raw.is_empty() { vec![Line::from(Span::styled("(identical)", Style::default().fg(DM)))] } else { raw.lines().map(|l| { if l.starts_with('+') && !l.starts_with("+++") { Line::from(Span::styled(l.to_string(), Style::default().fg(GN))) } else if l.starts_with('-') && !l.starts_with("---") { Line::from(Span::styled(l.to_string(), Style::default().fg(RD))) } else if l.starts_with('@') { Line::from(Span::styled(l.to_string(), Style::default().fg(CY))) } else { Line::from(Span::styled(l.to_string(), Style::default().fg(DM))) } }).collect() } } _ => vec![Line::from("diff failed")], } }
fn colorize_output(raw: &str) -> Vec<Line<'static>> { raw.lines().map(|l| { if l.starts_with('+') && !l.starts_with("+++") { Line::from(Span::styled(l.to_string(), Style::default().fg(GN))) } else if l.starts_with('-') && !l.starts_with("---") { Line::from(Span::styled(l.to_string(), Style::default().fg(RD))) } else if l.starts_with('@') { Line::from(Span::styled(l.to_string(), Style::default().fg(CY))) } else if l.starts_with("✓") || l.contains("Complete") { Line::from(Span::styled(l.to_string(), Style::default().fg(GN))) } else if l.starts_with("✗") || l.contains("Failed") { Line::from(Span::styled(l.to_string(), Style::default().fg(RD))) } else { Line::from(Span::styled(l.to_string(), Style::default().fg(FG))) } }).collect() }

fn bat_preview(path: &PathBuf, max_lines: usize) -> Vec<Line<'static>> {
    if !path.exists() || path.is_dir() { return vec![]; }
    let range = format!(":{}", max_lines);
    let out = Command::new("bat").args(["--color=never","--style=plain","--line-range",&range]).arg(path).output();
    match out { Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).lines().map(|l| Line::from(l.to_string())).collect(), _ => match fs::read_to_string(path) { Ok(s) => s.lines().take(max_lines).map(|l| Line::from(l.to_string())).collect(), Err(_) => vec![] } }
}

fn dir_preview(path: &PathBuf) -> Vec<Line<'static>> {
    if !path.exists() || !path.is_dir() { return vec![Line::from("(not a directory)")]; }
    let mut entries: Vec<_> = match fs::read_dir(path) { Ok(d) => d.filter_map(|e| e.ok()).filter(|e| !e.file_name().to_string_lossy().starts_with('.')).collect(), Err(e) => return vec![Line::from(format!("(error: {})", e))] };
    entries.sort_by_key(|e| e.file_name());
    let max = 15.min(entries.len());
    let mut lines = vec![Line::from(Span::styled(format!("{} items:", entries.len()), Style::default().fg(DM)))];
    for ent in entries.iter().take(max) {
        let name = ent.file_name().to_string_lossy().to_string();
        let is_dir = ent.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let suffix = if is_dir { "/" } else { "" };
        lines.push(Line::from(Span::styled(format!("{}{}", name, suffix), Style::default().fg(CY))));
        if !is_dir {
            if let Ok(data) = fs::read_to_string(ent.path()) {
                for l in data.lines().take(3) { lines.push(Line::from(Span::styled(format!("  {}", l), Style::default().fg(DM)))); }
            }
        }
    }
    if entries.len() > max { lines.push(Line::from(Span::styled(format!("... and {} more", entries.len() - max), Style::default().fg(DM)))); }
    lines
}

// ── Wizard ─────────────────────────────────────────────────────────────
fn wizard() -> io::Result<bool> { println!("\x1b[1;36m═══ cachyos-dotfiles Setup Wizard ═══\x1b[0m\n"); print!("  Checking GitHub authentication... "); io::stdout().flush()?; let gh = Command::new("gh").args(["auth","status"]).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false); if gh { println!("\x1b[32m✓ Authenticated\x1b[0m"); } else { println!("\x1b[33m✗ Not authenticated\x1b[0m"); println!("\n  Run: \x1b[36mgh auth login\x1b[0m\n  (HTTPS — no SSH key needed)\n"); return Ok(false); } println!("\n  What would you like to do?\n"); println!("    \x1b[36m[b]\x1b[0m  Backup my dotfiles to GitHub"); println!("    \x1b[36m[r]\x1b[0m  Restore dotfiles from GitHub"); println!("    \x1b[36m[q]\x1b[0m  Quit (launch TUI)"); print!("\n  Choice: "); io::stdout().flush()?; match read_char()? { 'b'|'B' => wiz_backup()?, 'r'|'R' => wiz_restore()?, _ => { println!("  Launching TUI...\n"); return Ok(true); } } Ok(true) }
fn wiz_backup() -> io::Result<()> { println!("\n  Do you have a GitHub repo for dotfiles?\n    \x1b[36m[y]\x1b[0m Yes  \x1b[36m[n]\x1b[0m No — create one"); print!("  Choice: "); io::stdout().flush()?; let has = read_char()?; println!(); let url: String = if has == 'y' || has == 'Y' { print!("  Repo URL: "); io::stdout().flush()?; let mut u = String::new(); io::stdin().read_line(&mut u)?; u.trim().to_string() } else { let user = String::from_utf8_lossy(&Command::new("gh").args(["api","user","--jq",".login"]).output().map(|o| o.stdout).unwrap_or_default()).trim().to_string(); print!("  Repo name [cachyos-dotfiles]: "); io::stdout().flush()?; let mut nn = String::new(); io::stdin().read_line(&mut nn)?; let name = if nn.trim().is_empty() { "cachyos-dotfiles" } else { nn.trim() }; print!("  Private? [Y/n]: "); io::stdout().flush()?; let is_priv = read_char()? != 'n'; println!(); let full = format!("{}/{}", user, name); let mut args = vec!["repo","create",&full,"--description","CachyOS dotfiles backup"]; if is_priv { args.push("--private"); } else { args.push("--public"); } print!("  Creating {}... ", full); io::stdout().flush()?; if Command::new("gh").args(&args).status().map(|s| s.success()).unwrap_or(false) { println!("\x1b[32m✓ Created\x1b[0m"); format!("https://github.com/{}/{}.git", user, name) } else { println!("\x1b[33mFailed\x1b[0m"); print!("  Enter URL manually: "); io::stdout().flush()?; let mut u = String::new(); io::stdin().read_line(&mut u)?; u.trim().to_string() } }; if url.is_empty() { println!("  Aborting.\n"); return Ok(()); } println!("  Initializing with {}...", url); let (_, _) = cli_blocking(&["init","--repo",&url]); let e = load(); println!("\n  Loaded {} files ({} enabled)", e.len(), e.iter().filter(|x| x.enabled).count()); print!("  Run backup now? [Y/n]: "); io::stdout().flush()?; if read_char()?.to_ascii_lowercase() != 'n' { let (_, o) = cli_blocking(&["backup"]); println!("{}", o); } println!("\n  \x1b[36m═══ Done! ═══\x1b[0m\n"); Ok(()) }
fn wiz_restore() -> io::Result<()> { print!("  GitHub repo URL: "); io::stdout().flush()?; let mut url = String::new(); io::stdin().read_line(&mut url)?; let url = url.trim().to_string(); if url.is_empty() { println!("  Aborting.\n"); return Ok(()); } cli_blocking(&["init","--repo",&url]); print!("\n  Run restore now? [Y/n]: "); io::stdout().flush()?; if read_char()?.to_ascii_lowercase() != 'n' { let (_, dry) = cli_blocking(&["restore","--dry-run","--yes"]); println!("{}", dry.chars().take(600).collect::<String>()); print!("\n  Proceed? [y/N]: "); io::stdout().flush()?; if read_char()?.to_ascii_lowercase() == 'y' { let (_, o) = cli_blocking(&["restore","--yes"]); println!("{}", o); } } println!("\n  \x1b[36m═══ Done! ═══\x1b[0m\n"); Ok(()) }
fn read_char() -> io::Result<char> { let mut l = String::new(); io::stdin().read_line(&mut l)?; Ok(l.chars().next().unwrap_or('\n')) }

// ── App ───────────────────────────────────────────────────────────────
struct App {
    entries: Vec<DotfileEntry>, category: usize, filter: String, sel: usize, scroll: usize,
    mode: Mode, dialog: Option<DialogKind>, status: String, repo_status: String, last_backup: String,
    list_state: ListState, rx: Option<mpsc::Receiver<(bool, String)>>, label: String,
    sp: usize, sp_tick: Instant, result: Option<(bool, String)>, deadline: Instant,
    output_lines: Vec<Line<'static>>, output_scroll: usize, output_title: String,
}
#[derive(PartialEq)] enum Mode { Normal, Filter, Help, Dialog, Output, Working }
enum DialogKind { ConfirmRestore(String), DiffPreview(String) }
const SPINNER: &[&str] = &["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"];
const CATS: &[&str] = &["All","System","KDE","CachyOS","User"];
const MAP: &[&str] = &["all","system","kde","cachyos","user"];

impl App {
    fn new() -> Self { let mut s = ListState::default(); s.select(Some(0)); let gs = git_status(); let lb = last_backup_time(); Self { entries: load(), category: 0, filter: String::new(), sel: 0, scroll: 0, mode: Mode::Normal, dialog: None, status: status_bar(&gs, &lb, "Space:toggle  b:backup  r:restore  d:diff  h:help  q:quit"), repo_status: gs, last_backup: lb, list_state: s, rx: None, label: String::new(), sp: 0, sp_tick: Instant::now(), result: None, deadline: Instant::now(), output_lines: Vec::new(), output_scroll: 0, output_title: String::new() } }
    fn filtered(&self) -> Vec<usize> { let ft = self.filter.to_lowercase(); self.entries.iter().enumerate().filter(|(_, e)| (self.category == 0 || MAP.get(self.category) == Some(&e.category.as_str())) && (ft.is_empty() || e.path.to_lowercase().contains(&ft) || e.description.to_lowercase().contains(&ft))).map(|(i, _)| i).collect() }
    fn real_idx(&self) -> Option<usize> { self.filtered().get(self.sel).copied() }
    fn count(&self) -> usize { self.filtered().len() }
    fn sync(&mut self, vis: u16) { let n = self.count(); let v = vis.saturating_sub(2) as usize; if n == 0 || v == 0 { self.list_state.select(None); return; } if self.sel >= n { self.sel = n.saturating_sub(1); } if self.sel < self.scroll { self.scroll = self.sel; } else if self.sel >= self.scroll.saturating_add(v) { self.scroll = self.sel.saturating_sub(v.saturating_sub(1)); } if n > v { self.scroll = self.scroll.min(n.saturating_sub(v)); } else { self.scroll = 0; } self.list_state.select(Some((self.sel.saturating_sub(self.scroll)).min(v))); }
    fn toggle(&mut self) { if self.count() == 0 { return; } if let Some(i) = self.real_idx() { self.entries[i].enabled = !self.entries[i].enabled; self.status = status_bar(&self.repo_status, &self.last_backup, &format!("{} {}", if self.entries[i].enabled { "✓" } else { "✗" }, self.entries[i].path)); save(&self.entries); } }
    fn set_status(&mut self, msg: &str) { self.status = status_bar(&self.repo_status, &self.last_backup, msg); }
    fn toggle_category(&mut self, cat: usize) { let ena = !self.entries.iter().any(|e| e.category == MAP[cat] && e.enabled); for e in &mut self.entries { if e.category == MAP[cat] { e.enabled = ena; } } save(&self.entries); self.status = status_bar(&self.repo_status, &self.last_backup, &format!("{} all {}", if ena { "✓" } else { "✗" }, CATS[cat])); }
    fn update_repo(&mut self) { self.repo_status = git_status(); self.last_backup = last_backup_time(); }
    fn launch(&mut self, label: &str, args: Vec<String>) { let (tx, rx) = mpsc::channel(); thread::spawn(move || { let ca: Vec<&str> = args.iter().map(|s| s.as_str()).collect(); let r = cli_blocking(&ca); let _ = tx.send(r); }); self.rx = Some(rx); self.label = label.to_string(); self.mode = Mode::Working; self.sp = 0; self.sp_tick = Instant::now(); self.result = None; self.deadline = Instant::now() + Duration::from_millis(500); }
    fn poll(&mut self) -> Option<(bool, String)> { if self.sp_tick.elapsed().as_millis() >= 80 { self.sp = (self.sp + 1) % SPINNER.len(); self.sp_tick = Instant::now(); } if let Some(ref rx) = self.rx { if let Ok(r) = rx.try_recv() { self.rx = None; self.result = Some(r); } } if Instant::now() >= self.deadline { return self.result.take(); } None }
}
fn status_bar(repo: &str, backup: &str, msg: &str) -> String { let lb = if backup.is_empty() { String::new() } else { format!("  Last: {}", backup) }; format!("[{}]{}  {}", repo, lb, msg) }

// ── Styling ───────────────────────────────────────────────────────────
const BG: Color = Color::Rgb(18,18,24);  const FG: Color = Color::Rgb(238,238,255); const CY: Color = Color::Rgb(0,230,230);
const GN: Color = Color::Rgb(0,255,120); const RD: Color = Color::Rgb(255,60,60);   const YW: Color = Color::Rgb(255,220,50);
const PK: Color = Color::Rgb(255,80,180); const DM: Color = Color::Rgb(130,140,160); const HL: Color = Color::Rgb(50,55,75);
const OR: Color = Color::Rgb(255,150,30);

fn entry_item(e: &DotfileEntry, changed: bool) -> ListItem<'static> { let ico = if e.enabled { "✓" } else { "✗" }; let ch = if changed && e.enabled { "* " } else { "" }; let col = if !e.enabled { RD } else if e.sudo { YW } else { GN }; ListItem::new(Line::from(Span::styled(format!("{}{} {}{}{}", ch, ico, e.path, if e.is_dir { "/" } else { "" }, if e.sudo { " [sudo]" } else { "" }), Style::default().fg(col)))) }
fn detail_lines(e: &DotfileEntry, changed: bool) -> Vec<Line<'static>> { let p = e.path.clone(); let c = e.category.clone(); let d = e.description.clone(); let mut lines = vec![Line::from(Span::styled(format!("Path:       {}", p), Style::default().fg(CY))),Line::from(Span::styled(format!("Category:   {}", c), Style::default().fg(DM))),Line::from(Span::styled(format!("Status:     {}", if e.enabled { "✓ Enabled" } else { "✗ Disabled" }), Style::default().fg(GN))),Line::from(Span::styled(format!("Needs sudo: {}", if e.sudo { "Yes" } else { "No" }), Style::default().fg(YW))),Line::default(),Line::from(Span::styled(d, Style::default().fg(DM)))]; if changed { lines.push(Line::from(Span::styled("* Modified since last backup", Style::default().fg(OR)))); } if p.contains(".ssh/") { lines.push(Line::default()); lines.push(Line::from(Span::styled("⚠ SECURITY", Style::default().fg(PK)))); } if p.contains("kwalletrc") { lines.push(Line::default()); lines.push(Line::from(Span::styled("⚠ Wallet config", Style::default().fg(YW)))); } if p.contains("kdeconnect") { lines.push(Line::default()); lines.push(Line::from(Span::styled("⚠ Device IDs", Style::default().fg(YW)))); } lines }

// ── Render ────────────────────────────────────────────────────────────
fn ui(f: &mut Frame, a: &mut App) {
    let area = f.area();
    let [header, body, stat] = Layout::vertical([Constraint::Length(1), Constraint::Min(3), Constraint::Length(2)]).areas(area);
    let mut spans: Vec<Span> = Vec::new();
    for (i, &cat) in CATS.iter().enumerate() {
        if i > 0 { spans.push(Span::raw(" │ ")); }
        let style = if i == a.category { Style::default().fg(CY).add_modifier(Modifier::BOLD) } else { Style::default().fg(DM) };
        spans.push(Span::styled(cat.to_string(), style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), header);
    let [left, right] = Layout::horizontal([Constraint::Length(52), Constraint::Min(20)]).areas(body);
    let idx = a.filtered(); let en = idx.iter().filter(|&&i| a.entries[i].enabled).count();
    a.sync(left.height);
    let vis = (left.height.saturating_sub(2) as usize).max(1);
    let vis_items: Vec<ListItem> = idx.iter().skip(a.scroll).take(vis).map(|&i| { let changed = a.entries[i].enabled && file_changed_since_backup(&a.entries[i]); entry_item(&a.entries[i], changed) }).collect();
    f.render_stateful_widget(List::new(vis_items).block(Block::default().borders(Borders::ALL).title(format!(" Files ({}/{}) ", en, idx.len())).border_style(Style::default().fg(DM))).highlight_style(Style::default().bg(HL).add_modifier(Modifier::BOLD)).highlight_symbol("▶ "), left, &mut a.list_state);
    let [detail_area, preview_area] = Layout::vertical([Constraint::Length(8), Constraint::Min(3)]).areas(right);
    if a.mode == Mode::Working { let lines = vec![Line::from(Span::styled(format!("{} {}", SPINNER[a.sp], a.label), Style::default().fg(CY).add_modifier(Modifier::BOLD))), Line::default(), Line::from(Span::styled("Running...", Style::default().fg(DM)))]; f.render_widget(Paragraph::new(Text::from(lines)).block(Block::default().borders(Borders::ALL).title(" Working ").border_style(Style::default().fg(CY))).style(Style::default().fg(FG)), right); }
    else if a.mode == Mode::Output {
        let vis_lines = (right.height.saturating_sub(2) as usize).max(1);
        let shown: Vec<Line> = a.output_lines.iter().skip(a.output_scroll).take(vis_lines).cloned().collect();
        let title = format!(" {} ({}/{}) ", a.output_title, a.output_scroll + 1, a.output_lines.len());
        f.render_widget(Paragraph::new(Text::from(shown)).block(Block::default().borders(Borders::ALL).title(title).border_style(Style::default().fg(CY))).style(Style::default().fg(FG)), right);
    } else {
        let dl = a.real_idx().map(|i| { let changed = a.entries[i].enabled && file_changed_since_backup(&a.entries[i]); detail_lines(&a.entries[i], changed) }).unwrap_or_else(|| vec![Line::from(Span::styled("Select a file", Style::default().fg(DM).add_modifier(Modifier::ITALIC)))]);
        f.render_widget(Paragraph::new(Text::from(dl)).block(Block::default().borders(Borders::ALL).title(" Details ").border_style(Style::default().fg(DM))).style(Style::default().fg(FG)), detail_area);
        if let Some(i) = a.real_idx() {
            let e = &a.entries[i];
            if !e.is_dir {
                let src = if e.path.starts_with("~/") { home().join(&e.path[2..]) } else { PathBuf::from(&e.path) };
                f.render_widget(Paragraph::new(Text::from(bat_preview(&src, 20))).block(Block::default().borders(Borders::ALL).title(" Preview ").border_style(Style::default().fg(DM))).style(Style::default().fg(FG)), preview_area);
            } else {
                let src = if e.path.starts_with("~/") { home().join(&e.path[2..]) } else { PathBuf::from(&e.path) };
                f.render_widget(Paragraph::new(Text::from(dir_preview(&src))).block(Block::default().borders(Borders::ALL).title(" Preview ").border_style(Style::default().fg(DM))).style(Style::default().fg(FG)), preview_area);
            }
        } else { f.render_widget(Paragraph::new("Select a file").block(Block::default().borders(Borders::ALL).title(" Preview ").border_style(Style::default().fg(DM))).style(Style::default().fg(DM)), preview_area); }
    }
    f.render_widget(Paragraph::new(Line::from(Span::styled(&a.status, Style::default().fg(CY)))).block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(DM))), stat);
    if a.mode == Mode::Help { help_pop(f, area); }
    if let Some(ref dk) = a.dialog { dlg_pop(f, area, dk); }
}

fn help_pop(f: &mut Frame, a: Rect) { let e: &[(&str,&str)] = &[("","cachyos-dotfiles TUI"),("",""),("↑↓jk","Move"),("Space","Toggle"),("!@#$","Toggle all (Sys|KDE|Cach|Usr)"),("1-4","Category"),("a","All"),("R","Restore selected"),("Tab","Next"),("/","Filter (Esc)"),("",""),("b","Backup"),("r","Restore all"),("d","Diff"),("",""),("q","Quit  h/Esc  Close")]; let text: Vec<Line> = e.iter().map(|(k,d)| if k.is_empty()&&d.is_empty(){Line::default()} else if k.is_empty(){Line::from(Span::styled(*d,Style::default().fg(FG)))} else{Line::from(vec![Span::styled(format!("{} ",k),Style::default().fg(CY).add_modifier(Modifier::BOLD)),Span::styled(*d,Style::default().fg(FG))])}).collect(); let pop = centered(a, 44, (text.len()+2) as u16); f.render_widget(Clear, pop); f.render_widget(Paragraph::new(Text::from(text)).block(Block::default().borders(Borders::ALL).title(" Help ").border_style(Style::default().fg(CY)).style(Style::default().bg(BG))), pop); }
fn dlg_pop(f: &mut Frame, a: Rect, dk: &DialogKind) { match dk { DialogKind::DiffPreview(text) => { let lines: Vec<Line> = text.lines().map(|l| { if l.starts_with('+') && !l.starts_with("+++") { Line::from(Span::styled(l.to_string(), Style::default().fg(GN))) } else if l.starts_with('-') && !l.starts_with("---") { Line::from(Span::styled(l.to_string(), Style::default().fg(RD))) } else if l.starts_with('@') { Line::from(Span::styled(l.to_string(), Style::default().fg(CY))) } else { Line::from(Span::styled(l.to_string(), Style::default().fg(DM))) } }).collect(); let pop = centered(a, 80, (lines.len() as u16 + 3).min(24)); f.render_widget(Clear, pop); f.render_widget(Paragraph::new(Text::from(lines)).block(Block::default().borders(Borders::ALL).title(" Dry-run Diff ").border_style(Style::default().fg(CY)).style(Style::default().bg(BG))), pop); } DialogKind::ConfirmRestore(dry) => { let lines = vec![Line::from(Span::styled("Pull from GitHub and overwrite system files.",Style::default().fg(YW))),Line::from(Span::styled("/etc/* uses sudo. Existing files backed up.",Style::default().fg(DM))),Line::default(),Line::from(Span::styled(dry.clone(),Style::default().fg(DM))),Line::default(),Line::from(Span::styled("[y] Yes    [n] No  [d] View diff",Style::default().fg(CY).add_modifier(Modifier::BOLD)))]; let pop = centered(a, 70, (lines.len()+2) as u16); f.render_widget(Clear, pop); f.render_widget(Paragraph::new(Text::from(lines)).block(Block::default().borders(Borders::ALL).title(" Restore? ").border_style(Style::default().fg(YW)).style(Style::default().bg(BG))), pop); } } }
fn centered(r: Rect, w: u16, h: u16) -> Rect { Rect { x: r.x+(r.width.saturating_sub(w)/2), y: r.y+(r.height.saturating_sub(h)/2), width: w.min(r.width), height: h.min(r.height) } }

// ── Input ─────────────────────────────────────────────────────────────
fn handle(k: KeyEvent, a: &mut App) -> bool { match a.mode { Mode::Working => true, Mode::Normal => key_normal(k, a), Mode::Filter => key_filter(k, a), Mode::Help => { a.mode = Mode::Normal; true }, Mode::Dialog => key_dlg(k, a), Mode::Output => key_output(k, a) } }
fn key_normal(k: KeyEvent, a: &mut App) -> bool { let n = a.count(); match k.code { KeyCode::Char('q') => false, KeyCode::Esc => true, KeyCode::Char('h') => { a.mode = Mode::Help; true }, KeyCode::Char(' ') | KeyCode::Enter => { a.toggle(); true }, KeyCode::Char('b') => { a.launch("Backing up...", vec!["backup".into()]); true }, KeyCode::Char('r') => { a.launch("Dry-run...", vec!["restore".into(),"--dry-run".into(),"--yes".into()]); true }, KeyCode::Char('R') => { if let Some(i) = a.real_idx() { let e = &a.entries[i]; let (rp, su) = repo_file(e); let src = source_path(e); if rp.exists() { if su { let _ = Command::new("sudo").args(["cp", rp.to_str().unwrap(), src.to_str().unwrap()]).spawn(); } else { let _ = fs::copy(&rp, &src); } a.set_status(&format!("✓ Restored {}", e.path)); } else { a.set_status(&format!("✗ Not in repo: {}", e.path)); } } true }, KeyCode::Char('d') => { if let Some(i) = a.real_idx() { let diff_lines = unified_diff(&a.entries[i]); a.output_lines = diff_lines; a.output_scroll = 0; a.output_title = format!("Diff: {}", a.entries[i].path); a.mode = Mode::Output; } true }, KeyCode::Char('1') => { a.category = 1; a.sel = 0; a.scroll = 0; true }, KeyCode::Char('2') => { a.category = 2; a.sel = 0; a.scroll = 0; true }, KeyCode::Char('3') => { a.category = 3; a.sel = 0; a.scroll = 0; true }, KeyCode::Char('4') => { a.category = 4; a.sel = 0; a.scroll = 0; true }, KeyCode::Char('!') => { a.toggle_category(1); true }, KeyCode::Char('@') => { a.toggle_category(2); true }, KeyCode::Char('#') => { a.toggle_category(3); true }, KeyCode::Char('$') => { a.toggle_category(4); true }, KeyCode::Char('a') => { a.category = 0; a.sel = 0; a.scroll = 0; true }, KeyCode::Char('/') => { a.mode = Mode::Filter; true }, KeyCode::Tab => { a.category = (a.category + 1) % CATS.len(); a.sel = 0; a.scroll = 0; true }, KeyCode::BackTab => { a.category = if a.category == 0 { CATS.len() - 1 } else { a.category - 1 }; a.sel = 0; a.scroll = 0; true }, KeyCode::Up | KeyCode::Char('k') => { if n > 0 { a.sel = if a.sel == 0 { n - 1 } else { a.sel - 1 }; } true }, KeyCode::Down | KeyCode::Char('j') => { if n > 0 { a.sel = (a.sel + 1) % n; } true }, _ => true } }
fn key_output(k: KeyEvent, a: &mut App) -> bool { match k.code { KeyCode::Esc | KeyCode::Char('q') => { a.mode = Mode::Normal; true }, KeyCode::Up | KeyCode::Char('k') => { if a.output_scroll > 0 { a.output_scroll -= 1; } true }, KeyCode::Down | KeyCode::Char('j') => { let max = a.output_lines.len().saturating_sub(1); if a.output_scroll < max { a.output_scroll += 1; } true }, _ => true } }
fn key_filter(k: KeyEvent, a: &mut App) -> bool { match k.code { KeyCode::Esc => { a.filter.clear(); a.mode = Mode::Normal; a.sel = 0; a.scroll = 0; true }, KeyCode::Enter => { a.mode = Mode::Normal; true }, KeyCode::Backspace => { a.filter.pop(); a.sel = 0; a.scroll = 0; true }, KeyCode::Char(c) => { a.filter.push(c); a.sel = 0; a.scroll = 0; true }, _ => true } }
fn key_dlg(k: KeyEvent, a: &mut App) -> bool { if let Some(DialogKind::ConfirmRestore(_)) = a.dialog { match k.code { KeyCode::Char('y')|KeyCode::Char('Y') => { a.launch("Restoring...", vec!["restore".into(),"--yes".into()]); true }, KeyCode::Char('d')|KeyCode::Char('D') => { let (_, dry) = cli_blocking(&["restore","--dry-run","--yes"]); a.dialog = Some(DialogKind::DiffPreview(dry)); true }, KeyCode::Char('n')|KeyCode::Char('N')|KeyCode::Esc => { a.dialog = None; a.mode = Mode::Normal; true }, _ => true } } else { a.dialog = None; a.mode = Mode::Normal; true } }
fn repo_file(e: &DotfileEntry) -> (PathBuf, bool) { if e.path.starts_with("~/") { (repo_dir().join("home").join(&e.path[2..]), false) } else if e.path.starts_with('/') { (repo_dir().join("root").join(&e.path[1..]), e.sudo) } else { (repo_dir().join("home").join(&e.path), false) } }
fn source_path(e: &DotfileEntry) -> PathBuf { if e.path.starts_with("~/") { home().join(&e.path[2..]) } else { PathBuf::from(&e.path) } }

fn main() -> io::Result<()> { let wiz = std::env::args().any(|a| a == "--wizard" || a == "-w") || !manifest().exists() || !config_file().exists(); if wiz { match wizard() { Ok(false) => return Ok(()), _ => {} } } if !manifest().exists() { eprintln!("Run with --wizard first."); std::process::exit(1); } let mut t = ratatui::init(); t.clear().unwrap(); let mut a = App::new(); let r = run(&mut t, &mut a); ratatui::restore(); r }
fn run(t: &mut ratatui::DefaultTerminal, a: &mut App) -> io::Result<()> { loop { t.draw(|f| ui(f, a))?; if a.mode == Mode::Working { if let Some((ok, out)) = a.poll() { a.update_repo(); if a.label.contains("Dry-run") { a.dialog = Some(DialogKind::ConfirmRestore(out.chars().take(400).collect())); a.mode = Mode::Dialog; } else { a.output_lines = colorize_output(&out); a.output_scroll = 0; a.output_title = a.label.split("...").next().unwrap_or("Result").to_string(); a.mode = Mode::Output; a.set_status(if ok { "✓ Done — q to close" } else { "✗ Failed — q to close" }); } continue; } while event::poll(Duration::from_millis(0))? { let _ = event::read()?; } thread::sleep(Duration::from_millis(16)); continue; } if event::poll(Duration::from_millis(100))? { if let Event::Key(k) = event::read()? { if !handle(k, a) { return Ok(()); } } } } }
