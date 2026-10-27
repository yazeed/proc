//! `proc on` - Port/process lookup
//!
//! Usage:
//!   proc on :3000              # What process is on port 3000?
//!   proc on :3000,:8080        # What's on multiple ports?
//!   proc on 1234               # What ports is PID 1234 listening on?
//!   proc on node               # What ports are node processes listening on?
//!   proc on node --in .        # Node processes in cwd and their ports

use crate::core::{
    find_ports_for_pid, parse_target, parse_targets, resolve_in_dir, resolve_target, PortInfo,
    Process, TargetType,
};
use crate::error::{ExitCode, ProcError, Result};
use crate::ui::{format_duration, format_memory, Printer};
use clap::Args;
use colored::*;
use serde::Serialize;
use std::io::Write;

/// What println! would say if stdout were closed
const WRITE_FAILED: &str = "failed printing to stdout";

/// Show what's on a port, or what ports a process is on
#[derive(Args, Debug)]
pub struct OnCommand {
    /// Target(s): :port, PID, or process name (comma-separated for multiple)
    #[arg(value_parser = crate::core::validate_target_arg)]
    pub target: String,

    /// Filter by directory (defaults to current directory if no path given)
    #[arg(long = "in", short = 'i', num_args = 0..=1, default_missing_value = ".")]
    pub in_dir: Option<String>,

    /// Filter by process name
    #[arg(long = "by", short = 'b')]
    pub by_name: Option<String>,

    /// Output as JSON
    #[arg(long, short = 'j')]
    pub json: bool,

    /// Show verbose output (full command line)
    #[arg(long, short = 'v')]
    pub verbose: bool,
}

impl OnCommand {
    /// Executes the on command, performing bidirectional port/process lookup.
    pub fn execute(&self) -> Result<()> {
        let targets = parse_targets(&self.target);

        // For single target, use original behavior
        if targets.len() == 1 {
            return match parse_target(&targets[0]) {
                TargetType::Port(port) => self.show_process_on_port(port),
                TargetType::Pid(pid) => self.show_ports_for_pid(pid),
                TargetType::Name(name) => self.show_ports_for_name(&name),
            };
        }

        if self.json {
            return self.show_multi_json(&targets);
        }

        // Multi-target handling
        let mut not_found = Vec::new();

        for target in &targets {
            match parse_target(target) {
                TargetType::Port(port) => {
                    if let Err(e) = self.show_process_on_port(port) {
                        println!("{} Port {}: {}", "⚠".yellow(), port, e);
                        not_found.push(target.clone());
                    }
                }
                TargetType::Pid(pid) => {
                    if let Err(e) = self.show_ports_for_pid(pid) {
                        println!("{} PID {}: {}", "⚠".yellow(), pid, e);
                        not_found.push(target.clone());
                    }
                }
                TargetType::Name(ref name) => {
                    if let Err(e) = self.show_ports_for_name(name) {
                        println!("{} '{}': {}", "⚠".yellow(), name, e);
                        not_found.push(target.clone());
                    }
                }
            }
        }

        if not_found.len() == targets.len() {
            // Each target already printed its own warning
            return Err(ProcError::reported(
                ExitCode::NotFound,
                format!("No process found matching '{}'", self.target),
            ));
        }

        Ok(())
    }

    /// Multi-target JSON: one envelope with a result per matched process
    fn show_multi_json(&self, targets: &[String]) -> Result<()> {
        let mut results: Vec<TargetResult> = Vec::new();
        let mut not_found: Vec<String> = Vec::new();

        for target in targets {
            let found = match parse_target(target) {
                TargetType::Port(port) => self
                    .lookup_port(port)
                    .map(|(port_info, process)| port_hit(port_info, process)),
                TargetType::Pid(pid) => self.lookup_pid(pid).map(|found| vec![found]),
                TargetType::Name(ref name) => self.lookup_name(name),
            };

            match found {
                Ok(found) if !found.is_empty() => {
                    results.extend(found.into_iter().map(|(process, ports)| TargetResult {
                        target: target.clone(),
                        process,
                        ports,
                    }))
                }
                _ => not_found.push(target.clone()),
            }
        }

        if results.is_empty() {
            return Err(ProcError::ProcessNotFound(self.target.clone()));
        }

        Printer::from_flags(true, self.verbose).print_json(&MultiTargetOutput {
            action: "on",
            success: true,
            count: results.len(),
            results,
            not_found,
        });

        Ok(())
    }

    /// Check if process matches --in filter
    fn matches_in_filter(&self, proc: &Process) -> bool {
        if let Some(ref dir_path) = resolve_in_dir(&self.in_dir) {
            if let Some(ref proc_cwd) = proc.cwd {
                crate::core::filters::cwd_within(proc_cwd, dir_path)
            } else {
                false
            }
        } else {
            true
        }
    }

