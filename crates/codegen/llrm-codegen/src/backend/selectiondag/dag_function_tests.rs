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

pub(super) const M16_STACK_ARGS: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
declare i16 @g(i16, i16)
define i16 @f(i16 %a) {
entry:
  %r = call i16 @g(i16 %a, i16 233)
  ret i16 %r
}
";

pub(super) const FAR_CALL: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"
declare void @far() addrspace(1)
declare void @near()
define void @f() {
entry:
  call addrspace(1) void @far()
  call void @near()
  ret void
}
";

pub(super) const QB_STRING_COMPARE: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
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
    let contracts = super::dag_function::RuntimeContracts {
        abi: &Stub { flags: false },
        segments: &crate::backend::target::BASIC,
        runtime: true,
        frame_zeroed: false,
        facts: None,
    };
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

const SUM_PLUS_TIMES_SEVEN: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %s, i32 %u, i32 %y) {
  %b = add i32 %s, %u
  %m = mul i32 %y, 7
  %t = add i32 %b, %m
  ret i32 %t
}
";

/// The pin's `m_Add` is commutative: `b + y * 7` is the hidden LEA whichever
/// operand is the difference. Ours took only `y * 7 + b`, so x_dct (`5a + 3c +
/// 7(b - d)`) kept `lea; sub; add; add` and ran 23 instructions over the pin.
#[test]
fn a_value_plus_a_shifted_difference_is_a_lea_and_a_sub() {
    let module = llrm_mir::parse::module(SUM_PLUS_TIMES_SEVEN).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<String> =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
    assert!(!names.iter().any(|n| n == "shl"), "{names:?}");
    assert_eq!(names.iter().filter(|n| matches!(n.as_str(), "add" | "sub" | "lea")).count(), 3, "{names:?}");
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

const WORD_MINUS_ONE_TWICE: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"
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

const WORD_FROM_COMPARE: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
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

const DWORD_AND_WORD_FROM_COMPARE: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
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

const SIGN_SELECTS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %x) {
  %c = icmp sgt i32 %x, -1
  %r = select i1 %c, i32 1, i32 -1
  ret i32 %r
}
define i32 @g(i32 %x) {
  %c = icmp slt i32 %x, 0
  %r = select i1 %c, i32 -6, i32 5
  ret i32 %r
}
";

