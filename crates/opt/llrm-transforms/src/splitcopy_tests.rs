//! A memcpy into a private local is the loads and stores of what is read.

use llrm_analysis::testing::DOS;
use llrm_mir::module::Module;
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Outer;

use super::split;
use crate::interprocedural::function_mut;
use crate::testing::{bodies, parsed, printed, results};

const MEMCPY: &str = "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)\ndeclare void @sink(ptr)\n";

fn module(body: &str) -> Module {
    parsed(&format!("{DOS}{MEMCPY}{body}"))
}

/// `body` with its memcpys split, and whether any was.
fn split_all(body: &str) -> (Module, bool) {
    let mut module = module(body);
    let (layout, outer) = (llrm_analysis::testing::layout(&module), Outer::of(&module, None));
    let mut changed = false;
    for id in bodies(&module) {
        let (context, function) = function_mut(&mut module, id);
        changed |= split(context, &layout, function, &outer);
    }
    printed(&module);
    (module, changed)
}

fn count(module: &Module, is: impl Fn(&Opcode) -> bool) -> usize {
    module.functions().flat_map(|(_, _, function)| function.walk().map(|(_, inst)| function.instruction(inst).opcode.clone()).collect::<Vec<_>>()).filter(|one| is(one)).count()
}

fn copies(module: &Module) -> usize {
    printed(module).matches("call void @llvm.memcpy").count()
}

const TWO_LEAVES: &str = "define i16 @f(i16 %x) {
  %from = alloca [8 x i8]
  %to = alloca [8 x i8]
  store i16 %x, ptr %from
  %p = getelementptr i8, ptr %from, i16 4
  store i16 7, ptr %p
  call void @llvm.memcpy.p0.p0.i16(ptr %to, ptr %from, i16 8, i1 false)
  %a = load i16, ptr %to
  %q = getelementptr i8, ptr %to, i16 4
  %b = load i16, ptr %q
  %r = add i16 %a, %b
  ret i16 %r
}
";

/// The two words a copy's destination is read as were the whole 8-byte
/// memcpy, which the optimizer could not see through: `level` ran 7391 more
/// instructions than with the copy as word loads and stores.
#[test]
fn test_a_memcpy_into_a_private_local_is_the_loads_of_what_is_read() {
    let (after, changed) = split_all(TWO_LEAVES);
    assert!(changed);
    assert_eq!(copies(&after), 0, "{}", printed(&after));
    assert_eq!(count(&after, |one| matches!(one, Opcode::Load { .. })), 4);
    let inputs: &[&[i128]] = &[&[0], &[1], &[40_000]];
    assert_eq!(results(&after, inputs), results(&module(TWO_LEAVES), inputs));
}

#[test]
fn test_a_copy_of_a_copy_is_the_loads_of_the_first_source() {
    let body = "define i16 @f(i16 %x) {
  %a = alloca [8 x i8]
  %b = alloca [8 x i8]
  %c = alloca [8 x i8]
  store i16 %x, ptr %a
  call void @llvm.memcpy.p0.p0.i16(ptr %b, ptr %a, i16 8, i1 false)
  call void @llvm.memcpy.p0.p0.i16(ptr %c, ptr %b, i16 8, i1 false)
  %v = load i16, ptr %c
  ret i16 %v
}
";
    let (after, changed) = split_all(body);
    assert!(changed);
    assert_eq!(copies(&after), 0, "{}", printed(&after));
    let inputs: &[&[i128]] = &[&[0], &[9]];
    assert_eq!(results(&after, inputs), results(&module(body), inputs));
}

/// A destination another function can reach is read as more than its loads here.
#[test]
fn test_a_memcpy_into_a_passed_local_stays_whole() {
    let body = "define i16 @f(i16 %x) {
  %from = alloca [8 x i8]
  %to = alloca [8 x i8]
  store i16 %x, ptr %from
  call void @llvm.memcpy.p0.p0.i16(ptr %to, ptr %from, i16 8, i1 false)
  call void @sink(ptr %to)
  %v = load i16, ptr %to
  ret i16 %v
}
";
    let (after, changed) = split_all(body);
    assert!(!changed, "{}", printed(&after));
}

/// A load across two leaves' bytes is no leaf: the copy stays.
#[test]
fn test_a_memcpy_read_across_leaves_stays_whole() {
    let body = "define i32 @f(i16 %x) {
  %from = alloca [8 x i8]
  %to = alloca [8 x i8]
  store i16 %x, ptr %from
  call void @llvm.memcpy.p0.p0.i16(ptr %to, ptr %from, i16 8, i1 false)
  %a = load i16, ptr %to
  %b = load i32, ptr %to
  ret i32 %b
}
";
    let (after, changed) = split_all(body);
    assert!(!changed, "{}", printed(&after));
}

fn stays(body: &str) {
    let (after, changed) = split_all(body);
    assert!(!changed, "{}", printed(&after));
}

/// A volatile copy is an access in its own right: it is not the loads and stores of leaves.
#[test]
fn test_a_volatile_memcpy_stays_whole() {
    stays("define i16 @f(i16 %x) {
  %from = alloca [8 x i8]
  %to = alloca [8 x i8]
  store i16 %x, ptr %from
  call void @llvm.memcpy.p0.p0.i16(ptr %to, ptr %from, i16 8, i1 true)
  %v = load i16, ptr %to
  ret i16 %v
}
");
}

/// The leaves are the destination's bytes; a length not known has none.
#[test]
fn test_a_memcpy_of_a_length_not_constant_stays_whole() {
    stays("define i16 @f(i16 %x, i16 %n) {
  %from = alloca [8 x i8]
  %to = alloca [8 x i8]
  store i16 %x, ptr %from
  call void @llvm.memcpy.p0.p0.i16(ptr %to, ptr %from, i16 %n, i1 false)
  %v = load i16, ptr %to
  ret i16 %v
}
");
}

/// A copy within one object overlaps itself: loads of the destination are
/// loads of the source's bytes too.
#[test]
fn test_a_memcpy_within_one_local_stays_whole() {
    stays("define i16 @f(i16 %x) {
  %a = alloca [8 x i8]
  store i16 %x, ptr %a
  %q = getelementptr i8, ptr %a, i16 2
  call void @llvm.memcpy.p0.p0.i16(ptr %q, ptr %a, i16 2, i1 false)
  %v = load i16, ptr %q
  ret i16 %v
}
");
}

/// A destination whose address is stored is reachable by whoever loads it.
#[test]
fn test_a_memcpy_into_a_captured_local_stays_whole() {
    stays("@slot = global ptr null
define i16 @f(i16 %x) {
  %from = alloca [8 x i8]
  %to = alloca [8 x i8]
  store i16 %x, ptr %from
  store ptr %to, ptr @slot
  call void @llvm.memcpy.p0.p0.i16(ptr %to, ptr %from, i16 8, i1 false)
  %v = load i16, ptr %to
  ret i16 %v
}
");
}
