//! `indirectbr` and `blockaddress` lowered natively (`jmp (An)`, a symbol at
//! the block's label, `.long` of it in a static table). Before, every
//! `goto *p` became a `switch` over block numbers behind one shared bounds
//! check and table jump (computed_goto at 68000 O2: 10,104 clk against gcc's
//! 8,774), and with the expansion off the jump was dropped and the address
//! was `lea (f+8388611,%pc)`.

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_m68k::driver::{self, Level};

use crate::support;

const CPUS: [&str; 2] = ["68000", "68020"];
const LEVELS: [Level; 3] = [Level::O1, Level::O2, Level::Os];
const DATALAYOUT: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16""#;

/// A jump through a static table of block addresses, one call per handler.
fn table_jump() -> String {
    format!(
        r#"{DATALAYOUT}
@tab = internal constant [3 x ptr] [ptr blockaddress(@table_jump, %a), ptr blockaddress(@table_jump, %b), ptr blockaddress(@table_jump, %c)]
declare i32 @g1()
declare i32 @g2()
declare i32 @g3()
define i32 @table_jump(i32 %x) {{
entry:
  %p = getelementptr [3 x ptr], ptr @tab, i32 0, i32 %x
  %t = load ptr, ptr %p
  indirectbr ptr %t, [label %a, label %b, label %c]
a:
  %ra = call i32 @g1()
  ret i32 %ra
b:
  %rb = call i32 @g2()
  ret i32 %rb
c:
  %rc = call i32 @g3()
  ret i32 %rc
}}
"#
    )
}

/// A jump through a pointer one of two paths stored (`void *t = c ? &&one :
/// &&two; goto *t`).
fn slot_jump() -> String {
    format!(
        r#"{DATALAYOUT}
@slot = global ptr null
define i32 @slot_jump(i32 %c, i32 %x) {{
entry:
  %t0 = icmp ne i32 %c, 0
  br i1 %t0, label %s1, label %s2
s1:
  store volatile ptr blockaddress(@slot_jump, %one), ptr @slot
  br label %join
s2:
  store volatile ptr blockaddress(@slot_jump, %two), ptr @slot
  br label %join
join:
  %t = load volatile ptr, ptr @slot
  indirectbr ptr %t, [label %one, label %two]
one:
  %r1 = add i32 %x, 1
  ret i32 %r1
two:
  %r2 = mul i32 %x, 2
  ret i32 %r2
}}
"#
    )
}

fn both() -> String {
    let slot = slot_jump();
    table_jump() + &slot[slot.find('\n').unwrap() + 1..]
}

#[test]
fn a_computed_goto_is_a_jump_through_a_register_at_every_cpu_and_level() {
    for cpu in CPUS {
        for level in LEVELS {
            let out = driver::assembly(&both(), driver::cpu_of(cpu).unwrap(), level).expect("compiles");
            let jumps = out.lines().filter(|line| line.starts_with("\tjmp\t(%a")).count();
            assert_eq!(jumps, 2, "{cpu} {level:?}: one jump per indirectbr, no switch route:\n{out}");
            assert!(!out.contains("LJTI"), "{cpu} {level:?}: a jump table:\n{out}");
        }
    }
}

#[test]
fn an_address_taken_block_keeps_its_label_and_the_table_names_it() {
    for cpu in CPUS {
        for level in LEVELS {
            let out = driver::assembly(&table_jump(), driver::cpu_of(cpu).unwrap(), level).expect("compiles");
            let labels = out.lines().filter(|line| line.starts_with("$BA") && line.ends_with(':')).count();
            let words = out.lines().filter(|line| line.starts_with("\t.long\t$BA")).count();
            assert_eq!((labels, words), (3, 3), "{cpu} {level:?}:\n{out}");
        }
    }
}

fn binutils(tool: &str) -> Option<Command> {
    let root = support::oracle_root()?;
    let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{tool}"));
    command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
    Some(command)
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/brind_rt/main.c")
}

/// Links `objects` with the fixture and runs it; `None` where the oracle or
/// qemu is skipped.
fn run(
    cpu: &str,
    objects: &[&Path],
) -> Option<i32> {
    let mut link = binutils("gcc")?;
    let qemu = support::qemu()?;
    let exe = support::unique_path("brind-exe");
    let linked = link
        .args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"])
        .arg(fixture())
        .args(objects)
        .arg("-o")
        .arg(&exe)
        .status()
        .unwrap();
    assert!(linked.success());
    support::run(&qemu, &[], &exe)
}

fn run_text(
    cpu: &str,
    level: Level,
) {
    let Some(mut assemble) = binutils("as") else { return };
    let text = driver::assembly(&both(), driver::cpu_of(cpu).unwrap(), level).expect("compiles");
    let (source, object) = (support::unique_path("brind-s"), support::unique_path("brind-o"));
    std::fs::write(&source, format!(".text\n.globl table_jump\n.globl slot_jump\n{text}")).unwrap();
    assert!(assemble.arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success(), "{text}");
    if let Some(code) = run(cpu, &[&object]) {
        assert_eq!(code, 0, "{cpu} {level:?} text:\n{text}");
    }
}

fn run_object(
    cpu: &str,
    level: Level,
) {
    let bytes = driver::object(&both(), driver::cpu_of(cpu).unwrap(), level, "brind").expect("compiles");
    let object = support::unique_path("brind-obj");
    std::fs::write(&object, bytes).unwrap();
    if let Some(code) = run(cpu, &[&object]) {
        assert_eq!(code, 0, "{cpu} {level:?} object");
    }
}

#[test]
fn the_assembly_runs_at_68000_and_68020_at_every_level() {
    for cpu in CPUS {
        for level in LEVELS {
            run_text(cpu, level);
        }
    }
}

#[test]
fn the_object_runs_at_68000_and_68020_at_every_level() {
    for cpu in CPUS {
        for level in LEVELS {
            run_object(cpu, level);
        }
    }
}