/// `x > -1 ? 1 : -1` and `x < 0 ? ~5 : 5` were a test and a branch (`mov; test;
/// jns; mov`); the pin's `SimplifySelectCC` makes the sign mask:
/// `sar eax, 31; or eax, 1` and `sar eax, 31; xor eax, 5`.
#[test]
fn a_select_on_the_sign_is_the_sign_mask_not_a_branch() {
    let module = llrm_mir::parse::module(SIGN_SELECTS).expect("parses");
    for name in ["f", "g"] {
        let done =
            selected(&module, name, &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
                .unwrap_or_else(|e| panic!("{}", e.0));
        let insns = done.body.insns();
        let seen: Vec<&str> = insns.iter().filter_map(|i| i.what.as_ref()).filter_map(|w| w.name.as_deref()).collect();
        assert!(seen.contains(&"sar") && !seen.iter().any(|n| n.starts_with('j')), "{name}: {seen:?}");
    }
}

/// `if (c & 1) g(); else h();` written as a compare of the masked value, in
/// both polarities and both arm orders, was `mov eax, [esp+4]; and al, 1; je`:
/// only the truncate form narrowed. The pin's `X86ISD::CMP` case makes every
/// one `test byte [esp+4], 1`.
#[test]
fn a_masked_compare_with_zero_tests_one_byte_in_memory_whatever_its_polarity() {
    use crate::model::ir::Loc;
    for cc in ["eq", "ne"] {
        for (yes, no) in [("a", "b"), ("b", "a")] {
            let ir = format!(
                "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare void @g()
declare void @h()
define void @f(i32 %c) {{
  %m = and i32 %c, 1
  %t = icmp {cc} i32 %m, 0
  br i1 %t, label %{yes}, label %{no}
a:
  call void @g()
  ret void
b:
  call void @h()
  ret void
}}
"
            );
            let module = llrm_mir::parse::module(&ir).expect("parses");
            let done =
                selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
                    .unwrap_or_else(|e| panic!("{}", e.0));
            let insns = done.body.insns();
            let whats: Vec<_> = insns.iter().filter_map(|i| i.what.as_ref()).collect();
            let reads: Vec<u32> = whats
                .iter()
                .flat_map(|w| w.sources.iter())
                .filter_map(|l| if let Loc::Mem(m) = l { Some(m.width) } else { None })
                .collect();
            assert_eq!(reads, [1], "{cc} {yes}/{no}: one byte read, no dword: {reads:?}");
            assert!(
                !whats.iter().any(|w| w.name.as_deref() == Some("and")),
                "{cc} {yes}/{no}: no and left before the test"
            );
        }
    }
}

/// A function that calls `setjmp` is marked on its body, as the old selector
/// marks it: after a second return from `setjmp` a frame slot another value
/// shared holds that value, so no slot may be shared (`slots_colored`, the
/// spill colouring, stands down for `LirBody::returns_twice`). The DAG path
/// never set the mark.
#[test]
fn a_function_calling_setjmp_is_marked_returns_twice() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare i32 @setjmp(ptr) returns_twice
define i32 @f() {
entry:
  %buf = alloca [16 x i8]
  %r = call i32 @setjmp(ptr %buf)
  ret i32 %r
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .expect("selects");
    assert!(done.body.returns_twice);
    let plain = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f() {
entry:
  ret i32 1
}
";
    let module = llrm_mir::parse::module(plain).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .expect("selects");
    assert!(!done.body.returns_twice);
}

/// A constant a phi receives was `mov v3, 0; mov v4, v3`: the constant load
/// made a value of its own and a copy carried it to the phi's register. The
/// pin's `MOV32r0` defines that register, so `precse` sees one zero on both
/// edges (x_life kept a second return block, +5 instructions).
#[test]
fn a_constant_a_phi_receives_is_defined_in_the_phis_register() {
    let ir = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(ptr %p, i32 %n, i32 %k) {
b1:
  %c = icmp ne i32 %k, 0
  br i1 %c, label %pre, label %b2
pre:
  br label %b6
b2:
  %s = phi i32 [ 0, %b1 ], [ %v11, %b6 ]
  ret i32 %s
b6:
  %p7 = phi ptr [ %p, %pre ], [ %p13, %b6 ]
  %n8 = phi i32 [ %n, %pre ], [ %n12, %b6 ]
  %a9 = phi i32 [ %v11, %b6 ], [ 0, %pre ]
  %v10 = load i32, ptr %p7
  %v11 = add i32 %v10, %a9
  %n12 = add i32 %n8, -1
  %p13 = getelementptr i8, ptr %p7, i32 4
  %e = icmp eq i32 %n12, 0
  br i1 %e, label %b2, label %b6
}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let copies = insns
        .iter()
        .filter_map(|i| i.what.as_ref())
        .filter(|w| w.name.as_deref() == Some("mov"))
        .filter(|w| matches!(w.sources.as_slice(), [crate::model::ir::Loc::Held(_)]))
        .count();
    assert_eq!(copies, 0, "{} register copies", copies);
}

/// `a << (i & 31)` on 64 bits kept the select on bit 5 of the amount as `xor
/// ecx, ecx; test cl, cl; je` after `shld; shl` (100 vs the pin's 72 insns in
/// x_ll_arith): bit 5 of `i & 31` is known zero, so the pin has no fixup.
#[test]
fn a_wide_shift_by_a_masked_amount_has_no_fixup_branch() {
    let ir = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @f(i64 %a, i32 %i) {
  %m = and i32 %i, 31
  %z = zext i32 %m to i64
  %r = shl i64 %a, %z
  ret i64 %r
}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let seen: Vec<&str> = insns.iter().filter_map(|i| i.what.as_ref()).filter_map(|w| w.name.as_deref()).collect();
    assert!(!seen.iter().any(|n| n.starts_with('j') || *n == "test"), "{seen:?}");
}

/// A 64-bit add made the carry a byte and put it back (`add; setb al; add al,
/// -1; adc`, three instructions more than the pin's `add; adc`): the pin's
/// `combineCarryThroughADD` reads the flags the `setb` read.
#[test]
fn a_wide_add_passes_the_carry_in_the_flags() {
    let ir = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i64 @f(i64 %a, i64 %b) {
  %r = add i64 %a, %b
  ret i64 %r
}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let seen: Vec<&str> = insns.iter().filter_map(|i| i.what.as_ref()).filter_map(|w| w.name.as_deref()).collect();
    assert!(seen.contains(&"adc") && !seen.contains(&"setb"), "{seen:?}");
}

const REMAINDER_BY_ELEVEN: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a) {
  %r = srem i32 %a, 11
  ret i32 %r
}
";

