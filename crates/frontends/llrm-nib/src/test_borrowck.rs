//! The borrow checker's soundness: each refused program once compiled.

use crate::test_language::output;

/// `line: message` of the error that refuses `source`.
fn refused_at(source: &str) -> String {
    match super::compile(source, "t") {
        Ok(_) => "accepted".into(),
        Err(error) => format!("{}: {}", error.span.line, error.message),
    }
}

#[test]
fn a_local_shadowing_a_parameter_is_not_returned_as_the_parameter() {
    // #120: `x` resolved to the parameter by name, so a pointer into the
    // callee's frame came back.
    let source = "\
fn pick(x: &i16, c: bool) -> &i16:
    if c:
        let x: i16 = 5
        return x
    return x

fn main() -> i16:
    let one: i16 = 1
    print(pick(one, false))
    return 0
";
    assert_eq!(refused_at(source), "4: a returned borrow of \"x\" would dangle; only a borrowed parameter's can be returned");
    assert_eq!(output(&source.replace("let x: i16 = 5", "let y: i16 = 5")), "1\n");
}

#[test]
fn a_borrow_of_an_owned_parameter_is_not_returned() {
    // #121: the callee dropped `v`'s buffer, then returned a pointer into it.
    let source = "\
fn first(v: vec[i16]) -> &i16:
    return v[0]

fn main() -> i16:
    let v: vec[i16] = [1, 2, 3]
    print(first(v))
    return 0
";
    assert_eq!(refused_at(&source.replace("let v: vec[i16] = [1, 2, 3]\n    print(first(v))", "print(first([1, 2, 3]))")), "2: a returned borrow of \"v\" would dangle; only a borrowed parameter's can be returned");
    assert_eq!(output(&source.replace("(v: vec[i16])", "(v: &[i16])")), "1\n");
}

#[test]
fn a_borrow_stored_into_a_container_never_outlives_what_it_borrows() {
    // #123: `push` went unchecked, so the caller's vec kept a pointer into
    // `stash`'s frame; a call that may store one argument in another hid the
    // same thing from the caller.
    let local = "\
fn stash(out: &mut vec[&i16]) -> void:
    let local: i16 = 7
    out.push(local)

fn main() -> i16:
    let mut keep: vec[&i16] = []
    stash(keep)
    print(keep.len)
    return 0
";
    assert_eq!(refused_at(local), "3: \"out\" would outlive \"local\", which it borrows");
    let lent = "\
fn stash(out: &mut vec[&i16], x: &i16) -> void:
    out.push(x)

fn main() -> i16:
    let mut keep: vec[&i16] = []
    if true:
        let inner: i16 = 7
        stash(keep, inner)
    print(keep.len)
    return 0
";
    assert_eq!(refused_at(lent), "8: \"keep\" would outlive \"inner\", which it borrows");
    assert_eq!(output(&lent.replace("    if true:\n        let inner: i16 = 7\n        stash(keep, inner)", "    let inner: i16 = 7\n    stash(keep, inner)")), "1\n");
}

#[test]
fn a_write_through_a_shared_reference_is_refused_on_any_path() {
    // #122: `h.r = 7` wrote through `&i16` and changed an immutable `let`.
    let field = "\
struct H:
    mut r: &i16

fn main() -> i16:
    let x: i16 = 1
    let mut h = H(r=x)
    h.r = 7
    print(x)
    return 0
";
    assert_eq!(refused_at(field), "7: cannot write through \"h.r\", a '&' reference");
    let element = "\
fn main() -> i16:
    let x: i16 = 1
    let mut v: vec[&i16] = [x]
    v[0] = 7
    print(x)
    return 0
";
    assert_eq!(refused_at(element), "4: cannot write through \"v[...]\", a '&' reference");
    let exclusive = field.replace("mut r: &i16", "mut r: &mut i16").replace("let x: i16", "let mut x: i16").replace("H(r=x)", "H(r=&mut x)");
    assert_eq!(output(&exclusive), "7\n");
}

