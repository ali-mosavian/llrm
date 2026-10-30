//! `MemoryDependenceAnalysis`: what a load, a store or a call depends on in its
//! block, and the cache after an instruction is removed.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::module::{Function, InstId, Module, ValueDef};

use super::*;
use crate::aa::{AAQueryInfo, MemoryEffects};
use crate::basicaa::BasicAA;
use crate::testing::{DOS, block, function, layout, parsed, value};

/// BasicAA with the instruction forms of `AAResults::getModRefInfo(Load/Store,
/// Loc)` (AliasAnalysis.cpp), which BasicAA S1 leaves at the top of the
/// lattice.
struct WithInstructions<'a>(BasicAA<'a>, &'a Unit<'a>);

impl AAResults for WithInstructions<'_> {
    fn alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        ctx: Option<InstId>,
    ) -> AliasResult {
        self.0.alias(a, b, aaqi, ctx)
    }

    fn get_mod_ref_info_mask(
        &mut self,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        ignore_locals: bool,
    ) -> ModRefInfo {
        self.0.get_mod_ref_info_mask(loc, aaqi, ignore_locals)
    }

    fn get_arg_mod_ref_info(
        &mut self,
        call: InstId,
        arg: u32,
    ) -> ModRefInfo {
        self.0.get_arg_mod_ref_info(call, arg)
    }

    fn get_memory_effects(
        &mut self,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> MemoryEffects {
        self.0.get_memory_effects(call, aaqi)
    }

    fn get_mod_ref_info(
        &mut self,
        inst: InstId,
        loc: Option<&MemoryLocation>,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        let unit = self.1;
        let (own, effect) = (
            MemoryLocation::get(unit, inst),
            match unit.function.instruction(inst).opcode {
                Opcode::Load { .. } => ModRefInfo::Ref,
                _ => ModRefInfo::Mod,
            },
        );
        match (own, loc) {
            (Some(own), Some(loc)) => {
                if self.0.alias(&own, loc, aaqi, None) == AliasResult::NoAlias {
                    ModRefInfo::NoModRef
                } else {
                    effect
                }
            }
            (Some(_), None) => effect,
            // Anything else that is no call touches no memory.
            _ if !matches!(
                unit.function.instruction(inst).opcode,
                Opcode::Call(_) | Opcode::Invoke(_)
            ) => {
                ModRefInfo::NoModRef
            }
            _ => self.0.get_mod_ref_info(inst, loc, aaqi),
        }
    }

    fn get_mod_ref_info_call(
        &mut self,
        inst: InstId,
        call: InstId,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        self.0.get_mod_ref_info_call(inst, call, aaqi)
    }

    fn call_captures_before(
        &mut self,
        inst: InstId,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        self.0.call_captures_before(inst, loc, aaqi)
    }
}

fn module(body: &str) -> Module {
    parsed(&format!("{DOS}declare void @g()\ndeclare i16 @pure() memory(read)\n\n{body}"))
}

/// The instruction that defines `%name`.
fn inst(
    function: &Function,
    name: &str,
) -> InstId {
    match function.value(value(function, name)).def {
        ValueDef::Instruction(inst) => inst,
        ValueDef::Argument(_) => panic!("%{name} is an argument"),
    }
}

/// The dependence of `%name` in `@f`, with `Def(%a)` as `Def("a")`.
fn named(
    function: &Function,
    result: MemDepResult,
) -> String {
    let name = |inst: InstId| {
        function
            .instruction(inst)
            .result
            .and_then(|value| function.value(value).name.clone())
            .unwrap_or_else(|| function.instruction(inst).opcode.mnemonic().to_owned())
    };
    match result {
        MemDepResult::Def(i) => format!("Def({})", name(i)),
        MemDepResult::Clobber(i) => format!("Clobber({})", name(i)),
        MemDepResult::Invalid(Some(i)) => format!("Invalid({})", name(i)),
        other => format!("{other:?}"),
    }
}

fn dependency(
    module: &Module,
    layout: &DataLayout,
    memdep: &mut MemoryDependenceResults,
    query: &str,
) -> String {
    let f = function(module, "f");
    let unit = Unit::of(module, layout, f);
    let mut aa = WithInstructions(BasicAA::new(&unit), &unit);
    let result = memdep.get_dependency(Env { unit: &unit, dt: &DomTree::of(unit.function) }, &mut aa, inst(f, query));
    named(f, result)
}

