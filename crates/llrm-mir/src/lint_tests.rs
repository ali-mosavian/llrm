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

#[test]
fn only_a_geps_inbounds_is_a_raise_s_to_mark() {
    let flagged = "define i16 @f(i16 %a, ptr %p) {\nentry:\n  %q = getelementptr inbounds i16, ptr %p, i16 1\n  %s = add nsw i16 %a, 1\n  ret i16 poison\n}\n";
    assert_eq!(findings(flagged), ["@f: add carries nsw", "@f: ret uses poison"]);
}
