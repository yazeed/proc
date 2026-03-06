//! `proc watch` - Real-time process monitoring
//!
//! Examples:
//!   proc watch                     # Watch all processes (alias: proc top)
//!   proc watch node                # Watch node processes
//!   proc watch :3000               # Watch process on port 3000
//!   proc watch --in . --by node    # Watch node processes in current directory
//!   proc watch -n 1 --sort mem     # 1s refresh, sorted by memory
//!   proc watch --json | head -2    # NDJSON output

use crate::core::{
    parse_target, resolve_in_dir, sort_processes, Process, ProcessStatus, SortKey, TargetType,
};
use crate::error::Result;
use crate::ui::format::{format_memory, truncate_string};
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
use std::path::PathBuf;
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};

/// Watch processes in real-time
#[derive(Args, Debug)]
pub struct WatchCommand {
    /// Target(s): PID, :port, or name (optional — omit for all processes)
    pub target: Option<String>,

    /// Refresh interval in seconds
    #[arg(long = "interval", short = 'n', default_value = "2")]
    pub interval: f64,

    /// Filter by directory
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,

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
    count: usize,
    processes: Vec<Process>,
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
        let self_pid = Pid::from_u32(std::process::id());
        let in_dir_filter = resolve_in_dir(&self.in_dir);

        // Parse targets once
        let targets: Vec<TargetType> = self
            .target
            .as_ref()
            .map(|t| {
                t.split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(parse_target)
                    .collect()
            })
            .unwrap_or_default();

        let mut seen_pids = HashSet::new();
        let mut processes = Vec::new();

        if targets.is_empty() {
            // No target: all processes
            for (pid, proc) in sys.processes() {
                if *pid == self_pid {
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
                        if let Ok(Some(port_info)) = crate::core::PortInfo::find_by_port(*port) {
                            let pid = Pid::from_u32(port_info.pid);
                            if let Some(proc) = sys.process(pid) {
                                if seen_pids.insert(port_info.pid) {
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
                        let pattern_lower = name.to_lowercase();
                        for (pid, proc) in sys.processes() {
                            if *pid == self_pid {
                                continue;
                            }
                            let proc_name = proc.name().to_string_lossy().to_string();
                            let cmd: String = proc
                                .cmd()
                                .iter()
                                .map(|s| s.to_string_lossy())
                                .collect::<Vec<_>>()
                                .join(" ");

                            if (proc_name.to_lowercase().contains(&pattern_lower)
                                || cmd.to_lowercase().contains(&pattern_lower))
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
            let pattern = by_name.to_lowercase();
            processes.retain(|p| {
                p.name.to_lowercase().contains(&pattern)
                    || p.command
                        .as_ref()
                        .map(|c| c.to_lowercase().contains(&pattern))
                        .unwrap_or(false)
            });
        }

        // Apply --in filter
        if let Some(ref dir_path) = in_dir_filter {
            processes.retain(|p| {
                if let Some(ref proc_cwd) = p.cwd {
                    PathBuf::from(proc_cwd).starts_with(dir_path)
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

        // Install panic hook to restore terminal on crash
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = terminal::disable_raw_mode();
            let _ = execute!(io::stdout(), cursor::Show, terminal::LeaveAlternateScreen);
            original_hook(panic_info);
        }));

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

        // Restore terminal
        let _ = terminal::disable_raw_mode();
        let _ = execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen);

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
                        match code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(())
                            }
                            _ => {}
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

        let target_label = self.target.as_deref().unwrap_or("all");

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
                output.push('\n');
            }
        } else {
            // Table mode
            let mut table = Table::new();
            table
                .load_preset(NOTHING)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_width(width);

            table.set_header(vec![
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
                Cell::new("STATUS")
                    .fg(Color::Blue)
                    .add_attribute(Attribute::Bold)
                    .set_alignment(CellAlignment::Right),
            ]);

            use comfy_table::ColumnConstraint::*;
            use comfy_table::Width::*;

            let args_max = (width / 2).max(30);

            table
                .column_mut(0)
                .expect("PID column")
                .set_constraint(Absolute(Fixed(8)));
            table
                .column_mut(1)
                .expect("DIR column")
                .set_constraint(LowerBoundary(Fixed(20)));
            table
                .column_mut(2)
                .expect("NAME column")
                .set_constraint(LowerBoundary(Fixed(10)));
            table
                .column_mut(3)
                .expect("ARGS column")
                .set_constraint(UpperBoundary(Fixed(args_max)));
            table
                .column_mut(4)
                .expect("CPU% column")
                .set_constraint(Absolute(Fixed(8)));
            table
                .column_mut(5)
                .expect("MEM column")
                .set_constraint(Absolute(Fixed(11)));
            table
                .column_mut(6)
                .expect("STATUS column")
                .set_constraint(Absolute(Fixed(12)));

            let display_count = processes.len().min(max_rows);
            for proc in processes.iter().take(display_count) {
                let status_str = format!("{:?}", proc.status);

                let path_display = proc.cwd.as_deref().unwrap_or("-").to_string();

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
                            if result.is_empty() {
                                "-".to_string()
                            } else {
                                truncate_string(&result, (args_max as usize).saturating_sub(2))
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

                table.add_row(vec![
                    Cell::new(proc.pid).fg(Color::Cyan),
                    Cell::new(&path_display).fg(Color::DarkGrey),
                    Cell::new(&proc.name).fg(Color::White),
                    Cell::new(&cmd_display).fg(Color::DarkGrey),
                    Cell::new(format!("{:.1}", proc.cpu_percent))
                        .set_alignment(CellAlignment::Right),
                    Cell::new(&mem_display).set_alignment(CellAlignment::Right),
                    Cell::new(&status_str)
                        .fg(status_color)
                        .set_alignment(CellAlignment::Right),
                ]);
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
                count: processes.len(),
                processes,
            };

            match serde_json::to_string(&output) {
                Ok(json) => {
                    // Use writeln to detect broken pipe
                    if writeln!(io::stdout(), "{}", json).is_err() {
                        return Ok(()); // Broken pipe — exit cleanly
                    }
                    if io::stdout().flush().is_err() {
                        return Ok(());
                    }
                }
                Err(e) => {
                    eprintln!("JSON serialization error: {}", e);
                }
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

        if self.json {
            let output = WatchJsonOutput {
                action: "watch",
                count: processes.len(),
                processes,
            };
            if let Ok(json) = serde_json::to_string_pretty(&output) {
                println!("{}", json);
            }
        } else {
            let printer = crate::ui::Printer::new(crate::ui::OutputFormat::Human, self.verbose);
            printer.print_processes(&processes);
        }

        Ok(())
    }
}