#[test]
fn a_struct_holding_borrows_is_itself_an_owner_to_borrow() {
    // `&h` rooted at what `h` held, so `t` kept a pointer to `h` after its
    // block ended; and a store into `s` hid what `s` was built with.
    let referenced = "\
struct S:
    r: &i16

struct T:
    mut p: &S

fn main() -> i16:
    let x: i16 = 5
    let s0 = S(r=x)
    let mut t = T(p=s0)
    if true:
        let h = S(r=x)
        t.p = &h
    print(t.p.r)
    return 0
";
    assert_eq!(refused_at(referenced), "13: \"t\" would outlive \"h\", which it borrows");
    let held = "\
struct S:
    mut r: &i16
    mut q: &i16

fn main() -> i16:
    let x: i16 = 0
    let b: i16 = 2
    let mut outer = S(r=x, q=x)
    if true:
        let a: i16 = 1
        let mut s = S(r=a, q=x)
        s.q = &b
        outer = s
    print(outer.r)
    return 0
";
    assert_eq!(refused_at(held), "13: \"outer\" would outlive \"a\", which it borrows");
}
#[test]
fn a_module_variable_is_not_lent_to_a_call_that_may_touch_it() {
    // #124: `poke` wrote `g` while `f` read it through `p: &P`, so `f`
    // returned 99 from a read-only borrow; `&mut` could not state noalias.
    let shared = "\
struct P:
    mut n: i16

var g: P = P(n=1)

fn poke() -> void:
    g.n = 99

fn f(p: &P) -> i16:
    poke()
    return p.n

fn main() -> i16:
    print(f(g))
    return 0
";
    assert_eq!(refused_at(shared), "14: \"g\" is lent to \"f\", which may write it");
    let exclusive = shared.replace("fn f(p: &P) -> i16:\n    poke()", "fn f(p: &mut P) -> i16:\n    p.n = 5\n    peek()").replace("fn poke() -> void:\n    g.n = 99", "fn peek() -> i16:\n    return g.n");
    assert_eq!(refused_at(&exclusive), "15: \"g\" is lent to \"f\", which may read it");
    assert_eq!(output(&shared.replace("    poke()\n", "")), "1\n");
}


#[test]
fn one_call_is_never_lent_an_owner_twice_when_one_lend_writes() {
    // The alias check compared spellings: `f(r, x)` with `r = &mut x` passed,
    // and `q: &i16`, stated noalias, read 5 where it was lent 1.
    let source = "\
fn f(p: &mut i16, q: &i16) -> i16:
    p = 5
    return q

fn main() -> i16:
    let mut x: i16 = 1
    let r = &mut x
    print(f(r, x))
    return 0
";
    assert_eq!(refused_at(source), "8: borrow of \"x\" aliases a mutable argument");
    let held = "\
struct H:
    r: &i16

fn f(p: &mut i16, h: &H) -> i16:
    p = 5
    return h.r

fn main() -> i16:
    let mut x: i16 = 1
    let r = &mut x
    let h = H(r=x)
    print(f(r, h))
    return 0
";
    assert_eq!(refused_at(held), "12: borrow of \"x\" aliases a mutable argument");
}

#[test]
fn foreign_code_touches_only_what_it_can_name_or_call_back() {
    // Foreign code was taken to write every module variable, so lending
    // one to a function that calls it was refused (the loop corpus's `bp`
    // cases); it reaches only shared variables and the module's entries.
    let source = "\
@extern(\"cdecl16\")
fn touch() -> void

var a: i16[4] = [1, 2, 3, 4]

fn total(values: &[i16]) -> i16:
    let mut s: i16 = 0
    for x in values:
        unsafe:
            touch()
        s += x
    return s

fn main() -> i16:
    print(total(&a))
    return 0
";
    assert_eq!(refused_at(source), "accepted");
    let called_back = source.replace("fn main", "@export(\"cdecl16\")\nfn reset() -> void:\n    a[0] = 0\n\nfn main");
    assert_eq!(refused_at(&called_back), "19: \"a\" is lent to \"total\", which may write it");
    let interrupted = source.replace("fn main", "@export(\"interrupt16\")\nfn tick() -> void:\n    a[0] = 0\n\nfn main").replace("        unsafe:\n            touch()\n", "");
    assert_eq!(refused_at(&interrupted), "17: \"a\" is lent to \"total\", which may write it");
}
