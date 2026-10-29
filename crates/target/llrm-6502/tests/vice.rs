//! The C64 PRG run in VICE's `x64sc`: the top-left screen cell ($0400) holds 7 and the code at $080D
//! is `jsr; sta $0400`. Needs `x64sc`, `xvfb-run` and the C64 ROMs (kernal-901227-03, basic-901226-01,
//! chargen-901225-01) in ~/.local/share/vice/C64, so it is ignored unless asked for:
//! `cargo test -p llrm-6502 --test vice -- --ignored`.

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::Duration;

fn ask(
    monitor: &mut TcpStream,
    command: &str,
) -> String {
    monitor.write_all(format!("{command}\n").as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(1000));
    monitor.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
    let mut out = Vec::new();
    let mut chunk = [0u8; 4096];
    while let Ok(n) = monitor.read(&mut chunk) {
        if n == 0 {
            break;
        }
        out.extend_from_slice(&chunk[..n]);
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
#[ignore = "needs x64sc, xvfb-run and the C64 ROMs in ~/.local/share/vice/C64"]
fn the_prg_runs_in_vice_and_leaves_7_on_the_screen() {
    let (_, _, prg) = common::build();
    let mut vice = Command::new("xvfb-run")
        .args(["-a", "x64sc", "-default", "+sound", "-warp", "-autostartprgmode", "1"])
        .args(["-remotemonitor", "-remotemonitoraddress", "ip4://127.0.0.1:6510", "-autostart"])
        .arg(&prg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("xvfb-run x64sc");
    let mut monitor = (0..60)
        .find_map(|_| TcpStream::connect("127.0.0.1:6510").ok().or_else(|| {
            std::thread::sleep(Duration::from_millis(500));
            None
        }))
        .expect("VICE's monitor");
    // boot, the injected RUN, the program
    std::thread::sleep(Duration::from_secs(8));
    let screen = ask(&mut monitor, "m 0400 0400");
    let code = ask(&mut monitor, "m 080d 0812");
    let _ = monitor.write_all(b"quit\n");
    std::thread::sleep(Duration::from_millis(500));
    let _ = vice.kill();

    assert!(screen.contains(">C:0400  07"), "screen cell: {screen}");
    // jsr $0816; sta $0400
    assert!(code.contains(">C:080d  20 16 08 8d  00 04"), "code at 2061: {code}");
}
