//! A byte-copy loop is one `ldir`. Before, `lz4`'s copy loops took 52 T a byte
//! against `ldir`'s 21, 18 000 of its 38 000 T at -O2.

use std::process::Command;

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16";

/// Copies `%n` (not zero) bytes from `%s` to `%d` one at a time and returns the
/// destination's end. `LOAD` is the load's keyword.
fn copy_loop(load: &str) -> String {
    format!(
        "target datalayout = \"{DATALAYOUT}\"\n\
         define ptr @cp(ptr %d, ptr %s, i16 %n) {{\nentry:\n  br label %loop\nloop:\n  \
         %p = phi ptr [ %s, %entry ], [ %p1, %loop ]\n  %q = phi ptr [ %d, %entry ], [ %q1, %loop ]\n  \
         %k = phi i16 [ %n, %entry ], [ %k1, %loop ]\n  %b = {load} i8, ptr %p\n  store i8 %b, ptr %q\n  \
         %p1 = getelementptr i8, ptr %p, i16 1\n  %q1 = getelementptr i8, ptr %q, i16 1\n  \
         %k1 = add i16 %k, -1\n  %done = icmp eq i16 %k1, 0\n  br i1 %done, label %out, label %loop\nout:\n  ret ptr %q1\n}}\n"
    )
}

fn have(tool: &str) -> bool {
    Command::new("which").arg(tool).output().is_ok_and(|o| o.status.success())
}

/// Bytes 1..=5 at 0x9000 copied by `cp(dst, 0x9000, n)`: the five bytes at
/// 0x9000 afterwards, and the pointer `cp` returns.
fn run(
    ir: &str,
    dst: u16,
    n: u16,
) -> (Vec<u8>, u16) {
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let id = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("z80-ldir-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("makes a directory");
    let body = text(ir, "cp", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let fill: String = (1..=5).map(|i| format!("\tld a,#{i}\n\tld (0x{:x}),a\n", 0x9000 + i - 1)).collect();
    let asm = format!(
        "\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n{fill}\tld hl,#{dst}\n\tld de,#0x9000\n\tld bc,#{n}\n\tcall _cp\n\
         \tld (0xa000),de\n\thalt\n\t.area _CODE\n\t.globl _cp\n{body}"
    );
    std::fs::write(dir.join("t.s"), asm).expect("writes");
    let sh = |cmd: &[&str]| {
        let out = Command::new(cmd[0]).args(&cmd[1..]).current_dir(&dir).output().expect("runs");
        assert!(
            out.status.success(),
            "{cmd:?}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    };
    sh(&["sdasz80", "-o", "t.rel", "t.s"]);
    sh(&["sdldz80", "-i", "-b", "_CODE=0x100", "t.ihx", "t.rel"]);
    std::fs::write(dir.join("cmds"), "run\ndump rom 0x9000 0x9004\ndump rom 0xa000 0xa001\nquit\n").expect("writes");
    let out = Command::new("sh")
        .args(["-c", "ucsim_z80 -t Z80 -q -w t.ihx -c - < cmds"])
        .current_dir(&dir)
        .output()
        .expect("runs");
    let out = String::from_utf8_lossy(&out.stdout).into_owned();
    let bytes = |start: &str, count: usize| -> Vec<u8> {
        let line =
            out.lines().find(|line| line.starts_with(start)).unwrap_or_else(|| panic!("no dump of {start}:\n{out}"));
        line.split_whitespace().skip(1).take(count).map(|byte| u8::from_str_radix(byte, 16).expect("hex")).collect()
    };
    let end = bytes("0xa000", 2);
    (bytes("0x9000", 5), u16::from(end[0]) | u16::from(end[1]) << 8)
}

/// The loop was a test of the count, eight instructions and 52 T a byte; it is
/// the copy, and a loop whose load is volatile (one access each, in order,
/// which `ldir` does not promise) stays a loop.
#[test]
fn a_byte_copy_loop_is_an_ldir_and_a_volatile_one_is_not() {
    for level in [Level::O2, Level::Oz] {
        let plain = text(&copy_loop("load"), "cp", 0, &Request::new(level)).expect("compiles");
        assert!(plain.contains("ldir") && !plain.contains("jr nz"), "{plain}");
        let volatile = text(&copy_loop("load volatile"), "cp", 0, &Request::new(level)).expect("compiles");
        assert!(!volatile.contains("ldir") && volatile.contains("jr nz"), "{volatile}");
    }
}

/// A destination one above the source repeats the first byte down the buffer,
/// as the byte loop did and `memmove` would not; the destination's end after
/// the loop is the one `ldir` leaves in `de`.
#[test]
fn an_overlapping_copy_repeats_and_the_end_pointer_is_the_destinations() {
    if !["sdasz80", "sdldz80", "ucsim_z80"].into_iter().all(have) {
        assert!(std::env::var("Z80_ORACLE_OPTIONAL").is_ok_and(|flag| flag == "1"), "sdcc tools not found");
        return;
    }
    let (bytes, end) = run(&copy_loop("load"), 0x9001, 4);
    assert_eq!(bytes, [1, 1, 1, 1, 1]);
    assert_eq!(end, 0x9005);
    let (bytes, end) = run(&copy_loop("load"), 0x9000, 1);
    assert_eq!(bytes, [1, 2, 3, 4, 5]);
    assert_eq!(end, 0x9001);
}
