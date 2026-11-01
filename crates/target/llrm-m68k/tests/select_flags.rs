//! A branch that reads the compare of a select. The select becomes a diamond
//! whose arms' copies set the condition codes again, so the branch past the
//! join read the arms', not the compare's; the expansion refused the function
//! ("a branch reads the flags of a compare past the select that follows it").

use std::path::PathBuf;

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const SELECT_THEN_BRANCH: &str = "define i32 @f(i32 %a, i32 %b) {
  %c = icmp sgt i32 %a, %b
  %s = select i1 %c, i32 %a, i32 %b
  br i1 %c, label %y, label %n
y:
  ret i32 %s
n:
  %t = add i32 %s, 1
  ret i32 %t
}";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

fn qemu() -> Option<PathBuf> {
    ["/usr/bin/qemu-m68k", "/usr/local/bin/qemu-m68k"].iter().map(PathBuf::from).find(|path| path.exists())
}

fn status(
    ir: &str,
    args: &[i64],
    options: Options,
) -> Option<i32> {
    let qemu = qemu()?;
    let mut all = start("f", args);
    all.extend(items_with(&module(ir), "f", Cpu::M68000, options).expect("compiles"));
    let path = support::unique_path("select-flags");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    // the 68000 model: `move.w %sr` is privileged from the 68010 on
    let code = support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    assert!(code.is_some(), "f({args:?}) ended on a signal");
    code
}

/// Compiles on both CPUs with the codes saved before the diamond and restored
/// after it, and computes `a > b ? a : b + 1` for both outcomes of the branch
/// and the equal case.
#[test]
fn a_branch_on_the_compare_of_a_select_compiles_and_branches_on_the_compare() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = text_with(&module(SELECT_THEN_BRANCH), "f", cpu, Options::default())
            .unwrap_or_else(|why| panic!("{cpu:?}: {why}"));
        assert!(text.contains("%ccr"), "{cpu:?}: the codes are not restored:\n{text}");
    }
    for (a, b, want) in [(5, 3, 5), (3, 5, 6), (4, 4, 5), (-1, -2, -1), (-2, -1, 0)] {
        if let Some(code) = status(SELECT_THEN_BRANCH, &[a, b], Options::default()) {
            assert_eq!(code, want & 0xFF, "f({a}, {b})");
        }
    }
}