/// `x % 11` at `-Os` was the multiply-by-reciprocal sequence on the DAG path
/// (imul, shifts, a 5-byte magic immediate; recsum.c m16 -Os 1.53 times the old
/// selector's clocks), where the old selector keeps the `idiv` at -Os.
#[test]
fn at_os_a_remainder_by_a_constant_keeps_its_idiv() {
    let module = llrm_mir::parse::module(REMAINDER_BY_ELEVEN).expect("parses");
    let names = |size: bool| {
        let mut cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
        cpu.size = size;
        let done = selected(&module, "f", &llrm_x86_m32::M32, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
        done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect::<Vec<_>>()
    };
    assert!(names(false).iter().all(|n| n != "idiv"), "speed: the reciprocal, as the pin");
    assert!(names(true).iter().any(|n| n == "idiv"), "-Os: the divide stays: {:?}", names(true));
}

/// `x * 24` at `-Os` was `shl 3` then `imul 3` (two 4-byte instructions at m16)
/// where the pin keeps the one `imul` (`combineMul`: `hasMinSize` returns
/// first): bench/mandel.nib m16 -Os, +8 B. The pin declines at `minsize`
/// only, so at m32 -Os the chain stays: histo's `x * 7` was `imul` (13+ clocks
/// on a 486) against the pin's `lea; sub`, +25.5% clk, x_dct +60% clk.
#[test]
fn at_os_a_multiply_by_a_constant_is_one_imul_at_m16_and_a_chain_at_m32() {
    let names = |pointer: &str, size: bool, factor: u32| {
        let module = llrm_mir::parse::module(&format!(
            "target datalayout = \"e-p:{pointer}:{pointer}-i64:32-n8:16:32\"
define i32 @f(i32 %a) {{
  %r = mul i32 %a, {factor}
  ret i32 %r
}}
"
        ))
        .expect("parses");
        let mut cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
        cpu.size = size;
        let done = selected(&module, "f", &llrm_x86_m32::M32, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
        done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect::<Vec<_>>()
    };
    let count = |names: &[String], one: &str| names.iter().filter(|n| *n == one).count();
    let speed = names("32", false, 24);
    assert_eq!((count(&speed, "imul"), count(&speed, "shl")), (0, 1), "speed: lea and shift");
    let m16 = names("16", true, 24);
    assert_eq!((count(&m16, "imul"), count(&m16, "shl")), (1, 0), "m16 -Os: {m16:?}");
    let m32 = names("32", true, 24);
    assert_eq!((count(&m32, "imul"), count(&m32, "shl")), (0, 1), "m32 -Os: {m32:?}");
    let seven = names("32", true, 7);
    assert_eq!(count(&seven, "imul"), 0, "m32 -Os x*7: {seven:?}");
}

/// `u8 % 6` at `-Os` with the divide kept was `CannotSelect("UDivRem")` (SD302:
/// no byte divide): examples/dice.nib and easing.nib stopped compiling.
#[test]
fn at_os_a_byte_remainder_by_a_constant_still_selects() {
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i8 @f(i8 %a) {
  %r = urem i8 %a, 6
  ret i8 %r
}
",
    )
    .expect("parses");
    let mut cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    cpu.size = true;
    selected(&module, "f", &llrm_x86_m32::M32, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
}

const COPY_4000_BYTES: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"
@a = global [2000 x i16] zeroinitializer, align 2
@b = global [2000 x i16] zeroinitializer, align 2
declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
define void @f() {
  call void @llvm.memcpy.p0.p0.i16(ptr @b, ptr @a, i16 4000, i1 false)
  ret void
}
";

/// A 4000-byte copy at m16 with no `memcpy` in the runtime was `rep movsb`
/// (4 times the iterations of the old selector's `rep movsd`; scroll.c).
#[test]
fn a_big_copy_without_a_memcpy_routine_moves_dwords() {
    let module = llrm_mir::parse::module(COPY_4000_BYTES).expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<_> =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
    assert!(names.iter().any(|n| n == "movsd"), "{names:?}");
}

/// The dword copy used a pointer-typed count: a far destination made it
/// "integer constant of Simple(Far)" and examples/level.nib stopped compiling.
#[test]
fn a_big_copy_to_a_far_pointer_still_selects() {
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32\"
@b = global [100 x i16] zeroinitializer, align 2
declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1), ptr, i16, i1)
define void @f(ptr addrspace(1) %d) {
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %d, ptr @b, i16 202, i1 false)
  ret void
}
",
    )
    .expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
}

/// A copy of `n` bytes at m16 with no `memcpy` in the runtime was one `rep
/// movsb`; the old selector moves `n / 4` dwords and then `n % 4` bytes
/// (`copy_idioms.c`, 4 times the iterations of the old copy).
#[test]
fn a_copy_of_a_variable_size_moves_dwords_and_then_bytes() {
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"
@a = global [100 x i16] zeroinitializer, align 2
@b = global [100 x i16] zeroinitializer, align 2
declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
define void @f(i16 %n) {
  call void @llvm.memcpy.p0.p0.i16(ptr @a, ptr @b, i16 %n, i1 false)
  ret void
}
",
    )
    .expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let names: Vec<_> =
        done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
    let at = |name: &str| names.iter().position(|n| n == name);
    assert!(at("movsd") < at("movsb") && at("movsd").is_some(), "{names:?}");
}

