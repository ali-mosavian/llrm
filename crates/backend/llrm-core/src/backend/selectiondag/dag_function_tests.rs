//! Tests of `dag_function::selected`, split by area. Where a new test goes:
//!
//! - `dag_function_aggregates_tests.rs`: first-class struct and array values.
//! - `dag_function_calls_tests.rs`: Calls: the call sequence, arguments and
//!   results in registers and on the stack, runtime routines, landing pads,
//!   ports and inline blocks.
//! - `dag_function_float_tests.rs`: Floating point: x87 results, conversions,
//!   compares, constants and pool entries, spills.
//! - `dag_function_flags_tests.rs`: Flags: what may sit between a compare and
//!   its branch, carries, selects on one compare, shifts, constant branches and
//!   dropped blocks.
//! - `dag_function_far_tests.rs`: Far and huge pointers at m16: selectors,
//!   segments, ES, phis of far pointers.
//! - `dag_function_switch_tests.rs`: Switch lowering: compare chains, jump
//!   tables, peeled cases, phis at the targets.
//! - `dag_function_frame_tests.rs`: Frame objects, cells and addresses, string
//!   operations and invariant loads.
//! - `dag_function_m64_tests.rs`: x86-64 (m64): quadword arithmetic, the SysV
//!   call sequence, loops over arrays.
//! - `dag_function_tail_tests.rs`: Sibling calls (tail calls): `TC_RETURN` and
//!   what follows it.
//! - this file: The first selections: a function of one block, a loop, a
//!   divide, byte registers. A test that fits no other file goes here. Items
//!   more than one area reads (`names`, shared IR constants) live here as
//!   `pub(super)`.

use super::dag_function::selected;

pub(super) fn names(text: &str) -> Result<Vec<String>, String> {
    let module = llrm_mir::parse::module(text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .map_err(|e| e.0)?;
    Ok(done
        .body
        .insns()
        .iter()
        .map(|i| i.what.as_ref().and_then(|w| w.name.as_deref().map(str::to_owned)).unwrap_or_default())
        .collect())
}

pub(super) const M16_STACK_ARGS: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
declare i16 @g(i16, i16)
define i16 @f(i16 %a) {
entry:
  %r = call i16 @g(i16 %a, i16 233)
  ret i16 %r
}
";

pub(super) const FAR_CALL: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32-ni:2\"
declare void @far() addrspace(1)
declare void @near()
define void @f() {
entry:
  call addrspace(1) void @far()
  call void @near()
  ret void
}
";

pub(super) const QB_STRING_COMPARE: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
define cc 1000 i16 @f(ptr %a, ptr %b) addrspace(1) {
entry:
  %r = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %a, ptr %b)
  ret i16 %r
}
declare cc 1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1)
";

/// An ABI whose runtime routines return their answer in the flags or not.
pub(super) struct Stub {
    pub(super) flags: bool,
}

impl crate::backend::assemble::Abi for Stub {
    fn contract(
        &self,
        callee: &str,
        pops: bool,
        pushed: i64,
    ) -> Result<crate::abi::runtime::Contract, String> {
        let cleanup =
            if pops { crate::hir::model::StackCleanup::Callee } else { crate::hir::model::StackCleanup::Caller };
        Ok(crate::abi::runtime::Contract {
            flags_result: self.flags,
            ..crate::abi::qb::ordinary(callee, cleanup, pushed, &Default::default())
        })
    }

    fn linked(
        &self,
        name: &str,
    ) -> String {
        name.to_owned()
    }

    fn runtime_contracts(&self) -> bool {
        true
    }
}

pub(super) fn verdicts(flags: bool) -> Vec<Result<super::builder::RuntimeAnswer, &'static str>> {
    let module = llrm_mir::parse::module(QB_STRING_COMPARE).expect("parses");
    let function = module.global(module.named("f").expect("f")).function().expect("a body");
    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let verdicts =
        super::dag_function::runtime_call_verdicts(&module, function, &layout, &llrm_x86_m16::M16, &Stub { flags });
    verdicts.values().copied().collect()
}

pub(super) fn select_with_contracts(text: &str) -> Result<crate::backend::isel::Selected, String> {
    let module = llrm_mir::parse::module(text).expect("parses");
    let contracts =
        super::dag_function::RuntimeContracts { abi: &Stub { flags: false }, segments: &crate::backend::target::BASIC };
    let pool = &mut crate::backend::constpool::Pool::new(module.globals.len() as i64);
    super::dag_function::selected_in_pool(
        &module,
        "f",
        &llrm_x86_m16::M16,
        crate::backend::cpu::profile("386").expect("a cpu"),
        &[],
        Some(&contracts),
        pool,
    )
    .map_err(|e| e.0)
}

