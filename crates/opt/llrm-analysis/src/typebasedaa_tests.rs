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
    let metadata = &module.metadata;
    let [int, float, chr, none, other, _, _] = tags(&module)[..] else { panic!() };
    // Premise: the loads carry their tags.
    assert!(int.is_some() && float.is_some() && chr.is_some() && none.is_none() && other.is_some());
    assert!(!aliases(metadata, int, float), "int4 and float4 are apart");
    assert!(aliases(metadata, int, chr), "char is int4's ancestor");
    assert!(aliases(metadata, float, chr));
    assert!(aliases(metadata, int, int));
    assert!(aliases(metadata, int, none), "no tag says nothing");
    assert!(aliases(metadata, int, other), "another root says nothing");
}

/// A tag whose base type differs from its access type, or a type with fields,
/// is beyond the scalar form: it says nothing.
#[test]
fn a_struct_path_tag_may_alias_everything() {
    let module = module("");
    let metadata = &module.metadata;
    let [int, float, _, _, _, path, _] = tags(&module)[..] else { panic!() };
    assert!(aliases(metadata, path, float));
    assert!(aliases(metadata, path, int));
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
                assert_eq!(!aliases(&module.metadata, Some(one), Some(other)), old_apart, "{name}: {one:?} {other:?}");
            }
        }
    }
    // Premise: the corpus has tagged accesses and some pairs are apart.
    assert!(pairs > 0 && apart > 0, "{pairs} pairs, {apart} apart");
}
