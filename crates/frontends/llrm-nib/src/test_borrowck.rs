//! The borrow checker's soundness: each refused program once compiled.

use crate::test_language::{output, output_without_leaks};

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
    assert_eq!(
        refused_at(source),
        "4: a returned borrow of \"x\" would dangle; only a borrowed parameter's can be returned"
    );
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
    assert_eq!(
        refused_at(&source.replace("let v: vec[i16] = [1, 2, 3]\n    print(first(v))", "print(first([1, 2, 3]))")),
        "2: a returned borrow of \"v\" would dangle; only a borrowed parameter's can be returned"
    );
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
    assert_eq!(
        output(&lent.replace(
            "    if true:\n        let inner: i16 = 7\n        stash(keep, inner)",
            "    let inner: i16 = 7\n    stash(keep, inner)"
        )),
        "1\n"
    );
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
    let exclusive = field
        .replace("mut r: &i16", "mut r: &mut i16")
        .replace("let x: i16", "let mut x: i16")
        .replace("H(r=x)", "H(r=&mut x)");
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
    let exclusive = shared
        .replace("fn f(p: &P) -> i16:\n    poke()", "fn f(p: &mut P) -> i16:\n    p.n = 5\n    peek()")
        .replace("fn poke() -> void:\n    g.n = 99", "fn peek() -> i16:\n    return g.n");
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
    let interrupted = source
        .replace("fn main", "@export(\"interrupt16\")\nfn tick() -> void:\n    a[0] = 0\n\nfn main")
        .replace("        unsafe:\n            touch()\n", "");
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
    let fresh = looped
        .replace("    let r = &v[0]\n", "")
        .replace("        print(r)\n", "        let r = &v[0]\n        print(r)\n");
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
    assert_eq!(
        refused_at(&fields.replace("let b = &mut p.y", "let b = &mut p.x")),
        "12: \"p\" is borrowed here, so it cannot be changed"
    );
    assert_eq!(
        refused_at(&walked.replace("self.count += x", "self.items.push(x)")),
        "7: \"self\" is borrowed here, so it cannot be changed"
    );
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
    assert_eq!(refused_at(walked), "5: \"a\" is borrowed here, so it cannot be changed");
    let ended = source.replace("    let r = &v[0]\n    a.push(3)\n", "    a.push(3)\n    let r = &v[0]\n");
    assert_eq!(output(&ended), "1\n");
}

#[test]
fn a_change_through_a_reference_changes_what_it_borrows() {
    // `a.push` was checked against borrows of `a`, not of `v`, so `r`, a
    // borrow of `v` taken through `a`, outlived the buffer `push` moved.
    let source = "\
fn main() -> i16:
    let mut v: vec[i16] = [1, 2]
    let a = &mut v
    let r = &a[0]
    a.push(3)
    print(r)
    return 0
";
    assert_eq!(refused_at(source), "5: \"a\" is borrowed here, so it cannot be changed");
    assert_eq!(output(&source.replace("    print(r)\n", "    print(a.len)\n")), "3\n");
}

#[test]
fn a_consumed_generator_borrows_what_it_is_lent() {
    // Its parameter, bound where the loop inlines it, borrowed nothing, so
    // with borrows ending at last use `v.push` inside the loop passed.
    let source = "\
fn walk(v: &[i16]) -> iter[i16]:
    for x in v:
        yield x

fn main() -> i16:
    let mut v: vec[i16] = [1, 2]
    for x in walk(v):
        v.push(x)
    print(v.len)
    return 0
";
    assert_eq!(refused_at(source), "8: \"v\" is borrowed here, so it cannot be changed");
}

#[test]
fn a_binding_holds_the_borrows_its_value_holds() {
    // Binding `o` recorded only what a struct literal kept, so with borrows
    // ending at last use, `r`'s last use was `.some(r)` and `v.push` passed.
    let source = "\
fn main() -> i16:
    let mut v: vec[i16] = [1, 2]
    let r = &v[0]
    let o: Option[&i16] = .some(r)
    v.push(3)
    match o:
        .some(p):
            print(p)
        .none:
            print(0)
    return 0
";
    assert_eq!(refused_at(source), "5: \"v\" is borrowed here, so it cannot be changed");
}

#[test]
fn a_mut_walk_changes_what_it_walks() {
    // `for x in &mut v` checked nothing on entry: `r`, a `&i16`, read the 9
    // the walk wrote.
    let source = "\
fn main() -> i16:
    let mut v: vec[i16] = [1, 2]
    let r = &v[0]
    for x in &mut v:
        x = 9
    print(r)
    return 0
";
    assert_eq!(refused_at(source), "4: \"v\" is borrowed here, so it cannot be changed");
    assert_eq!(output(&source.replace("    print(r)\n", "    print(v[0])\n")), "9\n");
}