const BYTE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  %s = add i32 %a, %b
  %t = trunc i32 %s to i8
  %z = zext i8 %t to i32
  ret i32 %z
}
";

/// A truncate then an extend selects `EXTRACT_SUBREG` and a `movzx`: the
/// subregister is the same value viewed one byte wide, so the `movzx` reads
/// the value the add wrote. It was refused (`Unmapped(EXTRACT_SUBREG)`) in
/// 44 functions of the corpora.
#[test]
fn a_subregister_is_the_same_value_viewed_narrower() {
    let module = llrm_mir::parse::module(BYTE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let add = insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("add"))).expect("an add");
    let ext =
        insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("movzx"))).expect("a movzx");
    let (add, ext) = (add.what.as_ref().unwrap(), ext.what.as_ref().unwrap());
    let (crate::model::ir::Loc::Held(written), crate::model::ir::Loc::Held(read)) = (&add.dests[0], &ext.sources[0])
    else {
        panic!("{add:?} {ext:?}")
    };
    assert_eq!((written.value, written.width, read.value, read.width), (written.value, 4, written.value, 1));
}

const FLOAT64: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define double @f(i64 %a) {
entry:
  %d = uitofp i64 %a to double
  ret double %d
}
";

/// A conversion of an `i64` to a float is lowered by the target's hooks while
/// the types are legalized; the driver gave the type legalizer none, so the
/// function stopped at `Legalize(UIntToFp ... expand operand)` (15 + 11 + 6
/// functions of the corpora).
#[test]
fn the_type_legalizer_has_the_targets_hooks() {
    let module = llrm_mir::parse::module(FLOAT64).expect("parses");
    let why = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .err()
        .map_or_else(String::new, |e| e.0);
    assert!(!why.contains("expand operand"), "{why}");
}

const DIVIDE: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  %q = sdiv i32 %a, %b
  ret i32 %q
}
";

/// A signed divide sign-extends EAX into EDX, divides by the operand and
/// reads the quotient out of EAX: `CDQ` and `IDIV32r` were unmapped in 14 + 14
/// functions of the corpora. The quotient is the divide's first result.
#[test]
fn a_divide_is_cdq_then_idiv_on_eax_and_edx() {
    let module = llrm_mir::parse::module(DIVIDE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<String> = done
        .body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .map(|w| w.name.as_deref().unwrap_or("").to_owned())
        .collect();
    let (cdq, idiv) =
        (names.iter().position(|n| n == "cdq").expect("cdq"), names.iter().position(|n| n == "idiv").expect("idiv"));
    assert!(cdq + 1 == idiv, "cdq right before idiv: {names:?}");
    // Physical EAX/EDX operands are invisible to the allocator: it emitted
    // `idiv eax` and trapped. The operands are values the form pins.
    let insns = done.body.insns();
    let divide = insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("idiv")).unwrap();
    // (The divisor may be the memory operand, as `IDIV32m`.)
    assert!(
        divide.dests.iter().chain(divide.sources.iter().take(2)).all(|l| matches!(l, crate::model::ir::Loc::Held(_))),
        "idiv's quotient, remainder and dividend are held values: {divide:?}"
    );
}

const STORES_THE_SAME_CONSTANT: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@a = global i32 0
@b = global i32 0
define void @f() {
entry:
  store i32 -1, ptr @a
  store i32 -1, ptr @b
  ret void
}
";

/// The DAG path never knew `-Os`: `opt_for_size` was a constant false, so
/// `-1` stored twice was two `mov [m], imm32` where the pin
/// (`shouldAvoidImmediateInstFormsForSize`) loads it once; 41 functions of the
/// corpus shadow at `-Os` (lru, scroll, `MOV32r1`) differed by this.
#[test]
fn at_os_a_constant_stored_twice_is_loaded_once() {
    let module = llrm_mir::parse::module(STORES_THE_SAME_CONSTANT).expect("parses");
    let moves = |size: bool| {
        let mut cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
        cpu.size = size;
        let done = selected(&module, "f", &llrm_x86_m32::M32, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
        done.body.insns().iter().filter(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("mov"))).count()
    };
    assert_eq!(moves(false), 2, "speed: both stores take the immediate");
    assert_eq!(moves(true), 3, "-Os: the constant is loaded once and both stores take the register");
}
const TIMES_SEVEN_PLUS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %y) {
  %b = xor i32 %a, %y
  %m = mul i32 %y, 7
  %t = add i32 %m, %b
  ret i32 %t
}
";

