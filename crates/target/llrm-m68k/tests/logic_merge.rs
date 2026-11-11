//! countgt compiled `slt %d3; and.l #255,%d3; and.l #1,%d3`: the zero-extension
//! of a compare's byte and the mask of its `i1`, the second making the first
//! dead (2 instructions and 6 bytes of 10). Logic rows with immediates on one
//! register are one row with the immediates combined.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::dagisel::local;
use llrm_m68k::mfunc::{MFunction, op_inst};
use llrm_m68k::narrow::merge_logic_immediates;
use llrm_m68k::object::{executable, start, write_executable};
use llrm_m68k::regalloc::Assignment;
use llrm_spike_mf::mfunc::Reg;
use llrm_spike_mf::{Block, Inst, Kind, Operand};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

const COMPARE: &str = "
define i32 @f(i32 %x, i32 %t) {
  %c = icmp sgt i32 %x, %t
  %z = zext i1 %c to i32
  ret i32 %z
}";

#[test]
fn a_compare_widened_to_an_int_is_one_mask() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module(COMPARE), "f", cpu, Options::default()).expect("compiles");
        // the one is a `moveq` since constants are priced by cost
        assert!(out.contains("moveq\t#1,") && out.contains("and.l\t%d"), "{cpu:?}: no mask of one:\n{out}");
        assert!(!out.contains("#255"), "{cpu:?}: the byte mask is left:\n{out}");
    }
}

#[test]
fn the_merged_compare_still_answers_zero_or_one() {
    let Some(qemu) = support::qemu() else { return };
    for (x, t, want) in [(5, 3, 1), (3, 5, 0), (3, 3, 0), (-1, -2, 1), (i64::from(i32::MIN), 0, 0)] {
        let mut all = start("f", &[x, t]);
        all.extend(items_with(&module(COMPARE), "f", Cpu::M68020, Options::default()).unwrap());
        let path = support::unique_path("logic-merge");
        write_executable(&path, &executable(&all).unwrap()).unwrap();
        let code = support::run(&qemu, &["-cpu", "m68020"], &path);
        assert_eq!(code, Some(want), "{x} > {t}");
    }
}

fn row(
    name: &'static str,
    register: u32,
    value: i64,
) -> Inst {
    let desc = local::code_of(name).and_then(local::desc).expect(name);
    op_inst(name, desc, vec![Operand::Def(Reg::Virt(register)), Operand::Use(Reg::Virt(register)), Operand::Imm(value)])
}

fn reader(register: u32) -> Inst {
    Inst::new(Kind::Op("USE"), vec![Operand::Use(Reg::Virt(register))])
}

/// The rows left, as `(name, immediate)`, after the pass over `insts` (v0 in
/// d0, v1 in d1).
fn merged(insts: Vec<Inst>) -> Vec<(String, Option<i64>)> {
    let mut function = MFunction::new("f");
    function.classes = vec![llrm_m68k::mfunc::Class::Dr32; 2];
    function.blocks.push(Block { id: 0, insts, phis: vec![] });
    let assignment = Assignment(vec![llrm_m68k::abi::register("d0"), llrm_m68k::abi::register("d1")]);
    merge_logic_immediates(&mut function, &assignment);
    function.blocks[0]
        .insts
        .iter()
        .map(|inst| {
            let Kind::Op(name) = inst.kind else { panic!("not an op") };
            (name.to_owned(), inst.operands.iter().find_map(|o| if let Operand::Imm(v) = o { Some(*v) } else { None }))
        })
        .collect()
}

fn one(
    name: &str,
    value: i64,
) -> Vec<(String, Option<i64>)> {
    vec![(name.to_owned(), Some(value))]
}

#[test]
fn and_or_and_eor_chains_are_one_row() {
    assert_eq!(merged(vec![row("AND32di", 0, 255), row("AND32di", 0, 1)]), one("AND32di", 1));
    assert_eq!(merged(vec![row("OR32di", 0, 0xF0), row("OR32di", 0, 0x0F)]), one("OR32di", 0xFF));
    assert_eq!(merged(vec![row("XOR32di", 0, 0xFF), row("XOR32di", 0, 0x0F)]), one("XOR32di", 0xF0));
    assert_eq!(
        merged(vec![row("AND32di", 0, 0xFFFF), row("AND32di", 0, 0xFF), row("AND32di", 0, 0x0F)]),
        one("AND32di", 0x0F)
    );
}

#[test]
fn rows_between_that_leave_the_register_and_the_codes_alone_do_not_stop_the_merge() {
    let other = Inst::new(Kind::Op("USE"), vec![Operand::Use(Reg::Virt(1))]);
    assert_eq!(merged(vec![row("AND32di", 0, 255), other, row("AND32di", 0, 1)]).len(), 2);
}

#[test]
fn a_row_between_that_reads_the_register_stops_the_merge() {
    assert_eq!(merged(vec![row("AND32di", 0, 255), reader(0), row("AND32di", 0, 1)]).len(), 3);
}

#[test]
fn another_register_or_another_operation_is_not_merged() {
    assert_eq!(merged(vec![row("AND32di", 0, 255), row("AND32di", 1, 1)]).len(), 2);
    assert_eq!(merged(vec![row("AND32di", 0, 255), row("OR32di", 0, 1)]).len(), 2);
}

#[test]
fn a_word_row_after_a_long_one_leaves_the_upper_half_to_the_long_mask() {
    // `and.l #0xFF` then `and.w #1`: the word row keeps the upper half, which
    // the long row already cleared
    assert_eq!(merged(vec![row("AND32di", 0, 255), row("AND16di", 0, 1)]), one("AND32di", 1));
}

/// A call-frame row (`ADJCALLSTACKDOWN`, which sets the codes) the scheduler left between the
/// byte mask and the mask of one kept `and.l #255,%d0` before `and.l #1,%d0` in
/// overflow_builtins at m68k-68000 (+4 B). A row that only sets the codes
/// cannot stop the merge: the second row sets them again.
#[test]
fn a_row_between_that_only_sets_the_codes_does_not_stop_the_merge() {
    let desc = local::code_of("ADJCALLSTACKDOWN").and_then(local::desc).expect("the call-frame row");
    let frame = op_inst("ADJCALLSTACKDOWN", desc, vec![Operand::Imm(4), Operand::Imm(0)]);
    assert!(frame.sets_flags, "the premise: the row sets the codes");
    assert_eq!(merged(vec![row("AND32di", 0, 255), frame, row("AND32di", 0, 1)]).len(), 2);
}