/// A copy out of a global was loaded a segment register (`pushw DGROUP; pop
/// fs`) and read through an `fs:` override; the old selector reads it through
/// DS, which holds DGROUP (copy_idioms.c: 3 bytes a copy).
#[test]
fn a_copy_out_of_a_global_reads_through_ds() {
    use crate::model::ir::Loc;
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"
@a = global [100 x i16] zeroinitializer, align 2
@b = global [100 x i16] zeroinitializer, align 2
declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
define void @f(i16 %n) {
  call void @llvm.memcpy.p0.p0.i16(ptr @a, ptr @b, i16 %n, i1 false)
  ret void
}
",
    )
    .expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let copies: Vec<_> =
        insns.iter().filter_map(|i| i.what.as_ref()).filter(|w| w.op == crate::model::ir::Operation::Copy).collect();
    assert!(!copies.is_empty());
    for copy in copies {
        let from = &copy.sources[copy.sources.len() - 2];
        assert!(matches!(from, Loc::Reg(r) if r.register == llrm_lir::registers::RegId::DS), "{from:?}");
    }
}

const SEGMENT_LOADED_IN_TWO_BLOCKS: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16-n8:16:32\"
define cc 1000 void @f() addrspace(1) {
b1:
  %0 = load volatile i8, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 64 to ptr addrspace(2)) to ptr addrspace(1)), i16 108)
  br label %b3
b3:
  %1 = load volatile i8, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr addrspace(2) inttoptr (i16 64 to ptr addrspace(2)) to ptr addrspace(1)), i16 108)
  %2 = icmp ne i8 %0, %1
  br i1 %2, label %b4, label %b3
b4:
  ret void
}
";

/// A segment constant put in ES in two blocks was one value defined twice
/// ('value N is defined 2 times in a body that should be in SSA': QB's
/// `t = PEEK(&H6C): DO: LOOP UNTIL PEEK(&H6C) <> t`, 59 tests of llrm-qb and
/// llrm-nib under the DAG default).
#[test]
fn a_selector_put_in_es_in_two_blocks_is_two_values() {
    let selected = select_with_contracts(SEGMENT_LOADED_IN_TWO_BLOCKS).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(crate::backend::verify::verify(&selected.body, true), Vec::<String>::new());
}

/// QB's runtime `gfxdev.c` `gd_fill_select`, under `LLRM_VERIFY`: the scheduler
/// cloned the `and` whose flags the branch reads, and the clone defined the
/// value the first one did ('value N is defined 2 times in a body that should
/// be in SSA'); `link_check` refused every batch that touched `runtime/`.
const FLAGS_PRODUCER_CLONED: &str = r#"target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"
@_boxes = internal constant <{ ptr addrspace(1), ptr addrspace(1), ptr addrspace(1), ptr addrspace(1) }> zeroinitializer
@_octets = internal constant <{ ptr addrspace(1), ptr addrspace(1), ptr addrspace(1), ptr addrspace(1) }> zeroinitializer
@_dots = internal constant <{ ptr addrspace(1), ptr addrspace(1), ptr addrspace(1), ptr addrspace(1), ptr addrspace(1), ptr addrspace(1), ptr addrspace(1), ptr addrspace(1) }> zeroinitializer
@_lines = internal constant <{ ptr addrspace(1), ptr addrspace(1), ptr addrspace(1), ptr addrspace(1) }> zeroinitializer
@_kind = internal unnamed_addr global [2 x i8] zeroinitializer
declare cc 1005 i16 @B$FSEL(i16, i16) addrspace(1)
declare cc 1005 i16 @B$FLSEL(i16, i16) addrspace(1)
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!7 = !{!6, !6, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
define cc 1005 void @f(ptr noundef nocapture nofree writeonly initializes((0, 26)) %0, i16 noundef %1, i16 noundef %2) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %3 = and i16 %1, 255
  %4 = load i16, ptr @_kind, !tbaa !9
  switch i16 %4, label %b2 [
  ]
b2:
  %5 = phi i16 [ %3, %b1 ], [ %26, %b3 ], [ %28, %b4 ], [ %30, %b5 ]
  %6 = and i16 %2, 3
  %7 = shl nuw i16 %4, 2
  %8 = getelementptr inbounds i8, ptr @_boxes, i16 %7
  %9 = load ptr addrspace(1), ptr %8, !tbaa !7
  %10 = shl nuw i16 %4, 3
  %11 = getelementptr inbounds i8, ptr @_dots, i16 %10
  %12 = icmp eq i16 %6, 0
  %13 = select i1 %12, i16 4, i16 0
  %14 = getelementptr inbounds nuw i8, ptr %11, i16 %13
  %15 = load ptr addrspace(1), ptr %14, align 4, !tbaa !7
  %16 = getelementptr inbounds nuw i8, ptr %0, i16 4
  store ptr addrspace(1) %15, ptr %16, !tbaa !7
  %17 = getelementptr inbounds i8, ptr @_lines, i16 %7
  %18 = load ptr addrspace(1), ptr %17, !tbaa !7
  %19 = getelementptr inbounds nuw i8, ptr %0, i16 8
  %20 = getelementptr inbounds i8, ptr @_octets, i16 %7
  %21 = load ptr addrspace(1), ptr %20, !tbaa !7
  %22 = getelementptr inbounds nuw i8, ptr %0, i16 12
  %23 = getelementptr inbounds nuw i8, ptr %0, i16 16
  %24 = getelementptr inbounds nuw i8, ptr %0, i16 24
  %25 = getelementptr inbounds nuw i8, ptr %0, i16 18
  br i1 %12, label %b7, label %b6
b3:
  %26 = and i16 %1, 15
  br label %b2
b4:
  %27 = and i16 %1, 3
  %28 = mul nuw nsw i16 %27, 85
  br label %b2
b5:
  %29 = trunc i16 %1 to i1
  %30 = select i1 %29, i16 255, i16 0
  br label %b2
b6:
  switch i16 %6, label %b13 [
    i16 1, label %b7
    i16 2, label %b14
  ]
b7:
  %31 = phi i16 [ 0, %b2 ], [ %5, %b6 ], [ %38, %b14 ], [ 255, %b13 ]
  %32 = getelementptr inbounds nuw i8, ptr %0, i16 20
  store i16 %31, ptr %32, !tbaa !9
  %33 = icmp eq i16 %6, 1
  %34 = select i1 %33, i16 0, i16 %5
  %35 = getelementptr inbounds nuw i8, ptr %0, i16 22
  %36 = tail call cc 1005 addrspace(1) i16 @B$FSEL(i16 noundef %6, i16 noundef %5)
  %37 = tail call cc 1005 addrspace(1) i16 @B$FLSEL(i16 noundef %6, i16 noundef %5)
  ret void
b13:
  br label %b7
b14:
  %38 = xor i16 %5, 255
  br label %b7
}
"#;