    /// Check if process matches --by filter
    fn matches_by_filter(&self, proc: &Process) -> bool {
        if let Some(ref name) = self.by_name {
            crate::core::matches_by_filter(proc, name)
        } else {
            true
        }
    }

    /// Check if process matches all filters (--in and --by)
    fn matches_filters(&self, proc: &Process) -> bool {
        self.matches_in_filter(proc) && self.matches_by_filter(proc)
    }

    /// Find the listener on a port and its process, applying --in/--by
    fn lookup_port(&self, port: u16) -> Result<(PortInfo, Option<Process>)> {
        let port_info = match PortInfo::find_by_port(port)? {
            Some(info) => info,
            None => return Err(ProcError::PortNotFound(port)),
        };

        let process = Process::find_by_pid(port_info.pid)?;
        self.filter_port_process(port, port_info, process)
    }

    /// Take the CPU reading of the process on a port and apply --in/--by to it.
    /// The process is `None` when it ended after its port was listed.
    fn filter_port_process(
        &self,
        port: u16,
        port_info: PortInfo,
        process: Option<Process>,
    ) -> Result<(PortInfo, Option<Process>)> {
        let Some(mut proc) = process else {
            return Ok((port_info, None));
        };
        // Real CPU reading for what's shown
        Process::measure_cpu(std::slice::from_mut(&mut proc));

        // Apply --in and --by filters if present
        if !self.matches_filters(&proc) {
            return Err(ProcError::ProcessNotFound(format!(
                "port {} (process not in specified directory)",
                port
            )));
        }

        Ok((port_info, Some(proc)))
    }

    /// Show what process is on a specific port
    fn show_process_on_port(&self, port: u16) -> Result<()> {
        let (port_info, process) = self.lookup_port(port)?;

        if self.json {
            let printer = Printer::from_flags(true, self.verbose);
            let output = PortLookupOutput {
                action: "on",
                query_type: "port_to_process",
                success: true,
                port: Some(port_info.port),
                protocol: Some(format!("{:?}", port_info.protocol).to_lowercase()),
                address: port_info.address.clone(),
                process: process.as_ref(),
                ports: None,
            };
            printer.print_json(&output);
        } else {
            self.print_process_on_port(&mut std::io::stdout(), &port_info, process.as_ref())
                .expect(WRITE_FAILED);
        }

        Ok(())
    }

    /// Find a process by PID and its listening ports, applying --in/--by
    fn lookup_pid(&self, pid: u32) -> Result<(Process, Vec<PortInfo>)> {
        let mut process = Process::find_by_pid(pid)?
            .ok_or_else(|| ProcError::ProcessNotFound(pid.to_string()))?;
        // Real CPU reading for what's shown
        Process::measure_cpu(std::slice::from_mut(&mut process));

        // Apply --in and --by filters if present
        if !self.matches_filters(&process) {
            return Err(ProcError::ProcessNotFound(format!(
                "PID {} (not in specified directory)",
                pid
            )));
        }

        let ports = find_ports_for_pid(pid)?;
        Ok((process, ports))
    }

    /// Show what ports a PID is listening on
    fn show_ports_for_pid(&self, pid: u32) -> Result<()> {
        let (process, ports) = self.lookup_pid(pid)?;

        if self.json {
            let printer = Printer::from_flags(true, self.verbose);
            let output = PortLookupOutput {
                action: "on",
                query_type: "process_to_ports",
                success: true,
                port: None,
                protocol: None,
                address: None,
                process: Some(&process),
                ports: Some(&ports),
            };
            printer.print_json(&output);
        } else {
            self.print_ports_for_process(&mut std::io::stdout(), &process, &ports)
                .expect(WRITE_FAILED);
        }

        Ok(())
    }

    /// Find processes by name and their listening ports, applying --in/--by
    fn lookup_name(&self, name: &str) -> Result<Vec<(Process, Vec<PortInfo>)>> {
        // A name that matches nothing is an error here, never an empty list
        let mut processes = resolve_target(name)?;

        // Apply --in and --by filters if present
        if self.in_dir.is_some() || self.by_name.is_some() {
            processes.retain(|p| self.matches_filters(p));
            if processes.is_empty() {
                return Err(ProcError::ProcessNotFound(format!(
                    "{} (no matches with specified filters)",
                    name
                )));
            }
        }

        // Real CPU readings for what's shown, one wait for all of them
        Process::measure_cpu(&mut processes);

        let mut all_results: Vec<(Process, Vec<PortInfo>)> = Vec::new();

        for proc in processes {
            let ports = find_ports_for_pid(proc.pid)?;
            all_results.push((proc, ports));
        }

        Ok(all_results)
    }

