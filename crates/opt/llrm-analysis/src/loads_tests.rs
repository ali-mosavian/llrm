//! `loads` against `FindAvailableLoadedValue` at the pin: what a load can
//! reuse and what ends the scan.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, InstId, Module, Operand, ValueDef};
use llrm_mir::opcode::Opcode;

use super::{
    Available, DEF_MAX_INSTS_TO_SCAN, ModRefOracle, find_available_loaded_value, find_available_loaded_value_before,
};
use crate::aa::{MemoryLocation, ModRefInfo};
use crate::memory::Unit;

/// What an available value is, for a test to say: an argument, the n-th load
/// of the function, or a number.
#[derive(Debug, Eq, PartialEq)]
enum Is {
    Arg(u32),
    Load(usize),
    Number(u128),
}

fn f(module: &Module) -> &llrm_mir::module::Function {
    module.global(module.named("f").expect("@f")).function().expect("a function")
}

fn parsed(text: &str) -> Module {
    llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn describe(
    module: &Module,
    value: Operand,
) -> Is {
    let function = f(&module);
    match value {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Argument(at) => Is::Arg(at),
            ValueDef::Instruction(inst) => {
                let loads: Vec<InstId> = function
                    .walk()
                    .map(|(_, inst)| inst)
                    .filter(|one| matches!(function.instruction(*one).opcode, Opcode::Load { .. }))
                    .collect();
                Is::Load(loads.iter().position(|one| *one == inst).expect("a load"))
            }
        },
        Operand::Constant(id) => {
            let (bits, _) = llrm_mir::valuetracking::Query::new(module, function, &DataLayout::default())
                .int_constant(Operand::Constant(id))
                .expect("a number");
            Is::Number(bits)
        }
        Operand::Block(_) => unreachable!("not a block"),
    }
}

/// The last load of `@f`, its block, and the scan of it over its own block:
/// what it finds, the cursor left, and the instructions counted.
fn scanned(
    text: &str,
    max_insts: u32,
) -> (Option<(Is, bool)>, Option<InstId>, u32) {
    let module = parsed(text);
    let layout = DataLayout::default();
    let function = f(&module);
    let (block, load) = last_load(function);
    let unit = Unit::of(&module, &layout, function);
    let (mut cursor, mut count) = (Some(load), 0);
    let found = find_available_loaded_value(&unit, load, block, &mut cursor, max_insts, None, Some(&mut count));
    (found.map(|Available { value, is_load_cse }| (describe(&module, value), is_load_cse)), cursor, count)
}

fn last_load(function: &llrm_mir::module::Function) -> (BlockId, InstId) {
    function
        .walk()
        .filter(|(_, inst)| matches!(function.instruction(*inst).opcode, Opcode::Load { .. }))
        .last()
        .expect("a load")
}

fn found(
    text: &str,
    max_insts: u32,
) -> Option<(Is, bool)> {
    scanned(text, max_insts).0
}

/// `%a` read again: the second load is the first, found as a load.
#[test]
fn a_load_of_the_address_is_available() {
    let text = "define i16 @f(ptr %p) {\nb0:\n  %a = load i16, ptr %p\n  %b = load i16, ptr %p\n  ret i16 %b\n}\n";
    assert_eq!(found(text, DEF_MAX_INSTS_TO_SCAN), Some((Is::Load(0), true)));
}

/// A store's value is what the load reads: found, but not as a load.
#[test]
fn a_store_to_the_address_gives_its_value() {
    let text =
        "define i16 @f(ptr %p, i16 %v) {\nb0:\n  store i16 %v, ptr %p\n  %a = load i16, ptr %p\n  ret i16 %a\n}\n";
    assert_eq!(found(text, DEF_MAX_INSTS_TO_SCAN), Some((Is::Arg(1), false)));
}

/// A store to another alloca is skipped, as the trivial alias analysis that is
/// important for reg2mem'd code.
#[test]
fn a_store_to_another_alloca_is_skipped() {
    let text = "define i16 @f(i16 %v) {
b0:
  %a = alloca i16
  %b = alloca i16
  store i16 %v, ptr %a
  store i16 0, ptr %b
  %x = load i16, ptr %a
  ret i16 %x
}
";
    assert_eq!(found(text, DEF_MAX_INSTS_TO_SCAN), Some((Is::Arg(0), false)));
}

