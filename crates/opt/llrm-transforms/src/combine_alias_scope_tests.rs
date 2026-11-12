//! `AliasScopeTracker` against InstCombine at the pin: a scope declaration no
//! `!alias.scope` and `!noalias` pair uses is deleted.

use llrm_mir::datalayout::DataLayout;

use crate::combine::{Visit, combine_with_declared};
use crate::testing::{parsed, printed};

const NODES: &str = "!0 = distinct !{!0, !\"domain\"}
!2 = distinct !{!2, !0, !\"scope\"}
!1 = !{!2}
";

fn combined(body: &str) -> String {
    let text = format!(
        "declare void @llvm.experimental.noalias.scope.decl()

define void @f(ptr %p, ptr %q) {{
b0:
{body}  ret void
}}

{NODES}"
    );
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, |_, _| Visit::None, &mut declared);
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// The inliner leaves a declaration whose scope nothing uses; it reached the
/// backend (wasm bitfield_long_long -O2, one instruction more than the pin).
#[test]
fn a_scope_declaration_nothing_uses_is_deleted() {
    let after = combined(
        "  call void @llvm.experimental.noalias.scope.decl(), !noalias.scope.decl !1
  store i32 1, ptr %p
",
    );
    assert!(!after.contains("call void @llvm.experimental.noalias.scope.decl"), "{after}");
}

/// Used on one side only, still dead: it needs a member and a non-member.
#[test]
fn a_scope_declaration_with_only_members_is_deleted() {
    let after = combined(
        "  call void @llvm.experimental.noalias.scope.decl(), !noalias.scope.decl !1
  store i32 1, ptr %p, !alias.scope !1
",
    );
    assert!(!after.contains("call void @llvm.experimental.noalias.scope.decl"), "{after}");
}

#[test]
fn a_scope_declaration_with_a_member_and_a_non_member_stays() {
    let after = combined(
        "  call void @llvm.experimental.noalias.scope.decl(), !noalias.scope.decl !1
  store i32 1, ptr %p, !alias.scope !1
  %v = load i32, ptr %q, !noalias !1
  store i32 %v, ptr %q
",
    );
    assert!(after.contains("call void @llvm.experimental.noalias.scope.decl"), "{after}");
}
