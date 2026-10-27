//! Output formatting for proc CLI
//!
//! Provides colored terminal output and JSON formatting.

use crate::core::{PortInfo, Process};
use crate::error::{ExitCode, ProcError, Result};
use crate::ui::format::{
    colorize_status, format_duration, format_memory, plural, process_table_budget, truncate_path,
    truncate_string,
};
use colored::*;
use comfy_table::presets::NOTHING;
use comfy_table::{Attribute, Cell, CellAlignment, Color, ContentArrangement, Table};
use dialoguer::Confirm;
use serde::Serialize;
use std::io::Write;

/// Run a writer-based print against stdout. A failed write panics, as
/// `println!` does.
fn to_stdout(write: impl FnOnce(&mut std::io::Stdout) -> std::io::Result<()>) {
    write(&mut std::io::stdout()).expect("failed printing to stdout");
}

/// Whether a command that changes processes should ask before acting.
///
/// The convention is the same in every output format: changing a process
/// needs a yes. `--yes` gives it up front (`Ok(false)`: go ahead without
/// asking); otherwise a person is asked (`Ok(true)`). `--json` only changes
/// how results are printed, so it never stands in for `--yes`, and without a
/// terminal there's nobody to ask: both are refused with exit 4 before
/// anything is touched. `--dry-run` never gets here; it previews without one.
///
/// Both refusals carry `processes`, the ones the command would have changed,
/// so a caller reading the JSON error can see what a retry with `--yes` hits.
pub fn must_confirm(action: &str, yes: bool, json: bool, processes: &[Process]) -> Result<bool> {
    use std::io::IsTerminal;
    if yes {
        return Ok(false);
    }
    if json {
        return Err(ProcError::ConfirmationRequired {
            message: format!(
                "{action} changes processes, so it needs --yes, with --json too \
                 (or --dry-run to preview)"
            ),
            processes: processes.to_vec(),
        });
    }
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return Err(ProcError::ConfirmationRequired {
            message: format!(
                "{action} asks before changing processes, and there's no terminal \
                 to ask in; pass --yes (or --dry-run to preview)"
            ),
            processes: processes.to_vec(),
        });
    }
    Ok(true)
}

/// Output format selection
#[derive(Debug, Clone, Copy, Default)]
pub enum OutputFormat {
    /// Colored, human-readable terminal output
    #[default]
    Human,
    /// Machine-readable JSON output for scripting
    Json,
}

/// Main printer for CLI output
pub struct Printer {
    format: OutputFormat,
    verbose: bool,
    /// Print full values instead of truncating them to the terminal width.
    wide: bool,
}

/// Detect terminal width, falling back to 120 when stdout is not a TTY.
fn terminal_width() -> u16 {
    crossterm::terminal::size().map(|(w, _)| w).unwrap_or(120)
}

impl Printer {
    /// Creates a new printer with the specified format and verbosity.
    pub fn new(format: OutputFormat, verbose: bool) -> Self {
        Self {
            format,
            verbose,
            wide: false,
        }
    }

    /// Opt into untruncated output (`--wide`). Values print in full and rows
    /// may exceed the terminal width rather than being elided.
    pub fn wide(mut self, wide: bool) -> Self {
        self.wide = wide;
        self
    }

    /// Create a printer from common CLI flags.
    pub fn from_flags(json: bool, verbose: bool) -> Self {
        Self::new(
            if json {
                OutputFormat::Json
            } else {
                OutputFormat::Human
            },
            verbose,
        )
    }

    /// Print a success message (human only — use `print_empty_result` for JSON-safe success)
    pub fn success(&self, message: &str) {
        to_stdout(|out| self.write_success(out, message));
    }

    fn write_success(&self, out: &mut impl Write, message: &str) -> std::io::Result<()> {
        match self.format {
            OutputFormat::Human => writeln!(out, "{} {}", "✓".green().bold(), message.green()),
            // JSON output handled separately
            OutputFormat::Json => Ok(()),
        }
    }

    /// Print a "nothing found" success result with JSON support.
    ///
    /// In human mode, prints a green success message. In JSON mode, outputs
    /// structured JSON so scripts/LLMs always get parseable output.
    pub fn print_empty_result(&self, action: &str, message: &str) {
        match self.format {
            OutputFormat::Human => self.success(message),
            OutputFormat::Json => self.print_json(&EmptyResult {
                action,
                success: true,
                count: 0,
                message,
            }),
        }
    }

    /// Print an error message
    pub fn error(&self, message: &str) {
        match self.format {
            OutputFormat::Human => {
                eprintln!("{} {}", "✗".red().bold(), message.red());
            }
            OutputFormat::Json => {
                // JSON output handled separately
            }
        }
    }

    /// Print a warning message
    pub fn warning(&self, message: &str) {
        to_stdout(|out| self.write_warning(out, message));
    }

    fn write_warning(&self, out: &mut impl Write, message: &str) -> std::io::Result<()> {
        match self.format {
            OutputFormat::Human => writeln!(out, "{} {}", "⚠".yellow().bold(), message.yellow()),
            // JSON output handled separately
            OutputFormat::Json => Ok(()),
        }
    }

