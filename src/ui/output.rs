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

/// Whether a command that changes processes should ask before acting.
///
/// The convention is the same in every output format: changing a process
/// needs a yes. `--yes` gives it up front (`Ok(false)`: go ahead without
/// asking); otherwise a person is asked (`Ok(true)`). `--json` only changes
/// how results are printed, so it never stands in for `--yes`, and without a
/// terminal there's nobody to ask: both are refused with exit 4 before
/// anything is touched. `--dry-run` never gets here; it previews without one.
pub fn must_confirm(action: &str, yes: bool, json: bool) -> Result<bool> {
    use std::io::IsTerminal;
    if yes {
        return Ok(false);
    }
    if json {
        return Err(ProcError::InvalidInput(format!(
            "{action} changes processes, so it needs --yes, with --json too \
             (or --dry-run to preview)"
        )));
    }
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return Err(ProcError::InvalidInput(format!(
            "{action} asks before changing processes, and there's no terminal \
             to ask in; pass --yes (or --dry-run to preview)"
        )));
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
        match self.format {
            OutputFormat::Human => {
                println!("{} {}", "✓".green().bold(), message.green());
            }
            OutputFormat::Json => {
                // JSON output handled separately
            }
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
        match self.format {
            OutputFormat::Human => {
                println!("{} {}", "⚠".yellow().bold(), message.yellow());
            }
            OutputFormat::Json => {
                // JSON output handled separately
            }
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
        if processes.is_empty() {
            let msg = match context {
                Some(ctx) => format!("No processes found {}", ctx),
                None => "No processes found".to_string(),
            };
            self.warning(&msg);
            return;
        }

        let context_str = context.map(|c| format!(" {}", c)).unwrap_or_default();
        println!(
            "{} Found {} process{}{}",
            "✓".green().bold(),
            processes.len().to_string().cyan().bold(),
            if processes.len() == 1 { "" } else { "es" },
            context_str.bright_black()
        );
        println!();

        self.print_process_rows(processes);
    }

    /// Like `print_processes_as`, for a caller that has printed its own headline
    /// (`⚠ Found 2 orphaned processes`): the generic `✓ Found 2 processes` line
    /// would repeat it, so only the table is printed.
    pub fn print_processes_under_headline(&self, action: &str, processes: &[Process]) {
        match self.format {
            OutputFormat::Human => {
                println!();
                self.print_process_rows(processes);
            }
            OutputFormat::Json => self.print_processes_as(action, processes, None),
        }
    }

    /// The process table (or the verbose list), without a headline
    fn print_process_rows(&self, processes: &[Process]) {
        if self.verbose {
            // Verbose: full details, nothing truncated
            for proc in processes {
                let status_str = format!("{:?}", proc.status);
                let status_colored = colorize_status(&proc.status, &status_str);

                println!(
                    "{} {} {}  {:.1}% CPU  {}  {}",
                    proc.pid.to_string().cyan().bold(),
                    proc.name.white().bold(),
                    format!("[{}]", status_colored).bright_black(),
                    proc.cpu_percent,
                    format_memory(proc.memory_mb),
                    proc.user.as_deref().unwrap_or("-").bright_black()
                );

                if let Some(ref cmd) = proc.command {
                    println!("    {} {}", "cmd:".bright_black(), cmd);
                }
                if let Some(ref path) = proc.exe_path {
                    println!("    {} {}", "exe:".bright_black(), path.bright_black());
                }
                if let Some(ref cwd) = proc.cwd {
                    println!("    {} {}", "cwd:".bright_black(), cwd.bright_black());
                }
                if let Some(ppid) = proc.parent_pid {
                    println!(
                        "    {} {}",
                        "parent:".bright_black(),
                        ppid.to_string().bright_black()
                    );
                }
                if let Some(uptime) = proc.uptime_secs {
                    println!(
                        "    {} {}",
                        "uptime:".bright_black(),
                        format_duration(uptime).bright_black()
                    );
                }
                println!();
            }
        } else {
            let width = terminal_width();
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

            println!("{table}");
        }
        println!();
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
        match serde_json::to_string_pretty(data) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("Failed to serialize JSON: {}", e),
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
        // Past tense for human display: "kill" → "Killed", "stop" → "Stopped"
        let past_tense = match verb {
            "kill" => "Killed".to_string(),
            "stop" => "Stopped".to_string(),
            "freeze" => "Frozen".to_string(),
            "thaw" => "Thawed".to_string(),
            _ => format!("{}{}ed", verb[..1].to_uppercase(), &verb[1..]),
        };

        match self.format {
            OutputFormat::Human => {
                if !succeeded.is_empty() {
                    println!(
                        "{} {} {} process{}",
                        "✓".green().bold(),
                        past_tense,
                        succeeded.len().to_string().cyan().bold(),
                        plural(succeeded.len())
                    );
                    for proc in succeeded {
                        println!(
                            "  {} {} [PID {}]",
                            "→".bright_black(),
                            proc.name.white(),
                            proc.pid.to_string().cyan()
                        );
                    }
                }
                if !failed.is_empty() {
                    println!(
                        "{} Failed to {} {} process{}",
                        "✗".red().bold(),
                        verb,
                        failed.len(),
                        plural(failed.len())
                    );
                    for (proc, err) in failed {
                        println!(
                            "  {} {} [PID {}]: {}",
                            "→".bright_black(),
                            proc.name.white(),
                            proc.pid.to_string().cyan(),
                            err.red()
                        );
                    }
                }
            }
            OutputFormat::Json => {
                self.print_json(&ActionOutput {
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
                });
            }
        }

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
        if !must_confirm(action, yes, json)? {
            return Ok(true);
        }
        match self.format {
            OutputFormat::Json => unreachable!("must_confirm refuses --json without --yes"),
            OutputFormat::Human => {
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
        }
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
        assert!(!must_confirm("kill", true, false).unwrap());
        assert!(!must_confirm("kill", true, true).unwrap());
        // --json without --yes is refused (exit 4), never treated as a yes
        let refused = must_confirm("kill", false, true).unwrap_err();
        assert!(matches!(refused, ProcError::InvalidInput(_)), "{refused}");
        assert!(refused.to_string().contains("--yes"));
    }
}
