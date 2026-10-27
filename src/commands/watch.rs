//! `proc watch` - Real-time process monitoring
//!
//! Examples:
//!   proc watch                     # Watch all processes (alias: proc top)
//!   proc watch node                # Watch the processes named node
//!   proc watch node --cmd          # ... or with node anywhere in their command line
//!   proc watch :3000               # Watch process on port 3000
//!   proc watch :3000 :8080 node    # Watch several targets: separate arguments
//!   proc watch --in . --by node    # Watch node processes in current directory
//!   proc watch -n 1 --sort mem     # 1s refresh, sorted by memory
//!   proc watch --json | head -2    # NDJSON output
//!
//! Commas still work (`proc watch :3000,:8080`) but are deprecated.

use crate::core::process::classify_sysinfo;
use crate::core::{
    flatten_targets, parse_target, resolve_in_dir, sort_processes, Hit, NameMatch, Process,
    ProcessStatus, SortKey, TargetType,
};
use crate::error::Result;
use crate::ui::format::{
    format_duration, format_memory, process_table_budget, truncate_path, truncate_string,
};
use clap::{Args, ValueEnum};
use comfy_table::presets::NOTHING;
use comfy_table::{Attribute, Cell, CellAlignment, Color, ContentArrangement, Table};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute, terminal,
};
use serde::Serialize;
use std::collections::HashSet;
use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};

/// A refresh interval must be a real number of seconds, and not so small that
/// watch spins in a busy loop (0, NaN, infinity and negatives used to panic or spin)
fn parse_interval(s: &str) -> std::result::Result<f64, String> {
    let secs: f64 = s
        .parse()
        .map_err(|_| format!("'{s}' is not a number of seconds"))?;
    if !secs.is_finite() || secs < 0.1 {
        return Err(format!("must be at least 0.1 seconds, got '{s}'"));
    }
    Ok(secs)
}

/// Watch processes in real-time
#[derive(Args, Debug)]
pub struct WatchCommand {
    /// Target(s): PID, :port, or name (separate arguments for several; omit for all processes)
    #[arg(num_args = 0.., value_parser = crate::core::validate_target_arg)]
    pub target: Vec<String>,

    /// Refresh interval in seconds (at least 0.1)
    #[arg(long = "interval", short = 'n', default_value = "2", value_parser = parse_interval)]
    pub interval: f64,

    /// Filter by directory; takes the next word as the directory unless it starts with '-'
    /// (bare --in is the current directory, --in=DIR is explicit)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,

    /// Match name targets and --by as a substring of the process name or full command line (like pkill -f)
    #[arg(long)]
    pub cmd: bool,

    /// Only show processes using more than this CPU %
    #[arg(long)]
    pub min_cpu: Option<f32>,

    /// Only show processes using more than this memory (MB)
    #[arg(long)]
    pub min_mem: Option<f64>,

    /// Output as JSON (newline-delimited, one object per refresh)
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Print full directories and arguments instead of fitting the terminal width
    #[arg(long, short = 'w')]
    pub wide: bool,

    /// Limit the number of results shown
    #[arg(long, short = 'l')]
    pub limit: Option<usize>,

    /// Sort by: cpu, mem, pid, name
    #[arg(long, short = 's', value_enum, default_value_t = WatchSortKey::Cpu)]
    pub sort: WatchSortKey,
}

/// Sort key for watch command (mirrors SortKey but with independent default)
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum WatchSortKey {
    /// Sort by CPU usage (descending)
    Cpu,
    /// Sort by memory usage (descending)
    Mem,
    /// Sort by process ID (ascending)
    Pid,
    /// Sort by process name (ascending)
    Name,
}

impl From<WatchSortKey> for SortKey {
    fn from(key: WatchSortKey) -> Self {
        match key {
            WatchSortKey::Cpu => SortKey::Cpu,
            WatchSortKey::Mem => SortKey::Mem,
            WatchSortKey::Pid => SortKey::Pid,
            WatchSortKey::Name => SortKey::Name,
        }
    }
}

#[derive(Serialize)]
struct WatchJsonOutput {
    action: &'static str,
    success: bool,
    count: usize,
    processes: Vec<Process>,
}

/// The PID of the process listening on `port`, if there is one
fn port_owner(port: u16) -> Option<u32> {
    crate::core::PortInfo::find_by_port(port)
        .ok()
        .flatten()
        .map(|info| info.pid)
}

/// Put the terminal back the way it was: raw mode off, cursor shown, normal screen
fn restore_terminal(out: &mut impl Write) {
    let _ = terminal::disable_raw_mode();
    let _ = execute!(out, cursor::Show, terminal::LeaveAlternateScreen);
}