    /// Print a list of processes with action name and optional context.
    ///
    /// The `action` parameter sets the JSON `action` field (e.g. "list", "by", "stuck").
    pub fn print_processes_as(&self, action: &str, processes: &[Process], context: Option<&str>) {
        match self.format {
            OutputFormat::Human => self.print_processes_human(processes, context),
            OutputFormat::Json => self.print_json(&ProcessListOutput {
                action,
                success: true,
                count: processes.len(),
                processes,
            }),
        }
    }

    /// Print a list of processes with optional context (e.g., "in /path/to/dir").
    /// Uses "list" as the JSON action name.
    pub fn print_processes_with_context(&self, processes: &[Process], context: Option<&str>) {
        self.print_processes_as("list", processes, context)
    }

    /// Print a list of processes. Uses "list" as the JSON action name.
    pub fn print_processes(&self, processes: &[Process]) {
        self.print_processes_with_context(processes, None)
    }

    fn print_processes_human(&self, processes: &[Process], context: Option<&str>) {
        to_stdout(|out| self.write_processes_human(out, terminal_width(), processes, context));
    }

    /// `width` is the terminal width the table is fitted to.
    fn write_processes_human(
        &self,
        out: &mut impl Write,
        width: u16,
        processes: &[Process],
        context: Option<&str>,
    ) -> std::io::Result<()> {
        if processes.is_empty() {
            let msg = match context {
                Some(ctx) => format!("No processes found {}", ctx),
                None => "No processes found".to_string(),
            };
            return self.write_warning(out, &msg);
        }

        let context_str = context.map(|c| format!(" {}", c)).unwrap_or_default();
        writeln!(
            out,
            "{} Found {} process{}{}",
            "✓".green().bold(),
            processes.len().to_string().cyan().bold(),
            if processes.len() == 1 { "" } else { "es" },
            context_str.bright_black()
        )?;
        writeln!(out)?;

        self.write_process_rows(out, width, processes)
    }

    /// Like `print_processes_as`, for a caller that has printed its own headline
    /// (`⚠ Found 2 orphaned processes`): the generic `✓ Found 2 processes` line
    /// would repeat it, so only the table is printed.
    pub fn print_processes_under_headline(&self, action: &str, processes: &[Process]) {
        match self.format {
            OutputFormat::Human => to_stdout(|out| {
                writeln!(out)?;
                self.write_process_rows(out, terminal_width(), processes)
            }),
            OutputFormat::Json => self.print_processes_as(action, processes, None),
        }
    }