    /// Show what ports processes with a given name are listening on
    fn show_ports_for_name(&self, name: &str) -> Result<()> {
        let all_results = self.lookup_name(name)?;

        if self.json {
            let printer = Printer::from_flags(true, self.verbose);
            let results: Vec<_> = all_results
                .iter()
                .map(|(proc, ports)| ProcessPortsJson {
                    process: proc,
                    ports,
                })
                .collect();
            let output = MultiProcessPortsOutput {
                action: "on",
                success: true,
                count: results.len(),
                results: &results,
            };
            printer.print_json(&output);
        } else {
            for (proc, ports) in &all_results {
                self.print_ports_for_process(&mut std::io::stdout(), proc, ports)
                    .expect(WRITE_FAILED);
            }
        }

        Ok(())
    }

    fn print_process_on_port(
        &self,
        out: &mut impl Write,
        port_info: &PortInfo,
        process: Option<&Process>,
    ) -> std::io::Result<()> {
        writeln!(
            out,
            "{} Port {} is used by:",
            "✓".green().bold(),
            port_info.port.to_string().cyan().bold()
        )?;
        writeln!(out)?;

        writeln!(
            out,
            "  {} {} (PID {})",
            "Process:".bright_black(),
            port_info.process_name.white().bold(),
            port_info.pid.to_string().cyan()
        )?;

        if let Some(proc) = process {
            if let Some(ref cwd) = proc.cwd {
                writeln!(out, "  {} {}", "Directory:".bright_black(), cwd)?;
            }
            if let Some(ref path) = proc.exe_path {
                writeln!(out, "  {} {}", "Path:".bright_black(), path.bright_black())?;
            }
        }

        let addr = port_info.address.as_deref().unwrap_or("*");
        writeln!(
            out,
            "  {} {} on {}",
            "Listening:".bright_black(),
            format!("{:?}", port_info.protocol).to_uppercase(),
            addr
        )?;

        if let Some(proc) = process {
            writeln!(
                out,
                "  {} {:.1}% CPU, {}",
                "Resources:".bright_black(),
                proc.cpu_percent,
                format_memory(proc.memory_mb)
            )?;

            if let Some(uptime) = proc.uptime_secs {
                writeln!(
                    out,
                    "  {} {}",
                    "Uptime:".bright_black(),
                    format_duration(uptime)
                )?;
            }

            if self.verbose {
                if let Some(ref cmd) = proc.command {
                    writeln!(
                        out,
                        "  {} {}",
                        "Command:".bright_black(),
                        cmd.bright_black()
                    )?;
                }
            }
        }

        writeln!(out)
    }

    fn print_ports_for_process(
        &self,
        out: &mut impl Write,
        process: &Process,
        ports: &[PortInfo],
    ) -> std::io::Result<()> {
        writeln!(
            out,
            "{} {} (PID {}) is listening on:",
            "✓".green().bold(),
            process.name.white().bold(),
            process.pid.to_string().cyan().bold()
        )?;
        writeln!(out)?;

        if ports.is_empty() {
            writeln!(out, "  {} No listening ports", "ℹ".blue())?;
        } else {
            for port_info in ports {
                let addr = port_info.address.as_deref().unwrap_or("*");
                writeln!(
                    out,
                    "  {} :{} ({} on {})",
                    "→".bright_black(),
                    port_info.port.to_string().cyan(),
                    format!("{:?}", port_info.protocol).to_uppercase(),
                    addr
                )?;
            }
        }

        if process.cwd.is_some() || process.uptime_secs.is_some() {
            writeln!(out)?;
        }
        if let Some(ref cwd) = process.cwd {
            writeln!(out, "  {} {}", "Directory:".bright_black(), cwd)?;
        }
        if let Some(uptime) = process.uptime_secs {
            writeln!(
                out,
                "  {} {}",
                "Uptime:".bright_black(),
                format_duration(uptime)
            )?;
        }

        if self.verbose {
            if let Some(ref path) = process.exe_path {
                writeln!(out, "  {} {}", "Path:".bright_black(), path.bright_black())?;
            }
            if let Some(ref cmd) = process.command {
                writeln!(
                    out,
                    "  {} {}",
                    "Command:".bright_black(),
                    cmd.bright_black()
                )?;
            }
        }

        writeln!(out)
    }
}

/// What a port lookup contributes to the multi-target JSON: the process on the
/// port with that port, or nothing when the process ended after the port was listed
fn port_hit(port_info: PortInfo, process: Option<Process>) -> Vec<(Process, Vec<PortInfo>)> {
    match process {
        Some(process) => vec![(process, vec![port_info])],
        None => Vec::new(),
    }
}

#[derive(Serialize)]
struct PortLookupOutput<'a> {
    action: &'static str,
    query_type: &'static str,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process: Option<&'a Process>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ports: Option<&'a [PortInfo]>,
}

#[derive(Serialize)]
struct ProcessPortsJson<'a> {
    process: &'a Process,
    ports: &'a [PortInfo],
}

