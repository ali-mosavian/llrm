//! `TypeBasedAliasAnalysis.cpp`'s rules for scalar tags.

use llrm_mir::module::Operand;

use super::*;
use crate::aa::{BatchAAResults, LocationSize};
use crate::aggregate::AggregateAA;
use crate::basicaa::BasicAA;
use crate::testing::{function, layout, parsed, value};

const TREE: &str = "!5 = !{!\"Simple C/C++ TBAA\"}
!6 = !{!\"omnipotent char\", !5, i64 0}
!7 = !{!6, !6, i64 0}
!10 = !{!\"int4\", !6, i64 0}
!11 = !{!10, !10, i64 0}
!12 = !{!\"float4\", !6, i64 0}
!13 = !{!12, !12, i64 0}
!14 = !{!\"Other TBAA\"}
!15 = !{!\"other\", !14, i64 0}
!16 = !{!15, !15, i64 0}
!17 = !{!10, !12, i64 4}
!18 = !{!10, !10, i64 0, i64 1}
";

/// `matchAccessTags` over the module's metadata and constants.
fn may_alias(
    module: &llrm_mir::module::Module,
    a: Option<MetadataId>,
    b: Option<MetadataId>,
) -> bool {
    let offset = |id| match module.context.get(id).kind {
        llrm_mir::context::ConstantKind::Int(bits) => u64::try_from(bits).ok(),
        _ => None,
    };
    aliases(&module.metadata, &offset, a, b)
}

fn module(extra: &str) -> llrm_mir::module::Module {
    parsed(&format!(
        "define void @f(ptr %p, ptr %q) {{\nb:\n  %a = load i32, ptr %p, !tbaa !11\n  %b = load i32, ptr %q, !tbaa !13\n  %c = load i32, ptr %q, !tbaa !7\n  %d = load i32, ptr %q\n  %e = load i32, ptr %q, !tbaa !16\n  %g = load i32, ptr %q, !tbaa !17\n  %h = load i32, ptr %q, !tbaa !18\n  ret void\n}}\n{TREE}{extra}"
    ))
}

/// The tag ids of the loads `%a` to `%h`.
fn tags(module: &llrm_mir::module::Module) -> Vec<Option<MetadataId>> {
    let f = function(module, "f");
    ["a", "b", "c", "d", "e", "g", "h"]
        .iter()
        .map(|name| {
            let llrm_mir::module::ValueDef::Instruction(inst) = f.value(value(f, name)).def else { panic!() };
            AAInfo::of(f.instruction(inst)).tbaa
        })
        .collect()
}

#[test]
fn unrelated_scalar_types_are_apart_and_an_ancestor_is_not() {
    let module = module("");
    let [int, float, chr, none, other, _, _] = tags(&module)[..] else { panic!() };
    // Premise: the loads carry their tags.
    assert!(int.is_some() && float.is_some() && chr.is_some() && none.is_none() && other.is_some());
    assert!(!may_alias(&module, int, float), "int4 and float4 are apart");
    assert!(may_alias(&module, int, chr), "char is int4's ancestor");
    assert!(may_alias(&module, float, chr));
    assert!(may_alias(&module, int, int));
    assert!(may_alias(&module, int, none), "no tag says nothing");
    assert!(may_alias(&module, int, other), "another root says nothing");
}

/// A struct-path tag whose field is of a type unrelated to the other access
/// stays apart from it, and one whose field is the other's ancestor or the same
/// type does not (`!17` is `float4` at offset 4 of `int4`).
#[test]
fn a_struct_path_tag_is_compared_by_its_field_type() {
    let module = module("");
    let [int, float, _, _, _, path, _] = tags(&module)[..] else { panic!() };
    assert!(may_alias(&module, path, float));
    assert!(may_alias(&module, path, int));
}

#[test]
fn an_immutable_tag_is_constant_memory() {
    let module = module("");
    let layout = layout(&module);
    let f = function(&module, "f");
    let unit = Unit::of(&module, &layout, f);
    let [int, _, _, _, _, _, immutable] = tags(&module)[..] else { panic!() };
    let at = |tag| MemoryLocation { aa_info: AAInfo { tbaa: tag, noalias_scope: None }, ..location(0) };
    let mut tbaa = TypeBasedAA::new(&unit);
    let aaqi = &mut AAQueryInfo::default();
    assert_eq!(tbaa.get_mod_ref_info_mask(&at(immutable), aaqi, false), ModRefInfo::NoModRef);
    assert_eq!(tbaa.get_mod_ref_info_mask(&at(int), aaqi, false), ModRefInfo::ModRef);
    assert_eq!(tbaa.get_mod_ref_info_mask(&at(None), aaqi, false), ModRefInfo::ModRef);
}

fn location(at: u32) -> MemoryLocation {
    MemoryLocation::new(Operand::Value(llrm_mir::module::ValueId(at)), LocationSize::Precise(4))
}

/// Two loads through different pointers: BasicAA cannot tell them apart; the
/// aggregate with the type-based provider can, from their tags.
#[test]
fn the_aggregate_with_tbaa_separates_pointers_basicaa_cannot() {
    let module = module("");
    let layout = layout(&module);
    let f = function(&module, "f");
    let unit = Unit::of(&module, &layout, f);
    let load = |name: &str| {
        let llrm_mir::module::ValueDef::Instruction(inst) = f.value(value(f, name)).def else { panic!() };
        MemoryLocation::get(&unit, inst).expect("a load")
    };
    let (int, float) = (load("a"), load("b"));
    let mut basic = BasicAA::new(&unit);
    assert_eq!(BatchAAResults::new(&mut basic).alias(&int, &float), AliasResult::MayAlias, "premise");
    let mut chain = AggregateAA::new();
    chain.add_aa_result(BasicAA::new(&unit));
    chain.add_aa_result(TypeBasedAA::new(&unit));
    assert_eq!(BatchAAResults::new(&mut chain).alias(&int, &float), AliasResult::NoAlias);
    let chr = load("c");
    assert_eq!(BatchAAResults::new(&mut chain).alias(&int, &chr), AliasResult::MayAlias);
}

/// On every tag pair of the corpus the translation agrees with the type-name
/// rule the passes use today (`typed_apart`'s `!tbaa` part).
#[test]
fn every_corpus_tag_pair_agrees_with_the_name_rule() {
    let (mut pairs, mut apart) = (0usize, 0usize);
    for (name, module) in crate::testing::corpus() {
        let tree = llrm_mir::tbaa::Tbaa::of(&module.metadata);
        let mut used: Vec<MetadataId> = Vec::new();
        for (_, _, f) in module.functions().filter(|(_, _, f)| !f.is_declaration()) {
            for (_, inst) in f.walk() {
                if let Some(tag) = AAInfo::of(f.instruction(inst)).tbaa
                    && !used.contains(&tag)
                {
                    used.push(tag);
                }
            }
        }
        for &one in &used {
            for &other in &used {
                let by_name =
                    |tag| (tree.name_of_tag(&module.metadata, tag), tree.shared_of_tag(&module.metadata, tag));
                let ((one_type, one_line), (other_type, other_line)) = (by_name(one), by_name(other));
                let (Some(one_type), Some(other_type)) = (one_type, other_type) else { continue };
                let old_apart = match (one_line.last(), other_line.last()) {
                    (Some(a), Some(b)) => {
                        a == b
                            && one_type != other_type
                            && !one_line.iter().any(|n| **n == *other_type)
                            && !other_line.iter().any(|n| **n == *one_type)
                    }
                    _ => false,
                };
                pairs += 1;
                apart += usize::from(old_apart);
                assert_eq!(!may_alias(&module, Some(one), Some(other)), old_apart, "{name}: {one:?} {other:?}");
            }
        }
    }
    // Premise: the corpus has tagged accesses and some pairs are apart.
    assert!(pairs > 0 && apart > 0, "{pairs} pairs, {apart} apart");
}

/// `AAResults::getModRefInfo(Store, Loc)` asks the aggregate's `alias`
/// (AliasAnalysis.cpp), so a store and a load of unrelated types through
/// different pointers do not touch each other. The aggregate met the members'
/// answers instead: BasicAA's store case asked BasicAA alone, and TBAA's
/// said anything for a store.
#[test]
fn a_store_and_a_load_of_unrelated_types_do_not_touch_through_the_aggregate() {
    let module = parsed(&format!(
        "define void @f(ptr %p, ptr %q) {{\nb:\n  store i32 1, ptr %p, !tbaa !11\n  %b = load i32, ptr %q, !tbaa !13\n  %c = load i32, ptr %q, !tbaa !7\n  ret void\n}}\n{TREE}"
    ));
    let layout = layout(&module);
    let f = function(&module, "f");
    let unit = Unit::of(&module, &layout, f);
    let store = f
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| matches!(f.instruction(inst).opcode, Opcode::Store { .. }))
        .unwrap();
    let load = |name: &str| {
        let llrm_mir::module::ValueDef::Instruction(inst) = f.value(value(f, name)).def else { panic!() };
        MemoryLocation::get(&unit, inst).expect("a load")
    };
    let mut chain = AggregateAA::new().with_unit(&unit);
    chain.add_aa_result(TypeBasedAA::new(&unit));
    chain.add_aa_result(BasicAA::new(&unit));
    let aaqi = &mut AAQueryInfo::default();
    assert_eq!(chain.get_mod_ref_info(store, Some(&load("b")), aaqi), ModRefInfo::NoModRef, "int against float");
    assert_eq!(chain.get_mod_ref_info(store, Some(&load("c")), aaqi), ModRefInfo::Mod, "int against char");
}

/// QB's `LhEntry`, as clang emits it: a `long`, two `char`s and an `any
/// pointer` at 16, with a global of `p1 omnipotent char` (a pointer to a char)
/// beside it.
const ENTRY: &str = "define void @g(ptr %p, ptr %q) {
b:
  %heap_top = load ptr, ptr %q, !tbaa !101
  %size = load i64, ptr %p, !tbaa !105
  %kind = load i8, ptr %p, !tbaa !106
  %next = load ptr, ptr %p, !tbaa !107
  %scalar_ptr = load ptr, ptr %q, !tbaa !103
  %scalar_long = load i64, ptr %q, !tbaa !104
  ret void
}
!100 = !{!\"Simple C/C++ TBAA\"}
!108 = !{!\"omnipotent char\", !100, i64 0}
!109 = !{!\"any pointer\", !108, i64 0}
!110 = !{!\"p1 omnipotent char\", !109, i64 0}
!111 = !{!\"long\", !108, i64 0}
!112 = !{!\"LhEntry\", !111, i64 0, !108, i64 8, !108, i64 9, !109, i64 16}
!101 = !{!110, !110, i64 0}
!103 = !{!109, !109, i64 0}
!104 = !{!111, !111, i64 0}
!105 = !{!112, !111, i64 0}
!106 = !{!112, !108, i64 8}
!107 = !{!112, !109, i64 16}
";

fn entry_tags() -> (llrm_mir::module::Module, Vec<(String, Option<MetadataId>)>) {
    let module = parsed(ENTRY);
    let f = function(&module, "g");
    let tags = ["heap_top", "size", "kind", "next", "scalar_ptr", "scalar_long"]
        .iter()
        .map(|name| {
            let llrm_mir::module::ValueDef::Instruction(inst) = f.value(value(f, name)).def else { panic!() };
            (name.to_string(), AAInfo::of(f.instruction(inst)).tbaa)
        })
        .collect();
    (module, tags)
}

/// `heap_top` (a `p1 omnipotent char` global) and a store of `LhEntry`'s `any
/// pointer` field were taken to alias because a struct-path tag "said nothing":
/// `first_fit` reloaded `heap_top` every trip of its loop (one more priced
/// load, 235 against the pin's 230).
#[test]
fn a_field_of_any_pointer_is_apart_from_a_pointer_to_char_and_from_the_other_fields() {
    let (module, tags) = entry_tags();
    let tag = |name: &str| tags.iter().find(|(n, _)| n == name).unwrap().1;
    assert!(!may_alias(&module, tag("heap_top"), tag("next")), "p1 omnipotent char vs LhEntry.next");
    assert!(!may_alias(&module, tag("size"), tag("next")), "LhEntry.size vs LhEntry.next");
    assert!(!may_alias(&module, tag("kind"), tag("next")));
    assert!(!may_alias(&module, tag("heap_top"), tag("size")));
}

/// A field and a scalar access of its own type, or of an ancestor of it, may
/// alias.
#[test]
fn a_field_may_alias_a_scalar_of_its_type_or_an_ancestor_of_it() {
    let (module, tags) = entry_tags();
    let tag = |name: &str| tags.iter().find(|(n, _)| n == name).unwrap().1;
    assert!(may_alias(&module, tag("size"), tag("scalar_long")), "LhEntry.size vs long");
    assert!(may_alias(&module, tag("next"), tag("scalar_ptr")), "LhEntry.next vs any pointer");
    assert!(may_alias(&module, tag("next"), tag("next")));
    assert!(!may_alias(&module, tag("next"), tag("scalar_long")), "any pointer vs long");
}
