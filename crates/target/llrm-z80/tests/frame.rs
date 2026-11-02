//! Functions with frame objects, run on ucsim_z80. A local whose address
//! reaches a call was refused ("LEAFI is not expanded yet"); a wrong `ld hl,
//! off; add hl, sp` would hand the callee the wrong cell.

use std::process::Command;

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

fn have(tool: &str) -> bool {
    Command::new("which").arg(tool).output().is_ok_and(|o| o.status.success())
}

/// `ir`'s function `f` called with a = `a` and l = `l`; the byte it returns.
fn run(
    ir: &str,
    a: u8,
    l: u8,
    callee: &str,
) -> u8 {
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("z80-frame-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("makes a directory");
    let body = text(ir, "f", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let asm = format!(
        "\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tld a,#{a}\n\tld l,#{l}\n\tcall _f\n\tld (0x8f00),a\n\thalt\n\
         \t.area _CODE\n\t.globl _f\n{body}{callee}"
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
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    sh(&["sdasz80", "-o", "t.rel", "t.s"]);
    sh(&["sdldz80", "-i", "-b", "_CODE=0x100", "t.ihx", "t.rel"]);
    let script = dir.join("cmds");
    std::fs::write(&script, "run\ndump rom 0x8f00 0x8f00\nquit\n").expect("writes");
    let out = Command::new("sh")
        .args(["-c", "ucsim_z80 -t Z80 -q -w t.ihx -c - < cmds"])
        .current_dir(&dir)
        .output()
        .expect("runs");
    let out = String::from_utf8_lossy(&out.stdout);
    let line = out.lines().find(|line| line.starts_with("0x8f00")).unwrap_or_else(|| panic!("no dump:\n{out}"));
    u8::from_str_radix(line.split_whitespace().nth(1).expect("a byte"), 16).expect("hex")
}

/// The callee returns the address it is given less the caller's `ix`, which it
/// keeps: where the object is in the frame.
const OFFSET: &str = "_ext::\n\tpush ix\n\tpop de\n\tor a\n\tsbc hl,de\n\tld a,l\n\tret\n";

/// Both cells are in the frame; the array is the second, 4 bytes below the
/// first, so 5 below `ix`.
const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
    declare i8 @ext(ptr)\n\
    define i8 @f(i8 %a, i8 %b) {\n  %cell = alloca i8\n  %buf = alloca [4 x i8]\n  store volatile i8 %a, ptr %cell\n  \
    %r = call i8 @ext(ptr %buf)\n  %c = load volatile i8, ptr %cell\n  %s = add i8 %r, %c\n  ret i8 %s\n}\n";

#[test]
fn a_local_whose_address_reaches_a_call_is_that_local() {
    if !["sdasz80", "sdldz80", "ucsim_z80"].into_iter().all(have) {
        assert!(std::env::var("Z80_ORACLE_OPTIONAL").is_ok_and(|flag| flag == "1"), "sdcc tools not found");
        return;
    }
    // (-5 + 10) mod 256
    assert_eq!(run(IR, 10, 20, OFFSET), 5);
}

/// The callee adds the bytes at 1, 2 and 3 from the address it is given.
const SUM3: &str = "_ext::\n\tinc hl\n\tld a,(hl)\n\tinc hl\n\tadd a,(hl)\n\tinc hl\n\tadd a,(hl)\n\tret\n";

/// Stored in place (`ld d(ix), r` and `ld d(ix), n`) and read through the
/// address `LEAFI` makes: the two must agree.
#[test]
fn a_local_written_in_place_is_where_its_address_says() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
        declare i8 @ext(ptr)\n\
        define i8 @f(i8 %a, i8 %b) {\n  %buf = alloca [8 x i8]\n  %p1 = getelementptr i8, ptr %buf, i8 1\n  \
        %p2 = getelementptr i8, ptr %buf, i8 2\n  %p3 = getelementptr i8, ptr %buf, i8 3\n  store i8 77, ptr %p1\n  \
        store i8 %a, ptr %p2\n  store i8 %b, ptr %p3\n  %r = call i8 @ext(ptr %buf)\n  ret i8 %r\n}\n";
    if !["sdasz80", "sdldz80", "ucsim_z80"].into_iter().all(have) {
        return;
    }
    assert_eq!(run(ir, 10, 20, SUM3), 107);
    let out = text(ir, "f", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(out.contains("(ix)") && !out.contains("add hl, de"), "{out}");
}

/// The callee returns the byte at 4 from the address it is given.
const AT4: &str = "_ext::\n\tinc hl\n\tinc hl\n\tinc hl\n\tinc hl\n\tld a,(hl)\n\tret\n";

/// A word stored at 4 and read back in place: the low byte is at 4, the high
/// byte at 5.
#[test]
fn a_word_local_is_its_two_bytes_in_order() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
        declare i8 @ext(ptr)\n\
        define i8 @f(i8 %a, i8 %b) {\n  %buf = alloca [8 x i8]\n  %p = getelementptr i8, ptr %buf, i8 4\n  \
        %ha = zext i8 %a to i16\n  %hb = zext i8 %b to i16\n  %hi = shl i16 %ha, 8\n  %w = or i16 %hi, %hb\n  \
        store volatile i16 %w, ptr %p\n  %r = call i8 @ext(ptr %buf)\n  %x = load volatile i16, ptr %p\n  \
        %xs = lshr i16 %x, 8\n  %xb = trunc i16 %xs to i8\n  %s = add i8 %r, %xb\n  ret i8 %s\n}\n";
    if !["sdasz80", "sdldz80", "ucsim_z80"].into_iter().all(have) {
        return;
    }
    assert_eq!(run(ir, 10, 20, AT4), 30);
}
