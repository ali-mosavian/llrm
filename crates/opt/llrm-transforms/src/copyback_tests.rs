use llrm_mir::program::Program;

use crate::pipeline::{self, Applied};
use crate::testing::{parsed, printed};

/// `text` through the pipeline, printed.
fn piped(text: &str) -> String {
    let mut module = parsed(&format!("{}{text}", llrm_analysis::testing::DOS));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    printed(&module)
}

/// One element of a far array passed by reference: copied into a slot, the
/// slot passed, the slot stored back.
fn call_site(callee: &str) -> String {
    call_site_storing(callee, "")
}

/// `call_site` where the callee may write the slot: the copy-out stores what
/// the slot holds after the call, not the value read before it.
fn call_site_writing(callee: &str) -> String {
    call_site_storing(callee, "  %back = load i16, ptr %slot\n")
}

fn call_site_storing(
    callee: &str,
    reload: &str,
) -> String {
    let stored = if reload.is_empty() { "%v" } else { "%back" };
    format!(
        "define i16 @f(ptr %d, i16 %i) {{
b0:
  %slot = alloca i16
  %w = load i16, ptr %d
  %seg = inttoptr i16 %w to ptr addrspace(2), !llrm.descriptor !0
  %far = addrspacecast ptr addrspace(2) %seg to ptr addrspace(1)
  %e = getelementptr i8, ptr addrspace(1) %far, i16 %i
  %v = load i16, ptr addrspace(1) %e
  store i16 %v, ptr %slot
  %r = call i16 {callee}(ptr %slot)
{reload}  store i16 {stored}, ptr addrspace(1) %e
  ret i16 %r
}}
!0 = !{{}}
"
    )
}

fn far_stores(text: &str) -> usize {
    text.lines().filter(|line| line.trim_start().starts_with("store") && line.contains("addrspace(1)")).count()
}

/// A callee that writes its parameter keeps the copy-out: the write is the
/// result.
#[test]
fn a_copy_out_after_a_callee_that_writes_the_parameter_is_kept() {
    let after = piped(&format!(
        "define internal i16 @callee(ptr nocapture %p) noinline {{
b0:
  store i16 9, ptr %p
  ret i16 0
}}
{}",
        call_site_writing("@callee")
    ));
    assert_eq!(far_stores(&after), 1, "{after}");
}

/// A callee that reads its slot but writes other memory could change the
/// element through the array during the call, and BC's copy-out then undoes
/// it: the copy-out stays.
#[test]
fn a_copy_out_after_a_callee_that_writes_other_memory_is_kept() {
    let after = piped(&format!(
        "@g = global i16 0
define internal i16 @callee(ptr nocapture readonly %p) noinline {{
b0:
  %v = load i16, ptr %p
  store i16 %v, ptr @g
  ret i16 %v
}}
{}",
        call_site("@callee")
    ));
    assert_eq!(far_stores(&after), 1, "{after}");
}

/// The walk's promotion mark is removed from every function before the second IPSCCP; the analyses that list declarations kept it on
/// the functions the solver did not restate (`LLRM_CHECK_MODULES`: "@ADDNODE kept ... Flag(llrm-promoted-in-walk), afresh without it").
#[test]
fn the_walks_promotion_mark_is_told_removed_to_the_analyses() {
    let mut module = parsed(include_str!("fixtures/bintree_before_ipo.ll"));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        llrm_mir::passes::with_module_check(|| pipeline::applied(program, &Applied::default()))
    })
    .and_then(|done| done)
    .unwrap();
}

/// bintree's own IR (llrm-qb, whole program, before the optimizer): `walk`
/// passes `t(3*at+1)` by reference and stores the slot back after each
/// recursive call. With the copy-out kept, `walk` took the descriptor by
/// pointer and reloaded it after every call: 308,599 clocks against C's
/// 120,280. The copy-out goes and the descriptor crosses the call as values.
#[test]
fn bintrees_walk_loses_its_copy_outs_and_takes_the_descriptor_as_values() {
    let mut module = parsed(include_str!("fixtures/bintree_before_ipo.ll"));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let after = printed(&module);
    let walk: String = after
        .split("define ")
        .find(|one| one.contains("@\"WALK&\""))
        .map(|one| one.split("\n}").next().unwrap_or(one).to_owned())
        .unwrap_or_default();
    assert!(!walk.is_empty(), "{after}");
    assert_eq!(far_stores(&walk), 0, "{walk}");
    assert!(!walk.contains("dereferenceable(18)"), "{walk}");
}