fn one(
    body: &str,
    query: &str,
) -> String {
    let module = module(body);
    let layout = layout(&module);
    dependency(&module, &layout, &mut MemoryDependenceResults::default(), query)
}

#[test]
fn test_a_must_aliased_store_is_the_def_of_a_load() {
    assert_eq!(
        one("define i16 @f(ptr %p) {\nb:\n  store i16 1, ptr %p\n  %x = load i16, ptr %p\n  ret i16 %x\n}\n", "x"),
        "Def(store)"
    );
}

#[test]
fn test_a_call_that_writes_unknown_memory_clobbers_a_load() {
    let body = "define i16 @f(ptr %p) {\nb:\n  store i16 1, ptr %p\n  call void @g()\n  %x = load i16, ptr %p\n  ret i16 %x\n}\n";
    assert_eq!(one(body, "x"), "Clobber(call)");
}

#[test]
fn test_a_store_to_another_object_is_scanned_past() {
    let body = "define i16 @f() {\nb:\n  %a = alloca i16\n  %b = alloca i16\n  store i16 1, ptr %a\n  store i16 2, ptr %b\n  %x = load i16, ptr %a\n  ret i16 %x\n}\n";
    let module = module(body);
    let layout = layout(&module);
    let f = function(&module, "f");
    let first_store =
        f.walk().map(|(_, i)| i).find(|&i| matches!(f.instruction(i).opcode, Opcode::Store { .. })).expect("a store");
    let got = dependency(&module, &layout, &mut MemoryDependenceResults::default(), "x");
    assert_eq!(got, "Def(store)");
    let unit = Unit::of(&module, &layout, f);
    let mut memdep = MemoryDependenceResults::default();
    let mut aa = BasicAA::new(&unit);
    assert_eq!(
        memdep.get_dependency(Env { unit: &unit, dt: &DomTree::of(unit.function) }, &mut aa, inst(f, "x")),
        MemDepResult::Def(first_store)
    );
}

#[test]
fn test_a_load_with_nothing_before_it_is_non_local_or_non_function_local() {
    let body = "define i16 @f(ptr %p, i1 %c) {\nb0:\n  br i1 %c, label %b1, label %b2\n\nb1:\n  %x = load i16, ptr %p\n  br label %b2\n\nb2:\n  ret i16 0\n}\n";
    assert_eq!(one(body, "x"), "NonLocal");
    let entry = "define i16 @f(ptr %p) {\nb0:\n  %x = load i16, ptr %p\n  ret i16 %x\n}\n";
    assert_eq!(one(entry, "x"), "NonFuncLocal");
}

#[test]
fn test_must_aliased_loads_are_defs_of_each_other() {
    let body = "define i16 @f(ptr %p) {\nb:\n  %a = load i16, ptr %p\n  %x = load i16, ptr %p\n  ret i16 %x\n}\n";
    assert_eq!(one(body, "x"), "Def(a)");
}

#[test]
fn test_a_scan_past_the_limit_is_unknown() {
    let allocas: String = (0..8).map(|at| format!("  %a{at} = alloca i16\n")).collect();
    let stores: String = (0..8).map(|at| format!("  store i16 {at}, ptr %a{at}\n")).collect();
    let body = format!("define i16 @f(ptr %p) {{\nb:\n{allocas}{stores}  %x = load i16, ptr %p\n  ret i16 %x\n}}\n");
    let module = module(&body);
    let layout = layout(&module);
    let mut memdep = MemoryDependenceResults::new(5);
    assert_eq!(dependency(&module, &layout, &mut memdep, "x"), "Unknown");
}

#[test]
fn test_a_may_aliased_store_of_what_the_location_holds_is_scanned_past() {
    let body = |stored: &str| {
        format!(
            "define i16 @f(ptr %p, ptr %q, i16 %v) {{\nb:\n  %a = load i16, ptr %p\n  store i16 {stored}, ptr %q\n  %x = load i16, ptr %p\n  ret i16 %x\n}}\n"
        )
    };
    // Storing %a, just read from %p, changes nothing whichever way %q aliases.
    assert_eq!(one(&body("%a"), "x"), "Def(a)");
    assert_eq!(one(&body("%v"), "x"), "Clobber(store)");
}

