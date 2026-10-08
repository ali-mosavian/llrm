//! DOSBox-X for the tests that drive DOS tools: a one-shot run of commands, and a session on the fork's debug
//! socket (`DOSBOX_DEBUG_PORT`, a private port) that types keys and reads the screen. The socket stops the
//! emulator at every INT 3, which debuggers use for their breakpoints, so a reader thread answers those with
//! `continue`; replies come back in the order commands went out, so each command's owner is queued.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The DOSBox-X this crate builds, beside the compiler binaries.
pub fn binary() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_llrm-c")).parent().unwrap().join("dosbox-x")
}

/// A test that cannot run says so on stderr, and fails where `require` names an environment variable that is set, so
/// a gate that has the tools cannot pass by skipping.
pub fn skipped(require: &str, reason: &str) {
    // Written to the stderr itself, which the harness does not capture: seen when the test passes.
    let _ = Write::write_all(&mut std::io::stderr(), format!("SKIPPED: {reason}\n").as_bytes());
    assert!(std::env::var_os(require).is_none(), "{require} is set, and: {reason}");
}

fn conf(mounts: &[(char, &Path)], path: &str, commands: &[String]) -> String {
    let mut conf = String::from("[sdl]\noutput=surface\n[dosbox]\nmemsize=32\nstartquiet=true\nstartbanner=false\nquit warning=false\n[cpu]\ncore=normal\ncputype=pentium\ncycles=max\n[dos]\nxms=true\n[mixer]\nnosound=true\n[autoexec]\n");
    for (letter, directory) in mounts {
        conf.push_str(&format!("mount {letter} {}\n", directory.display()));
    }
    conf.push_str(&format!("path z:\\;{path}\n"));
    for command in commands {
        conf.push_str(command);
        conf.push('\n');
    }
    conf
}

/// Runs `commands` in DOS with `mounts` (the last is the working drive) and exits.
pub fn run(mounts: &[(char, &Path)], path: &str, commands: &[String]) {
    let work = mounts.last().expect("a working drive");
    let mut lines = vec![format!("{}:", work.0)];
    lines.extend(commands.iter().cloned());
    lines.push("exit".into());
    let file = work.1.join("dos.conf");
    std::fs::write(&file, conf(mounts, path, &lines)).unwrap();
    Command::new(binary()).args(["-nolog", "-conf"]).arg(&file).env("SDL_VIDEODRIVER", "dummy").output().unwrap();
}

pub struct Session {
    child: Child,
    stream: TcpStream,
    owners: Arc<Mutex<VecDeque<bool>>>,
    replies: Receiver<serde_json::Value>,
}

impl Session {
    /// DOS up with `mounts` (the last is the working drive) and the socket connected.
    pub fn start(mounts: &[(char, &Path)], path: &str) -> Session {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let work = mounts.last().expect("a working drive");
        let file = work.1.join(format!("sock{port}.conf"));
        std::fs::write(&file, conf(mounts, path, &[format!("{}:", work.0)])).unwrap();
        let child = Command::new(binary()).args(["-nolog", "-conf"]).arg(&file).env("SDL_VIDEODRIVER", "dummy").env("DOSBOX_DEBUG_PORT", port.to_string()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        let mut stream = None;
        for _ in 0..100 {
            if let Ok(one) = TcpStream::connect(("127.0.0.1", port)) {
                stream = Some(one);
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let stream = stream.expect("the debug socket");
        let owners: Arc<Mutex<VecDeque<bool>>> = Arc::default();
        let (send, replies) = channel();
        let (mut writer, reader) = (stream.try_clone().unwrap(), BufReader::new(stream.try_clone().unwrap()));
        let ours = Arc::clone(&owners);
        std::thread::spawn(move || {
            for line in reader.lines().map_while(Result::ok) {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
                if value.get("event").is_some() {
                    if value["event"] == "stopped" && value["int"] == 3 {
                        ours.lock().unwrap().push_back(false);
                        let _ = writeln!(writer, "{{\"cmd\":\"continue\"}}");
                    }
                } else if ours.lock().unwrap().pop_front().unwrap_or(true) {
                    let _ = send.send(value);
                }
            }
        });
        let this = Session { child, stream, owners, replies };
        this.command(&serde_json::json!({"cmd": "continue"}));
        std::thread::sleep(Duration::from_secs(1));
        // The shell is up before anything is armed or run: a breakpoint on the next program's load, set while the shell is
        // still starting, stops it at the shell's own load, and no command is taken after that.
        let waiting = std::time::Instant::now();
        while this.command(&serde_json::json!({"cmd": "wait_for_shell", "timeoutMs": 5000}))["status"] == "error" {
            assert!(waiting.elapsed() < Duration::from_secs(120), "the shell did not come up");
        }
        this
    }

    pub fn command(&self, value: &serde_json::Value) -> serde_json::Value {
        self.owners.lock().unwrap().push_back(true);
        writeln!(&self.stream, "{value}").unwrap();
        self.replies.recv_timeout(Duration::from_secs(20)).expect("a reply")
    }

    /// Runs a DOS command line in the shell.
    pub fn dos(&self, line: &str) {
        let answer = self.command(&serde_json::json!({"cmd": "dos_cmd", "command": line}));
        assert_ne!(answer["status"], "error", "the shell refused `{line}`: {answer}");
    }

    /// Types `line` and Enter through the keyboard controller, which is how a debugger that hooks the keyboard
    /// interrupt reads keys (the BIOS buffer `key` fills is not): letters, digits and space only.
    pub fn type_line(&self, line: &str) {
        for character in line.chars() {
            let key = if character == ' ' { "space".to_owned() } else { character.to_string() };
            self.command(&serde_json::json!({"cmd": "key_hw", "key": key}));
            std::thread::sleep(Duration::from_millis(150));
        }
        self.command(&serde_json::json!({"cmd": "key_hw", "key": "enter"}));
    }

    /// The text on the screen: the 80x25 characters of the active page of the colour text buffer. A debugger
    /// that draws its own screen is not in the shell's text (`text_screen` reads that).
    pub fn video_text(&self) -> Vec<String> {
        let page = self.command(&serde_json::json!({"cmd": "mem_read_linear", "addr": 0x462, "len": 1}))["data"].as_str().and_then(|hex| u8::from_str_radix(hex, 16).ok()).unwrap_or(0);
        let start = 0xB8000 + usize::from(page) * 0x1000;
        let hex = self.command(&serde_json::json!({"cmd": "mem_read_linear", "addr": start, "len": 4000}))["data"].as_str().unwrap_or("").to_owned();
        let bytes: Vec<u8> = (0..hex.len() / 2).filter_map(|at| u8::from_str_radix(&hex[at * 2..at * 2 + 2], 16).ok()).collect();
        bytes.chunks(160).map(|row| row.iter().step_by(2).map(|&one| if (32..127).contains(&one) { one as char } else { ' ' }).collect::<String>().trim_end().to_owned()).collect()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