#[test]
fn whatever_bundles_borrows_lends_them_disjoint() {
    // An escaping generator's state, a struct literal and a variant took
    // `&mut p` beside `&p` unchecked: `gen(p, p)` ran `bump` with its noalias
    // `src` loaded once, before the loop, and gave 4 where 8 is right.
    let generator = "\
struct P:
    mut x: i16
    mut y: i16

fn bump(dst: &mut P, src: &P, n: i16) -> i16:
    for i in 0..n:
        dst.y += src.y
    return dst.y

fn gen(a: &mut P, b: &P) -> iter[i16]:
    yield bump(a, b, 3)

fn main() -> i16:
    let mut p = P(x=0, y=1)
    let it = gen(p, p)
    for v in it:
        print(v)
    return 0
";
    assert_eq!(refused_at(generator), "15: borrow of \"p\" aliases a mutable argument");
    let apart = generator.replace("    let it = gen(p, p)\n", "    let q = P(x=0, y=1)\n    let it = gen(p, q)\n");
    assert_eq!(output(&apart), "4\n");
    let literal = "\
struct H2:
    a: &mut i16
    b: &i16

fn main() -> i16:
    let mut x: i16 = 1
    let h = H2(a=&mut x, b=&x)
    h.a = 5
    print(h.b)
    return 0
";
    assert_eq!(refused_at(literal), "7: borrow of \"x\" aliases a mutable argument");
}

#[test]
fn an_inlined_generator_lends_what_its_caller_lent_it() {
    // Its parameter borrowed nothing, so `g` reached `f`'s `&mut` unchecked
    // while `f` read `g`.
    let source = "\
var g: i16 = 1

fn f(p: &mut i16, n: i16) -> i16:
    let mut s: i16 = 0
    for i in 0..n:
        p += 1
        s += g
    return s

fn gen(x: &mut i16, n: i16) -> iter[i16]:
    yield f(x, n)

fn main() -> i16:
    for y in gen(g, 3):
        print(y)
    return 0
";
    assert_eq!(refused_at(source), "11: \"g\" is lent to \"f\", which may read it");
}

#[test]
fn a_value_never_holds_a_borrow_of_itself() {
    // `n.p = &n.v` passed; `f(n)` then stated noalias on `n` while `n.p`
    // reached `n.v`.
    let source = "\
struct N:
    mut v: i16
    mut p: &i16

fn main() -> i16:
    let z: i16 = 0
    let mut n = N(v=1, p=z)
    n.p = &n.v
    print(n.p)
    return 0
";
    assert_eq!(refused_at(source), "8: \"n\" would hold a borrow of itself");
}

#[test]
fn a_module_variable_borrowed_across_a_call_is_not_written_by_it() {
    // Only lends were checked: `r` read the 9 that `setg` wrote under it.
    let source = "\
var g: i16[2] = [1, 2]

fn setg() -> void:
    g[0] = 9

fn main() -> i16:
    let r = &g[0]
    setg()
    print(r)
    return 0
";
    assert_eq!(refused_at(source), "8: \"g\" is borrowed across a call to \"setg\", which may write it");
}

#[test]
fn a_lambda_changes_what_its_caller_borrows() {
    // Inlined in its own scopes, the lambda's `v.push` saw no borrow of `v`,
    // so `r` outlived the buffer it moved.
    let source = "\
fn main() -> i16:
    let mut v: vec[i16] = [1]
    let k = |d: i16| v.push(d)
    let r = &v[0]
    k(1)
    print(r)
    return 0
";
    assert_eq!(refused_at(source), "3: \"v\" is borrowed here, so it cannot be changed");
    assert_eq!(output(&source.replace("    print(r)\n", "    print(v.len)\n")), "2\n");
}

#[test]
fn a_module_borrow_ends_before_a_call_that_writes_it() {
    // A borrow of `g` no longer used counts across no call.
    let source = "\
var g: i16[2] = [1, 2]

fn setg() -> void:
    g[0] = 9

fn main() -> i16:
    let r = &g[0]
    print(r)
    setg()
    print(g[0])
    return 0
";
    assert_eq!(output(source), "1\n9\n");
}

#[test]
fn an_owner_is_not_dropped_while_a_later_drop_reads_a_borrow_of_it() {
    // #131: `v`, declared after `g`, dropped first; `g`'s drop then read
    // `v[0]` from freed memory.
    let source = "\
struct G:
    mut r: &i16

fn G.drop(self: &mut G) -> void:
    print(self.r)

fn main() -> i16:
    let z: i16 = 0
    let mut g = G(r=z)
    let v: vec[i16] = [7, 8]
    g.r = &v[0]
    return 0
";
    assert_eq!(refused_at(source), "12: \"v\" is dropped here while still borrowed");
    // With no drop to read it, the borrow ends at its last use.
    assert_eq!(output(&source.replace("fn G.drop(self: &mut G) -> void:\n    print(self.r)\n\n", "")), "");
}