#[test]
fn a_cloned_flag_producer_defines_a_value_of_its_own() {
    let selected = select_with_contracts(FLAGS_PRODUCER_CLONED).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(crate::backend::verify::verify(&selected.body, true), Vec::<String>::new());
}

fn move_3840_at_m16(metadata: &str) -> crate::backend::isel::Selected {
    let module = llrm_mir::parse::module(&format!(
        "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"
@s = global [2000 x i16] zeroinitializer, align 2
declare void @llvm.memmove.p0.p0.i16(ptr, ptr, i16, i1)
define void @f() {{
  call void @llvm.memmove.p0.p0.i16(ptr @s, ptr getelementptr (i8, ptr @s, i16 160), i16 3840, i1 false){metadata}
  ret void
}}
!0 = !{{}}
"
    ))
    .expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0))
}

/// A memmove a pass proved forward or backward was a call of the runtime's
/// `memmove` at m16 (scroll.c: 2.2x the old selector's clocks, a frame for the
/// value kept across the call); the old selector moves it inline, `rep movsd`
/// up or `push ds; pop es; std; rep movsd; cld` down.
#[test]
fn a_proved_move_at_m16_is_inline_string_code() {
    let up = move_3840_at_m16(", !llrm.forward !0");
    let names: Vec<_> =
        up.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
    assert!(names.iter().any(|n| n == "movsd") && names.iter().all(|n| n != "call"), "{names:?}");
    let down = move_3840_at_m16(", !llrm.backward !0");
    assert!(
        down.inline.values().any(|bytes| bytes[..] == [0x1E, 0x07, 0xFD, 0xF3, 0x66, 0xA5, 0xFC]),
        "{:?}",
        down.inline
    );
}

/// A forward string move of 3840 bytes (a multiple of four) was followed by
/// `xor cx, cx; rep movsb`, a copy of zero bytes (scroll.c m16 -Os, +3 B).
#[test]
fn a_forward_string_move_of_whole_dwords_has_no_byte_tail() {
    let up = move_3840_at_m16(", !llrm.forward !0");
    let names: Vec<_> =
        up.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.as_deref().map(str::to_owned)).collect();
    assert!(names.iter().any(|n| n == "movsd") && names.iter().all(|n| n != "movsb"), "{names:?}");
}

