# Nib's borrow checker against Rust's

TL;DR: Nib's checker is lexical and name-based: a borrow is a binding in scope, rooted at an owner *name*. It is sound for the common shapes but has 5 proven holes (#120–#124) and rejects sound code Rust's NLL accepts (no flow-sensitive liveness, no field disjointness). Probes ran with `target/release/llrm-nib` (built 2026-10-01) and `llrm-run`; Rust 1.98.1 checked the Rust equivalents.

Rust: `compiler/rustc_borrowck/src` of rust-lang/rust (shallow clone). Nib: `crates/frontends/llrm-nib/src/semantic`. Spec: [language-spec.md](language-spec.md) §8.

## 1. What each checker enforces

| Rule | Rust | Nib |
|---|---|---|
| Shared XOR mutable | `lib.rs: access_place`, `check_access_for_conflict`; E0499/E0502; `tests/ui/borrowck/borrowck-closures-mut-and-imm` | `borrows.rs: is_borrowed` (any binding in scope whose roots hold the owner), `calls.rs` "aliases a mutable argument" (per call) |
| Referent outlives reference | region inference, `nll.rs: compute_regions`, `region_infer/mod.rs`; E0597 | `borrows.rs: check_outlives`, `lifetime_depth` (scope depth of a name) |
| Moves, use after move | `lib.rs: check_if_path_or_subpath_is_moved`; E0382/E0505 | `moves.rs` (moved set per HIR block, flows along edges), `borrows.rs: check_movable` |
| Partial moves, disjoint fields | `places_conflict.rs: place_components_conflict`; `tests/ui/borrowck/borrowck-field-sensitivity.rs` | none: a field never moves (`ownership.rs: consume`); a borrow of any part roots at the whole owner name |
| Reborrows | `prefixes.rs`, supporting prefixes | `borrows.rs: roots` follows `Member`/`Index`/`Slice` to the base name |
| Two-phase borrows | `borrow_set.rs: TwoPhaseActivation`; `tests/ui/borrowck/two-phase-*` | not needed: `v.push(v.len)` is accepted because args evaluate before the `&mut` (probe k) |
| Branches, loops (flow sensitivity) | liveness over MIR (`type_check/liveness`), `polonius/` | moves only (`moves.rs: flow_moves`). Borrows: lexical scope. |
| Returning references | outlives constraints, E0515; `tests/ui/borrowck/borrowck-borrow-from-temporary` | `borrows.rs: check_returned_borrows`: roots must be *names of parameters*; "tied to every borrowed input" (`call_roots`) |
| Refs in structs/closures/generators | struct lifetime params; closure captures in MIR | structs: `keep_borrows`, `kept_views`; lambdas inlined at the call (`lambdas.rs`), cannot be kept (`instances.rs`); generators keep only caller-lent borrows (`escaping.rs`) |
| Iterator invalidation | falls out of `&v` outliving the loop | `iterated` list in `is_borrowed`; `for x in &v: v.push(x)` refused |
| Slices, indexing | place conflicts on `Index` projections | `&v[a:b]` roots at `v`; `f(v[0], v[1])` refused as one owner |
| Drop order, destructor borrows | `check_for_invalidation_at_exit`, drop-liveness, `#[may_dangle]`; `tests/ui/nll/drop-no-may-dangle.rs` | `drops.rs`, `ownership.rs: drop_scopes`; no destructor-borrow rule (a `drop` sees only `&mut self`) |
| Module variables | `static mut` access needs `unsafe` | ordinary names in scope 0 (`lifetime_depth` = -2); no access rule |
| Raw pointers | unchecked | unchecked, `unsafe` only (§8) |
| Mutability of the path | E0594/E0596, `check_access_permissions` | `let` vs `let mut` for owners; `check_mutable_fields`; **not** through reference fields (#122) |

## 2. Shortfalls: accepted, unsound

Probes are in `~/scratch/nibborrow-probe/p` (copied below). Each was compiled with `llrm-nib` (accepted) and run with `llrm-run`.

### S1 Shadowed parameter name — #120

```nib
fn pick(x: &i16, c: bool) -> &i16:
    if c:
        let x: i16 = 5
        return x
    return x
fn main() -> i16:
    let one: i16 = 1
    let r = pick(one, true)
    let a: i16[4] = [100, 200, 300, 400]
    print(r)
    return 0
```

`llrm-run`: `llrm-run: access through a pointer into a returned call's frame`. Rust: E0515 (`shadow.rs`, "cannot return reference to local variable `x`").

### S2 Borrow of an owned parameter returned — #121

```nib
fn first(v: vec[i16]) -> &i16:
    return v[0]
fn main() -> i16:
    let r = first([1, 2, 3])
    print(r)
    return 0
```

Accepted. Optimised MIR of `first`: `call N$BDRP(v2)` (drop the buffer) then `return v6` (pointer into it). `llrm-run` prints `1` only because its free neither clears nor reuses. Rust: E0515 "cannot return value referencing function parameter `v`" (`tests/ui/borrowck/cannot-return-ref-to-fn-param-in-filter-map`).

### S3 Write through a `&T` field — #122

```nib
struct H:
    mut r: &i16
fn main() -> i16:
    let x: i16 = 1
    let mut h = H(r=x)
    h.r = 7
    print(x)
    return 0
```

Accepted; prints `7`: an immutable `let` is changed through a shared reference. A `&i16` parameter is refused ("binding is immutable"); the field path is not checked. Rust: E0594 (`*h.r = 7`).

### S4 Module variable lent and written by the callee — #124

```nib
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
```

Accepted; prints `99` from a `&P` that is supposed to be read-only. The `&mut` form (`p.n = 5; poke(); return p.n`) also prints `99`, where a `noalias` `p` would let the load fold to `5`. Rust has no safe equivalent: `static mut` needs `unsafe`. This is why `&mut` cannot state `noalias` (#113, PR #119). Module variables hold no heap value, so the damage is aliasing, not a free.

### S5 Borrow of a local pushed into a caller's `vec[&T]` — #123

```nib
fn stash(out: &mut vec[&i16]) -> void:
    let local: i16 = 7
    out.push(local)
fn main() -> i16:
    let mut keep: vec[&i16] = []
    stash(keep)
    print(keep.len)
    return 0
```

Accepted. Source MIR of `stash` stores `address cell(frame1…)` into the caller's vec. Reading an element back (`keep[0]`) does not compile ("expected i16, found *far pointer"; `print(keep[0])` panics the compiler at `semantic/mod.rs:1482`), so no wrong value was shown; the dangling store is in the MIR. The same push inside an `if` block is accepted. Direct stores (`out = .some(x)`) are refused (`check_assigned_borrows`); method stores are not checked. Rust: E0597/E0521.

### Accepted, differs from Rust, no memory harm (no issue)

Lambda captures are late-bound: `let f = |x| x + a; a = 50; f(1)` gives `51` (a lambda is compiled at its call, `lambdas.rs`). Rust: E0506 (`lambda.rs`). Sound only because lambdas cannot be stored or passed capturing (`instances.rs:265`).

## 2b. Over-strict: sound, Nib rejects, Rust accepts

| Program | Nib | Rust |
|---|---|---|
| `let r = &v[0]; print(r); v.push(3)` (probe l_nll) | `"v" is borrowed here, so it cannot be changed` | accepted (NLL) |
| `let m = &mut c; m.inc(); c.inc()` (d9) | `"c" is borrowed here` | accepted |
| `let a = &mut p.x; let b = &mut p.y` (j_fields) | `"p" is borrowed here` | accepted |
| `f(s.a, s.b)` with `&mut` params (n2) | `borrow of "s" aliases a mutable argument` | accepted |
| `for x in &self.items: self.count += 1` (e9) | `"self" is borrowed here` | accepted |

Cause: borrow = binding in scope, rooted at the whole owner name.

## 3. Why, and the general fix

Ranked by likelihood × damage.

1. **Roots are names, checked against names (S1, S2).** `is_parameter` and `lifetime_depth` look names up again later, and treat any parameter as returnable. Fix: roots become binding identities (`Storage` slot ids, not strings) carrying a kind {borrowed parameter, owned parameter, local, module}; `check_returned_borrows` accepts only borrowed-parameter roots. One owner for "what a borrow roots in".
2. **Stores are checked per syntax form (S5).** `check_assigned_borrows` runs on assignment only. Fix: every store of a reference-holding value into a place (assignment, method argument whose parameter type holds a reference, `push`) goes through one `store_borrow(target_root, value_roots)`; the callee-side rule is that a `&mut` parameter's contents may only receive roots of other parameters.
3. **No mutability path through references (S3).** Fix: a place's writability is derived from the chain of types it goes through (`&T` anywhere on the path makes it read-only); one function `place_writable(place)`, used by assignment, `&mut` borrow and calls.
4. **Module variables are unmodelled (S4).** Fix: give each function a mod/ref summary of module variables (transitive over calls); lending a module variable as `&`/`&mut` is refused when the callee's summary writes (or, for `&mut`, reads) it, or the argument is copied. Reuses the alias check in `calls.rs`.
5. **Lexical scope instead of liveness (2b).** Fix: a borrow is live from creation to the last use of the binding along each HIR path (backward liveness over the HIR blocks that `moves.rs` already walks), not to scope end. Same mechanism as moves: a per-block set flowing along edges.
6. **Whole-owner roots (2b).** Fix: roots are place paths (`p.x`), with a conflict test of prefix/disjoint (as `places_conflict.rs`), not names. Array elements stay one place (indices are dynamic).

## 4. Facts the checker could state

`noalias` on a `&mut` parameter (and `readonly` on `&T`, `nocapture`) is sound when the checker guarantees, per call:

- no other path reaches the referent during the call: `&mut` arguments pairwise disjoint (done), no `&` argument or borrowed struct field over the same owner (done), **and the callee's transitive module-variable summary excludes it (S4, fix 4)**;
- `&T` is never written through (S3, fix 3) — `readonly`;
- no borrow of the parameter is stored anywhere the caller sees except the return value (S5, fix 2) — `nocapture` except via return.

One owner per fact: the checker states each in HIR through `facts.state(...)` (#113) after proving it; frontends do not guess.

## Issues

#120 shadowed parameter · #121 owned parameter returned · #122 write through `&T` field · #123 vec of refs push · #124 module variable lent.