/// Restore the terminal if watch panics, so the shell is not left in raw mode
/// on the alternate screen, then run the hook that was there before
fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal(&mut io::stdout());
        original_hook(panic_info);
    }));
}

/// Whether a key press ends the watch: q, Esc or Ctrl+C (raw mode turns Ctrl+C
/// into a key instead of a signal)
fn is_quit_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => true,
        KeyCode::Char('c') => modifiers.contains(KeyModifiers::CONTROL),
        _ => false,
    }
}

/// Write one NDJSON line and flush it. Returns false once the reader is gone
/// (a broken pipe), which ends the stream; a frame that cannot be serialized is
/// reported on `err` and skipped.
fn write_json_frame(out: &mut impl Write, err: &mut impl Write, frame: &impl Serialize) -> bool {
    match serde_json::to_string(frame) {
        Ok(json) => writeln!(out, "{}", json).is_ok() && out.flush().is_ok(),
        Err(e) => {
            let _ = writeln!(err, "JSON serialization error: {}", e);
            true
        }
    }
}

impl WatchCommand {
    /// Execute the watch command — real-time process monitoring.
    pub fn execute(&self) -> Result<()> {
        let is_tty = io::stdout().is_terminal();

        if self.json {
            self.run_json_loop()
        } else if is_tty {
            self.run_tui_loop()
        } else {
            // Non-TTY: single snapshot like `list`
            self.run_snapshot()
        }
    }

    /// Collect processes for one cycle using the persistent System instance
    fn collect_processes(&self, sys: &System) -> Vec<Process> {
        self.collect_with(sys, port_owner)
    }

    /// `collect_processes` with the port lookup passed in: the owner of a port
    /// is read at a different moment than `sys`, so it can be a process `sys`
    /// has not seen
    fn collect_with(&self, sys: &System, port_owner: impl Fn(u16) -> Option<u32>) -> Vec<Process> {
        let self_pid = Pid::from_u32(std::process::id());
        let in_dir_filter = resolve_in_dir(&self.in_dir);
        let mode = NameMatch::from_flag(self.cmd);

        // Parse targets once
        let targets: Vec<TargetType> = flatten_targets(&self.target)
            .iter()
            .map(|t| parse_target(t))
            .collect();

        let mut seen_pids = HashSet::new();
        let mut processes = Vec::new();

        if targets.is_empty() {
            // No target: all processes
            for (pid, proc) in sys.processes() {
                if *pid == self_pid || crate::core::is_thread(proc) {
                    continue;
                }
                if seen_pids.insert(pid.as_u32()) {
                    processes.push(Process::from_sysinfo(*pid, proc));
                }
            }
        } else {
            for target in &targets {
                match target {
                    TargetType::Port(port) => {
                        if let Some(owner) = port_owner(*port) {
                            let pid = Pid::from_u32(owner);
                            if let Some(proc) = sys.process(pid) {
                                if seen_pids.insert(owner) {
                                    processes.push(Process::from_sysinfo(pid, proc));
                                }
                            }
                        }
                    }
                    TargetType::Pid(pid) => {
                        let sysinfo_pid = Pid::from_u32(*pid);
                        if let Some(proc) = sys.process(sysinfo_pid) {
                            if seen_pids.insert(*pid) {
                                processes.push(Process::from_sysinfo(sysinfo_pid, proc));
                            }
                        }
                    }
                    TargetType::Name(name) => {
                        for (pid, proc) in sys.processes() {
                            if *pid == self_pid || crate::core::is_thread(proc) {
                                continue;
                            }
                            if classify_sysinfo(name, proc, mode) == Hit::Match
                                && seen_pids.insert(pid.as_u32())
                            {
                                processes.push(Process::from_sysinfo(*pid, proc));
                            }
                        }
                    }
                }
            }
        }

        // Apply --by filter
        if let Some(ref by_name) = self.by_name {
            processes.retain(|p| crate::core::matches_by_filter(p, by_name, mode));
        }

        // Apply --in filter
        if let Some(ref dir_path) = in_dir_filter {
            processes.retain(|p| {
                if let Some(ref proc_cwd) = p.cwd {
                    crate::core::filters::cwd_within(proc_cwd, dir_path)
                } else {
                    false
                }
            });
        }

        // Apply --min-cpu filter
        if let Some(min_cpu) = self.min_cpu {
            processes.retain(|p| p.cpu_percent >= min_cpu);
        }

        // Apply --min-mem filter
        if let Some(min_mem) = self.min_mem {
            processes.retain(|p| p.memory_mb >= min_mem);
        }

        // Sort
        sort_processes(&mut processes, self.sort.into());

        // Apply limit
        if let Some(limit) = self.limit {
            processes.truncate(limit);
        }

        processes
    }

