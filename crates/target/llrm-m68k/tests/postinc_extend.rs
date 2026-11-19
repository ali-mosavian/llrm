//! A byte loaded and extended through a walking pointer was `moveq #0, %d2;
//! move.b (%a0), %d2` and an `addq.l #1, %a0` (bench/library/crc16 spent 2 B
//! more than gcc's `move.b (%a0)+, %d2`): the writeback rows covered plain
//! loads only.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::object::{executable, start, write_executable};

mod support;

/// Whether `pattern`, with `{}` as an address register's number, occurs for any
/// register (the allocator's pick is not the property: it moved from %a0 to %a1
/// and failed both tests).
fn any_address_register(
    out: &str,
    pattern: &str,
) -> bool {
    (0..=6).any(|n| out.contains(&pattern.replace("{}", &n.to_string())))
}

fn walk(extension: &str) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @k(ptr %p, i32 %n) {{
entry:
  br label %loop
loop:
  %q = phi ptr [ %p, %entry ], [ %q1, %loop ]
  %i = phi i32 [ %n, %entry ], [ %i1, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %b = load i8, ptr %q
  %z = {extension} i8 %b to i32
  %s1 = add i32 %s, %z
  %q1 = getelementptr i8, ptr %q, i32 1
  %i1 = sub i32 %i, 1
  %c = icmp ne i32 %i1, 0
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %s1
}}
"
    )
}

#[test]
fn a_zero_extended_byte_walk_loads_through_post_increment() {
    for cpu in ["68000", "68020"] {
        let out = assembly(&walk("zext"), cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(
            any_address_register(&out, "moveq\t#0, %d2\n\tmove.b\t(%a{})+, %d2"),
            "{cpu}: no cleared (An)+ load:\n{out}"
        );
        assert!(!any_address_register(&out, "addq.l\t#1, %a{}"), "{cpu}: a separate step:\n{out}");
    }
}

#[test]
fn a_sign_extended_byte_walk_loads_through_post_increment() {
    for cpu in ["68000", "68020"] {
        let out = assembly(&walk("sext"), cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(any_address_register(&out, "move.b\t(%a{})+, %d"), "{cpu}: no (An)+ load:\n{out}");
        assert!(!any_address_register(&out, "addq.l\t#1, %a{}"), "{cpu}: a separate step:\n{out}");
    }
}

/// The walk, run: `{255, 1, 2, 3}` sums to 261 zero-extended and 5
/// sign-extended, so `sum >> 8` is 1 and 0. A post-increment load that extended
/// wrongly, or stepped the pointer twice, changes the exit code.
#[test]
fn the_post_increment_walks_sum_the_extended_bytes() {
    let Some(qemu) = support::qemu() else { return };
    for (extension, expected) in [("zext", 1), ("sext", 0)] {
        for cpu in [Cpu::M68000, Cpu::M68020] {
            let text = format!(
                "{}define i32 @f() {{
  %buf = alloca [4 x i8]
  store i8 255, ptr %buf
  %b1 = getelementptr i8, ptr %buf, i32 1
  store i8 1, ptr %b1
  %b2 = getelementptr i8, ptr %buf, i32 2
  store i8 2, ptr %b2
  %b3 = getelementptr i8, ptr %buf, i32 3
  store i8 3, ptr %b3
  %r = call i32 @k(ptr %buf, i32 4)
  %h = ashr i32 %r, 8
  ret i32 %h
}}
",
                walk(extension)
            );
            let module = llrm_mir::parse::module(&text).expect("parses");
            let options = Options { size: true, ..Options::default() };
            let mut all = start("f", &[]);
            for name in ["f", "k"] {
                all.extend(items_with(&module, name, cpu, options).expect("compiles"));
            }
            let path = support::unique_path("postinc_extend");
            write_executable(&path, &executable(&all).unwrap()).unwrap();
            let code = support::run(&qemu, &[], &path);
            std::fs::remove_file(&path).ok();
            assert_eq!(code, Some(expected), "{extension} at {cpu:?}");
        }
    }
}