/// A byte argument was extended to the dword register (`mov edx, 12`, 6 bytes
/// at m16) though the callee truncates it; the old selector writes `dl`
/// (mandel.nib: 4 bytes a call).
#[test]
fn a_byte_argument_at_m16_goes_in_the_byte_register() {
    use llrm_lir::registers::RegId;
    let module = llrm_mir::parse::module(
        "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"
declare cc 1005 void @g(i32, i8 zeroext)
define void @f() {
  call cc 1005 void @g(i32 204, i8 zeroext 12)
  ret void
}
",
    )
    .expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu").clone();
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|i| i.call.is_some()).expect("a call");
    let registers: Vec<RegId> = call.requires.iter().map(|(_, register)| *register).collect();
    assert!(registers.contains(&RegId::DL) && !registers.contains(&RegId::EDX), "{registers:?}");
}

/// `(sext a * sext b) >> 32` kept the double-word multiply's cross products
/// (20 instructions); both operands are `SEXTLOAD i32`, so the pin has one
/// `imul`.
#[test]
fn the_high_word_of_a_signed_widening_multiply_is_one_imul() {
    let ir = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"
define i32 @f(i32 %a, i32 %b) {
  %x = sext i32 %a to i64
  %y = sext i32 %b to i64
  %m = mul i64 %x, %y
  %s = lshr i64 %m, 32
  %r = trunc i64 %s to i32
  ret i32 %r
}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let seen: Vec<&str> = insns.iter().filter_map(|i| i.what.as_ref()).filter_map(|w| w.name.as_deref()).collect();
    assert!(seen.len() <= 4 && seen.iter().any(|n| n.starts_with("imul")), "{seen:?}");
}

/// m16 `a * 10` on `long long` (f_mul_c) multiplied the high word with `imul
/// ecx, ecx, 5`; the pin's hidden `lea` takes the 386's 32-bit form there
/// (SD3 keeps scaled indexes out of 16-bit addressing, not out of an `lea`).
#[test]
fn at_m16_a_multiply_by_five_is_an_lea() {
    let ir = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"
define i32 @f(i32 %a) {
  %r = mul i32 %a, 5
  ret i32 %r
}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let seen: Vec<&str> = insns.iter().filter_map(|i| i.what.as_ref()).filter_map(|w| w.name.as_deref()).collect();
    assert!(seen.iter().any(|n| n.starts_with("lea")) && !seen.iter().any(|n| n.starts_with("imul")), "{seen:?}");
}

/// m16 `long long a * 10` (f_mul_c) added the high words with `add ecx, ecx;
/// add ecx, edx; mov edx, ecx`; the pin's dword `lea` takes the scaled
/// index: `lea ecx, [ecx+ecx*4]; lea edx, [edx+ecx*2]`.
#[test]
fn at_m16_a_qword_multiply_by_ten_is_two_leas_and_a_mul() {
    let ir = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"
define i64 @f(i64 %a) {
  %r = mul i64 %a, 10
  ret i64 %r
}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let seen: Vec<&str> = insns.iter().filter_map(|i| i.what.as_ref()).filter_map(|w| w.name.as_deref()).collect();
    assert_eq!(seen.iter().filter(|n| n.starts_with("lea")).count(), 2, "{seen:?}");
    assert!(!seen.iter().any(|n| n.starts_with("add") || n.starts_with("imul")), "{seen:?}");
}

/// The m16 dword `lea` with a scaled index carries the 0x66 and 0x67
/// prefixes: at `-Os` it grew the corpus by 214 bytes (SD250), so the
/// multiply by five stays an `imul` there.
#[test]
fn at_m16_os_a_multiply_by_five_stays_an_imul() {
    let ir = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3\"
define i32 @f(i32 %a) {
  %r = mul i32 %a, 5
  ret i32 %r
}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let mut cpu = crate::backend::cpu::profile("386").expect("a cpu").clone();
    cpu.size = true;
    let done = selected(&module, "f", &llrm_x86_m16::M16, &cpu, &[]).unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let seen: Vec<&str> = insns.iter().filter_map(|i| i.what.as_ref()).filter_map(|w| w.name.as_deref()).collect();
    assert!(seen.iter().any(|n| n.starts_with("imul")) && !seen.iter().any(|n| n.starts_with("lea")), "{seen:?}");
}

/// `bitcast float %f to i32` of an argument was `fld; fstp; and` through a
/// stack temporary (7 instructions); the pin reads the bits with the integer
/// load (`(conv (load x))` is `(load (conv*)x)`).
#[test]
fn a_float_argument_read_as_bits_is_an_integer_load() {
    let ir = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"
define i32 @f(float %x) {
  %i = bitcast float %x to i32
  %a = and i32 %i, 2147483647
  ret i32 %a
}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let seen: Vec<&str> = insns.iter().filter_map(|i| i.what.as_ref()).filter_map(|w| w.name.as_deref()).collect();
    assert!(!seen.iter().any(|n| n.starts_with("fld") || n.starts_with("fst")), "{seen:?}");
}

