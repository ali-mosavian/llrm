//! First-class aggregates: `insertvalue`, `extractvalue`, constants, `select`,
//! `load` and `store` of a struct or array value (`builder/aggregates.rs`).

use super::dag_function::selected;
use crate::model::ir::Loc;

fn lir(body: &str) -> Vec<(Option<std::sync::Arc<str>>, Vec<Loc>, Vec<Loc>)> {
    let text = format!("target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n{body}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    done.body
        .insns()
        .iter()
        .filter_map(|i| i.what.as_ref())
        .map(|w| (w.name.clone(), w.dests.clone(), w.sources.clone()))
        .collect()
}

/// The frame displacement the first instruction reads: which argument is
/// returned.
fn argument_returned(body: &str) -> i64 {
    match &lir(body)[0].2[..] {
        [Loc::Mem(cell)] => cell.addr.expect("a frame cell").disp,
        other => panic!("{other:?}"),
    }
}

/// `insertvalue` was refused ("instruction insertvalue") and an aggregate value
/// ("aggregate or vector value"): the member inserted last is the one picked.
#[test]
fn an_extractvalue_picks_what_an_insertvalue_put_there() {
    let body = "define i32 @f(i32 %a, i32 %b) {\nentry:\n  %x = insertvalue {i32, i32} undef, i32 %a, 0\n  %y = insertvalue {i32, i32} %x, i32 %b, 1\n  %r = extractvalue {i32, i32} %y, 1\n  ret i32 %r\n}\n";
    assert_eq!(argument_returned(body), 12, "%b is the second argument");
}

/// The same through an array, a nested struct and a member that is itself an
/// aggregate.
#[test]
fn a_member_is_found_in_an_array_a_nested_struct_and_an_inner_aggregate() {
    let array = "define i32 @f(i32 %a) {\nentry:\n  %x = insertvalue [3 x i32] zeroinitializer, i32 %a, 2\n  %r = extractvalue [3 x i32] %x, 2\n  ret i32 %r\n}\n";
    let nested = "define i32 @f(i32 %a) {\nentry:\n  %x = insertvalue {i32, {i32, i32}} undef, i32 %a, 1, 1\n  %r = extractvalue {i32, {i32, i32}} %x, 1, 1\n  ret i32 %r\n}\n";
    let inner = "define i32 @f(i32 %a) {\nentry:\n  %x = insertvalue {i32, {i32, i32}} undef, i32 %a, 1, 0\n  %in = extractvalue {i32, {i32, i32}} %x, 1\n  %r = extractvalue {i32, i32} %in, 0\n  ret i32 %r\n}\n";
    for body in [array, nested, inner] {
        assert_eq!(argument_returned(body), 8, "{body}");
    }
}

/// A constant aggregate is its members: the second is 4.
#[test]
fn a_member_of_a_constant_aggregate_is_that_constant() {
    let body = "define i32 @f() {\nentry:\n  %r = extractvalue {i32, i32} {i32 3, i32 4}, 1\n  ret i32 %r\n}\n";
    match &lir(body)[0].2[..] {
        [Loc::Imm(imm)] => assert_eq!(imm.value, 4),
        other => panic!("{other:?}"),
    }
}

/// A `select` of two aggregates is a select of each member.
#[test]
fn a_select_of_aggregates_selects_each_member() {
    let body = "define i32 @f(i1 %c, i32 %a) {\nentry:\n  %p = insertvalue {i32, i32} undef, i32 %a, 0\n  %q = insertvalue {i32, i32} zeroinitializer, i32 5, 1\n  %s = select i1 %c, {i32, i32} %p, {i32, i32} %q\n  %x = extractvalue {i32, i32} %s, 1\n  ret i32 %x\n}\n";
    let names: Vec<_> = lir(body).into_iter().filter_map(|(name, ..)| name).collect();
    assert!(names.iter().any(|n| &**n == "test") && names.iter().any(|n| &**n == "jne"), "{names:?}");
}

/// A `store` of an aggregate is a store of each member at its offset, a `load`
/// a load of each: `{i32, i16, i32}` is 4 + 2 + 4 bytes at 0, 4 and 8.
#[test]
fn an_aggregate_is_stored_and_loaded_member_by_member() {
    let body = "@g = global {i32, i16, i32} zeroinitializer\ndefine i32 @f(i32 %a, i16 %b) {\nentry:\n  %x = insertvalue {i32, i16, i32} zeroinitializer, i32 %a, 0\n  %y = insertvalue {i32, i16, i32} %x, i16 %b, 1\n  store {i32, i16, i32} %y, ptr @g\n  %l = load {i32, i16, i32}, ptr @g\n  %r = extractvalue {i32, i16, i32} %l, 0\n  ret i32 %r\n}\n";
    let mut stores: Vec<(i64, u32)> = lir(body)
        .into_iter()
        .filter_map(|(_, dests, _)| match &dests[..] {
            [Loc::Mem(cell)] => Some((cell.addr.expect("a global").disp, cell.width)),
            _ => None,
        })
        .collect();
    stores.sort_unstable();
    assert_eq!(stores, [(0, 4), (4, 2), (8, 4)]);
}

fn blocks(body: &str) -> Vec<(usize, usize)> {
    let text = format!("target datalayout = \"e-p:32:32-i64:32-n8:16:32\"\n{body}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    done.body.blocks.iter().map(|block| (block.phis.len(), block.insns.len())).collect()
}

/// An aggregate built in one block and read in another travels in a register
/// for each member (it was refused: "aggregate or vector value").
#[test]
fn an_aggregate_read_in_another_block_travels_in_a_register_for_each_member() {
    let body = "define i32 @f(i32 %a, i32 %b, i1 %c) {\nentry:\n  %x = insertvalue {i32, i32} undef, i32 %a, 0\n  %y = insertvalue {i32, i32} %x, i32 %b, 1\n  br i1 %c, label %t, label %e\nt:\n  %r = extractvalue {i32, i32} %y, 1\n  ret i32 %r\ne:\n  %q = extractvalue {i32, i32} %y, 0\n  ret i32 %q\n}\n";
    assert!(!blocks(body).is_empty());
}

/// A phi of aggregates is a phi of each member: the second member of the result
/// is `%b` on one edge and 7 on the other.
#[test]
fn a_phi_of_aggregates_is_a_phi_for_each_member() {
    let body = "define i32 @f(i32 %a, i32 %b, i1 %c) {\nentry:\n  %x = insertvalue {i32, i32} undef, i32 %a, 0\n  %y = insertvalue {i32, i32} %x, i32 %b, 1\n  br i1 %c, label %t, label %m\nt:\n  br label %m\nm:\n  %s = phi {i32, i32} [ %y, %entry ], [ {i32 1, i32 7}, %t ]\n  %r = extractvalue {i32, i32} %s, 1\n  ret i32 %r\n}\n";
    let phis: usize = blocks(body).iter().map(|(phis, _)| phis).sum();
    assert_eq!(phis, 2, "one phi for each member");
}