/// `y * 7 + b` was `shl; sub; add` (three); the pin's `combineAdd` makes the
/// hidden LEA: `lea b + y*8; sub y` (histo, 13 functions at `-Os`).
#[test]
fn a_shifted_difference_plus_a_value_is_a_lea_and_a_sub() {
    let module = llrm_mir::parse::module(TIMES_SEVEN_PLUS).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<String> =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
    assert!(names.iter().any(|n| n == "lea"), "{names:?}");
    assert!(!names.iter().any(|n| n == "shl"), "{names:?}");
}

const WORD_CHAIN: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i16 %x) {
  %m = mul i16 %x, 25173
  %a = add i16 %m, 13849
  %c = and i16 %a, 4095
  %z = zext i16 %c to i32
  ret i32 %z
}
";

/// A chain of word operations at m32 is done in dwords, and the casts between
/// them fold (`aext (trunc x)` is `x`): it was `mov; imul; add; and` each
/// narrowed and widened again, three `INSERT_SUBREG` and a `movzx` more than
/// the pin (bintree, 13 functions of the corpus shadow).
#[test]
fn a_chain_of_word_operations_has_no_casts_between_them() {
    let module = llrm_mir::parse::module(WORD_CHAIN).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<String> =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
    assert!(!names.iter().any(|n| n.starts_with("movzx") || n.starts_with("movsx")), "{names:?}");
    assert_eq!(names.iter().filter(|n| matches!(n.as_str(), "imul" | "add" | "and")).count(), 3, "{names:?}");
}

/// `read_before_defined` kept a hash set of every value defined on every path
/// to each block: a chain of N blocks defining N values was N^2 hashes (28% of
/// a whole compile at the flip). A chain of 4000 blocks with 16000 values is a
/// few words per block now; it also still finds a value one path leaves out.
#[test]
fn the_check_for_a_value_read_before_it_is_defined_is_linear_in_the_body() {
    use std::sync::Arc;

    use super::emit::EmitBlock;
    use crate::model::ir::{Held, Loc, Operation, Semantics};
    use crate::model::lir::Insn;
    let defining = |at: i64, defines: Vec<u32>, uses: Vec<u32>| {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(Held { value: defines.first().copied().unwrap_or(0), width: 4 })],
            sources: Vec::new(),
            ..Semantics::new(Operation::Move)
        };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    };
    let blocks: Vec<EmitBlock> = (0..4000)
        .map(|at| EmitBlock {
            at,
            phis: Vec::new(),
            insns: (0..4)
                .map(|k| {
                    let value = (at * 4 + k + 1) as u32;
                    defining(at, vec![value], if at > 0 && k == 0 { vec![(at * 4) as u32] } else { vec![] })
                })
                .collect(),
        })
        .collect();
    let succ: Vec<Vec<i64>> = (0..4000).map(|at| if at + 1 < 4000 { vec![at + 1] } else { vec![] }).collect();
    let started = std::time::Instant::now();
    assert_eq!(super::dag_function::read_before_defined(&blocks, &succ, None), None);
    assert!(started.elapsed().as_secs_f64() < 2.0, "{:?}", started.elapsed());
    // a use of a value no block defines is found
    let mut missing = blocks;
    missing[3999].insns.push(defining(3999, vec![], vec![999_999]));
    assert_eq!(super::dag_function::read_before_defined(&missing, &succ, None).map(|(value, _)| value), Some(999_999));
}

const COMPARE_WITH_LOADED: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(ptr %p, i32 %k) {
  %v = load i32, ptr %p
  %c = icmp slt i32 %k, %v
  br i1 %c, label %a, label %b
a:
  ret i32 1
b:
  ret i32 2
}
";

/// A node the combiner deleted still counted as a user of its operands in the
/// matcher, so a load read by the compare never folded (`hasOneUse`): `mov;
/// sub r, r` where the pin has `sub r, [mem]` (bintree, 6 functions of the
/// corpus shadow; the same for `imul r, [mem], imm`).
#[test]
fn a_load_read_by_a_compare_is_folded_into_it() {
    use crate::model::ir::Loc;
    let module = llrm_mir::parse::module(COMPARE_WITH_LOADED).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let compare = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .find(|w| matches!(w.name.as_deref(), Some("sub" | "cmp")))
        .expect("a compare");
    assert!(compare.sources.iter().any(|l| matches!(l, Loc::Mem(_))), "{compare:?}");
}

const WORD_MINUS_ONE_TWICE: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"
@a = global i16 0
@b = global i16 0
define void @f() {
  store i16 -1, ptr @a
  store i16 -1, ptr @b
  ret void
}
";