/// `(a[i] >> 8) & 255` was `mov; shr; movzx` (x_radix, +5.3% clk at -O3); the
/// pin reads the byte at `a + 1` with one `movzx`.
#[test]
fn a_byte_of_a_loaded_word_is_one_byte_load() {
    let ir = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(ptr %a, i32 %i) {
  %p = getelementptr i32, ptr %a, i32 %i
  %v = load i32, ptr %p
  %s = lshr i32 %v, 8
  %m = and i32 %s, 255
  ret i32 %m
}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let seen: Vec<&str> = insns.iter().filter_map(|i| i.what.as_ref()).filter_map(|w| w.name.as_deref()).collect();
    assert!(!seen.iter().any(|n| n.starts_with("sh")), "{seen:?}");
    assert!(seen.iter().any(|n| n.starts_with("movzx")), "{seen:?}");
}

/// `llvm.fshl` by a variable was expanded in MIR to `shl`, `shr 1`, `not`,
/// `and` and `shr` before selection (8 instructions at i32); the pin selects
/// one `shld` for a word or a dword, and for a byte (`LowerFunnelShift`)
/// widens to a dword, shifts and takes the high byte.
#[test]
fn a_word_or_dword_funnel_shift_is_one_shld_and_a_byte_one_selects() {
    for (bits, shld) in [(32, 1), (16, 1), (8, 0)] {
        let ir = format!(
            "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"
declare i{bits} @llvm.fshl.i{bits}(i{bits}, i{bits}, i{bits})
define i{bits} @f(i{bits} %a, i{bits} %b, i{bits} %y) {{
b0:
  %r = call i{bits} @llvm.fshl.i{bits}(i{bits} %a, i{bits} %b, i{bits} %y)
  ret i{bits} %r
}}
"
        );
        let mut module = llrm_mir::parse::module(&ir).expect("parses");
        crate::backend::funnel::expanded(
            &mut module,
            |_| true,
            |bits| crate::backend::funnel::kept_for_isel(crate::backend::shadow::Selector::Dag, &format!("i{bits}")),
        )
        .expect("expands");
        let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
            .unwrap_or_else(|e| panic!("i{bits}: {}", e.0));
        let names: Vec<_> = done.body.insns().iter().filter_map(|i| i.what.as_ref()?.name.clone()).collect();
        assert_eq!(names.iter().filter(|n| &***n == "shld").count(), shld, "i{bits}: {names:?}");
    }
}

/// `-g` under the DAG default produced no line table: the emitter carried no
/// source line (9 `.debug_line` rows under the old selector, 0 under the DAG).
/// Each instruction takes the `!dbg` line of the IR instruction its node was
/// built for.
#[test]
fn a_dbg_line_reaches_the_instructions_selected_from_it() {
    let ir = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"
define i32 @f(i32 %a, i32 %b) {
  %s = sub i32 %a, %b, !dbg !0
  %t = mul i32 %s, %b, !dbg !1
  ret i32 %t, !dbg !1
}
!0 = !{i32 7}
!1 = !{i32 8}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let lines: Vec<u32> = done.body.insns().iter().filter_map(|one| one.line.map(|at| at.line)).collect();
    assert!(lines.contains(&7) && lines.contains(&8), "{lines:?}");
}

/// A `!DILocation` column was dropped: every row of the line table said
/// column 0. The instructions selected from it carry (line, column).
#[test]
fn a_dbg_column_reaches_the_instructions_selected_from_it() {
    let ir = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"
define i32 @f(i32 %a, i32 %b) {
  %s = sub i32 %a, %b, !dbg !0
  %t = mul i32 %s, %b, !dbg !1
  ret i32 %t, !dbg !1
}
!0 = !DILocation(line: 7, column: 5)
!1 = !DILocation(line: 8, column: 12)
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let places: Vec<(u32, u32)> =
        done.body.insns().iter().filter_map(|one| one.line.map(|at| (at.line, at.column))).collect();
    assert!(places.contains(&(7, 5)) && places.contains(&(8, 12)), "{places:?}");
}