#[test]
fn test_removing_the_def_dirties_what_depended_on_it_and_a_rescan_finds_the_next() {
    let body = "define i16 @f(ptr %p) {\nb:\n  %a = load i16, ptr %p\n  store i16 1, ptr %p\n  %x = load i16, ptr %p\n  ret i16 %x\n}\n";
    let module = module(body);
    let layout = layout(&module);
    let f = function(&module, "f");
    let mut memdep = MemoryDependenceResults::default();
    assert_eq!(dependency(&module, &layout, &mut memdep, "x"), "Def(store)");
    let unit_before = Unit::of(&module, &layout, f);
    let store =
        f.walk().map(|(_, i)| i).find(|&i| matches!(f.instruction(i).opcode, Opcode::Store { .. })).expect("a store");
    memdep.remove_instruction(&unit_before, store);
    // `x` is dirty and will rescan from the instruction after the store.
    assert_eq!(named(f, memdep.local_deps[&inst(f, "x")]), "Invalid(x)");
    // The function without the store: the load depends on the earlier load.
    let mut after = f.clone();
    after.erase(store).expect("erased");
    let (unit_module, unit_layout) = (&module, &layout);
    let unit = Unit::of(unit_module, unit_layout, &after);
    let mut aa = BasicAA::new(&unit);
    let got = memdep.get_dependency(Env { unit: &unit, dt: &DomTree::of(unit.function) }, &mut aa, inst(&after, "x"));
    assert_eq!(named(&after, got), "Def(a)");
    memdep.verify_removed(f, store);
}

#[test]
fn test_a_non_local_call_dependency_walks_the_predecessors_and_is_cached() {
    let body = "define void @f(i1 %c) {\nb0:\n  call void @g()\n  br i1 %c, label %b1, label %b2\n\nb1:\n  br label %b3\n\nb2:\n  br label %b3\n\nb3:\n  %x = call i16 @pure()\n  ret void\n}\n";
    let module = module(body);
    let layout = layout(&module);
    let f = function(&module, "f");
    let unit = Unit::of(&module, &layout, f);
    let mut aa = BasicAA::new(&unit);
    let mut memdep = MemoryDependenceResults::default();
    let query = inst(f, "x");
    let info = memdep
        .get_non_local_call_dependency(Env { unit: &unit, dt: &DomTree::of(unit.function) }, &mut aa, query)
        .clone();
    let by_block: Vec<(BlockId, String)> = info.iter().map(|entry| (entry.bb, named(f, entry.result))).collect();
    // The two arms are empty (non-local); b0 is reached through both and holds
    // the clobbering call.
    assert!(by_block.contains(&(block(f, "b0"), "Clobber(call)".to_owned())), "{by_block:?}");
    assert!(by_block.contains(&(block(f, "b1"), "NonLocal".to_owned())), "{by_block:?}");
    assert_eq!(
        memdep
            .get_non_local_call_dependency(Env { unit: &unit, dt: &DomTree::of(unit.function) }, &mut aa, query)
            .len(),
        info.len(),
        "cached"
    );
}

#[test]
#[should_panic(expected = "invariant.group is not modelled")]
fn test_invariant_group_metadata_is_refused() {
    let body = "define i16 @f(ptr %p) {\nb:\n  %x = load i16, ptr %p, !invariant.group !0\n  ret i16 %x\n}\n!0 = !{}\n";
    one(&format!("{body}"), "x");
    // A load alone in its block is answered before the scan: put one before it.
    let two = "define i16 @f(ptr %p) {\nb:\n  store i16 1, ptr %p\n  %x = load i16, ptr %p, !invariant.group !0\n  ret i16 %x\n}\n!0 = !{}\n";
    one(two, "x");
}

/// The non-local dependence of `%name` in `@f`, each as `block:Result@address`.
fn non_local(
    memdep: &mut MemoryDependenceResults,
    module: &Module,
    layout: &DataLayout,
    query: &str,
) -> Vec<String> {
    let f = function(module, "f");
    let unit = Unit::of(module, layout, f);
    let tree = DomTree::of(f);
    let mut aa = WithInstructions(BasicAA::new(&unit), &unit);
    let mut results = Vec::new();
    memdep.get_non_local_pointer_dependency(Env { unit: &unit, dt: &tree }, &mut aa, inst(f, query), &mut results);
    let block_name = |bb: BlockId| f.block(bb).name.clone().expect("a named block");
    let address = |address: SelectAddr| match address {
        SelectAddr::Addr(None) => "none".to_owned(),
        SelectAddr::Addr(Some(Operand::Value(v))) => format!("%{}", f.value(v).name.clone().unwrap_or_default()),
        SelectAddr::Addr(Some(_)) => "constant".to_owned(),
        SelectAddr::Select { .. } => "select".to_owned(),
    };
    let mut lines: Vec<String> = results
        .iter()
        .map(|one| format!("{}:{}@{}", block_name(one.bb), named(f, one.result), address(one.address)))
        .collect();
    lines.sort();
    lines
}

