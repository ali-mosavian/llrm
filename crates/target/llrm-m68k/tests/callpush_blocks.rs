//! A call sequence belongs to one block. Two paths each called `b(x, n)` and
//! met in a block that cleaned up both; the next cleanup after `b1`'s call was
//! the odd path's own, so `b1`'s pushes were paired with it and the odd path
//! popped 8 bytes nothing pushed (`shift_int32_variable` printed the unshifted
//! value at every odd count, 12 cells).

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_m68k::callpush::plan;
use llrm_m68k::driver::{self, Level};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::mfunc::{Inst, Kind, Operand, Reg};
use llrm_m68k::reg;
use llrm_m68k::regalloc::Assignment;
use llrm_m68k::timing::Price;

mod support;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_rt").join(name)
}

fn run_with(cpu: &str) {
    let Some(root) = support::oracle_root() else { return };
    let Some(qemu) = support::qemu() else { return };
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    let ir = std::fs::read_to_string(fixture("diamond_calls.ll")).unwrap();
    let text = driver::assembly(&ir, driver::cpu_of(cpu).expect("a cpu"), Level::O2).expect("compiles");
    let source = support::unique_path("blocks-s");
    let object = support::unique_path("blocks-o");
    let exe = support::unique_path("blocks-exe");
    std::fs::write(&source, format!(".text\n.globl diamonds\n{text}")).unwrap();
    assert!(
        tool("as").arg(format!("-m{cpu}")).arg("-o").arg(&object).arg(&source).status().unwrap().success(),
        "{text}"
    );
    let linked = tool("gcc")
        .args(["-msoft-float", &format!("-m{cpu}"), "-O1", "-nostdlib", "-static", "-ffreestanding"])
        .arg(fixture("diamond_calls.c"))
        .arg(&object)
        .arg("-o")
        .arg(&exe)
        .status()
        .unwrap();
    assert!(linked.success(), "{text}");
    let code = support::run(&qemu, &[], &exe);
    for path in [source, object, exe] {
        std::fs::remove_file(path).ok();
    }
    assert_eq!(code, Some(0), "{cpu}: the shifted values summed wrong:\n{text}");
}

#[test]
fn a_call_on_each_path_of_a_diamond_keeps_its_own_cleanup_at_68000() {
    run_with("68000");
}

#[test]
fn a_call_on_each_path_of_a_diamond_keeps_its_own_cleanup_at_68020() {
    run_with("68020");
}

/// The arguments of a call are stored before a diamond the call is after
/// (`compare_to_int64`: the `pea 25:w` of a `__mulsi3`). Pairing a call with a
/// cleanup only from its own block dropped that push too: 4 bytes more. Built
/// on the call sequence itself: the whole-program shape came from the unroller
/// and the inliner, so a price change moved it away (11-11: the test read 3
/// `pea` for 5 and passed again with the fixture's callees `noinline`, the
/// revert of the fix included).
#[test]
fn arguments_stored_before_a_diamond_stay_pushes_at_68000() {
    let sp = || Operand::Use(Reg::Phys(reg::SP));
    let mut call = Inst::new(Kind::Op("JSR32"), vec![]);
    call.call = true;
    let insts = [
        Inst::new(Kind::Op("ADJCALLSTACKDOWN"), vec![Operand::Imm(8)]),
        Inst::new(Kind::Op("MOV32jr"), vec![sp(), Operand::Use(Reg::Phys(reg::D5))]),
        Inst::new(Kind::Op("MOV32pi"), vec![Operand::Imm(4), sp(), Operand::Imm(25)]),
        // the two arms of a diamond between the stores and the call
        Inst::new(Kind::Op("ADD32dr"), vec![Operand::Def(Reg::Phys(reg::D4)), Operand::Use(Reg::Phys(reg::D0))]),
        Inst::new(Kind::Op("ADD32dr"), vec![Operand::Def(Reg::Phys(reg::D4)), Operand::Use(Reg::Phys(reg::D1))]),
        call,
        Inst::new(Kind::Op("ADJCALLSTACKUP"), vec![Operand::Imm(8), Operand::Imm(0)]),
    ];
    let refs: Vec<&Inst> = insts.iter().collect();
    let blocks = [0, 0, 0, 1, 2, 3, 3];
    let found = plan(&refs, &blocks, &Assignment::default(), Cpu::M68000, Price::Rate, true, |area| {
        Ok(if area > 0 { (8, 16) } else { (0, 0) })
    })
    .expect("plans");
    assert!(found.deferred_offset(1).is_some(), "the stores became pushes nowhere: 4 bytes more");
}