/// The scope and inlining call of a `!DILocation` were dropped: a function
/// inlined twice left no way to tell the two bodies' instructions apart.
#[test]
fn a_dbg_scope_and_inlined_at_reach_the_instructions_selected_from_it() {
    let ir = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"
define i32 @f(i32 %a, i32 %b) {
  %s = sub i32 %a, %b, !dbg !1
  %t = mul i32 %s, %b, !dbg !3
  ret i32 %t, !dbg !3
}
!0 = distinct !DISubprogram(name: \"f\")
!1 = !DILocation(line: 7, column: 5, scope: !0)
!2 = !DILocation(line: 9, column: 1, scope: !0)
!3 = !DILocation(line: 8, column: 12, scope: !0, inlinedAt: !2)
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let places: Vec<(u32, u32, u32)> =
        done.body.insns().iter().filter_map(|one| one.line.map(|at| (at.line, at.scope, at.inlined_at))).collect();
    assert!(places.contains(&(7, 1, 0)) && places.contains(&(8, 1, 3)), "{places:?}");
}

/// A runtime routine called under a convention its declaration does not state
/// destroys what that convention names, not the contract's whole list: a QB
/// `PUT32` held `rest` and `i` in ESI and EDI across `B$SMID` in the old
/// selector and spilled both here (jpeg_single bq32 -Os, +4.5% instructions).
#[test]
fn a_runtime_call_destroys_what_the_convention_of_the_call_names() {
    use llrm_lir::registers::RegId;
    let ir = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
declare i32 @llrm.qb.B$X(i32)
define cc 1000 i32 @f(i32 %a) {
  %r = call cc 1002 i32 @llrm.qb.B$X(i32 %a)
  ret i32 %r
}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let contracts = super::dag_function::RuntimeContracts {
        abi: &Stub { flags: false },
        segments: &crate::backend::target::BASIC,
        runtime: true,
        frame_zeroed: false,
        facts: None,
    };
    let pool = &mut crate::backend::constpool::Pool::new(module.globals.len() as i64);
    let done = super::dag_function::selected_in_pool(
        &module,
        "f",
        &llrm_x86_m32::M32,
        crate::backend::cpu::profile("486").expect("a cpu"),
        &[],
        Some(&contracts),
        pool,
    )
    .unwrap_or_else(|why| panic!("{}", why.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|one| one.call.is_some()).expect("a call");
    assert!(!call.clobbers.contains(&RegId::ESI) && !call.clobbers.contains(&RegId::EDI), "{:?}", call.clobbers);
}

/// At m16 a value kept in SI across `B$DDIM` was wrong code: runtime.toml names
/// SI and DI clobbered for it and the call here left them alone (tests/check/
/// qb/split_frame_address.bas hoisted `lea si, [bp-44]` over two of them, #5838
/// for m32 had dropped the contract's list for all targets).
#[test]
fn at_m16_a_runtime_call_destroys_what_its_contract_names() {
    use llrm_lir::registers::RegId;
    let ir = "target datalayout = \"e-p:16:16-i32:16-n8:16\"
declare i16 @llrm.qb.B$X(i16)
define cc 1000 i16 @f(i16 %a) {
  %r = call cc 1002 i16 @llrm.qb.B$X(i16 %a)
  ret i16 %r
}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let contracts = super::dag_function::RuntimeContracts {
        abi: &Stub { flags: false },
        segments: &crate::backend::target::BASIC,
        runtime: true,
        frame_zeroed: false,
        facts: None,
    };
    let pool = &mut crate::backend::constpool::Pool::new(module.globals.len() as i64);
    let done = super::dag_function::selected_in_pool(
        &module,
        "f",
        &llrm_x86_m16::M16,
        crate::backend::cpu::profile("386").expect("a cpu"),
        &[],
        Some(&contracts),
        pool,
    )
    .unwrap_or_else(|why| panic!("{}", why.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|one| one.call.is_some()).expect("a call");
    assert!(call.clobbers.contains(&RegId::ESI) && call.clobbers.contains(&RegId::EDI), "{:?}", call.clobbers);
}

/// A caller's own disturbed set took only the convention's registers for a
/// runtime routine (302389dd83), so a caller that held SI or DI across
/// `B$DDIM` did not save them for its caller: QCport died with CPU exception 6.
/// The routine's contract (runtime.toml) names SI and DI.
#[test]
fn at_m16_a_runtime_call_disturbs_what_its_contract_names() {
    use llrm_lir::registers::RegId;
    let ir = "target datalayout = \"e-p:16:16-i32:16-n8:16\"
declare i16 @llrm.qb.B$X(i16)
define cc 1000 i16 @f(i16 %a) {
  %r = call cc 1002 i16 @llrm.qb.B$X(i16 %a)
  ret i16 %r
}
";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let contracts = super::dag_function::RuntimeContracts {
        abi: &Stub { flags: false },
        segments: &crate::backend::target::BASIC,
        runtime: true,
        frame_zeroed: false,
        facts: None,
    };
    let pool = &mut crate::backend::constpool::Pool::new(module.globals.len() as i64);
    let done = super::dag_function::selected_in_pool(
        &module,
        "f",
        &llrm_x86_m16::M16,
        crate::backend::cpu::profile("386").expect("a cpu"),
        &[],
        Some(&contracts),
        pool,
    )
    .unwrap_or_else(|why| panic!("{}", why.0));
    let insns = done.body.insns();
    let call = insns.iter().find(|one| one.call.is_some()).expect("a call");
    let disturbs = &call.call.as_ref().expect("a call").disturbs;
    assert!(disturbs.contains(&RegId::ESI) && disturbs.contains(&RegId::EDI), "{disturbs:?}");
}
