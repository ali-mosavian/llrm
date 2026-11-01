//! Functions with frame objects, run on ucsim_z80. A local whose address reaches a call was refused
//! ("LEAFI is not expanded yet"); a wrong `ld hl, off; add hl, sp` would hand the callee the wrong cell.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};
use std::process::Command;

fn have(tool: &str) -> bool {
    Command::new("which").arg(tool).output().is_ok_and(|o| o.status.success())
}

/// `ir`'s function `f` called with a = `a` and l = `l`; the byte it returns.
fn run(ir: &str, a: u8, l: u8, callee: &str) -> u8 {
    let dir = std::env::temp_dir().join(format!("z80-frame-{}-{a}-{l}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("makes a directory");
    let body = text(ir, "f", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let asm = format!(
        "\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tld a,#{a}\n\tld l,#{l}\n\tcall _f\n\tld (0x8f00),a\n\thalt\n\
         \t.area _CODE\n\t.globl _f\n{body}{callee}"
    );
    std::fs::write(dir.join("t.s"), asm).expect("writes");
    let sh = |cmd: &[&str]| {
        let out = Command::new(cmd[0]).args(&cmd[1..]).current_dir(&dir).output().expect("runs");
        assert!(out.status.success(), "{cmd:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
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

/// The callee returns the address it is given less the caller's `ix`, which it keeps: where the object is in the frame.
const OFFSET: &str = "_ext::\n\tpush ix\n\tpop de\n\tor a\n\tsbc hl,de\n\tld a,l\n\tret\n";

/// Both cells are in the frame; the array is the second, 4 bytes below the first, so 5 below `ix`.
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