#[derive(Serialize)]
struct TargetResult {
    target: String,
    process: Process,
    ports: Vec<PortInfo>,
}

#[derive(Serialize)]
struct MultiTargetOutput {
    action: &'static str,
    success: bool,
    count: usize,
    results: Vec<TargetResult>,
    not_found: Vec<String>,
}

#[derive(Serialize)]
struct MultiProcessPortsOutput<'a> {
    action: &'static str,
    success: bool,
    count: usize,
    results: &'a Vec<ProcessPortsJson<'a>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ProcessStatus, Protocol};
    use crate::test_support::assert_stops_at_a_failed_write;

    fn command(in_dir: Option<&str>) -> OnCommand {
        OnCommand {
            target: "x".to_string(),
            in_dir: in_dir.map(String::from),
            by_name: None,
            json: false,
            verbose: false,
        }
    }

    /// A process record with no working directory, which some processes really
    /// are (a zombie has none on Linux)
    fn without_cwd() -> Process {
        Process {
            pid: 1,
            name: "x".to_string(),
            exe_path: None,
            cwd: None,
            command: None,
            cpu_percent: 0.0,
            memory_mb: 0.0,
            status: ProcessStatus::Sleeping,
            user: None,
            parent_pid: None,
            start_time: None,
            uptime_secs: None,
        }
    }

    fn listener() -> PortInfo {
        PortInfo {
            port: 4000,
            protocol: Protocol::Tcp,
            pid: 1,
            process_name: "x".to_string(),
            address: None,
        }
    }

    #[test]
    fn a_process_with_no_working_directory_fails_in() {
        assert!(!command(Some("/")).matches_in_filter(&without_cwd()));
        assert!(command(None).matches_in_filter(&without_cwd()));
    }

    #[test]
    fn a_port_whose_process_ended_has_no_process_to_filter() {
        let (info, process) = command(Some("/"))
            .filter_port_process(4000, listener(), None)
            .unwrap();
        assert_eq!(info.port, 4000);
        assert!(process.is_none());
    }

    #[test]
    fn a_port_whose_process_ended_adds_nothing_to_the_json_results() {
        assert!(port_hit(listener(), None).is_empty());
        assert_eq!(port_hit(listener(), Some(without_cwd())).len(), 1);
    }

    /// What a print function writes, with colours off
    fn printed(print: impl FnOnce(&mut Vec<u8>) -> std::io::Result<()>) -> String {
        colored::control::set_override(false);
        let mut out = Vec::new();
        print(&mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn a_port_without_a_process_still_prints() {
        // Neither the process nor its details: the address shows as `*`
        let mut cmd = command(None);
        cmd.verbose = true;
        let out = printed(|w| cmd.print_process_on_port(w, &listener(), None));
        assert_eq!(
            out,
            "✓ Port 4000 is used by:\n\n  Process: x (PID 1)\n  Listening: TCP on *\n\n"
        );
    }

    #[test]
    fn a_process_missing_its_details_prints_without_them() {
        let mut cmd = command(None);
        cmd.verbose = true;

        let out = printed(|w| cmd.print_process_on_port(w, &listener(), Some(&without_cwd())));
        assert!(out.contains("Listening: TCP on *\n"), "{out}");
        assert!(out.contains("Resources: 0.0% CPU"), "{out}");
        for label in ["Directory:", "Path:", "Uptime:", "Command:"] {
            assert!(!out.contains(label), "{label} printed for nothing: {out}");
        }

        let out = printed(|w| cmd.print_ports_for_process(w, &without_cwd(), &[listener()]));
        assert_eq!(
            out,
            "✓ x (PID 1) is listening on:\n\n  → :4000 (TCP on *)\n\n"
        );
    }

    /// A process with every detail the printers can show
    fn detailed() -> Process {
        let mut p = without_cwd();
        p.cwd = Some("/work/app".to_string());
        p.exe_path = Some("/usr/bin/x".to_string());
        p.command = Some("x --serve".to_string());
        p.uptime_secs = Some(90);
        p
    }

    #[test]
    fn a_failed_write_ends_the_port_lookup() {
        let mut cmd = command(None);
        cmd.verbose = true;
        let p = detailed();
        assert_stops_at_a_failed_write(|w| cmd.print_process_on_port(w, &listener(), Some(&p)));
        assert_stops_at_a_failed_write(|w| cmd.print_process_on_port(w, &listener(), None));
    }

    #[test]
    fn a_failed_write_ends_the_process_lookup() {
        let mut cmd = command(None);
        cmd.verbose = true;
        let p = detailed();
        assert_stops_at_a_failed_write(|w| cmd.print_ports_for_process(w, &p, &[listener()]));
        assert_stops_at_a_failed_write(|w| cmd.print_ports_for_process(w, &p, &[]));
    }
}