#[test]
fn an_escaping_generator_lends_its_arguments_as_a_call_does() {
    // #137: its frame was built as a struct literal, which stored nothing,
    // so `keep` kept the `&inner` that `stash` pushed, after `inner` ended.
    let source = "\
fn stash(out: &mut vec[&i16], x: &i16) -> iter[i16]:
    out.push(x)
    yield 1

fn main() -> i16:
    let mut keep: vec[&i16] = []
    if true:
        let inner: i16 = 7
        let it = stash(keep, inner)
        for v in it:
            print(v)
    print(keep.len)
    return 0
";
    assert_eq!(refused_at(source), "9: \"keep\" would outlive \"inner\", which it borrows");
    let outer = source.replace("    if true:\n        let inner: i16 = 7\n        let it = stash(keep, inner)\n        for v in it:\n            print(v)\n", "    let inner: i16 = 7\n    let it = stash(keep, inner)\n    for v in it:\n        print(v)\n");
    assert_eq!(output(&outer), "1\n1\n");
}

#[test]
fn a_field_moves_out_alone() {
    // #135: a field never moved, so `take(p.a)` was refused though `p.b`
    // stays whole.
    let source = "\
struct P:
    a: string
    b: string

fn take(s: string) -> void:
    print(s)

fn main() -> i16:
    let p = P(a=\"x\".copy(), b=\"y\".copy())
    take(p.a)
    print(p.b)
    return 0
";
    assert_eq!(crate::test_language::output_without_leaks(source), "x\ny\n");
    // What moved cannot be used, nor the whole it was part of.
    assert_eq!(
        refused_at(&source.replace("    print(p.b)\n", "    print(p.a)\n")),
        "11: \"p.a\" was moved; copy it with .copy() to keep using it"
    );
    let whole = source
        .replace("fn main", "fn keep(p: P) -> void:\n    print(p.b)\n\nfn main")
        .replace("    print(p.b)\n    return 0", "    keep(p)\n    return 0");
    assert_eq!(refused_at(&whole), "14: \"p\" was partly moved: \"p.a\"");
    // Given a value again, it is whole again.
    let again = source
        .replace("    a: string", "    mut a: string")
        .replace("let p = ", "let mut p = ")
        .replace("    print(p.b)\n", "    p.a = \"z\".copy()\n    print(p.a)\n");
    assert_eq!(crate::test_language::output_without_leaks(&again), "x\nz\n");
    // A struct with a drop is dropped whole: nothing moves out of it.
    let dropped = source.replace("fn take", "fn P.drop(self: &mut P) -> void:\n    print(\"bye\")\n\nfn take");
    assert_eq!(refused_at(&dropped), "13: cannot move a field out of P, which has a drop");
    // A nested struct moves out the same way.
    let nested = "\
struct Q:
    s: string

struct P:
    q: Q
    t: string

fn take(q: Q) -> void:
    print(q.s)

fn main() -> i16:
    let p = P(q=Q(s=\"x\".copy()), t=\"y\".copy())
    take(p.q)
    print(p.t)
    return 0
";
    assert_eq!(crate::test_language::output_without_leaks(nested), "x\ny\n");
    assert_eq!(
        refused_at(&nested.replace("    print(p.t)\n", "    print(p.q.s)\n")),
        "14: \"p.q\" was moved; copy it with .copy() to keep using it"
    );
}

#[test]
fn a_field_with_a_drop_moves_out_and_is_dropped_only_where_it_stayed() {
    // A field holding a type with a `drop` could not move out: it has no
    // null to leave. Now what surely moved is not dropped at all, and what
    // moved on one path only is dropped under a flag.
    let source = "\
struct R:
    n: i16

fn R.drop(self: &mut R) -> void:
    print(f\"drop {self.n}\")

struct P:
    mut a: R
    b: R

fn take(r: R) -> void:
    print(r.n)

fn refill(c: bool) -> void:
    let mut p = P(a=R(n=7), b=R(n=8))
    take(p.a)
    p.a = R(n=9)
    if c:
        take(p.a)

fn always() -> void:
    let p = P(a=R(n=1), b=R(n=2))
    take(p.a)
    print(p.b.n)

fn maybe(c: bool) -> void:
    let p = P(a=R(n=3), b=R(n=4))
    if c:
        take(p.a)

fn main() -> i16:
    always()
    maybe(true)
    maybe(false)
    refill(false)
    refill(true)
    let mut i: i16 = 0
    while i < 2:
        let q = P(a=R(n=5), b=R(n=6))
        if i == 0:
            take(q.a)
        i += 1
    return 0
";
    assert_eq!(
        crate::test_language::output_without_leaks(source),
        "1\ndrop 1\n2\ndrop 2\n3\ndrop 3\ndrop 4\ndrop 3\ndrop 4\n7\ndrop 7\ndrop 9\ndrop 8\n7\ndrop 7\n9\ndrop 9\ndrop 8\n5\ndrop 5\ndrop 6\ndrop 5\ndrop 6\n"
    );
}

#[test]
fn a_native_enums_tag_is_stated_to_hold_only_its_variants() {
    // The tag was loaded and compared against each variant in turn, the
    // last arm's test as live as the first: nothing said the tag has no
    // other value.
    let source = "\
enum Shape:
    dot
    line(i16)
    box(i16, i16)

fn area(s: &Shape) -> i16:
    match s:
        .dot:
            return 0
        .line(n):
            return n
        .box(w, h):
            return w * h

fn main() -> i16:
    let s = Shape.box(2, 3)
    print(area(s))
    return 0
";
    let text = super::compile(source, "t").unwrap_or_else(|error| panic!("{}", error.message));
    let hir: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    let ranges: Vec<(i64, i64)> = hir["modules"][0]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|one| one["fact"] == "range" && one["subject"] == "field")
        .map(|one| (one["value"].as_i64().unwrap(), one["second"].as_i64().unwrap()))
        .collect();
    // Stated once, of the tag member of the enum; each arm's test loads it as that member.
    assert_eq!(ranges, [(0, 2)]);
    assert!(text.matches("\"member\"").count() >= 2, "{text}");
}

/// The `nocapture` facts the program states, as `function.ordinal` of each
/// parameter, by the parameter's index in the function's HIR.
fn uncaptured(source: &str) -> Vec<String> {
    let hir: serde_json::Value = serde_json::from_str(
        &super::compile(source, "t").unwrap_or_else(|error| panic!("{}: {}", error.span.line, error.message)),
    )
    .expect("JSON");
    let facts = hir["modules"][0]["facts"].as_array().cloned().unwrap_or_default();
    let name = |id: i64| {
        hir["modules"][0]["callables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|one| one["id"].as_i64() == Some(id))
            .map(|one| one["name"].as_str().unwrap().to_owned())
            .unwrap()
    };
    let mut found: Vec<String> = facts
        .iter()
        .filter(|one| one["fact"] == "nocapture" && one["subject"] == "param")
        .map(|one| format!("{}.{}", name(one["function"].as_i64().unwrap()), one["id"]))
        .collect();
    found.sort();
    found
}

#[test]
fn a_borrow_the_function_does_not_keep_is_nocapture() {
    // `&T` and `&mut T` stated no `nocapture`: a function may return a
    // borrowed parameter or store it where the caller keeps it. The checker
    // knows where each borrow goes, so the facts follow from it.
    let source = "\
fn read(a: &i16) -> i16:
    return a

fn pick(a: &i16, b: &i16) -> &i16:
    return a

fn stash(out: &mut vec[&i16], x: &i16) -> void:
    out.push(x)

fn through(x: &i16) -> void:
    let mut keep: vec[&i16] = []
    stash(keep, x)

fn reads_through(x: &i16) -> i16:
    return read(x)

fn main() -> i16:
    let a: i16 = 1
    let b: i16 = 2
    let mut out: vec[&i16] = []
    let p = pick(a, b)
    print(read(a) + reads_through(a))
    print(p)
    stash(out, a)
    through(a)
    return 0
";
    // read.0, pick.1 (b), stash.0 (out is only pushed to), reads_through.0
    assert_eq!(uncaptured(source), ["pick.1", "read.0", "reads_through.0", "stash.0"]);
    // A view is a borrow too: returning a slice of it keeps it; summing it does not.
    let views = "\
fn tail(xs: &[i16]) -> &[i16]:
    return &xs[1:]

fn sum(xs: &[i16]) -> i16:
    let mut s: i16 = 0
    for x in xs:
        s += x
    return s

fn main() -> i16:
    let a: i16[3] = [1, 2, 3]
    print(sum(a))
    print(sum(tail(a)))
    return 0
";
    assert_eq!(uncaptured(views), ["sum.0"]);
    // A generator's frame outlives its call; a raw pointer goes where nothing follows it.
    let escapes = "\
fn walk(v: &[i16]) -> iter[i16]:
    for x in v:
        yield x

fn address(x: &i16) -> void:
    unsafe:
        let p: *far i16 = &x

fn main() -> i16:
    let a: i16[2] = [1, 2]
    let it = walk(a)
    return 0
";
    // Only the frame's own `next(self)`, which keeps nothing of itself.
    assert_eq!(uncaptured(escapes), ["$state0.next.0"]);
}

#[test]
fn an_owned_aggregate_parameter_is_unaliased() {
    // The caller copies a by-value struct for the call, so the callee's
    // pointer reaches nothing else; none was stated, and a store through a
    // borrow could not be told apart from a store to the copy.
    let source = "\
struct P:
    mut x: i16
    y: i16

fn take(p: P, q: &mut P) -> i16:
    q.x = 5
    return p.x

fn main() -> i16:
    let mut a = P(x=1, y=2)
    let mut b = P(x=3, y=4)
    print(take(a, b))
    return 0
";
    let hir: serde_json::Value =
        serde_json::from_str(&super::compile(source, "t").unwrap_or_else(|error| panic!("{}", error.message)))
            .expect("JSON");
    let take =
        hir["modules"][0]["callables"].as_array().unwrap().iter().find(|one| one["name"] == "take").unwrap()["id"]
            .as_i64()
            .unwrap();
    let noalias: Vec<i64> = hir["modules"][0]["facts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|one| one["fact"] == "noalias" && one["function"].as_i64() == Some(take))
        .map(|one| one["id"].as_i64().unwrap())
        .collect();
    assert_eq!(noalias, [0, 1]);
}

/// What a Nib match states of a tag reached the reader: the enum's tag load, bounded
/// by its variants, from source through the HIR's facts and their lowering to the
/// interval ranges reads. The `!range` was written and nothing read it.
#[test]
fn a_matched_enums_tag_is_bounded_by_its_variants_where_ranges_reads_it() {
    let source = "\
enum Shape:
    dot
    line(i16)
    box(i16, i16)

fn area(s: &Shape) -> i16:
    match s:
        .dot:
            return 0
        .line(n):
            return n
        .box(w, h):
            return w * h

fn main() -> i16:
    let s = Shape.box(2, 3)
    print(area(s))
    return 0
";
    let text = super::compile(source, "t").unwrap_or_else(|error| panic!("{}", error.message));
    let program = llrm_hir::codec::decode(&text).expect("HIR");
    let module = llrm_core::hir::mir::emit(&program, &llrm_x86_m16::layout()).remove(0).module;
    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap_or("")).expect("a layout");
    let function = module
        .functions()
        .find(|(_, global, _)| global.name.as_deref().is_some_and(|name| name.contains("area")))
        .expect("area")
        .2;
    let unit = llrm_analysis::memory::Unit::of(&module, &layout, function);
    let registers = llrm_analysis::consts::known(&unit, None, None, None);
    let shape = llrm_analysis::cfg::Shape::of(function);
    let unit = unit.with_registers(&registers).with_shape(&shape);
    let known = llrm_analysis::ranges::scoped(&unit).expect("ranges");
    let tag_loads: Vec<_> = function
        .walk()
        .filter(|&(_, inst)| {
            matches!(
                function.instruction(inst).opcode,
                llrm_mir::opcode::Opcode::Load { .. }
            )
                && function.instruction(inst).metadata.iter().any(|(kind, _)| kind == "range")
        })
        .collect();
    assert!(!tag_loads.is_empty(), "the tag loads carry !range");
    for (block, inst) in tag_loads {
        let result = function.instruction(inst).result.expect("a tag");
        let found = known.get(&llrm_analysis::cfg::id(block)).and_then(|at| at.get(&result));
        let bound = found.map(|one| (one.low.clone(), one.high.clone()));
        assert_eq!(bound, Some((0.into(), 2.into())), "the tag in block {}", llrm_analysis::cfg::id(block));
    }
}

#[test]
fn a_borrow_is_refused_wherever_it_could_outlive_or_overlap_a_change() {
    let longer =
        "fn longer(a: &string, b: &string) -> &string:\n    if a.len >= b.len:\n        return a\n    return b\n\n";
    let cases = [
        // A view of the buffer that `push` may move.
        ("fn main() -> i16:\n    let mut v: vec[i16] = [1, 2, 3]\n    let w = &v[1:3]\n    v.push(9)\n    print(w[0])\n    return 0\n".to_owned(), "4: \"v\" is borrowed here, so it cannot be changed"),
        // The result may be either argument, so both stay borrowed.
        (format!("{longer}fn main() -> i16:\n    let x = \"north\"\n    let mut y = \"east\"\n    let r = longer(x, y)\n    y = \"changed\"\n    print(r)\n    return 0\n"), "10: \"y\" is borrowed here, so it cannot be changed"),
        // A field reseated to a local of an inner block.
        ("struct Holder:\n    mut r: &i16\n\nfn main() -> i16:\n    let a: i16 = 1\n    let mut h = Holder(r=a)\n    if a > 0:\n        let b: i16 = 5\n        h.r = &b\n    print(h.r)\n    return 0\n".to_owned(), "9: \"h\" would outlive \"b\", which it borrows"),
        // A borrow held across the loop's back edge.
        ("fn main() -> i16:\n    let mut v: vec[i16] = [5]\n    let last = &v[0]\n    for i in 0..3:\n        v.push(i)\n        print(last)\n    return 0\n".to_owned(), "5: \"v\" is borrowed here, so it cannot be changed"),
        // A method taking all of `self` while one field is walked.
        ("struct Bag:\n    mut items: vec[i16]\n\nfn Bag.add(self: &mut Bag, x: i16) -> void:\n    self.items.push(x)\n\nfn Bag.double(self: &mut Bag) -> void:\n    for x in &self.items:\n        self.add(x)\n\nfn main() -> i16:\n    let mut b = Bag(items=[1, 2])\n    b.double()\n    return 0\n".to_owned(), "9: \"self\" is borrowed here, so it cannot be changed"),
        // A borrow of a local returned inside a struct.
        ("struct Holder:\n    r: &i16\n\nfn make() -> Holder:\n    let local: i16 = 3\n    return Holder(r=local)\n\nfn main() -> i16:\n    print(make().r)\n    return 0\n".to_owned(), "6: a returned borrow of \"local\" would dangle; only a borrowed parameter's can be returned"),
    ];
    for (source, refusal) in cases {
        assert_eq!(refused_at(&source), refusal, "{source}");
    }
}

#[test]
fn a_returned_generator_does_not_borrow_the_returning_functions_local() {
    // #415: `return evens(local)` was accepted when `make()` was consumed in
    // place, though refused once it escaped: the returned generator's state
    // would hold a borrow of `make`'s local.
    let source = "\
fn evens(v: &vec[i16]) -> iter[i16]:
    for x in v:
        if x % 2 == 0:
            yield x

fn make() -> iter[i16]:
    let local: vec[i16] = [1, 2, 3, 4]
    return evens(local)

fn main() -> i16:
    for x in make():
        print(x)
    return 0
";
    let message = "8: a returned borrow of \"local\" would dangle; only a borrowed parameter's can be returned";
    assert_eq!(refused_at(source), message);
    assert_eq!(refused_at(&source.replace("for x in make():", "let it = make()\n    for x in it:")), message);
    // What the caller lent outlives the returned generator.
    let lent = source
        .replace(
            "fn make() -> iter[i16]:\n    let local: vec[i16] = [1, 2, 3, 4]\n    return evens(local)",
            "fn make(local: &vec[i16]) -> iter[i16]:\n    return evens(local)",
        )
        .replace("for x in make():", "let local: vec[i16] = [1, 2, 3, 4]\n    for x in make(local):");
    assert_eq!(output(&lent), "2\n4\n");
    // A generator expression over the local is the same state.
    let expression = source.replace("return evens(local)", "return (x for x in local if x % 2 == 0)");
    assert_eq!(refused_at(&expression), message);
}

#[test]
fn a_loop_variable_over_a_borrowed_parameter_returns_as_a_borrow() {
    // #422: `p` of `for p in v` rooted in the loop binding, so returning it
    // was refused as dangling though the element lives in the caller's `v`.
    let source = "\
struct P:
    n: i16

fn hit(v: &[P], n: i16) -> &P:
    for p in v:
        if p.n == n:
            return p
    return v[0]

fn main() -> i16:
    let v: vec[P] = [P(n=1), P(n=2)]
    print(hit(v, 2).n)
    return 0
";
    assert_eq!(output_without_leaks(source), "2\n");
    let optional = source
        .replace("-> &P:", "-> Option[&P]:")
        .replace("return p", "return .some(p)")
        .replace("return v[0]", "return .none")
        .replace(
            "print(hit(v, 2).n)",
            "match hit(v, 2):\n        .some(p):\n            print(p.n)\n        .none:\n            print(0)",
        );
    assert_eq!(output_without_leaks(&optional), "2\n");
    assert_eq!(
        output_without_leaks(&source.replace("v: &[P]", "v: &vec[P]").replace("for p in v", "for p in &v")),
        "2\n"
    );
    // The element of a local's loop is the local's: it dangles.
    let local = "\
struct P:
    n: i16

fn first() -> &P:
    let v: vec[P] = [P(n=1)]
    for p in v:
        return p
    return v[0]

fn main() -> i16:
    print(first().n)
    return 0
";
    assert_eq!(
        refused_at(local),
        "7: a returned borrow of \"v\" would dangle; only a borrowed parameter's can be returned"
    );
}

#[test]
fn a_conditional_of_borrowed_strings_is_a_borrow() {
    // #419: `?:` with `&string` arms was "the arms of '?:' need a known type".
    let source = "\
fn longer(a: &string, b: &string) -> &string:
    return a.len >= b.len ? a : b

fn main() -> i16:
    let x = \"ab\"
    let y = \"abc\"
    print(longer(x, y))
    print(longer(y, x))
    return 0
";
    assert_eq!(output_without_leaks(source), "abc\nabc\n");
    let local = source.replace(
        "fn longer(a: &string, b: &string) -> &string:\n    return a.len >= b.len ? a : b",
        "fn longer(a: &string, b: &string) -> &string:\n    let c = \"zz\"\n    return a.len >= b.len ? a : c",
    );
    assert_eq!(
        refused_at(&local),
        "3: a returned borrow of \"c\" would dangle; only a borrowed parameter's can be returned"
    );
}

#[test]
fn a_let_mut_borrow_is_reseated_by_assigning_a_borrow() {
    // #417: `last = &v[1]` of a `&i16` was "binding is immutable"; `&string`
    // reseated. A reference now lives in a cell the assignment rewrites.
    let source = "\
fn main() -> i16:
    let mut v: vec[i16] = [5, 6]
    let mut last = &v[0]
    print(last)
    last = &v[1]
    print(last)
    return 0
";
    assert_eq!(output_without_leaks(source), "5\n6\n");
    // Reseated in a branch and in a loop, read after each.
    let flow = "\
fn main() -> i16:
    let mut v: vec[i16] = [5, 6, 7]
    let mut cur = &v[0]
    let mut i: i16 = 0
    while cur < 7:
        print(cur)
        i += 1
        cur = &v[i]
    if i == 2:
        cur = &v[0]
    print(cur)
    return 0
";
    assert_eq!(output_without_leaks(flow), "5\n6\n5\n");
    // Still a borrow of `v` until reseated (E0502).
    let held = source.replace("    last = &v[1]\n    print(last)\n", "    v.push(9)\n    print(last)\n");
    assert_eq!(refused_at(&held), "5: \"v\" is borrowed here, so it cannot be changed");
    // A reseated borrow is of the new owner, which a return may not leak (E0515).
    let leak = "\
fn bad() -> &i16:
    let v: vec[i16] = [1]
    let mut r = &v[0]
    r = &v[0]
    return r

fn main() -> i16:
    print(bad())
    return 0
";
    assert_eq!(
        refused_at(leak),
        "5: a returned borrow of \"v\" would dangle; only a borrowed parameter's can be returned"
    );
}

#[test]
fn reseating_a_borrow_ends_its_old_borrow() {
    // #418: a borrow lived to the binding's last use, not the value's, so a
    // reseated binding kept its owner borrowed. Rust's NLL accepts these.
    for (element, first, second, shown) in [("string", "\"a\"", "\"b\"", "a\nb\n"), ("i16", "1", "2", "1\n2\n")] {
        let source = format!(
            "\
fn main() -> i16:
    let mut v: vec[{element}] = [{first}]
    let mut last = &v[0]
    print(last)
    v.push({second})
    last = &v[1]
    print(last)
    return 0
"
        );
        assert_eq!(output_without_leaks(&source), shown, "{element}");
        // Without the reseat the second `print` reads a buffer `push` may move (E0502).
        let held = source.replace("    last = &v[1]\n", "");
        assert_eq!(refused_at(&held), "5: \"v\" is borrowed here, so it cannot be changed", "{element}");
    }
    // In a loop: print, push, reseat.
    let looped = "\
fn main() -> i16:
    let mut words: vec[string] = [\"a\"]
    let mut last = &words[0]
    for i in 0..3:
        print(last)
        words.push(\"b\")
        last = &words[i + 1]
    print(last)
    return 0
";
    assert_eq!(output_without_leaks(looped), "a\nb\nb\nb\n");
    // A loop that pushes without reseating reads the stale borrow (E0502).
    assert_eq!(
        refused_at(&looped.replace("        last = &words[i + 1]\n", "")),
        "6: \"words\" is borrowed here, so it cannot be changed"
    );
    // A branch that may leave the old borrow in place keeps it.
    let branch = "\
fn main() -> i16:
    let mut v: vec[i16] = [1, 2]
    let mut last = &v[0]
    let c = v.len > 1
    if c:
        last = &v[1]
    v.push(3)
    print(last)
    return 0
";
    assert_eq!(refused_at(branch), "7: \"v\" is borrowed here, so it cannot be changed");
}

#[test]
fn a_string_behind_a_borrowed_struct_field_is_read_and_borrowed() {
    // #423: `h.src.text.len` was "scalar or array field cannot be used as a
    // struct", `&h.src.text` "only a place can be borrowed".
    let source = "\
struct Source:
    text: string
    count: u16

struct Holder:
    src: &Source

fn first(h: &Holder) -> char:
    return h.src.text[0]

fn view(h: &Holder) -> &string:
    return &h.src.text

fn size(h: &Holder) -> u16:
    let t = &h.src.text
    return t.len + h.src.text.len + h.src.count

fn main() -> i16:
    let s = Source(text=\"move north\", count=3)
    let h = Holder(src=s)
    print(first(h))
    print(view(h))
    print(size(h))
    return 0
";
    assert_eq!(output_without_leaks(source), "m\nmove north\n23\n");
}

#[test]
fn a_borrow_copied_out_of_a_field_is_not_tied_to_the_struct_holding_it() {
    // #424: `let src = self.src` rooted `src` in `self`, so `self.at += 1`
    // was refused while it lived. The copy points at the caller's value.
    let source = "\
struct Source:
    count: u16

struct Scanner:
    src: &Source
    mut at: u16

fn Scanner.size(self: &mut Scanner) -> u16:
    let src = self.src
    self.at += 1
    return src.count + self.at

fn bump(s: Scanner) -> u16:
    let src = s.src
    return src.count

fn main() -> i16:
    let src = Source(count=7)
    let mut s = Scanner(src=src, at=0)
    print(s.size())
    print(bump(s))
    return 0
";
    assert_eq!(output_without_leaks(source), "8\n7\n");
    // The copy is still the caller's `src`: changing it while the copy lives is refused (E0506).
    let caller = "\
struct Source:
    mut count: u16

struct Scanner:
    src: &Source
    mut at: u16

fn main() -> i16:
    let mut src = Source(count=7)
    let s = Scanner(src=src, at=0)
    let r = s.src
    src.count = 9
    print(r.count)
    return 0
";
    assert_eq!(refused_at(caller), "12: \"src\" is borrowed here, so it cannot be changed");
}

#[test]
fn a_view_parameter_is_kept_in_a_view_field() {
    // #425: a `&string` parameter, a slice or a returned view stored in a
    // `&string` field was "borrow of \"src\" has the wrong type"; `&[T]` was
    // "expected a type name". A view field is the 8-byte descriptor.
    let source = "\
struct Scanner:
    src: &string
    nums: &[i16]
    mut at: u16

fn scan(src: &string, nums: &[i16]) -> Scanner:
    return Scanner(src=src, nums=nums, at=0)

fn main() -> i16:
    let line = \"move north\"
    let v: vec[i16] = [4, 5, 6]
    let s = scan(line, v)
    print(s.src)
    print(s.src.len)
    print(s.nums[1])
    print(s.nums.len)
    let t = Scanner(src=&line[5:10], nums=&v[1:3], at=1)
    print(t.src)
    print(t.nums[0])
    print(size_of[Scanner]())
    return 0
";
    assert_eq!(output_without_leaks(source), "move north\n10\n5\n3\nnorth\n5\n18\n");
    // The field borrows what the view does: a local's is gone when the struct is returned (E0515)...
    let local = "\
struct Scanner:
    src: &string
    mut at: u16

fn make() -> Scanner:
    let line = \"move north\"
    return Scanner(src=line, at=0)

fn main() -> i16:
    let s = make()
    print(s.src)
    return 0
";
    assert_eq!(
        refused_at(local),
        "7: a returned borrow of \"line\" would dangle; only a borrowed parameter's can be returned"
    );
    // ...and an owner changed while a slice of it is held is refused (E0506).
    let changed = "\
struct Scanner:
    src: &string
    mut at: u16

fn main() -> i16:
    let mut line = \"move north\"
    let s = Scanner(src=&line[0:4], at=0)
    line = \"other\"
    print(s.src)
    return 0
";
    assert_eq!(refused_at(changed), "8: \"line\" is borrowed here, so it cannot be changed");
}

#[test]
fn a_method_is_called_on_a_borrowed_struct_field() {
    // #426: `self.src.run(...)` was "a *far pointer is not a sequence".
    let source = "\
struct Source:
    n: u16

struct Scanner:
    src: &Source
    mut at: u16

fn Source.run(self: &Source, at: u16) -> u16:
    return at + self.n

fn Scanner.word(self: &mut Scanner) -> u16:
    let word = self.src.run(self.at)
    self.at += 1
    return word

fn main() -> i16:
    let src = Source(n=3)
    let mut s = Scanner(src=src, at=0)
    print(s.word())
    return 0
";
    assert_eq!(output_without_leaks(source), "3\n");
    // A `&` field is read-only: a `&mut self` method is refused.
    let writes = "\
struct Source:
    mut n: u16

struct Scanner:
    src: &Source
    mut at: u16

fn Source.bump(self: &mut Source) -> void:
    self.n += 1

fn Scanner.word(self: &mut Scanner) -> void:
    self.src.bump()

fn main() -> i16:
    let src = Source(n=3)
    let mut s = Scanner(src=src, at=0)
    s.word()
    return 0
";
    assert_eq!(refused_at(writes), "12: field \"src\" of Scanner is not declared 'mut'");
}
