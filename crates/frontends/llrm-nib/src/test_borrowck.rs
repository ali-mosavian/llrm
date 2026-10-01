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

#[test]
fn a_value_holding_a_mut_borrow_is_lent_as_one() {
    // The alias check saw only borrowed arguments: `h` holding `&mut x` went
    // with `&x`, and `a`, stated noalias, read the 5 written through `h.r`.
    let source = "\
struct H:
    r: &mut i16

fn f(a: &i16, h: H) -> i16:
    h.r = 5
    return a

fn main() -> i16:
    let mut x: i16 = 1
    print(f(&x, H(r=&mut x)))
    return 0
";
    assert_eq!(refused_at(source), "10: borrow of \"x\" aliases a mutable argument");
    let shared = source.replace("r: &mut i16", "r: &i16").replace("    h.r = 5\n", "").replace("&mut x", "&x");
    assert_eq!(output(&shared), "1\n");
}
#[test]
fn a_borrow_ends_at_its_last_use_on_every_path() {
    // A borrow lived until its binding's scope ended, so changing `v` after
    // the last read of `r` was refused.
    let after = "\
fn main() -> i16:
    let mut v: vec[i16] = [1, 2]
    let r = &v[0]
    print(r)
    v.push(3)
    print(v.len)
    return 0
";
    assert_eq!(output(after), "1\n3\n");
    let exclusive = "\
struct C:
    mut n: i16

fn C.inc(self: &mut C) -> void:
    self.n += 1

fn main() -> i16:
    let mut c = C(n=0)
    let m = &mut c
    m.inc()
    c.inc()
    print(c.n)
    return 0
";
    assert_eq!(output(exclusive), "2\n");
    // Still in use later, on some path: a branch, or the loop's next turn.
    let branch = after.replace("    print(v.len)\n", "    if v.len > 2:\n        print(r)\n");
    assert_eq!(refused_at(&branch), "5: \"v\" is borrowed here, so it cannot be changed");
    let looped = "\
fn main() -> i16:
    let mut v: vec[i16] = [1, 2]
    let r = &v[0]
    let mut i: i16 = 0
    while i < 3:
        print(r)
        v.push(i)
        i += 1
    return 0
";
    assert_eq!(refused_at(looped), "7: \"v\" is borrowed here, so it cannot be changed");
    // A borrow made anew each turn ends with its turn.
    let fresh = looped.replace("    let r = &v[0]\n", "").replace("        print(r)\n", "        let r = &v[0]\n        print(r)\n");
    assert_eq!(output(&fresh), "1\n1\n1\n");
}

#[test]
fn borrows_of_disjoint_fields_do_not_conflict() {
    // A borrow rooted at its owner's whole name, so `&mut p.y` beside
    // `&mut p.x` was refused, and a method walking `self.items` could not
    // count in `self.count`.
    let fields = "\
struct P:
    mut x: i16
    mut y: i16

fn set(a: &mut i16, b: &mut i16) -> void:
    a = 5
    b = 6

fn main() -> i16:
    let mut p = P(x=1, y=2)
    let a = &mut p.x
    let b = &mut p.y
    a = 3
    b = 4
    print(p.x + p.y)
    set(p.x, p.y)
    print(p.x + p.y)
    return 0
";
    assert_eq!(output(fields), "7\n11\n");
    let walked = "\
struct S:
    mut items: vec[i16]
    mut count: i16

fn S.bump(self: &mut S) -> void:
    for x in &self.items:
        self.count += x

fn main() -> i16:
    let mut s = S(items=[1, 2], count=0)
    s.bump()
    print(s.count)
    return 0
";
    assert_eq!(output(walked), "3\n");
    // The same field, a walked sequence, or a borrow a call returned from
    // somewhere in `p`, still conflict.
    assert_eq!(refused_at(&fields.replace("let b = &mut p.y", "let b = &mut p.x")), "12: \"p\" is borrowed here, so it cannot be changed");
    assert_eq!(refused_at(&walked.replace("self.count += x", "self.items.push(x)")), "7: \"self\" is borrowed here, so it cannot be changed");
    let returned = "\
struct P:
    mut x: i16
    mut y: i16

fn pick(p: &mut P) -> &mut i16:
    return p.x

fn main() -> i16:
    let mut p = P(x=1, y=2)
    let a = pick(p)
    let b = &mut p.y
    a = 3
    b = 4
    print(p.x + p.y)
    return 0
";
    assert_eq!(refused_at(returned), "11: \"p\" is borrowed here, so it cannot be changed");
}

#[test]
fn nothing_borrows_what_a_live_mut_borrow_may_change() {
    // Only a change checked the borrows of what it changed: `&v[0]` beside a
    // live `a = &mut v` passed, and `a.push` moved the buffer `r` points into.
    let source = "\
fn main() -> i16:
    let mut v: vec[i16] = [1, 2]
    let a = &mut v
    let r = &v[0]
    a.push(3)
    print(r)
    return 0
";
    assert_eq!(refused_at(source), "4: \"v\" is mutably borrowed here, so it cannot be borrowed");
    let walked = "\
fn main() -> i16:
    let mut v: vec[i16] = [1, 2]
    let a = &mut v
    for x in &v:
        a.push(x)
    return 0
";
    assert_eq!(refused_at(walked), "4: \"v\" is mutably borrowed here, so it cannot be borrowed");
    let ended = source.replace("    let r = &v[0]\n    a.push(3)\n", "    a.push(3)\n    let r = &v[0]\n");
    assert_eq!(output(&ended), "1\n");
}