    /// The process table (or the verbose list), without a headline, fitted to
    /// a terminal `width` columns wide
    fn write_process_rows(
        &self,
        out: &mut impl Write,
        width: u16,
        processes: &[Process],
    ) -> std::io::Result<()> {
        if self.verbose {
            // Verbose: full details, nothing truncated
            for proc in processes {
                let status_str = format!("{:?}", proc.status);
                let status_colored = colorize_status(&proc.status, &status_str);

                writeln!(
                    out,
                    "{} {} {}  {:.1}% CPU  {}  {}",
                    proc.pid.to_string().cyan().bold(),
                    proc.name.white().bold(),
                    format!("[{}]", status_colored).bright_black(),
                    proc.cpu_percent,
                    format_memory(proc.memory_mb),
                    proc.user.as_deref().unwrap_or("-").bright_black()
                )?;

                if let Some(ref cmd) = proc.command {
                    writeln!(out, "    {} {}", "cmd:".bright_black(), cmd)?;
                }
                if let Some(ref path) = proc.exe_path {
                    writeln!(out, "    {} {}", "exe:".bright_black(), path.bright_black())?;
                }
                if let Some(ref cwd) = proc.cwd {
                    writeln!(out, "    {} {}", "cwd:".bright_black(), cwd.bright_black())?;
                }
                if let Some(ppid) = proc.parent_pid {
                    writeln!(
                        out,
                        "    {} {}",
                        "parent:".bright_black(),
                        ppid.to_string().bright_black()
                    )?;
                }
                if let Some(uptime) = proc.uptime_secs {
                    writeln!(
                        out,
                        "    {} {}",
                        "uptime:".bright_black(),
                        format_duration(uptime).bright_black()
                    )?;
                }
                writeln!(out)?;
            }
        } else {
            let budget = process_table_budget(width);
            let show_uptime = budget.show_uptime;
            let (dir_content, name_content, args_content) = (budget.dir, budget.name, budget.args);

            let mut table = Table::new();
            // Disabled never reflows a cell, so one process is always one line.
            // Fitting the terminal is handled by truncating content above, not
            // by letting comfy-table wrap — a wrapped cell breaks the column
            // alignment that makes the table readable in the first place.
            table
                .load_preset(NOTHING)
                .set_content_arrangement(ContentArrangement::Disabled);

            // Header
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

            // Constraints mirror the budget above. In --wide mode no constraint
            // is set at all, so every column sizes to its full content.
            if !self.wide {
                use comfy_table::ColumnConstraint::*;
                use comfy_table::Width::*;
                table
                    .column_mut(0)
                    .expect("PID column")
                    .set_constraint(Absolute(Fixed(budget.pid as u16))); // 6 content — fits "999999"
                table
                    .column_mut(4)
                    .expect("CPU% column")
                    .set_constraint(Absolute(Fixed(budget.cpu as u16))); // 6 content — fits "100.0"
                table
                    .column_mut(5)
                    .expect("MEM column")
                    .set_constraint(Absolute(Fixed(budget.mem as u16))); // 9 content — fits "9999.9MB"
                let status_column = if show_uptime {
                    table
                        .column_mut(6)
                        .expect("UPTIME column")
                        .set_constraint(Absolute(Fixed(budget.uptime as u16))); // 8 content
                    7
                } else {
                    6
                };
                table
                    .column_mut(status_column)
                    .expect("STATUS column")
                    .set_constraint(Absolute(Fixed(budget.status as u16))); // 10 content
            }

            for proc in processes {
                let status_str = format!("{:?}", proc.status);

                // Show working directory (where the process was started from).
                // Middle elision keeps the root and the tail that differs
                // between rows; truncating either end alone loses one of them.
                let cwd = proc.cwd.as_deref().unwrap_or("-");
                let path_display = if self.wide {
                    cwd.to_string()
                } else {
                    truncate_path(cwd, dir_content)
                };
                let name_display = if self.wide {
                    proc.name.clone()
                } else {
                    truncate_string(&proc.name, name_content)
                };

                // Show command args (skip executable, simplify paths to filenames)
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
                            } else if self.wide {
                                result
                            } else {
                                truncate_string(&result, args_content)
                            }
                        } else {
                            // No args beyond the executable itself
                            "-".to_string()
                        }
                    })
                    .unwrap_or_else(|| "-".to_string());

                let mem_display = format_memory(proc.memory_mb);

                let status_color = match proc.status {
                    crate::core::ProcessStatus::Running => Color::Green,
                    crate::core::ProcessStatus::Sleeping => Color::Blue,
                    crate::core::ProcessStatus::Stopped => Color::Yellow,
                    crate::core::ProcessStatus::Zombie => Color::Red,
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

            writeln!(out, "{table}")?;
        }
        writeln!(out)
    }

    /// Print port information
    pub fn print_ports(&self, ports: &[PortInfo]) {
        match self.format {
            OutputFormat::Human => self.print_ports_human(ports),
            OutputFormat::Json => self.print_json(&PortListOutput {
                action: "ports",
                success: true,
                count: ports.len(),
                ports,
            }),
        }
    }

    fn print_ports_human(&self, ports: &[PortInfo]) {
        if ports.is_empty() {
            self.warning("No listening ports found");
            return;
        }

        println!(
            "{} Found {} listening port{}",
            "✓".green().bold(),
            ports.len().to_string().cyan().bold(),
            if ports.len() == 1 { "" } else { "s" }
        );
        println!();

        let width = terminal_width();

        let mut table = Table::new();
        table
            .load_preset(NOTHING)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_width(width);

        table.set_header(vec![
            Cell::new("PORT")
                .fg(Color::Blue)
                .add_attribute(Attribute::Bold),
            Cell::new("PROTO")
                .fg(Color::Blue)
                .add_attribute(Attribute::Bold),
            Cell::new("PID")
                .fg(Color::Blue)
                .add_attribute(Attribute::Bold),
            Cell::new("PROCESS")
                .fg(Color::Blue)
                .add_attribute(Attribute::Bold),
            Cell::new("ADDRESS")
                .fg(Color::Blue)
                .add_attribute(Attribute::Bold),
        ]);

        use comfy_table::ColumnConstraint::*;
        use comfy_table::Width::*;
        table
            .column_mut(0)
            .expect("PORT column")
            .set_constraint(Absolute(Fixed(8)));
        table
            .column_mut(1)
            .expect("PROTO column")
            .set_constraint(Absolute(Fixed(6)));
        table
            .column_mut(2)
            .expect("PID column")
            .set_constraint(Absolute(Fixed(8)));
        table
            .column_mut(3)
            .expect("PROCESS column")
            .set_constraint(LowerBoundary(Fixed(12)));
        table
            .column_mut(4)
            .expect("ADDRESS column")
            .set_constraint(LowerBoundary(Fixed(10)));

        for port in ports {
            let addr = port.address.as_deref().unwrap_or("*");
            let proto = format!("{:?}", port.protocol).to_uppercase();

            table.add_row(vec![
                Cell::new(port.port).fg(Color::Cyan),
                Cell::new(&proto).fg(Color::White),
                Cell::new(port.pid).fg(Color::Cyan),
                Cell::new(truncate_string(&port.process_name, 19)).fg(Color::White),
                Cell::new(addr).fg(Color::DarkGrey),
            ]);
        }

        println!("{table}");
        println!();
    }

    /// Print a single port info (for `proc on :port`)
    pub fn print_port_info(&self, port_info: &PortInfo) {
        match self.format {
            OutputFormat::Human => {
                println!(
                    "{} Process on port {}:",
                    "✓".green().bold(),
                    port_info.port.to_string().cyan().bold()
                );
                println!();
                println!(
                    "  {} {}",
                    "Name:".bright_black(),
                    port_info.process_name.white().bold()
                );
                println!(
                    "  {} {}",
                    "PID:".bright_black(),
                    port_info.pid.to_string().cyan()
                );
                println!("  {} {:?}", "Protocol:".bright_black(), port_info.protocol);
                if let Some(ref addr) = port_info.address {
                    println!("  {} {}", "Address:".bright_black(), addr);
                }
                println!();
            }
            OutputFormat::Json => self.print_json(&SinglePortOutput {
                action: "on",
                success: true,
                port: port_info,
            }),
        }
    }

    /// Print JSON output for any serializable type
    pub fn print_json<T: Serialize>(&self, data: &T) {
        to_stdout(|out| self.write_json(out, &mut std::io::stderr(), data));
    }

    /// The JSON goes to `out`; a value that can't be serialized is reported on `err`.
    fn write_json<T: Serialize>(
        &self,
        out: &mut impl Write,
        err: &mut impl Write,
        data: &T,
    ) -> std::io::Result<()> {
        match serde_json::to_string_pretty(data) {
            Ok(json) => writeln!(out, "{}", json),
            Err(e) => writeln!(err, "Failed to serialize JSON: {}", e),
        }
    }

    /// Print action result (generalized for kill/stop/freeze/thaw).
    ///
    /// The `action` parameter is the command name (e.g. "kill", "stop", "freeze").
    /// It is used as-is in JSON output and as the verb for human display.
    ///
    /// Returns `Err(ProcError::Reported)` when any process failed, so the command
    /// exits non-zero without printing a second result.
    pub fn print_action_result(
        &self,
        action: &str,
        succeeded: &[Process],
        failed: &[(Process, String)],
    ) -> Result<()> {
        self.print_action_result_as(action, action, succeeded, failed)
    }

    /// Print action result where the JSON `action` differs from the verb shown
    /// to humans (e.g. `proc orphans --kill` reports action "orphans", verb "kill").
    pub fn print_action_result_as(
        &self,
        action: &str,
        verb: &str,
        succeeded: &[Process],
        failed: &[(Process, String)],
    ) -> Result<()> {
        self.write_action_result_as(&mut std::io::stdout(), action, verb, succeeded, failed)
    }

    fn write_action_result_as(
        &self,
        out: &mut impl Write,
        action: &str,
        verb: &str,
        succeeded: &[Process],
        failed: &[(Process, String)],
    ) -> Result<()> {
        // Past tense for human display: "kill" → "Killed", "stop" → "Stopped"
        let past_tense = match verb {
            "kill" => "Killed".to_string(),
            "stop" => "Stopped".to_string(),
            "freeze" => "Frozen".to_string(),
            "thaw" => "Thawed".to_string(),
            _ => format!("{}{}ed", verb[..1].to_uppercase(), &verb[1..]),
        };

        // A failed write panics, as `println!` does
        let printed = match self.format {
            OutputFormat::Human => {
                Self::write_action_lines(out, &past_tense, verb, succeeded, failed)
            }
            OutputFormat::Json => self.write_json(
                out,
                &mut std::io::stderr(),
                &ActionOutput {
                    action,
                    success: failed.is_empty(),
                    succeeded_count: succeeded.len(),
                    failed_count: failed.len(),
                    succeeded,
                    failed: &failed
                        .iter()
                        .map(|(p, e)| FailedAction {
                            process: p,
                            error: e,
                        })
                        .collect::<Vec<_>>(),
                },
            ),
        };
        printed.expect("failed printing to stdout");

        if failed.is_empty() {
            Ok(())
        } else {
            Err(ProcError::reported(
                ExitCode::GeneralError,
                format!(
                    "Failed to {} {} process{}",
                    verb,
                    failed.len(),
                    plural(failed.len())
                ),
            ))
        }
    }

    /// The human lines of an action result: what worked, then what failed
    fn write_action_lines(
        out: &mut impl Write,
        past_tense: &str,
        verb: &str,
        succeeded: &[Process],
        failed: &[(Process, String)],
    ) -> std::io::Result<()> {
        if !succeeded.is_empty() {
            writeln!(
                out,
                "{} {} {} process{}",
                "✓".green().bold(),
                past_tense,
                succeeded.len().to_string().cyan().bold(),
                plural(succeeded.len())
            )?;
            for proc in succeeded {
                writeln!(
                    out,
                    "  {} {} [PID {}]",
                    "→".bright_black(),
                    proc.name.white(),
                    proc.pid.to_string().cyan()
                )?;
            }
        }
        if !failed.is_empty() {
            writeln!(
                out,
                "{} Failed to {} {} process{}",
                "✗".red().bold(),
                verb,
                failed.len(),
                plural(failed.len())
            )?;
            for (proc, err) in failed {
                writeln!(
                    out,
                    "  {} {} [PID {}]: {}",
                    "→".bright_black(),
                    proc.name.white(),
                    proc.pid.to_string().cyan(),
                    err.red()
                )?;
            }
        }
        Ok(())
    }

    /// Print kill result (delegates to print_action_result)
    pub fn print_kill_result(
        &self,
        killed: &[Process],
        failed: &[(Process, String)],
    ) -> Result<()> {
        self.print_action_result("kill", killed, failed)
    }

    /// Print dry-run summary for a destructive command.
    ///
    /// The `action` is the command name (e.g. "kill", "stop", "freeze", "thaw"),
    /// used both as the JSON `action` and as the verb in the human summary.
    pub fn print_dry_run(&self, action: &str, processes: &[Process]) {
        self.print_dry_run_with(
            action,
            processes,
            &format!(
                "Dry run: would {} {} process{}",
                action,
                processes.len(),
                plural(processes.len())
            ),
        );
    }

    /// Print dry-run summary with a custom human message.
    ///
    /// JSON output is always `{"action", "success", "dry_run": true, "count", "processes"}`.
    pub fn print_dry_run_with(&self, action: &str, processes: &[Process], message: &str) {
        match self.format {
            OutputFormat::Human => {
                self.print_processes_human(processes, None);
                self.warning(message);
            }
            OutputFormat::Json => self.print_json(&DryRunOutput {
                action,
                success: true,
                dry_run: true,
                count: processes.len(),
                processes,
            }),
        }
    }

    /// Show confirmation prompt and return whether the user confirmed.
    ///
    /// Returns `Ok(true)` if confirmed or skipped (--yes), `Ok(false)` if the
    /// user declined (prints "Cancelled"), and an error when there's nobody to
    /// ask (see [`must_confirm`]).
    pub fn ask_confirm(&self, action: &str, processes: &[Process], yes: bool) -> Result<bool> {
        let json = matches!(self.format, OutputFormat::Json);
        if !must_confirm(action, yes, json, processes)? {
            return Ok(true);
        }
        // A person is being asked: `must_confirm` never gets here with --json
        self.print_confirmation(action, processes);
        let prompt = format!(
            "{}{} {} process{}?",
            action[..1].to_uppercase(),
            &action[1..],
            processes.len(),
            plural(processes.len())
        );
        let confirmed = Confirm::new()
            .with_prompt(prompt)
            .default(false)
            .interact()?;
        if !confirmed {
            self.warning("Cancelled");
        }
        Ok(confirmed)
    }

    /// Print a confirmation prompt showing processes about to be acted on
    pub fn print_confirmation(&self, action: &str, processes: &[Process]) {
        println!(
            "\n{} Found {} process{} to {}:\n",
            "⚠".yellow().bold(),
            processes.len().to_string().cyan().bold(),
            if processes.len() == 1 { "" } else { "es" },
            action
        );

        for proc in processes {
            println!(
                "  {} {} [PID {}] - CPU: {:.1}%, MEM: {}",
                "→".bright_black(),
                proc.name.white().bold(),
                proc.pid.to_string().cyan(),
                proc.cpu_percent,
                format_memory(proc.memory_mb)
            );
        }
        println!();
    }
}

