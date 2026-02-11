//! Output formatting for proc CLI
//!
//! Provides colored terminal output and JSON formatting.

use crate::core::{PortInfo, Process};
use crate::ui::format::{colorize_status, truncate_path, truncate_string};
use colored::*;
use comfy_table::presets::NOTHING;
use comfy_table::{Attribute, Cell, CellAlignment, Color, ContentArrangement, Table};
use serde::Serialize;

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
}

/// Detect terminal width, falling back to 120 when stdout is not a TTY.
fn terminal_width() -> u16 {
    crossterm::terminal::size().map(|(w, _)| w).unwrap_or(120)
}

impl Printer {
    /// Creates a new printer with the specified format and verbosity.
    pub fn new(format: OutputFormat, verbose: bool) -> Self {
        Self { format, verbose }
    }

    /// Print a success message
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

    /// Print a list of processes with optional context (e.g., "in /path/to/dir")
    pub fn print_processes_with_context(&self, processes: &[Process], context: Option<&str>) {
        match self.format {
            OutputFormat::Human => self.print_processes_human(processes, context),
            OutputFormat::Json => self.print_json(&ProcessListOutput {
                action: "list",
                success: true,
                count: processes.len(),
                processes,
            }),
        }
    }

    /// Print a list of processes
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

        if self.verbose {
            // Verbose: full details, nothing truncated
            for proc in processes {
                let status_str = format!("{:?}", proc.status);
                let status_colored = colorize_status(&proc.status, &status_str);

                println!(
                    "{} {} {}  {:.1}% CPU  {:.1} MB  {}",
                    proc.pid.to_string().cyan().bold(),
                    proc.name.white().bold(),
                    format!("[{}]", status_colored).bright_black(),
                    proc.cpu_percent,
                    proc.memory_mb,
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
                println!();
            }
        } else {
            let width = terminal_width();

            let mut table = Table::new();
            table
                .load_preset(NOTHING)
                .set_content_arrangement(ContentArrangement::Dynamic)
                .set_width(width);

            // Header
            table.set_header(vec![
                Cell::new("PID")
                    .fg(Color::Blue)
                    .add_attribute(Attribute::Bold),
                Cell::new("PATH")
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

            // Set fixed-width columns and flexible ones
            use comfy_table::ColumnConstraint::*;
            use comfy_table::Width::*;
            table
                .column_mut(0)
                .expect("PID column")
                .set_constraint(Absolute(Fixed(7)));
            table
                .column_mut(1)
                .expect("PATH column")
                .set_constraint(LowerBoundary(Fixed(10)));
            table
                .column_mut(2)
                .expect("NAME column")
                .set_constraint(LowerBoundary(Fixed(10)));
            // ARGS column is flexible — gets remaining space
            table
                .column_mut(4)
                .expect("CPU% column")
                .set_constraint(Absolute(Fixed(6)));
            table
                .column_mut(5)
                .expect("MEM column")
                .set_constraint(Absolute(Fixed(9)));
            table
                .column_mut(6)
                .expect("STATUS column")
                .set_constraint(Absolute(Fixed(8)));

            for proc in processes {
                let status_str = format!("{:?}", proc.status);

                // Show directory of executable
                let path_display = proc
                    .exe_path
                    .as_ref()
                    .map(|p| {
                        std::path::Path::new(p)
                            .parent()
                            .map(|parent| truncate_path(&parent.to_string_lossy(), 19))
                            .unwrap_or_else(|| "-".to_string())
                    })
                    .unwrap_or_else(|| "-".to_string());

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
                            args.join(" ")
                        } else {
                            c.clone()
                        }
                    })
                    .unwrap_or_else(|| "-".to_string());

                let mem_display = format!("{:.1}MB", proc.memory_mb);

                let status_color = match proc.status {
                    crate::core::ProcessStatus::Running => Color::Green,
                    crate::core::ProcessStatus::Sleeping => Color::Blue,
                    crate::core::ProcessStatus::Stopped => Color::Yellow,
                    crate::core::ProcessStatus::Zombie => Color::Red,
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

    /// Print action result (generalized for kill/stop/unstick)
    pub fn print_action_result(
        &self,
        action: &str,
        succeeded: &[Process],
        failed: &[(Process, String)],
    ) {
        match self.format {
            OutputFormat::Human => {
                if !succeeded.is_empty() {
                    println!(
                        "{} {} {} process{}",
                        "✓".green().bold(),
                        action,
                        succeeded.len().to_string().cyan().bold(),
                        if succeeded.len() == 1 { "" } else { "es" }
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
                        action.to_lowercase(),
                        failed.len(),
                        if failed.len() == 1 { "" } else { "es" }
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
    }

    /// Print kill result (delegates to print_action_result for backwards compatibility)
    pub fn print_kill_result(&self, killed: &[Process], failed: &[(Process, String)]) {
        self.print_action_result("Killed", killed, failed);
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
                "  {} {} [PID {}] - CPU: {:.1}%, MEM: {:.1}MB",
                "→".bright_black(),
                proc.name.white().bold(),
                proc.pid.to_string().cyan(),
                proc.cpu_percent,
                proc.memory_mb
            );
        }
        println!();
    }
}

// JSON output structures
#[derive(Serialize)]
struct ProcessListOutput<'a> {
    action: &'static str,
    success: bool,
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