/// A store to a pointer that may alias ends the scan, and the cursor is left
/// after it: no value, and the count is that store's.
#[test]
fn a_store_that_may_alias_ends_the_scan() {
    let text = "define i16 @f(ptr %p, ptr %q, i16 %v) {
b0:
  store i16 %v, ptr %p
  store i16 0, ptr %q
  %x = load i16, ptr %p
  ret i16 %x
}
";
    let (value, cursor, count) = scanned(text, DEF_MAX_INSTS_TO_SCAN);
    assert_eq!((value, count), (None, 1));
    let module = parsed(text);
    let function = f(&module);
    let (_, load) = last_load(function);
    assert_eq!(cursor, Some(load), "the scan stops after the store, at the load it started from");
}

/// The same base, constant offsets that do not meet: the store is skipped
/// without an alias analysis (the inliner's form).
#[test]
fn a_store_beside_it_on_the_same_base_is_skipped() {
    let text = "define i16 @f(ptr %p, i16 %v) {
b0:
  store i16 %v, ptr %p
  %g = getelementptr inbounds i16, ptr %p, i16 1
  store i16 0, ptr %g
  %x = load i16, ptr %p
  ret i16 %x
}
";
    assert_eq!(found(text, DEF_MAX_INSTS_TO_SCAN), Some((Is::Arg(1), false)));
}

/// Six instructions are looked at: the store 6 back is found, 7 back is not.
#[test]
fn the_scan_looks_at_six_instructions() {
    let text = |between: usize| {
        let adds: String = (0..between).map(|i| format!("  %t{i} = add i16 %v, {i}\n")).collect();
        format!(
            "define i16 @f(ptr %p, i16 %v) {{\nb0:\n  store i16 %v, ptr %p\n{adds}  %x = load i16, ptr %p\n  ret i16 %x\n}}\n"
        )
    };
    assert_eq!(found(&text(5), DEF_MAX_INSTS_TO_SCAN), Some((Is::Arg(1), false)));
    assert_eq!(found(&text(6), DEF_MAX_INSTS_TO_SCAN), None);
    assert_eq!(found(&text(6), 0), Some((Is::Arg(1), false)), "0 is no limit");
}

/// A volatile load has no available value.
#[test]
fn a_volatile_load_has_none() {
    let text =
        "define i16 @f(ptr %p) {\nb0:\n  %a = load i16, ptr %p\n  %b = load volatile i16, ptr %p\n  ret i16 %b\n}\n";
    assert_eq!(found(text, DEF_MAX_INSTS_TO_SCAN), None);
}

/// A constant `memset` that covers the load gives its bytes.
#[test]
fn a_memset_that_covers_the_load_gives_its_bytes() {
    let text = "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)

define i16 @f(ptr %p) {
b0:
  call void @llvm.memset.p0.i16(ptr %p, i8 7, i16 4, i1 false)
  %x = load i16, ptr %p
  ret i16 %x
}
";
    assert_eq!(found(text, DEF_MAX_INSTS_TO_SCAN), Some((Is::Number(0x0707), false)));
}

/// An alias analysis that knows nothing says a store modifies it.
struct Says(ModRefInfo);

impl ModRefOracle for Says {
    fn get_mod_ref_info(
        &mut self,
        _: InstId,
        _: &MemoryLocation,
    ) -> ModRefInfo {
        self.0
    }
}

/// The overload over the whole block asks the alias analysis only about what
/// stands between the load and the value it found.
#[test]
fn the_whole_block_overload_asks_what_lies_between() {
    let text = "define i16 @f(ptr %p, ptr %q, i16 %v) {
b0:
  store i16 %v, ptr %p
  store i16 0, ptr %q
  %x = load i16, ptr %p
  ret i16 %x
}
";
    let module = parsed(text);
    let layout = DataLayout::default();
    let function = f(&module);
    let (_, load) = last_load(function);
    let unit = Unit::of(&module, &layout, function);
    let mut modifies = Says(ModRefInfo::Mod);
    // The nearest access that gives a value is the store to %q, no: it is a
    // different address, so the scan reaches the store to %p; between them
    // the store to %q, which the analysis says modifies it.
    assert_eq!(find_available_loaded_value_before(&unit, load, &mut modifies, DEF_MAX_INSTS_TO_SCAN), None);
    let mut apart = Says(ModRefInfo::NoModRef);
    let found = find_available_loaded_value_before(&unit, load, &mut apart, DEF_MAX_INSTS_TO_SCAN);
    assert_eq!(found.map(|one| (describe(&module, one.value), one.is_load_cse)), Some((Is::Arg(2), false)));
}