// JSON output structures
#[derive(Serialize)]
struct EmptyResult<'a> {
    action: &'a str,
    success: bool,
    count: usize,
    message: &'a str,
}

#[derive(Serialize)]
struct ProcessListOutput<'a> {
    action: &'a str,
    success: bool,
    count: usize,
    processes: &'a [Process],
}

#[derive(Serialize)]
struct DryRunOutput<'a> {
    action: &'a str,
    success: bool,
    dry_run: bool,
    count: usize,
    processes: &'a [Process],
}

#[derive(Serialize)]
struct PortListOutput<'a> {
    action: &'static str,
    success: bool,
    count: usize,
    ports: &'a [PortInfo],
}

#[derive(Serialize)]
struct SinglePortOutput<'a> {
    action: &'static str,
    success: bool,
    port: &'a PortInfo,
}

#[derive(Serialize)]
struct ActionOutput<'a> {
    action: &'a str,
    success: bool,
    succeeded_count: usize,
    failed_count: usize,
    succeeded: &'a [Process],
    failed: &'a [FailedAction<'a>],
}

#[derive(Serialize)]
struct FailedAction<'a> {
    process: &'a Process,
    error: &'a str,
}

impl Default for Printer {
    fn default() -> Self {
        Self::new(OutputFormat::Human, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_never_stands_in_for_yes() {
        // --yes: go ahead without asking, in either output format
        assert!(!must_confirm("kill", true, false, &[]).unwrap());
        assert!(!must_confirm("kill", true, true, &[]).unwrap());
        // --json without --yes is refused (exit 4), never treated as a yes
        let refused = must_confirm("kill", false, true, &[]).unwrap_err();
        assert!(
            matches!(refused, ProcError::ConfirmationRequired { .. }),
            "{refused}"
        );
        assert!(refused.to_string().contains("--yes"));
    }

    use crate::core::ProcessStatus;

    /// What a writer-based print writes, with colours off
    fn written(print: impl FnOnce(&mut Vec<u8>) -> std::io::Result<()>) -> String {
        colored::control::set_override(false);
        let mut out = Vec::new();
        print(&mut out).unwrap();
        plain(&String::from_utf8(out).unwrap())
    }

    /// `text` without the escape sequences a table adds when run in a terminal
    fn plain(text: &str) -> String {
        regex::Regex::new("\x1b\\[[0-9;]*m")
            .unwrap()
            .replace_all(text, "")
            .into_owned()
    }

    fn process(pid: u32, name: &str) -> Process {
        Process {
            pid,
            name: name.to_string(),
            exe_path: None,
            cwd: None,
            command: None,
            cpu_percent: 0.0,
            memory_mb: 0.0,
            status: ProcessStatus::Running,
            user: None,
            parent_pid: None,
            start_time: None,
            uptime_secs: None,
        }
    }

    /// Output with every run of spaces collapsed, one entry per non-empty line,
    /// so a table row can be compared without its column padding
    fn rows(text: &str) -> Vec<String> {
        text.lines()
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|l| !l.is_empty())
            .collect()
    }

    fn human() -> Printer {
        Printer::new(OutputFormat::Human, false)
    }

    fn json() -> Printer {
        Printer::new(OutputFormat::Json, false)
    }

    #[test]
    fn success_and_warning_print_a_line_for_people_and_nothing_for_json() {
        assert_eq!(
            written(|w| human().write_success(w, "all done")),
            "✓ all done\n"
        );
        assert_eq!(
            written(|w| human().write_warning(w, "careful")),
            "⚠ careful\n"
        );
        assert_eq!(written(|w| json().write_success(w, "all done")), "");
        assert_eq!(written(|w| json().write_warning(w, "careful")), "");
    }

    #[test]
    fn no_processes_says_so_with_or_without_a_context() {
        let out = written(|w| human().write_processes_human(w, 120, &[], None));
        assert_eq!(out, "⚠ No processes found\n");
        let out = written(|w| human().write_processes_human(w, 120, &[], Some("in /srv")));
        assert_eq!(out, "⚠ No processes found in /srv\n");
    }

    #[test]
    fn the_headline_counts_processes_and_names_the_context() {
        let one = [process(42, "node")];
        let out = written(|w| human().write_processes_human(w, 120, &one, Some("in /a")));
        assert_eq!(out.lines().next(), Some("✓ Found 1 process in /a"));
        assert_eq!(out.lines().nth(1), Some(""));

        let two = [process(42, "node"), process(43, "deno")];
        let out = written(|w| human().write_processes_human(w, 120, &two, None));
        assert_eq!(out.lines().next(), Some("✓ Found 2 processes"));
    }

    #[test]
    fn the_table_lists_a_process_per_row_and_ends_with_a_blank_line() {
        let mut p = process(42, "node");
        p.cwd = Some("/a/b".into());
        p.command = Some("node /x/y/server.js --port 3000".into());
        p.cpu_percent = 1.25;
        p.memory_mb = 12.5;
        p.uptime_secs = Some(65);
        let out = written(|w| human().write_process_rows(w, 120, &[p]));
        assert_eq!(
            rows(&out),
            [
                "PID DIR NAME ARGS CPU% MEM UPTIME STATUS",
                "42 /a/b node server.js --port 3000 1.2 12.5MB 1m 5s Running"
            ]
        );
        assert!(out.ends_with("\n\n"), "{out:?}");
    }

    #[test]
    fn a_narrow_terminal_drops_the_uptime_column_and_cuts_the_args() {
        let mut p = process(42, "node");
        p.cwd = Some("/a/b".into());
        p.command = Some("node /x/y/server.js --port 3000".into());
        p.uptime_secs = Some(65);
        let out = written(|w| human().write_process_rows(w, 80, &[p.clone()]));
        assert_eq!(
            rows(&out),
            [
                "PID DIR NAME ARGS CPU% MEM STATUS",
                "42 /a/b node server.js -… 0.0 0KB Running"
            ]
        );
        // 100 columns is the first width with room for UPTIME
        let out = written(|w| human().write_process_rows(w, 99, &[p.clone()]));
        assert!(!out.contains("UPTIME"), "{out}");
        let out = written(|w| human().write_process_rows(w, 100, &[p]));
        assert!(out.contains("UPTIME"), "{out}");
    }

    #[test]
    fn wide_prints_every_value_in_full() {
        let mut p = process(42, "a-process-with-a-very-long-name");
        p.cwd = Some("/srv/some/deeply/nested/working/directory/of/the/app".into());
        p.command = Some("a-process /x/y/server.js --listen-on-all-interfaces-please 3000".into());

        let cut = written(|w| human().write_process_rows(w, 80, &[p.clone()]));
        assert!(cut.matches('…').count() >= 2, "{cut}");

        let wide = written(|w| human().wide(true).write_process_rows(w, 80, &[p]));
        assert_eq!(
            rows(&wide)[1],
            "42 /srv/some/deeply/nested/working/directory/of/the/app \
             a-process-with-a-very-long-name \
             server.js --listen-on-all-interfaces-please 3000 0.0 0KB Running"
        );
    }

    #[test]
    fn args_show_file_names_for_paths_and_dashes_when_there_are_none() {
        let args_of = |command: Option<&str>| {
            let mut p = process(42, "node");
            p.command = command.map(str::to_string);
            let out = written(|w| human().wide(true).write_process_rows(w, 120, &[p]));
            // PID DIR NAME ARGS ...: the fourth cell of the row
            rows(&out)[1]
                .split(' ')
                .skip(3)
                .take_while(|c| *c != "0.0")
                .collect::<Vec<_>>()
                .join(" ")
        };
        // Paths shrink to their file name, flags and plain words stay
        assert_eq!(
            args_of(Some(
                "/usr/bin/node /opt/app/server.js -c /etc/x.conf -p/q start"
            )),
            "server.js -c x.conf -p/q start"
        );
        // Paths with no file name stay as they are
        assert_eq!(args_of(Some("node / a/..")), "/ a/..");
        // The executable alone has no args; a process with no command line neither
        assert_eq!(args_of(Some("/usr/bin/node")), "-");
        assert_eq!(args_of(None), "-");
    }

    #[test]
    fn a_missing_directory_or_uptime_shows_a_dash() {
        let p = process(42, "node");
        let out = written(|w| human().write_process_rows(w, 120, &[p]));
        assert_eq!(rows(&out)[1], "42 - node - 0.0 0KB - Running");
    }

    #[test]
    fn the_table_names_every_status() {
        let statuses = [
            ProcessStatus::Running,
            ProcessStatus::Sleeping,
            ProcessStatus::Stopped,
            ProcessStatus::Zombie,
            ProcessStatus::Dead,
            ProcessStatus::Unknown,
        ];
        let list: Vec<Process> = statuses
            .iter()
            .enumerate()
            .map(|(i, status)| {
                let mut p = process(100 + i as u32, "node");
                p.status = *status;
                p
            })
            .collect();
        let table = written(|w| human().write_process_rows(w, 120, &list));
        let verbose =
            written(|w| Printer::new(OutputFormat::Human, true).write_process_rows(w, 120, &list));
        for (i, status) in statuses.iter().enumerate() {
            let name = format!("{status:?}");
            assert!(rows(&table)[i + 1].ends_with(&name), "{table}");
            assert!(verbose.contains(&format!("[{name}]")), "{verbose}");
        }
    }

    #[test]
    fn verbose_lists_every_known_detail_of_each_process() {
        let mut p = process(42, "node");
        p.cpu_percent = 2.5;
        p.memory_mb = 1500.0;
        p.user = Some("zee".into());
        p.command = Some("node server.js".into());
        p.exe_path = Some("/usr/bin/node".into());
        p.cwd = Some("/srv/app".into());
        p.parent_pid = Some(7);
        p.uptime_secs = Some(3700);
        let bare = process(43, "deno");
        let verbose = Printer::new(OutputFormat::Human, true);
        let out = written(|w| verbose.write_process_rows(w, 120, &[p, bare]));
        assert_eq!(
            out,
            "42 node [Running]  2.5% CPU  1.5GB  zee\n\
             \x20   cmd: node server.js\n\
             \x20   exe: /usr/bin/node\n\
             \x20   cwd: /srv/app\n\
             \x20   parent: 7\n\
             \x20   uptime: 1h 1m\n\
             \n\
             43 deno [Running]  0.0% CPU  0KB  -\n\
             \n\
             \n"
        );
    }

    #[test]
    fn an_action_result_lists_what_worked_and_what_failed() {
        let ok = [process(1, "a"), process(2, "b")];
        let out = written(|w| {
            human()
                .write_action_result_as(w, "kill", "kill", &ok, &[])
                .map_err(std::io::Error::other)
        });
        assert_eq!(out, "✓ Killed 2 processes\n  → a [PID 1]\n  → b [PID 2]\n");

        let one = [process(1, "a")];
        let out = written(|w| {
            human()
                .write_action_result_as(w, "stop", "stop", &one, &[])
                .map_err(std::io::Error::other)
        });
        assert_eq!(out, "✓ Stopped 1 process\n  → a [PID 1]\n");

        // Mixed: both blocks, then the error a command exits with
        let failed = [(process(3, "c"), "Permission denied".to_string())];
        let mut buf = Vec::new();
        let err = human()
            .write_action_result_as(&mut buf, "freeze", "freeze", &one, &failed)
            .unwrap_err();
        assert_eq!(
            plain(&String::from_utf8(buf).unwrap()),
            "✓ Frozen 1 process\n  → a [PID 1]\n\
             ✗ Failed to freeze 1 process\n  → c [PID 3]: Permission denied\n"
        );
        assert_eq!(err.to_string(), "Failed to freeze 1 process");
        assert_eq!(ExitCode::from(&err) as i32, 1);
    }

    #[test]
    fn only_failures_print_no_success_block_and_pluralize() {
        let failed = [
            (process(3, "c"), "gone".to_string()),
            (process(4, "d"), "busy".to_string()),
        ];
        let mut buf = Vec::new();
        let err = human()
            .write_action_result_as(&mut buf, "thaw", "thaw", &[], &failed)
            .unwrap_err();
        assert_eq!(
            plain(&String::from_utf8(buf).unwrap()),
            "✗ Failed to thaw 2 processes\n  → c [PID 3]: gone\n  → d [PID 4]: busy\n"
        );
        assert_eq!(err.to_string(), "Failed to thaw 2 processes");
    }

    #[test]
    fn a_verb_without_its_own_past_tense_gets_ed() {
        let one = [process(1, "a")];
        let out = written(|w| {
            human()
                .write_action_result_as(w, "restart", "restart", &one, &[])
                .map_err(std::io::Error::other)
        });
        assert!(out.starts_with("✓ Restarted 1 process\n"), "{out}");
    }

    #[test]
    fn json_action_results_carry_the_failures() {
        let ok = [process(1, "a")];
        let failed = [(process(3, "c"), "Permission denied".to_string())];
        let mut buf = Vec::new();
        let err = json()
            .write_action_result_as(&mut buf, "orphans", "kill", &ok, &failed)
            .unwrap_err();
        let doc: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(doc["action"], "orphans");
        assert_eq!(doc["success"], false);
        assert_eq!(doc["succeeded_count"], 1);
        assert_eq!(doc["failed_count"], 1);
        assert_eq!(doc["succeeded"][0]["pid"], 1);
        assert_eq!(doc["failed"][0]["process"]["pid"], 3);
        assert_eq!(doc["failed"][0]["error"], "Permission denied");
        assert_eq!(err.to_string(), "Failed to kill 1 process");
    }

    #[test]
    fn a_value_that_cannot_be_serialized_is_reported_on_stderr() {
        struct Broken;
        impl Serialize for Broken {
            fn serialize<S: serde::Serializer>(
                &self,
                _: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("no can do"))
            }
        }
        let (mut out, mut err) = (Vec::new(), Vec::new());
        json().write_json(&mut out, &mut err, &Broken).unwrap();
        assert!(out.is_empty());
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "Failed to serialize JSON: no can do\n"
        );

