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
    assert_eq!(findings(&body(3)), ["@f: call uses %x before it is stored", "@f: load uses %x before it is stored"]);
}

#[test]
fn only_a_geps_inbounds_is_a_raise_s_to_mark() {
    let flagged = "define i16 @f(i16 %a, ptr %p) {\nentry:\n  %q = getelementptr inbounds i16, ptr %p, i16 1\n  %s = add nsw i16 %a, 1\n  ret i16 poison\n}\n";
    assert_eq!(findings(flagged), ["@f: add carries nsw", "@f: ret uses poison"]);
}