const DIAMOND: &str = "define i16 @f(ptr %x, ptr %y, i1 %c) {
b0:
  store i16 1, ptr %x
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %v = load i16, ptr %x
  ret i16 %v
}
";

#[test]
fn test_a_load_after_a_join_depends_on_the_store_before_the_branch() {
    let module = module(DIAMOND);
    let layout = layout(&module);
    let mut memdep = MemoryDependenceResults::default();
    assert_eq!(non_local(&mut memdep, &module, &layout, "v"), ["b0:Def(store)@%x"]);
}

#[test]
fn test_a_clobber_on_one_arm_is_that_arms_dependence() {
    let body = DIAMOND.replace("b1:\n  br label %b3", "b1:\n  call void @g()\n  br label %b3");
    let module = module(&body);
    let layout = layout(&module);
    let mut memdep = MemoryDependenceResults::default();
    assert_eq!(non_local(&mut memdep, &module, &layout, "v"), ["b0:Def(store)@%x", "b1:Clobber(call)@%x"]);
}

#[test]
fn test_a_second_query_is_answered_from_the_cache() {
    let module = module(DIAMOND);
    let layout = layout(&module);
    let mut memdep = MemoryDependenceResults::default();
    let first = non_local(&mut memdep, &module, &layout, "v");
    assert_eq!(non_local(&mut memdep, &module, &layout, "v"), first);
    let f = function(&module, "f");
    let info = &memdep.non_local_pointer_deps[&(Operand::Value(value(f, "x")), true)];
    assert_eq!(info.pair, (Some(block(f, "b3")), true), "the whole query is cached");
}

#[test]
fn test_a_phi_pointer_is_translated_to_each_predecessor() {
    let body = "define i16 @f(ptr %x, ptr %y, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 1, ptr %x
  br label %b3

b2:
  store i16 2, ptr %y
  br label %b3

b3:
  %p = phi ptr [ %x, %b1 ], [ %y, %b2 ]
  %v = load i16, ptr %p
  ret i16 %v
}
";
    let module = module(body);
    let layout = layout(&module);
    let mut memdep = MemoryDependenceResults::default();
    assert_eq!(non_local(&mut memdep, &module, &layout, "v"), ["b1:Def(store)@%x", "b2:Def(store)@%y"]);
}

#[test]
fn test_a_constant_address_made_by_translation_is_looked_up() {
    // Translating `gep %x, %i` into b0 gives a poison pointer:
    // `simplifyGEPInst` makes the constant, interned into the real context;
    // b0 answers for it, it is not Unknown.
    let body = "define i16 @f(ptr %x, i16 %n, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  %i = phi i16 [ poison, %b0 ], [ %n, %b1 ]
  %q = getelementptr i8, ptr %x, i16 %i
  %v = load i16, ptr %q
  ret i16 %v
}
";
    let module = module(body);
    let layout = layout(&module);
    let mut memdep = MemoryDependenceResults::default();
    let got = non_local(&mut memdep, &module, &layout, "v");
    assert!(got.iter().any(|line| line == "b0:NonFuncLocal@constant"), "{got:?}");
}

#[test]
fn test_removing_the_def_of_a_cached_pointer_query_dirties_it() {
    let module = module(DIAMOND);
    let layout = layout(&module);
    let f = function(&module, "f");
    let mut memdep = MemoryDependenceResults::default();
    non_local(&mut memdep, &module, &layout, "v");
    let store =
        f.walk().map(|(_, i)| i).find(|&i| matches!(f.instruction(i).opcode, Opcode::Store { .. })).expect("a store");
    let unit = Unit::of(&module, &layout, f);
    memdep.remove_instruction(&unit, store);
    let info = &memdep.non_local_pointer_deps[&(Operand::Value(value(f, "x")), true)];
    assert_eq!(info.pair, (None, false), "no longer a whole query");
    assert!(info.non_local_deps.iter().any(|entry| entry.result.is_dirty()), "{info:?}");
    memdep.verify_removed(f, store);
}