        let (mut out, mut err) = (Vec::new(), Vec::new());
        json()
            .write_json(&mut out, &mut err, &serde_json::json!({"a": 1}))
            .unwrap();
        assert!(err.is_empty());
        assert_eq!(String::from_utf8(out).unwrap(), "{\n  \"a\": 1\n}\n");
    }

    #[test]
    fn the_default_printer_is_plain_human_output() {
        let printer = Printer::default();
        assert!(matches!(printer.format, OutputFormat::Human));
        assert!(!printer.verbose);
        assert!(!printer.wide);
    }

    /// A writer that accepts `left` more bytes and then fails
    struct FailsAfter {
        left: usize,
        written: Vec<u8>,
    }

    impl Write for FailsAfter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.left == 0 {
                return Err(std::io::Error::other("disk full"));
            }
            let n = buf.len().min(self.left);
            self.written.extend_from_slice(&buf[..n]);
            self.left -= n;
            Ok(n)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Run `print` against a writer that fails after every possible number of
    /// bytes: it must write exactly what fits, in order, and hand back the
    /// writer's error instead of carrying on or panicking.
    fn assert_stops_at_a_failed_write(print: impl Fn(&mut FailsAfter) -> std::io::Result<()>) {
        let mut all = FailsAfter {
            left: usize::MAX,
            written: Vec::new(),
        };
        print(&mut all).unwrap();
        all.flush().unwrap();
        let full = all.written;
        assert!(!full.is_empty());
        for limit in 0..full.len() {
            let mut out = FailsAfter {
                left: limit,
                written: Vec::new(),
            };
            let err = print(&mut out).expect_err(&format!("limit {limit}"));
            assert_eq!(err.to_string(), "disk full", "limit {limit}");
            assert_eq!(out.written, full[..limit], "limit {limit}");
        }
    }

    #[test]
    fn a_failed_write_ends_the_table() {
        let mut p = process(42, "node");
        p.cwd = Some("/a/b".into());
        p.command = Some("node server.js".into());
        assert_stops_at_a_failed_write(|w| {
            human().write_processes_human(w, 120, &[p.clone()], Some("in /a"))
        });
        assert_stops_at_a_failed_write(|w| human().write_processes_human(w, 120, &[], None));
    }

    #[test]
    fn a_failed_write_ends_the_verbose_list() {
        let mut p = process(42, "node");
        p.command = Some("node server.js".into());
        p.exe_path = Some("/usr/bin/node".into());
        p.cwd = Some("/srv/app".into());
        p.parent_pid = Some(7);
        p.uptime_secs = Some(3700);
        let verbose = Printer::new(OutputFormat::Human, true);
        assert_stops_at_a_failed_write(|w| verbose.write_process_rows(w, 120, &[p.clone()]));
    }

    #[test]
    fn a_failed_write_ends_the_action_result() {
        let ok = [process(1, "a")];
        let failed = [(process(3, "c"), "Permission denied".to_string())];
        assert_stops_at_a_failed_write(|w| {
            Printer::write_action_lines(w, "Killed", "kill", &ok, &failed)
        });
        assert_stops_at_a_failed_write(|w| human().write_success(w, "done"));
        assert_stops_at_a_failed_write(|w| human().write_warning(w, "careful"));
    }
}