    /// TUI loop: alternate screen + raw mode
    fn run_tui_loop(&self) -> Result<()> {
        let mut stdout = io::stdout();

        install_panic_hook();

        // Enter alternate screen + raw mode
        execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)
            .map_err(|e| crate::error::ProcError::SystemError(e.to_string()))?;
        terminal::enable_raw_mode()
            .map_err(|e| crate::error::ProcError::SystemError(e.to_string()))?;

        let mut sys = System::new_all();
        let interval = Duration::from_secs_f64(self.interval);

        // Initial refresh to get baseline CPU readings
        sys.refresh_all();
        std::thread::sleep(Duration::from_millis(250));

        let result = self.tui_event_loop(&mut sys, &mut stdout, interval);

        restore_terminal(&mut stdout);

        result
    }

    fn tui_event_loop(
        &self,
        sys: &mut System,
        stdout: &mut io::Stdout,
        interval: Duration,
    ) -> Result<()> {
        loop {
            sys.refresh_all();
            let processes = self.collect_processes(sys);

            // Get terminal size
            let (width, height) = terminal::size().unwrap_or((120, 40));

            // Build frame
            let frame = self.render_frame(&processes, width, height);

            // Write frame with \r\n line endings (required in raw mode)
            execute!(stdout, cursor::MoveTo(0, 0))
                .map_err(|e| crate::error::ProcError::SystemError(e.to_string()))?;
            // Clear screen
            execute!(stdout, terminal::Clear(terminal::ClearType::All))
                .map_err(|e| crate::error::ProcError::SystemError(e.to_string()))?;
            execute!(stdout, cursor::MoveTo(0, 0))
                .map_err(|e| crate::error::ProcError::SystemError(e.to_string()))?;

            for line in frame.lines() {
                write!(stdout, "{}\r\n", line)
                    .map_err(|e| crate::error::ProcError::SystemError(e.to_string()))?;
            }
            stdout
                .flush()
                .map_err(|e| crate::error::ProcError::SystemError(e.to_string()))?;

            // Wait for interval or keypress
            let deadline = Instant::now() + interval;
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }

                if event::poll(remaining)
                    .map_err(|e| crate::error::ProcError::SystemError(e.to_string()))?
                {
                    if let Event::Key(KeyEvent {
                        code, modifiers, ..
                    }) = event::read()
                        .map_err(|e| crate::error::ProcError::SystemError(e.to_string()))?
                    {
                        if is_quit_key(code, modifiers) {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    /// Render a frame for the TUI
    fn render_frame(&self, processes: &[Process], width: u16, height: u16) -> String {
        let sort_label = match self.sort {
            WatchSortKey::Cpu => "CPU",
            WatchSortKey::Mem => "Memory",
            WatchSortKey::Pid => "PID",
            WatchSortKey::Name => "Name",
        };

        let target_label = if self.target.is_empty() {
            "all".to_string()
        } else {
            self.target.join(" ")
        };

        // Header line
        let header = format!(
            " Watching {} | {} processes | Sort: {} | Refresh: {:.1}s | q to exit",
            target_label,
            processes.len(),
            sort_label,
            self.interval,
        );

        let mut output = String::new();
        output.push_str(&header);
        output.push('\n');
        output.push('\n');

        if processes.is_empty() {
            output.push_str(" No matching processes found.");
            return output;
        }

        // Cap rows to terminal height minus header (2 lines) + table header (1) + padding (1)
        let max_rows = (height as usize).saturating_sub(5);

        if self.verbose {
            // Verbose mode: detailed per-process output
            for (i, proc) in processes.iter().enumerate() {
                if i >= max_rows {
                    output.push_str(&format!(" ... and {} more", processes.len() - i));
                    break;
                }
                let status_str = format!("{:?}", proc.status);
                output.push_str(&format!(
                    " {} {} [{}]  {:.1}% CPU  {}  {}",
                    proc.pid,
                    proc.name,
                    status_str,
                    proc.cpu_percent,
                    format_memory(proc.memory_mb),
                    proc.user.as_deref().unwrap_or("-")
                ));
                output.push('\n');
                if let Some(ref cmd) = proc.command {
                    output.push_str(&format!("    cmd: {}", cmd));
                    output.push('\n');
                }
                if let Some(ref path) = proc.exe_path {
                    output.push_str(&format!("    exe: {}", path));
                    output.push('\n');
                }
                if let Some(ref cwd) = proc.cwd {
                    output.push_str(&format!("    cwd: {}", cwd));
                    output.push('\n');
                }
                if let Some(uptime) = proc.uptime_secs {
                    output.push_str(&format!("    uptime: {}", format_duration(uptime)));
                    output.push('\n');
                }
                output.push('\n');
            }
        } else {
            // Table mode
            // Same budget as the other process table, so the two cannot drift.
            let budget = process_table_budget(width);
            let show_uptime = budget.show_uptime;

            let mut table = Table::new();
            // Disabled never reflows a cell, so one process is always one line.
            // Fitting the terminal is done by truncating content below, not by
            // letting comfy-table wrap.
            table
                .load_preset(NOTHING)
                .set_content_arrangement(ContentArrangement::Disabled);

            let mut header = vec![
                Cell::new("PID")
                    .fg(Color::Blue)
                    .add_attribute(Attribute::Bold),
                Cell::new("DIR")
                    .fg(Color::Blue)
                    .add_attribute(Attribute::Bold),
                Cell::new("NAME")
                    .fg(Color::Blue)
                    .add_attribute(Attribute::Bold),
                Cell::new("ARGS")
                    .fg(Color::Blue)
                    .add_attribute(Attribute::Bold),
                Cell::new("CPU%")
                    .fg(Color::Blue)
                    .add_attribute(Attribute::Bold)
                    .set_alignment(CellAlignment::Right),
                Cell::new("MEM")
                    .fg(Color::Blue)
                    .add_attribute(Attribute::Bold)
                    .set_alignment(CellAlignment::Right),
            ];
            if show_uptime {
                header.push(
                    Cell::new("UPTIME")
                        .fg(Color::Blue)
                        .add_attribute(Attribute::Bold)
                        .set_alignment(CellAlignment::Right),
                );
            }
            header.push(
                Cell::new("STATUS")
                    .fg(Color::Blue)
                    .add_attribute(Attribute::Bold)
                    .set_alignment(CellAlignment::Right),
            );
            table.set_header(header);

            if !self.wide {
                use comfy_table::ColumnConstraint::*;
                use comfy_table::Width::*;
                table
                    .column_mut(0)
                    .expect("PID column")
                    .set_constraint(Absolute(Fixed(budget.pid as u16)));
                table
                    .column_mut(4)
                    .expect("CPU% column")
                    .set_constraint(Absolute(Fixed(budget.cpu as u16)));
                table
                    .column_mut(5)
                    .expect("MEM column")
                    .set_constraint(Absolute(Fixed(budget.mem as u16)));
                let status_column = if show_uptime {
                    table
                        .column_mut(6)
                        .expect("UPTIME column")
                        .set_constraint(Absolute(Fixed(budget.uptime as u16)));
                    7
                } else {
                    6
                };
                table
                    .column_mut(status_column)
                    .expect("STATUS column")
                    .set_constraint(Absolute(Fixed(budget.status as u16)));
            }

            let display_count = processes.len().min(max_rows);
            for proc in processes.iter().take(display_count) {
                let status_str = format!("{:?}", proc.status);

                let cwd = proc.cwd.as_deref().unwrap_or("-");
                let path_display = if self.wide {
                    cwd.to_string()
                } else {
                    truncate_path(cwd, budget.dir)
                };
                let name_display = if self.wide {
                    proc.name.clone()
                } else {
                    truncate_string(&proc.name, budget.name)
                };

                let cmd_display = proc
                    .command
                    .as_ref()
                    .map(|c| {
                        let parts: Vec<&str> = c.split_whitespace().collect();
                        if parts.len() > 1 {
                            let args: Vec<String> = parts[1..]
                                .iter()
                                .map(|arg| {
                                    if arg.contains('/') && !arg.starts_with('-') {
                                        std::path::Path::new(arg)
                                            .file_name()
                                            .map(|f| f.to_string_lossy().to_string())
                                            .unwrap_or_else(|| arg.to_string())
                                    } else {
                                        arg.to_string()
                                    }
                                })
                                .collect();
                            let result = args.join(" ");
                            if self.wide {
                                result
                            } else {
                                truncate_string(&result, budget.args)
                            }
                        } else {
                            "-".to_string()
                        }
                    })
                    .unwrap_or_else(|| "-".to_string());

                let mem_display = format_memory(proc.memory_mb);

                let status_color = match proc.status {
                    ProcessStatus::Running => Color::Green,
                    ProcessStatus::Sleeping => Color::Blue,
                    ProcessStatus::Stopped => Color::Yellow,
                    ProcessStatus::Zombie => Color::Red,
                    _ => Color::White,
                };

                let mut row = vec![
                    Cell::new(proc.pid).fg(Color::Cyan),
                    Cell::new(&path_display).fg(Color::DarkGrey),
                    Cell::new(&name_display).fg(Color::White),
                    Cell::new(&cmd_display).fg(Color::DarkGrey),
                    Cell::new(format!("{:.1}", proc.cpu_percent))
                        .set_alignment(CellAlignment::Right),
                    Cell::new(&mem_display).set_alignment(CellAlignment::Right),
                ];
                if show_uptime {
                    let uptime_display = proc
                        .uptime_secs
                        .map(format_duration)
                        .unwrap_or_else(|| "-".to_string());
                    row.push(Cell::new(uptime_display).set_alignment(CellAlignment::Right));
                }
                row.push(
                    Cell::new(&status_str)
                        .fg(status_color)
                        .set_alignment(CellAlignment::Right),
                );
                table.add_row(row);
            }

            output.push_str(&table.to_string());

            if processes.len() > display_count {
                output.push('\n');
                output.push_str(&format!(
                    " ... and {} more (use --limit to control)",
                    processes.len() - display_count
                ));
            }
        }

        output
    }

    /// JSON loop: NDJSON output, one line per refresh
    fn run_json_loop(&self) -> Result<()> {
        let mut sys = System::new_all();
        let interval = Duration::from_secs_f64(self.interval);

        // Initial refresh for baseline CPU
        sys.refresh_all();
        std::thread::sleep(Duration::from_millis(250));

        loop {
            sys.refresh_all();
            let processes = self.collect_processes(&sys);

            let output = WatchJsonOutput {
                action: "watch",
                success: true,
                count: processes.len(),
                processes,
            };

            if !write_json_frame(&mut io::stdout(), &mut io::stderr(), &output) {
                return Ok(()); // Broken pipe — exit cleanly
            }

            std::thread::sleep(interval);
        }
    }

    /// Single snapshot (non-TTY mode)
    fn run_snapshot(&self) -> Result<()> {
        let mut sys = System::new_all();
        sys.refresh_all();
        std::thread::sleep(Duration::from_millis(250));
        sys.refresh_all();

        let processes = self.collect_processes(&sys);

        let printer = crate::ui::Printer::from_flags(false, self.verbose);
        printer.print_processes(&processes);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FailsAfter;
    use serde::ser::Error as _;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn watch_args() -> WatchCommand {
        WatchCommand {
            target: Vec::new(),
            interval: 2.0,
            in_dir: None,
            by_name: None,
            cmd: false,
            min_cpu: None,
            min_mem: None,
            json: false,
            verbose: false,
            wide: false,
            limit: None,
            sort: WatchSortKey::Cpu,
        }
    }

    fn process(pid: u32, name: &str) -> Process {
        Process {
            pid,
            name: name.to_string(),
            exe_path: None,
            cwd: None,
            command: None,
            argv0: None,
            cpu_percent: 1.5,
            memory_mb: 12.0,
            status: ProcessStatus::Sleeping,
            user: None,
            parent_pid: None,
            start_time: None,
            uptime_secs: None,
        }
    }

    /// The text of a frame without colour codes, which comfy-table adds only
    /// when stdout is a terminal
    fn plain(frame: &str) -> String {
        let mut out = String::new();
        let mut chars = frame.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                for end in chars.by_ref() {
                    if end.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn colour_codes_are_stripped_from_a_frame() {
        assert_eq!(
            plain("\x1b[1;36m4242\x1b[0m  \x1b[32mRunning\x1b[0m"),
            "4242  Running"
        );
    }

    fn lines_with<'a>(frame: &'a str, needle: &str) -> Vec<&'a str> {
        frame.lines().filter(|l| l.contains(needle)).collect()
    }

    // ---- keys ----

    #[test]
    fn q_escape_and_ctrl_c_quit() {
        assert!(is_quit_key(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(is_quit_key(KeyCode::Esc, KeyModifiers::NONE));
        assert!(is_quit_key(KeyCode::Char('c'), KeyModifiers::CONTROL));
    }

    #[test]
    fn other_keys_do_not_quit() {
        assert!(!is_quit_key(KeyCode::Char('c'), KeyModifiers::NONE));
        assert!(!is_quit_key(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(!is_quit_key(KeyCode::Enter, KeyModifiers::CONTROL));
        assert!(!is_quit_key(KeyCode::Char('x'), KeyModifiers::CONTROL));
    }

    // ---- the terminal ----

    #[test]
    fn restoring_shows_the_cursor_and_leaves_the_alternate_screen() {
        let mut out = Vec::new();
        restore_terminal(&mut out);
        assert_eq!(String::from_utf8(out).unwrap(), "\x1b[?25h\x1b[?1049l");
    }

    #[test]
    fn a_panic_runs_the_hook_that_was_there_before() {
        let before = std::panic::take_hook();
        let ran = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&ran);
        std::panic::set_hook(Box::new(move |_| flag.store(true, Ordering::SeqCst)));

        install_panic_hook();
        let caught = std::panic::catch_unwind(|| panic!("watch crashed"));
        // Drop ours, put back what the test run had
        let _ = std::panic::take_hook();
        std::panic::set_hook(before);

        assert!(caught.is_err());
        assert!(
            ran.load(Ordering::SeqCst),
            "the earlier hook runs after the terminal is restored"
        );
    }

    // ---- the stream ----

    struct FlushFails;

    impl Write for FlushFails {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
    }

    struct CannotSerialize;

    impl Serialize for CannotSerialize {
        fn serialize<S: serde::Serializer>(&self, _: S) -> std::result::Result<S::Ok, S::Error> {
            Err(S::Error::custom("no frame today"))
        }
    }

    fn frame_of(processes: Vec<Process>) -> WatchJsonOutput {
        WatchJsonOutput {
            action: "watch",
            success: true,
            count: processes.len(),
            processes,
        }
    }

    #[test]
    fn a_frame_is_one_json_line() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert!(write_json_frame(
            &mut out,
            &mut err,
            &frame_of(vec![process(7, "sh")])
        ));
        let text = String::from_utf8(out).unwrap();
        assert!(text.ends_with('\n'));
        assert_eq!(text.matches('\n').count(), 1);
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["action"], "watch");
        assert_eq!(json["count"], 1);
        assert_eq!(json["processes"][0]["pid"], 7);
        assert!(err.is_empty());
    }

    #[test]
    fn a_closed_reader_ends_the_stream() {
        let mut err = Vec::new();
        let mut closed = FailsAfter {
            left: 0,
            written: Vec::new(),
        };
        assert!(!write_json_frame(&mut closed, &mut err, &frame_of(vec![])));
        assert!(err.is_empty());
    }

    #[test]
    fn a_reader_that_goes_away_during_the_flush_ends_the_stream() {
        let mut err = Vec::new();
        assert!(!write_json_frame(
            &mut FlushFails,
            &mut err,
            &frame_of(vec![])
        ));
        assert!(err.is_empty());
    }

    #[test]
    fn a_frame_that_cannot_be_serialized_is_reported_and_skipped() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert!(
            write_json_frame(&mut out, &mut err, &CannotSerialize),
            "the stream goes on"
        );
        assert!(out.is_empty());
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "JSON serialization error: no frame today\n"
        );
    }

    // ---- the sort keys ----

    #[test]
    fn each_sort_key_maps_to_the_shared_one() {
        assert!(matches!(SortKey::from(WatchSortKey::Cpu), SortKey::Cpu));
        assert!(matches!(SortKey::from(WatchSortKey::Mem), SortKey::Mem));
        assert!(matches!(SortKey::from(WatchSortKey::Pid), SortKey::Pid));
        assert!(matches!(SortKey::from(WatchSortKey::Name), SortKey::Name));
    }

    // ---- the frame ----

    #[test]
    fn the_header_names_the_target_sort_and_refresh() {
        let mut cmd = watch_args();
        cmd.target = vec![":3000".into()];
        cmd.interval = 0.5;
        for (sort, label) in [
            (WatchSortKey::Cpu, "Sort: CPU"),
            (WatchSortKey::Mem, "Sort: Memory"),
            (WatchSortKey::Pid, "Sort: PID"),
            (WatchSortKey::Name, "Sort: Name"),
        ] {
            cmd.sort = sort;
            let frame = cmd.render_frame(&[process(1, "a"), process(2, "b")], 120, 40);
            let first = frame.lines().next().unwrap();
            assert_eq!(
                first,
                format!(" Watching :3000 | 2 processes | {label} | Refresh: 0.5s | q to exit")
            );
        }
    }

    #[test]
    fn the_header_names_several_targets_as_typed_joined_by_spaces() {
        let mut cmd = watch_args();
        cmd.target = vec![":3000".into(), "node,python".into()];
        let frame = cmd.render_frame(&[process(1, "a")], 120, 40);
        assert!(
            frame.starts_with(" Watching :3000 node,python | 1 processes |"),
            "{frame}"
        );
    }

    #[test]
    fn with_no_target_the_header_says_all() {
        let frame = watch_args().render_frame(&[process(1, "a")], 120, 40);
        assert!(
            frame.starts_with(" Watching all | 1 processes |"),
            "{frame}"
        );
    }

    #[test]
    fn no_processes_says_so() {
        let frame = watch_args().render_frame(&[], 120, 40);
        assert_eq!(
            frame.lines().last().unwrap(),
            " No matching processes found."
        );
        assert!(frame.contains("0 processes"));
    }

    #[test]
    fn the_table_lists_each_process_with_its_columns() {
        let mut p = process(4242, "server");
        p.cwd = Some("/srv/app".into());
        p.command = Some("/usr/bin/server --port 80".into());
        p.uptime_secs = Some(3725);
        p.cpu_percent = 12.34;
        p.memory_mb = 256.0;
        let frame = plain(&watch_args().render_frame(&[p], 120, 40));
        let header = lines_with(&frame, "PID")[0];
        for column in [
            "PID", "DIR", "NAME", "ARGS", "CPU%", "MEM", "UPTIME", "STATUS",
        ] {
            assert!(header.contains(column), "{column} in {header:?}");
        }
        let row = lines_with(&frame, "4242")[0];
        for cell in [
            "/srv/app",
            "server",
            "--port 80",
            "12.3",
            "256.0MB",
            "1h 2m",
            "Sleeping",
        ] {
            assert!(row.contains(cell), "{cell} in {row:?}");
        }
    }

    #[test]
    fn a_narrow_terminal_drops_the_uptime_column() {
        let mut p = process(4242, "server");
        p.uptime_secs = Some(3725);
        let narrow = plain(&watch_args().render_frame(&[p.clone()], 99, 40));
        assert!(!narrow.contains("UPTIME"), "{narrow}");
        assert!(!narrow.contains("1h 2m"), "{narrow}");
        let wide = plain(&watch_args().render_frame(&[p], 100, 40));
        assert!(wide.contains("UPTIME"), "{wide}");
        assert!(wide.contains("1h 2m"), "{wide}");
    }

    #[test]
    fn an_unknown_uptime_is_a_dash() {
        let frame = plain(&watch_args().render_frame(&[process(4242, "server")], 120, 40));
        let row = lines_with(&frame, "4242")[0];
        // directory, args and uptime are all unknown
        assert_eq!(
            row.split_whitespace().collect::<Vec<_>>(),
            ["4242", "-", "server", "-", "1.5", "12.0MB", "-", "Sleeping"]
        );
    }

    #[test]
    fn arguments_show_file_names_not_paths() {
        let mut p = process(4242, "node");
        p.command = Some("/usr/bin/node /srv/app/server.js --flag ./cfg/x.json plain".into());
        let mut cmd = watch_args();
        cmd.wide = true;
        let frame = plain(&cmd.render_frame(&[p], 120, 40));
        let row = lines_with(&frame, "4242")[0];
        assert!(row.contains("server.js --flag x.json plain"), "{row:?}");
        assert!(!row.contains("/srv/app"), "{row:?}");
    }

    #[test]
    fn a_command_with_no_arguments_has_a_dash_for_args() {
        let mut p = process(4242, "tool");
        p.command = Some("/usr/bin/tool".into());
        let frame = plain(&watch_args().render_frame(&[p], 120, 40));
        let row = lines_with(&frame, "4242")[0];
        let after_name = row.split("tool").nth(1).unwrap();
        assert!(after_name.trim_start().starts_with('-'), "{row:?}");
    }

    #[test]
    fn long_text_is_cut_to_the_terminal_unless_wide() {
        let long_dir = format!("/{}/deep", "d".repeat(80));
        let long_name = "n".repeat(40);
        let long_arg = "a".repeat(80);
        let mut p = process(4242, &long_name);
        p.cwd = Some(long_dir.clone());
        p.command = Some(format!("tool {long_arg}"));

        let cut = plain(&watch_args().render_frame(&[p.clone()], 80, 40));
        let row = lines_with(&cut, "4242")[0];
        assert!(row.contains('…'), "{row:?}");
        assert!(!row.contains(&long_name), "{row:?}");
        assert!(!row.contains(&long_arg), "{row:?}");

        let mut wide = watch_args();
        wide.wide = true;
        let full = plain(&wide.render_frame(&[p], 80, 40));
        let row = lines_with(&full, "4242")[0];
        assert!(row.contains(&long_dir), "{row:?}");
        assert!(row.contains(&long_name), "{row:?}");
        assert!(row.contains(&long_arg), "{row:?}");
        assert!(!row.contains('…'), "{row:?}");
    }

    #[test]
    fn every_status_is_spelled_out() {
        for (status, word) in [
            (ProcessStatus::Running, "Running"),
            (ProcessStatus::Sleeping, "Sleeping"),
            (ProcessStatus::Stopped, "Stopped"),
            (ProcessStatus::Zombie, "Zombie"),
            (ProcessStatus::Dead, "Dead"),
            (ProcessStatus::Unknown, "Unknown"),
        ] {
            let mut p = process(4242, "x");
            p.status = status;
            let frame = plain(&watch_args().render_frame(&[p], 120, 40));
            assert!(lines_with(&frame, "4242")[0].contains(word), "{word}");
        }
    }

    #[test]
    fn rows_beyond_the_screen_are_counted_not_drawn() {
        let processes: Vec<Process> = (1..=10).map(|i| process(1000 + i, "p")).collect();
        // 12 rows leave 7 for processes
        let frame = plain(&watch_args().render_frame(&processes, 120, 12));
        assert_eq!(lines_with(&frame, " p ").len(), 7, "{frame}");
        assert_eq!(
            frame.lines().last().unwrap(),
            " ... and 3 more (use --limit to control)"
        );
        let all = plain(&watch_args().render_frame(&processes, 120, 40));
        assert!(!all.contains("more"), "{all}");
    }

    #[test]
    fn verbose_lists_every_field_of_each_process() {
        let mut p = process(4242, "server");
        p.status = ProcessStatus::Running;
        p.user = Some("zee".into());
        p.command = Some("/usr/bin/server --port 80".into());
        p.exe_path = Some("/usr/bin/server".into());
        p.cwd = Some("/srv/app".into());
        p.uptime_secs = Some(65);
        let mut cmd = watch_args();
        cmd.verbose = true;
        let frame = cmd.render_frame(&[p], 120, 40);
        let lines: Vec<&str> = frame.lines().collect();
        assert_eq!(
            lines[2..],
            [
                " 4242 server [Running]  1.5% CPU  12.0MB  zee",
                "    cmd: /usr/bin/server --port 80",
                "    exe: /usr/bin/server",
                "    cwd: /srv/app",
                "    uptime: 1m 5s",
                "",
            ]
        );
    }

    #[test]
    fn verbose_leaves_out_what_a_process_does_not_have() {
        let mut cmd = watch_args();
        cmd.verbose = true;
        let frame = cmd.render_frame(&[process(4242, "bare")], 120, 40);
        let lines: Vec<&str> = frame.lines().collect();
        assert_eq!(
            lines[2..],
            [" 4242 bare [Sleeping]  1.5% CPU  12.0MB  -", ""]
        );
    }

    #[test]
    fn verbose_counts_the_processes_that_do_not_fit() {
        let processes: Vec<Process> = (1..=5).map(|i| process(1000 + i, "p")).collect();
        let mut cmd = watch_args();
        cmd.verbose = true;
        // 7 rows leave 2 for processes
        let frame = cmd.render_frame(&processes, 120, 7);
        assert_eq!(lines_with(&frame, "[Sleeping]").len(), 2, "{frame}");
        assert!(frame.ends_with(" ... and 3 more"), "{frame:?}");
    }

    // ---- what is collected ----

    #[test]
    fn collecting_applies_the_sort_and_the_limit() {
        let mut sys = System::new_all();
        sys.refresh_all();
        let mut cmd = watch_args();
        cmd.sort = WatchSortKey::Pid;
        let all = cmd.collect_processes(&sys);
        assert!(all.len() > 1);
        assert!(all.windows(2).all(|w| w[0].pid <= w[1].pid));
        assert!(all.iter().all(|p| p.pid != std::process::id()));

        cmd.limit = Some(1);
        let first = cmd.collect_processes(&sys);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].pid, all[0].pid);
    }

    #[test]
    fn a_port_owner_the_snapshot_has_not_seen_is_left_out() {
        let mut sys = System::new_all();
        sys.refresh_all();
        let mut cmd = watch_args();
        cmd.target = vec![":8080".into()];
        // This test's own process is in the snapshot; PID 4294967294 is not
        let me = std::process::id();
        let known = cmd.collect_with(&sys, |port| (port == 8080).then_some(me));
        assert_eq!(known.iter().map(|p| p.pid).collect::<Vec<_>>(), [me]);
        let unseen = cmd.collect_with(&sys, |_| Some(u32::MAX - 1));
        assert!(unseen.is_empty(), "{unseen:?}");
        let nobody = cmd.collect_with(&sys, |_| None);
        assert!(nobody.is_empty(), "{nobody:?}");
    }

    #[test]
    fn the_owner_of_a_port_is_the_process_listening_on_it() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert_eq!(port_owner(port), Some(std::process::id()));
        drop(listener);
        assert_eq!(port_owner(port), None);
    }
}
