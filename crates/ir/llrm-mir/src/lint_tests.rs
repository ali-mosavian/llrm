use crate::lint::poison;
use crate::parse;

fn findings(text: &str) -> Vec<String> {
    poison(&parse::module(text).unwrap_or_else(|error| panic!("{error}")))
}

/// BASIC reads an unassigned local as 0; an alloca read before any store
/// is uninitialized, which a pass may take as any value.
#[test]
fn a_read_before_the_first_store_is_found() {
    let unstored = "define i16 @f(i1 %c) {\nentry:\n  %x = alloca i16\n  br i1 %c, label %set, label %join\n\nset:\n  store i16 1, ptr %x\n  br label %join\n\njoin:\n  %v = load i16, ptr %x\n  ret i16 %v\n}\n";
    assert_eq!(findings(unstored), ["@f: load uses %x before it is stored"]);
    let zeroed = "define i16 @f() {\nentry:\n  %x = alloca i16\n  store i16 0, ptr %x\n  %v = load i16, ptr %x\n  ret i16 %v\n}\n";
    assert_eq!(findings(zeroed), Vec::<String>::new());
}

/// A memset over the whole alloca stores it, as clang zeroes an aggregate;
/// it was read as a use before any store.
#[test]
fn a_memset_of_the_whole_alloca_stores_it() {
    let head = "target datalayout = \"e-p:16:16\"\ndeclare void @llvm.memset.p0.i16(ptr, i8, i16, i1)\n";
    let body = |size: u32| format!("{head}define i8 @f() {{\nentry:\n  %x = alloca [4 x i8]\n  call void @llvm.memset.p0.i16(ptr %x, i8 0, i16 {size}, i1 false)\n  %v = load i8, ptr %x\n  ret i8 %v\n}}\n");
    assert_eq!(findings(&body(4)), Vec::<String>::new());
    assert_eq!(findings(&body(3)), ["@f: load uses %x before it is stored"]);
}

/// A flag on an instruction is the frontend's stated promise (`nsw`, `nuw`,
/// `inbounds`, ...), since the facts mechanism: it was reported as poison on
/// every stated wrap, hiding 56 of 124 programs from the corpus tool. A
/// `poison` constant used is still poison.
#[test]
fn a_stated_flag_is_no_poison_but_a_poison_constant_is() {
    let flagged = "define i16 @f(i16 %a, ptr %p) {\nentry:\n  %q = getelementptr inbounds i16, ptr %p, i16 1\n  %s = add nsw i16 %a, 1\n  %t = add nuw i16 %s, 1\n  ret i16 poison\n}\n";
    assert_eq!(findings(flagged), ["@f: ret uses poison"]);
}

/// Nib fills an array an element at a time through GEPs, a whole fill the
/// lint could not see: every GEP, store and cast of the slot was reported
/// as a use before a store. Only a load is a read, and a load before every
/// store is still found.
#[test]
fn an_array_filled_by_element_stores_is_stored_before_it_is_read() {
    let filled = "define i16 @f() {\nentry:\n  %a = alloca [4 x i8]\n  %p = getelementptr inbounds i16, ptr %a, i16 0\n  store i16 1, ptr %p\n  %q = getelementptr inbounds i16, ptr %a, i16 1\n  store i16 2, ptr %q\n  %v = load i16, ptr %q\n  ret i16 %v\n}\n";
    assert_eq!(findings(filled), Vec::<String>::new());
    let unfilled = "define i16 @f() {\nentry:\n  %a = alloca [4 x i8]\n  %q = getelementptr inbounds i16, ptr %a, i16 1\n  %v = load i16, ptr %q\n  ret i16 %v\n}\n";
    assert_eq!(findings(unfilled), ["@f: load uses %a before it is stored"]);
}

/// An enum stored by its tag alone and read whole carries undefined payload and
/// padding bytes wherever the whole value goes (#290): found. Stored byte for
/// byte, or with a store whose offset is not known, it is not.
#[test]
fn a_value_read_whole_after_only_part_of_it_was_stored_is_found() {
    let head = "target datalayout = \"e-p:16:16\"\n";
    let tag_only = format!("{head}define i32 @f() {{\nentry:\n  %x = alloca [4 x i8]\n  store i8 1, ptr %x\n  %v = load i32, ptr %x\n  ret i32 %v\n}}\n");
    assert_eq!(findings(&tag_only), ["@f: load uses %x before it is stored"]);
    let all_bytes = format!("{head}define i32 @f() {{\nentry:\n  %x = alloca [4 x i8]\n  store i16 1, ptr %x\n  %p = getelementptr i8, ptr %x, i16 2\n  store i16 0, ptr %p\n  %v = load i32, ptr %x\n  ret i32 %v\n}}\n");
    assert_eq!(findings(&all_bytes), Vec::<String>::new());
    let some_bytes = format!("{head}define i16 @f() {{\nentry:\n  %x = alloca [4 x i8]\n  store i16 1, ptr %x\n  %v = load i16, ptr %x\n  ret i16 %v\n}}\n");
    assert_eq!(findings(&some_bytes), Vec::<String>::new(), "a read of the part that was stored");
}
