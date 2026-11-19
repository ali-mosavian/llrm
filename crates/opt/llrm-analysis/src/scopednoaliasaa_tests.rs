//! `ScopedNoAliasAA.cpp`'s rules for scope lists.

use super::*;
use crate::aa::BatchAAResults;
use crate::aggregate::AggregateAA;
use crate::basicaa::BasicAA;
use crate::testing::{function, layout, parsed, value};

const SCOPES: &str = "!0 = distinct !{!0, !\"domain\"}
!1 = distinct !{!1, !0, !\"first\"}
!2 = distinct !{!2, !0, !\"second\"}
!3 = !{!1}
!4 = !{!2}
!5 = !{!1, !2}
!6 = distinct !{!6, !\"other domain\"}
!7 = distinct !{!7, !6, !\"third\"}
!8 = !{!7}
";

fn module() -> llrm_mir::module::Module {
    parsed(&format!(
        "define void @f(ptr %p, ptr %q) {{
b:
  %a = load i32, ptr %p, !alias.scope !3, !noalias !4
  %b = load i32, ptr %q, !alias.scope !4, !noalias !3
  %c = load i32, ptr %q, !alias.scope !4
  %d = load i32, ptr %q, !alias.scope !5
  %e = load i32, ptr %q, !alias.scope !8, !noalias !8
  %g = load i32, ptr %q
  ret void
}}
{SCOPES}"
    ))
}

fn alias_of(
    module: &llrm_mir::module::Module,
    first: &str,
    second: &str,
    with_scopes: bool,
) -> AliasResult {
    let layout = layout(module);
    let f = function(module, "f");
    let unit = Unit::of(module, &layout, f);
    let load = |name: &str| {
        let llrm_mir::module::ValueDef::Instruction(inst) = f.value(value(f, name)).def else { panic!() };
        MemoryLocation::get(&unit, inst).expect("a load")
    };
    let (a, b) = (load(first), load(second));
    let mut chain = AggregateAA::new();
    chain.add_aa_result(BasicAA::new(&unit));
    if with_scopes {
        chain.add_aa_result(ScopedNoAliasAA::new(&unit));
    }
    BatchAAResults::new(&mut chain).alias(&a, &b)
}

/// Two pointers BasicAA cannot tell apart, each in a scope the other names
/// `!noalias`: the aggregate with the provider separates them, without it
/// they may alias.
#[test]
fn a_scope_named_noalias_by_the_other_access_makes_them_apart() {
    let module = module();
    assert_eq!(alias_of(&module, "a", "b", false), AliasResult::MayAlias, "premise");
    assert_eq!(alias_of(&module, "a", "b", true), AliasResult::NoAlias);
}

/// One side's list is enough: `%c` is in `!4` and `%a` says `!noalias !4`.
#[test]
fn one_accesss_noalias_covering_the_others_scopes_is_enough() {
    let module = module();
    assert_eq!(alias_of(&module, "a", "c", true), AliasResult::NoAlias);
}

/// Every scope of the domain must be covered: `%d` is in `!1` and `!2`, and
/// `%a` names only `!2`.
#[test]
fn a_scope_the_noalias_list_does_not_name_keeps_them_aliasing() {
    let module = module();
    assert_eq!(alias_of(&module, "a", "d", true), AliasResult::MayAlias);
}

/// A scope of another domain says nothing, and an access with no lists
/// is never apart.
#[test]
fn other_domains_and_missing_lists_say_nothing() {
    let module = module();
    assert_eq!(alias_of(&module, "a", "e", true), AliasResult::MayAlias);
    assert_eq!(alias_of(&module, "a", "g", true), AliasResult::MayAlias);
}
