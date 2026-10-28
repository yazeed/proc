//! Interactive behavior, through a real pseudo-terminal: the confirmation
//! prompts of every command that asks, and the `watch` screen.
//!
//! With no terminal on stdin a missing `--yes` is an error, so only a PTY
//! reaches the prompt. Every target is a process the test starts, named by PID,
//! port, or a fresh temp directory.
//!
//! Windows runs them through ConPTY. It rewrites what a program prints: it
//! draws with cursor movements instead of line breaks, may leave an unchanged
//! line out of a redraw, and answers nothing a program asks of the terminal
//! (conhost asks for the cursor position at start-up, and this harness
//! answers). So on Windows the output is read with its escape sequences taken
//! out (`plain`), a repeated frame is not waited for, and the cases that need
//! signals (`freeze`, `thaw`), a real orphan (Windows has no PID 1 to adopt
//! one), exact escape sequences, or a resize stay Unix only.

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use proc_cli::core::filters::strip_verbatim_prefix;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[cfg(unix)]
const LEAVE_ALT_SCREEN: &str = "\x1b[?1049l";

// ---- the pseudo-terminal harness ----

/// One step of a scripted session
#[derive(Clone, Copy, Debug)]
enum Step {
    /// Wait until this text has appeared in the output, after the previous match
    Expect(&'static str),
    /// Type this (raw bytes: "\r" is Enter, "\x03" is Ctrl+C, "\x1b" is Esc)
    Send(&'static str),
    /// Resize the terminal to this many columns and rows, as dragging the window does
    #[cfg_attr(not(unix), allow(dead_code))]
    Resize(u16, u16),
    /// Do nothing for this long. For a session that must go through some
    /// intervals with no key pressed; nothing is asserted about what happened
    /// in the meantime, so a slow machine only makes the wait longer than needed.
    #[cfg_attr(unix, allow(dead_code))]
    Pause(Duration),
}

/// What a session printed, and how the child ended
#[derive(Debug)]
struct PtyRun {
    output: String,
    exit_code: u32,
}

/// The whole session may take this long; then the child is killed and the test fails
const SESSION_LIMIT: Duration = Duration::from_secs(20);

/// Kills and reaps the child on every path out of a session, including a
/// failed assertion or a timeout
struct ChildGuard(Box<dyn portable_pty::Child + Send + Sync>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// What a terminal shows of `raw`, as text: on Windows, ConPTY's output with the
/// escape sequences taken out (a cursor move forward becomes spaces, a move to
/// another line becomes a line break, an incomplete sequence at the end is left
/// for the next read). On Unix the output is read as it is.
fn plain(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw).into_owned();
    if cfg!(unix) {
        return text;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '\x1b' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        match chars.get(i + 1) {
            // an escape at the very end: the rest has not arrived
            None => break,
            Some('[') => {
                let mut j = i + 2;
                while j < chars.len() && !('\x40'..='\x7e').contains(&chars[j]) {
                    j += 1;
                }
                if j == chars.len() {
                    break;
                }
                let params: String = chars[i + 2..j].iter().collect();
                match chars[j] {
                    'C' => {
                        let n: usize = params.parse().unwrap_or(1);
                        out.push_str(&" ".repeat(n));
                    }
                    'H' | 'd' | 'B' | 'E' => out.push('\n'),
                    _ => {}
                }
                i = j + 1;
            }
            Some(']') => {
                // a title or hyperlink, ended by BEL or ESC \
                let mut j = i + 2;
                while j < chars.len()
                    && chars[j] != '\u{7}'
                    && !(chars[j] == '\x1b' && chars.get(j + 1) == Some(&'\\'))
                {
                    j += 1;
                }
                if j >= chars.len() {
                    break;
                }
                i = if chars[j] == '\u{7}' { j + 1 } else { j + 2 };
            }
            Some(_) => i += 2,
        }
    }
    out
}

/// conhost asks the terminal for the cursor position (`ESC [ 6 n`) when a
/// session starts and waits for the answer; this is the answer, once for each
/// time it asked. Nothing on Unix asks.
#[cfg(windows)]
fn answer_cursor_queries(raw: &[u8], answered: &mut usize, writer: &mut dyn Write) {
    let asked = raw.windows(4).filter(|w| *w == b"\x1b[6n").count();
    while *answered < asked {
        writer
            .write_all(b"\x1b[1;1R")
            .expect("answer the cursor query");
        writer.flush().expect("flush the pty");
        *answered += 1;
    }
}

/// Run `proc <args>` with a pseudo-terminal as stdin, stdout and stderr, play
/// `script` against it, and return everything it printed and its exit code.
///
/// The terminal is read on its own thread, so a child that blocks or stops
/// printing cannot hang the test: `Expect` polls with a deadline. The whole
/// session is limited to [`SESSION_LIMIT`].
fn run_in_pty(args: &[&str], script: &[Step]) -> PtyRun {
    let deadline = Instant::now() + SESSION_LIMIT;
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open a pseudo-terminal");

    let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_proc"));
    cmd.args(args);
    cmd.env("TERM", "xterm-256color");
    cmd.env_remove("NO_COLOR");
    let child = pair
        .slave
        .spawn_command(cmd)
        .expect("spawn proc in the pty");
    // The child holds the only other end; with ours closed, the reader sees
    // end-of-file when the child is gone
    drop(pair.slave);
    let mut guard = ChildGuard(child);

    let mut writer = pair.master.take_writer().expect("pty writer");
    let mut reader = pair.master.try_clone_reader().expect("pty reader");
    let captured = Arc::new(Mutex::new(Vec::<u8>::new()));
    let sink = Arc::clone(&captured);
    let reading = std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        // Linux reports the closed terminal as an error (EIO), macOS as 0 bytes
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;
            }
            sink.lock().unwrap().extend_from_slice(&chunk[..n]);
        }
    });

    let text = || plain(&captured.lock().unwrap());
    #[cfg(windows)]
    let mut answered = 0;
    #[cfg(windows)]
    let mut answer = |writer: &mut Box<dyn Write + Send>| {
        answer_cursor_queries(&captured.lock().unwrap(), &mut answered, writer)
    };
    let mut cursor = 0;
    for step in script {
        match *step {
            Step::Send(keys) => {
                writer
                    .write_all(keys.as_bytes())
                    .expect("type into the pty");
                writer.flush().expect("flush the pty");
            }
            Step::Pause(span) => std::thread::sleep(span),
            Step::Resize(cols, rows) => {
                pair.master
                    .resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    })
                    .expect("resize the pty");
            }
            Step::Expect(wanted) => loop {
                #[cfg(windows)]
                answer(&mut writer);
                let seen = text();
                if let Some(at) = seen[cursor..].find(wanted) {
                    cursor += at + wanted.len();
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for {wanted:?}; proc printed:\n{seen:?}"
                );
                std::thread::sleep(Duration::from_millis(10));
            },
        }
    }

    let status = loop {
        #[cfg(windows)]
        answer(&mut writer);
        match guard.0.try_wait().expect("poll the child") {
            Some(status) => break status,
            None => {
                assert!(
                    Instant::now() < deadline,
                    "proc did not exit in {SESSION_LIMIT:?}; it printed:\n{:?}",
                    text()
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };
    // Whatever it printed last is still in the pipe: let the reader reach the end
    let drained = Instant::now() + Duration::from_secs(2);
    // ConPTY keeps the pipe open until the terminal is closed, so on Windows
    // the end of the output is a quiet spell, not an end of file
    let mut length = captured.lock().unwrap().len();
    let mut quiet_since = Instant::now();
    while !reading.is_finished() && Instant::now() < drained {
        std::thread::sleep(Duration::from_millis(5));
        let now = captured.lock().unwrap().len();
        if now != length {
            length = now;
            quiet_since = Instant::now();
        } else if cfg!(windows) && quiet_since.elapsed() > Duration::from_millis(400) {
            break;
        }
    }
    PtyRun {
        output: text(),
        exit_code: status.exit_code(),
    }
}

#[test]
fn harness_runs_a_script() {
    let run = run_in_pty(&["--version"], &[]);
    assert_eq!(run.exit_code, 0, "{}", run.output);
    assert!(run.output.contains("proc"), "{}", run.output);
}

// Exact escape sequences: ConPTY rewrites them, so Unix only
#[cfg(unix)]
#[test]
fn the_comma_notice_is_coloured_on_a_terminal() {
    // PIDs above every OS maximum: a dry run that finds nothing, exit 2
    let run = run_in_pty(&["kill", "99999999,99999998", "--dry-run"], &[]);
    assert_eq!(run.exit_code, 2, "{}", run.output);
    assert!(
        run.output
            .contains("\x1b[1;36mnote:\x1b[0m comma-separated targets are deprecated"),
        "{:?}",
        run.output
    );
}

// ---- fixtures ----

/// Held for as long as a busy process runs, so no more than one spins at a time
static BUSY: Mutex<()> = Mutex::new(());

/// A shell loop that uses a whole core and ends itself after two minutes, so a
/// test binary killed without running `Drop` cannot leave it spinning for long
#[cfg(unix)]
const SPIN: &str = "end=$(( $(date +%s) + 120 )); n=0; \
    while :; do n=$((n+1)); \
    if [ $((n % 50000)) -eq 0 ] && [ \"$(date +%s)\" -ge \"$end\" ]; then exit 0; fi; done";

/// A process the test owns, killed and reaped on drop whatever a case did to it
struct Owned {
    child: Child,
    port: Option<u16>,
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    /// Set for a busy process; released after `Drop` has killed it
    _busy: Option<MutexGuard<'static, ()>>,
}

fn fresh_dir() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    // A process reports its cwd with symlinks resolved (/var is /private/var
    // on macOS) and no Windows `\\?\` prefix, and `--in` compares paths literally
    let dir = tmp.path().canonicalize().unwrap();
    let dir = PathBuf::from(strip_verbatim_prefix(&dir.to_string_lossy()));
    (tmp, dir)
}

impl Owned {
    /// The listener example: owns a port, sleeps, ends on SIGTERM
    fn listener() -> Self {
        let (tmp, dir) = fresh_dir();
        let mut child = Command::new(listener_path())
            .current_dir(&dir)
            .stdout(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
            .expect("spawn the listener example");
        let mut line = String::new();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let read = stdout.read_line(&mut line);
        let port = read
            .ok()
            .and_then(|_| line.trim().strip_prefix("PORT "))
            .and_then(|p| p.parse().ok());
        if port.is_none() {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the listener printed {line:?} instead of `PORT <n>`");
        }
        Owned {
            child,
            port,
            _tmp: tmp,
            dir,
            _busy: None,
        }
    }

    /// A process spinning in a busy loop in a fresh directory: stuck by proc's
    /// measure (over half a core in two readings a second apart). A shell on
    /// Unix, the listener example on Windows. Only one spins at a time, and it
    /// ends itself after two minutes.
    fn busy() -> Self {
        let guard = BUSY.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let (tmp, dir) = fresh_dir();
        #[cfg(unix)]
        let mut command = {
            let mut command = Command::new("sh");
            command.args(["-c", SPIN]);
            command
        };
        #[cfg(windows)]
        let mut command = {
            let mut command = Command::new(listener_path());
            command.arg("--busy");
            command
        };
        let child = command
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn a busy process");
        let busy = Owned {
            child,
            port: None,
            _tmp: tmp,
            dir,
            _busy: Some(guard),
        };
        busy.wait_until_stuck();
        busy
    }

    /// Return once `proc stuck` lists this process. proc has no CPU reading for
    /// a process that has used no CPU yet, so one started this instant is not
    /// found until a later look; a command that runs before then would find
    /// nothing.
    fn wait_until_stuck(&self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let out = Command::new(env!("CARGO_BIN_EXE_proc"))
                .args(["stuck", "--timeout", "0", "--in", &self.dir(), "--json"])
                .output()
                .expect("run proc stuck");
            let json: serde_json::Value =
                serde_json::from_slice(&out.stdout).expect("proc stuck prints JSON");
            let listed = json["processes"].as_array().is_some_and(|items| {
                items
                    .iter()
                    .any(|p| p["pid"].as_u64() == Some(u64::from(self.child.id())))
            });
            if listed {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the busy shell was never seen as stuck: {json}"
            );
        }
    }

    fn pid(&self) -> String {
        self.child.id().to_string()
    }

    fn dir(&self) -> String {
        self.dir.to_string_lossy().into_owned()
    }

    fn port(&self) -> String {
        self.port.expect("a listener").to_string()
    }

    #[cfg(unix)]
    fn signal(&self, sig: i32) {
        // SAFETY: signalling a child this test started and has not reaped
        unsafe {
            libc::kill(self.child.id() as i32, sig);
        }
    }

    /// Whether the child is still there: not exited, and not a zombie either
    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Wait for the child to end (it is ours, so waiting also reaps it)
    fn ended_within(&mut self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            if !self.alive() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    /// The process state letters from `ps` ("T" while stopped), empty if gone
    #[cfg(unix)]
    fn state(&self) -> String {
        let out = Command::new("ps")
            .args(["-o", "stat=", "-p", &self.pid()])
            .output()
            .expect("run ps");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[cfg(unix)]
    fn wait_state(&self, stopped: bool, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            if self.state().starts_with('T') == stopped {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A process whose parent has already exited: a shell starts `sleep` in the
/// background and ends, so the init process (PID 1) adopts the sleeper. Unix
/// only: on Windows nothing adopts an orphan, and proc has none to find.
#[cfg(unix)]
struct Orphan {
    pid: i32,
    /// Set once the sleeper has been seen gone, so `drop` never signals the PID
    /// again: the kernel may have given it to some other process by then
    gone: std::cell::Cell<bool>,
    _tmp: tempfile::TempDir,
    dir: PathBuf,
}

#[cfg(unix)]
impl Orphan {
    fn start() -> Self {
        let (tmp, dir) = fresh_dir();
        let out = Command::new("sh")
            .args(["-c", "sleep 300 >/dev/null 2>&1 </dev/null & echo $!"])
            .current_dir(&dir)
            .stdin(Stdio::null())
            .output()
            .expect("spawn the orphan's parent");
        let pid: i32 = String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .expect("the shell prints the sleeper's pid");
        let orphan = Orphan {
            pid,
            gone: std::cell::Cell::new(false),
            _tmp: tmp,
            dir,
        };
        // Wait until the kernel has handed it to init
        let deadline = Instant::now() + Duration::from_secs(5);
        while orphan.ppid() != Some(1) {
            assert!(Instant::now() < deadline, "the sleeper was not reparented");
            std::thread::sleep(Duration::from_millis(50));
        }
        orphan
    }

    fn ppid(&self) -> Option<i32> {
        let out = Command::new("ps")
            .args(["-o", "ppid=", "-p", &self.pid.to_string()])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }

    fn dir(&self) -> String {
        self.dir.to_string_lossy().into_owned()
    }

    fn alive(&self) -> bool {
        // SAFETY: signal 0 only checks that the process exists
        let alive = unsafe { libc::kill(self.pid, 0) == 0 };
        if !alive {
            self.gone.set(true);
        }
        alive
    }

    /// The sleeper is not our child, so nobody reaps it but init: gone means
    /// signalling it fails (a zombie waiting on init is shown by `ps` as `Z`)
    fn gone_within(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        while Instant::now() < deadline {
            let out = Command::new("ps")
                .args(["-o", "stat=", "-p", &self.pid.to_string()])
                .output()
                .expect("run ps");
            let state = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if state.is_empty() || state.starts_with('Z') {
                self.gone.set(true);
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }
}

#[cfg(unix)]
impl Drop for Orphan {
    fn drop(&mut self) {
        if self.gone.get() {
            return;
        }
        // SAFETY: the pid is the sleeper this test started, and it has not been
        // seen gone, so the PID is still its own
        unsafe {
            libc::kill(self.pid, libc::SIGKILL);
        }
    }
}

/// `target/debug/examples/listener`, found relative to this test binary
/// (`target/debug/deps/terminal-<hash>`)
fn listener_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("examples");
    path.push(format!("listener{}", std::env::consts::EXE_SUFFIX));
    assert!(path.exists(), "build the example first: {}", path.display());
    path
}

// ---- prompt cases ----

const WAIT: Duration = Duration::from_secs(5);

/// Ask, answer `y`: the command acts
fn answers_y(args: &[&str], prompt: &'static str) -> PtyRun {
    run_in_pty(
        args,
        &[Step::Expect(prompt), Step::Send("y"), Step::Send("\r")],
    )
}

/// Ask, answer `n`: nothing happens, and the command says `declined`
fn answers_n(args: &[&str], prompt: &'static str, declined: &'static str) -> PtyRun {
    let run = run_in_pty(
        args,
        &[Step::Expect(prompt), Step::Send("n"), Step::Send("\r")],
    );
    assert!(run.output.contains(declined), "{}", run.output);
    run
}

/// Ask, press Enter alone: the default is no
fn answers_enter(args: &[&str], prompt: &'static str, declined: &'static str) -> PtyRun {
    let run = run_in_pty(args, &[Step::Expect(prompt), Step::Send("\r")]);
    assert!(run.output.contains(declined), "{}", run.output);
    run
}

fn assert_ok(run: &PtyRun) {
    assert_eq!(run.exit_code, 0, "{}", run.output);
}

#[test]
fn kill_asks_first() {
    let mut p = Owned::listener();
    let args = ["kill", &p.pid()];
    let run = answers_n(&args, "Kill 1 process", "Cancelled");
    assert_ok(&run);
    assert!(p.alive(), "answering n left it running");

    let run = answers_enter(&args, "Kill 1 process", "Cancelled");
    assert_ok(&run);
    assert!(p.alive(), "Enter alone left it running");

    let run = answers_y(&args, "Kill 1 process");
    assert_ok(&run);
    assert!(
        p.ended_within(WAIT),
        "answering y killed it: {}",
        run.output
    );
}

#[test]
fn stop_asks_first() {
    let mut p = Owned::listener();
    // Windows cannot ask a console program to close, so stop waits out the
    // timeout before it kills
    let args = ["stop", &p.pid(), "--timeout", "1"];
    let run = answers_n(&args, "Stop 1 process", "Cancelled");
    assert_ok(&run);
    assert!(p.alive());

    let run = answers_enter(&args, "Stop 1 process", "Cancelled");
    assert_ok(&run);
    assert!(p.alive());

    let run = answers_y(&args, "Stop 1 process");
    assert_ok(&run);
    assert!(
        p.ended_within(WAIT),
        "answering y stopped it: {}",
        run.output
    );
}

#[test]
fn free_asks_first() {
    let mut p = Owned::listener();
    let target = format!(":{}", p.port());
    let args = ["free", &target];
    let run = answers_n(&args, "Kill 1 process to free 1 port", "Aborted");
    assert_ok(&run);
    assert!(p.alive());

    let run = answers_enter(&args, "Kill 1 process to free 1 port", "Aborted");
    assert_ok(&run);
    assert!(p.alive());

    let run = answers_y(&args, "Kill 1 process to free 1 port");
    assert_ok(&run);
    assert!(p.ended_within(WAIT), "answering y freed it: {}", run.output);
}

#[cfg(unix)]
#[test]
fn freeze_asks_first() {
    let p = Owned::listener();
    let args = ["freeze", &p.pid()];
    let run = answers_n(&args, "Freeze 1 process", "Cancelled");
    assert_ok(&run);
    assert!(!p.state().starts_with('T'), "answering n left it running");

    let run = answers_enter(&args, "Freeze 1 process", "Cancelled");
    assert_ok(&run);
    assert!(!p.state().starts_with('T'));

    let run = answers_y(&args, "Freeze 1 process");
    assert_ok(&run);
    assert!(
        p.wait_state(true, WAIT),
        "answering y froze it ({}): {}",
        p.state(),
        run.output
    );
}

#[cfg(unix)]
#[test]
fn thaw_asks_first() {
    let p = Owned::listener();
    p.signal(libc::SIGSTOP);
    assert!(p.wait_state(true, WAIT), "the test froze its listener");
    let args = ["thaw", &p.pid()];
    let run = answers_n(&args, "Thaw 1 process", "Cancelled");
    assert_ok(&run);
    assert!(p.state().starts_with('T'), "answering n left it frozen");

    let run = answers_enter(&args, "Thaw 1 process", "Cancelled");
    assert_ok(&run);
    assert!(p.state().starts_with('T'));

    let run = answers_y(&args, "Thaw 1 process");
    assert_ok(&run);
    assert!(
        p.wait_state(false, WAIT),
        "answering y thawed it ({}): {}",
        p.state(),
        run.output
    );
}

#[test]
fn unstick_asks_first() {
    let mut p = Owned::busy();
    let dir = p.dir();
    let args = ["unstick", "--force", "--timeout", "0", "--in", &dir];
    let run = answers_n(&args, "Unstick 1 process", "Aborted");
    assert_ok(&run);
    assert!(p.alive());

    let run = answers_enter(&args, "Unstick 1 process", "Aborted");
    assert_ok(&run);
    assert!(p.alive());

    let run = answers_y(&args, "Unstick 1 process");
    assert_ok(&run);
    assert!(
        p.ended_within(WAIT),
        "answering y terminated it: {}",
        run.output
    );
}

#[test]
fn unstick_without_force_says_it_will_only_try_to_recover() {
    let mut p = Owned::busy();
    let dir = p.dir();
    let args = ["unstick", "--timeout", "0", "--in", &dir];
    let run = answers_n(&args, "Unstick 1 process", "Aborted");
    assert_ok(&run);
    assert!(
        run.output.contains("Will attempt recovery only"),
        "{}",
        run.output
    );
    assert!(p.alive());
}

#[test]
fn stuck_kill_asks_first() {
    let mut p = Owned::busy();
    let dir = p.dir();
    let args = ["stuck", "--kill", "--timeout", "0", "--in", &dir];
    let run = answers_n(&args, "Kill 1 stuck process", "Cancelled");
    assert_ok(&run);
    assert!(p.alive());

    let run = answers_enter(&args, "Kill 1 stuck process", "Cancelled");
    assert_ok(&run);
    assert!(p.alive());

    let run = answers_y(&args, "Kill 1 stuck process");
    assert_ok(&run);
    assert!(
        p.ended_within(WAIT),
        "answering y killed it: {}",
        run.output
    );
}

#[cfg(unix)]
#[test]
fn orphans_kill_asks_first() {
    let o = Orphan::start();
    let dir = o.dir();
    let args = ["orphans", "--kill", "--in", &dir];
    let run = answers_n(&args, "Kill 1 orphaned process", "Cancelled");
    assert_ok(&run);
    assert!(o.alive());

    let run = answers_enter(&args, "Kill 1 orphaned process", "Cancelled");
    assert_ok(&run);
    assert!(o.alive());

    let run = answers_y(&args, "Kill 1 orphaned process");
    assert_ok(&run);
    assert!(o.gone_within(WAIT), "answering y killed it: {}", run.output);
}

// ---- watch ----

/// Start `watch`, wait for its first screen, then send `keys`
fn watch_quits_on(keys: &'static str) {
    let p = Owned::listener();
    let pid = p.pid();
    let run = run_in_pty(
        &["watch", &pid],
        &[Step::Expect("Watching"), Step::Send(keys)],
    );
    assert_ok(&run);
    #[cfg(unix)]
    assert!(
        run.output.ends_with(LEAVE_ALT_SCREEN),
        "the screen is left last: {:?}",
        &run.output[run.output.len().saturating_sub(60)..]
    );
}

#[test]
fn watch_quits_on_q() {
    watch_quits_on("q");
}

// A lone Escape byte is a key only after a timeout in ConPTY, and Ctrl+C is a
// signal there unless the program has turned that off: Unix only
#[cfg(unix)]
#[test]
fn watch_quits_on_escape() {
    watch_quits_on("\x1b");
}

#[cfg(unix)]
#[test]
fn watch_quits_on_ctrl_c() {
    // A byte typed into raw mode, not a signal
    watch_quits_on("\x03");
}

#[test]
fn watch_shows_the_pid() {
    let p = Owned::listener();
    let pid = p.pid();
    let run = run_in_pty(
        &["watch", &pid],
        &[
            Step::Expect("Watching"),
            Step::Expect("listener"),
            Step::Send("q"),
        ],
    );
    assert_ok(&run);
    assert!(run.output.contains(&pid), "{}", run.output);
}

// ConPTY leaves a line that did not change out of a redraw, so a repeated
// frame is not there to wait for: Unix only
#[cfg(unix)]
#[test]
fn watch_redraws_every_interval() {
    let p = Owned::listener();
    let pid = p.pid();
    let run = run_in_pty(
        &["watch", &pid, "--interval", "0.2"],
        &[
            Step::Expect("Watching"),
            Step::Expect("Watching"),
            Step::Expect("Watching"),
            Step::Send("q"),
        ],
    );
    assert_ok(&run);
    assert!(run.output.contains("Refresh: 0.2s"), "{}", run.output);
    assert!(
        run.output.ends_with(LEAVE_ALT_SCREEN),
        "the screen is left last"
    );
}

#[cfg(unix)]
#[test]
fn watch_keeps_running_when_another_key_is_pressed() {
    let p = Owned::listener();
    let pid = p.pid();
    let run = run_in_pty(
        &["watch", &pid, "--interval", "0.2"],
        &[
            Step::Expect("Watching"),
            Step::Send("x\r"),
            // Another frame after the keys: it did not quit
            Step::Expect("Watching"),
            Step::Expect("Watching"),
            Step::Send("q"),
        ],
    );
    assert_ok(&run);
}

/// The same on Windows, where a second frame is not waited for: a key that is
/// not a quit key is read first, then `q` ends the session
#[cfg(windows)]
#[test]
fn watch_ignores_other_keys_and_quits_on_q() {
    let p = Owned::listener();
    let pid = p.pid();
    let run = run_in_pty(
        &["watch", &pid, "--interval", "0.2"],
        &[Step::Expect("Watching"), Step::Send("x"), Step::Send("q")],
    );
    assert_ok(&run);
}

/// With no key pressed the loop waits out each interval and draws again; a few
/// intervals go by here (on Windows, where a repeated frame is not waited for)
#[cfg(windows)]
#[test]
fn watch_goes_on_when_no_key_is_pressed() {
    let p = Owned::listener();
    let pid = p.pid();
    let run = run_in_pty(
        &["watch", &pid, "--interval", "0.2"],
        &[
            Step::Expect("Watching"),
            Step::Pause(Duration::from_millis(900)),
            Step::Send("q"),
        ],
    );
    assert_ok(&run);
}

#[cfg(unix)]
#[test]
fn watch_draws_the_new_size_after_a_resize() {
    let p = Owned::listener();
    let pid = p.pid();
    let run = run_in_pty(
        &["watch", &pid, "--interval", "0.2"],
        &[
            Step::Expect("Watching"),
            Step::Expect("listener"),
            // Five rows leave none for processes: the one process is counted instead
            Step::Resize(120, 5),
            Step::Expect(" ... and 1 more (use --limit to control)"),
            // Back to a normal size, narrow enough to drop the uptime column
            Step::Resize(80, 40),
            Step::Expect("listener"),
            Step::Send("q"),
        ],
    );
    assert_ok(&run);
    assert!(
        run.output.ends_with(LEAVE_ALT_SCREEN),
        "the screen is left last"
    );
}

#[test]
fn watch_says_when_nothing_matches() {
    let run = run_in_pty(
        &["watch", "99999999"],
        &[
            Step::Expect("No matching processes found."),
            Step::Send("q"),
        ],
    );
    assert_ok(&run);
    assert!(run.output.contains("0 processes"), "{}", run.output);
}

#[test]
fn watch_shows_the_sort_in_its_header() {
    let p = Owned::listener();
    let pid = p.pid();
    let run = run_in_pty(
        &["watch", &pid, "--sort", "mem"],
        &[Step::Expect("Sort: Memory"), Step::Send("q")],
    );
    assert_ok(&run);
}

#[test]
fn watch_verbose_lists_the_details() {
    let p = Owned::listener();
    let pid = p.pid();
    let dir = p.dir();
    let run = run_in_pty(
        &["watch", &pid, "--verbose"],
        &[
            Step::Expect("cwd: "),
            Step::Expect("uptime: "),
            Step::Send("q"),
        ],
    );
    assert_ok(&run);
    // Windows may report the directory in its 8.3 short form
    #[cfg(unix)]
    assert!(
        run.output.contains(&format!("cwd: {dir}")),
        "{}",
        run.output
    );
    #[cfg(windows)]
    let _ = dir;
    assert!(run.output.contains("cmd: "), "{}", run.output);
}