/// At -Os the pin's `OptForSize, Not64BitMode` patterns make a word -1 by
/// `MOV32r_1`, a dword `mov eax, -1` (6 bytes at -m16 with the 0x66 prefix;
/// QCport weapons.c); the word constant is `mov ax, -1` (3).
#[test]
fn at_os_m16_a_word_constant_is_made_in_a_word_register() {
    use crate::model::ir::Loc;
    let module = llrm_mir::parse::module(WORD_MINUS_ONE_TWICE).expect("parses");
    let mut cpu = crate::backend::cpu::profile("386").expect("a cpu").clone();
    cpu.size = true;
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let wide: Vec<_> = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| matches!(w.dests.as_slice(), [Loc::Held(h)] if h.width == 4))
        .collect();
    assert!(wide.is_empty(), "{wide:?}");
}

const TESTS_THE_LOW_BIT_OF_A_LOAD: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @g()
declare void @h()
define void @f(i32 %c) {
  %t = trunc i32 %c to i1
  br i1 %t, label %a, label %b
a:
  call void @g()
  ret void
b:
  call void @h()
  ret void
}
";

/// `(c & 1) ? g() : h()` was `mov eax, [esp+4]; and al, 1; je` (6 bytes
/// and a register): the pin narrows the load of the low byte and tests it in
/// memory, `test byte [esp+4], 1` (5). `reduceLoadWidth` of the truncate, and
/// no dead `brcond` counted as a reader of the `and`. x_crc32 -O3: +160 bytes.
#[test]
fn the_low_bit_of_an_argument_is_tested_in_memory() {
    use crate::model::ir::Loc;
    let module = llrm_mir::parse::module(TESTS_THE_LOW_BIT_OF_A_LOAD).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let reads: Vec<u32> = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .flat_map(|w| w.sources.iter())
        .filter_map(|l| if let Loc::Mem(m) = l { Some(m.width) } else { None })
        .collect();
    assert_eq!(reads, [1], "one byte read of the argument, no dword: {reads:?}");
}

const WORD_FROM_COMPARE: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
@slot = global i16 0
declare cc 1005 i16 @c(i16)
declare cc 1005 void @u(i16)
define cc 1005 void @f(i16 %a, i16 %b) {
  %v = call cc 1005 i16 @c(i16 %a)
  %t = icmp ne i16 %v, 0
  %z = zext i1 %t to i16
  store i16 %z, ptr @slot
  call cc 1005 void @u(i16 %b)
  %s = load i16, ptr @slot
  %r = add i16 %s, %z
  store i16 %r, ptr @slot
  ret void
}
";

/// The pin cuts a word from `movzx r32, r8`; held as that dword, a flag that
/// lives across a call was spilled and reloaded as a dword and could not sit
/// in SI/DI (QCport `-Os`: `xor ecx, ecx; setne cl; mov dword ptr [bp-N]`).
/// The `movzx` is made at word width.
#[test]
fn a_word_zero_extended_from_a_byte_is_a_word_value() {
    use crate::model::ir::Loc;
    let module = llrm_mir::parse::module(WORD_FROM_COMPARE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let extend =
        insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("movzx")).expect("a movzx");
    assert!(matches!(extend.dests.as_slice(), [Loc::Held(h)] if h.width == 2), "{extend:?}");
}

const DWORD_AND_WORD_FROM_COMPARE: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
@word = global i16 0
@dword = global i32 0
declare cc 1005 i16 @c(i16)
define cc 1005 void @f(i16 %a) {
  %v = call cc 1005 i16 @c(i16 %a)
  %t = icmp ne i16 %v, 0
  %z = zext i1 %t to i32
  %w = trunc i32 %z to i16
  store i16 %w, ptr @word
  store i32 %z, ptr @dword
  ret void
}
";

/// The dword a `movzx` made is narrowed only when the word is all that is
/// read: with a dword reader too, `movzx ax, al` left the high half stale
/// (regparm3_matrix_31 and _4 printed wrong sums).
#[test]
fn a_zero_extended_byte_read_as_a_dword_too_stays_a_dword() {
    use crate::model::ir::Loc;
    let module = llrm_mir::parse::module(DWORD_AND_WORD_FROM_COMPARE).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let extend =
        insns.iter().filter_map(|i| i.what.as_ref()).find(|w| w.name.as_deref() == Some("movzx")).expect("a movzx");
    assert!(matches!(extend.dests.as_slice(), [Loc::Held(h)] if h.width == 4), "{extend:?}");
}
