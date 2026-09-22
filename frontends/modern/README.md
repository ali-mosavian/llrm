# Modern language frontend

`modern` is a provisional frontend identifier, not the language's final name.

This crate implements the first source-language slice:

- indentation-delimited functions and blocks;
- the complete scalar set: `bool`, `char`, signed and unsigned 8/16/32-bit
  integers, `f32`, `f64`, `addr`, and `void`;
- named signed fixed-point types backed by `i16` or `i32` storage;
- typed parameters and return values;
- `let` and `var` bindings;
- assignment and every compound assignment, calls, `return`, `if`/`else`,
  `while`, and typed half-open integer ranges;
- `break` and `continue`;
- the language's operators at its precedence: strictly typed arithmetic,
  bitwise operators and shifts, non-chaining comparisons, and short-circuit
  `and`/`or`;
- conversions written `T(x)`;
- fixed one-dimensional arrays, written `[T; length]`, with prefix descriptors,
  literals, rank-one repeat literals `[value; length]`, indexed loads and
  stores, and intrinsic metadata methods;
- source-ordered, nested `struct` layouts, local struct values, struct literals
  and copies, plus allocation-free array iteration through explicit references;
- scoped `&T` and `&mut T` parameters, including unsized array views written
  `&[T]` and `&mut [T]`;
- fixed-range slices, bounded list and dictionary comprehensions, and fused
  non-escaping generators; and
- byte strings, allocation-free f-strings, and `print`.

`char` is one target-code-page byte; `\xNN` spells any code unit without
making the compiler or runtime Unicode-aware. Decimal floating literals are
stored once in read-only module data. Integers and floats convert implicitly
as in C: operands narrower than `int` (`i16` here, set per target in
`src/conversions.rs`) are promoted, mixed operands meet at their common type,
and destinations convert what is assigned to them. Where C would convert
a signed operand to an unsigned type of its width, it is an error instead. Native code for float arithmetic
and conversions waits on strict `f32`/`f64` evaluation; the HIR executor runs
them.

Fixed-point types make their representation explicit:

```text
type fixed8 = fixed i16, fraction=8
type fixed16 = fixed i32, fraction=16
```

The fractional count is part of the named type and must be between one and
one less than the storage width. Distinct declarations are distinct types,
even when their storage and fraction match. Integer literals are scaled
exactly at compile time. Decimal literals are rounded once to the nearest
representable quantum, with ties away from zero. There are no implicit
conversions between fixed types, integers, or floats. An explicit one names
the type, `fixed16(count)`: integers scale in exactly, and fixed values leave
or rescale truncating toward zero. Floats do not yet convert to or from fixed
point.

Addition, subtraction, negation, and comparison use the signed stored
representation directly. Multiplication and division use a double-width
mathematical intermediate and then rescale, so storage-width overflow cannot
destroy the intermediate product. For `i16` storage that intermediate is an
ordinary `i32`; an `i32`-based fixed operation stays intact until target
lowering uses the 386's native EDX:EAX product or dividend pair. It never
becomes a first-class `i64` operation or a generic 64-bit helper.
Multiplication discards low fractional bits with an arithmetic shift;
division shifts the widened numerator before signed division. Narrowing back
to storage follows the language's current wrapping integer policy. No
descriptor, heap object, or fixed-point arithmetic runtime routine is
generated. Printing uses the existing output boundary with the raw value and
a compile-time fractional-bit argument. Both direct `print(value)` and fixed
values inside f-strings render
canonical signed base-10 `integer.fraction`: at least one digit appears on
each side of the decimal point, and redundant trailing fractional zeroes are
removed. Thus Q8 raw values `384`, `-64`, and `512` print as `1.5`, `-0.25`,
and `2.0`, respectively; the scaled storage integer is never printed as the
source value.

Strings deliberately match the real-mode systems boundary. A string value is
a 16-bit near pointer to NUL-terminated payload bytes. Two little-endian
16-bit words immediately precede the payload: `length`, then `capacity`.
String literals are read-only module objects. In this slice an f-string is
valid only as a direct `print` argument; it streams literal and interpolated
pieces to short typed runtime calls (`_pt`, `_pi2`, `_pf4`, and so on), so
formatting introduces no allocation or hidden general-purpose runtime.

Every array object has the same prefix representation as a string: two
little-endian 16-bit words, `length` and `capacity`, immediately before the
payload. A fixed `[T; N]` array has `length == capacity == N`; its value and its
systems ABI address both point at element zero, not at the descriptor. Thus C
and assembly receive a conventional direct `T *`, while code that owns an
array can recover its metadata at pointer offsets `-4` and `-2`. The descriptor
is part of the ABI and is initialized even when the current source never asks
for it.

An array parameter is an unsized borrowed view, written `values: &[T]` or
`values: &mut [T]`. A call passes exactly one far pointer; it never passes a
separate hidden length. The pointer names an eight-byte scoped view containing
`length`, `capacity`, and a 16:16 payload pointer. This indirection is necessary
for `&values[start:end]`: an interior payload cannot claim the owner's prefix
as its own descriptor. The view is stack-scoped and has no allocator or
destructor. Owned arrays retain the direct prefix-plus-payload representation
above. `array.data()` is the explicit systems escape hatch: it returns an
opaque far `addr` pointing directly at element zero. A string's `data()` keeps
its native near `string` pointer.

Fixed arrays have a zero lower bound. `array.len()`, `array.capacity()`,
`array.data()`, and `array.dim(0)` are intrinsic operations. For a fixed array the compiler knows
all three values and folds them without emitting a helper or descriptor load;
the physical descriptor remains available to interop. On a borrowed `[T]`
view metadata loads through the scoped descriptor and indexing loads its data
pointer. Strings provide `len()`, `capacity()`, and byte-value iteration over
`char`. Only rank one is implemented, so any other dimension is currently
rejected.

Struct fields stay in source order, with at most two-byte alignment for the
16-bit target. Indexing and field selection are structural HIR and lower to
ordinary address arithmetic; no array, field-access, metadata, or iterator
helper is emitted.
Structs may be bound with an explicit type or inferred from a literal or copy:

```text
var acc = vec2i { x: 0, y: 0 }
let delta = vec2i { x: other.x - current.x, y: other.y - current.y }
acc.x += delta.x
```

When an aggregate's type is already known, the literal omits its nominal type.
That context flows through arrays and nested fields. Fields may be named, or
given positionally in declaration order:

```text
var named: [body; 1] = [
    { pos: { x: -15, y: -12 }, vel: { x: 0, y: 0 } },
]
var compact: [body; 1] = [
    {{-15, -12}, {0, 0}},
]
```

A literal cannot mix named and positional fields, and it must initialize every
field. An anonymous literal without an expected struct type is an error; write
the type once, as in `vec2i { x: 0, y: 0 }`, and inference handles the binding.

Whole-struct assignment is a structural field copy. All source leaves are
evaluated and loaded before any destination leaf is stored, so overlapping
copies and literals which read their destination have value semantics. This
does not introduce a general aggregate runtime operation. Compound assignment
resolves its destination once, applies the corresponding built-in operation,
and stores the result.
There is no operator overloading.
`for item in array` copies a scalar element. `for item in &array` gives `item`
an immutable scoped view of the element;
`for item in &mut array` requests a mutable view and is rejected for an
immutable array. By-value struct iteration remains reserved until aggregate
move semantics are implemented. A mutable view's field stores update the
original element. `for item in &array[begin:end]` iterates only that checked
compile-time range.

A list comprehension over a fixed array materializes another fixed,
descriptor-backed stack array whose capacity is known from the source:

```text
let doubled = [value * 2 for value in values]
```

A dictionary comprehension uses the same bounded-storage rule. It maintains a
runtime unique-key length, updates the existing value for a duplicate key, and
offers `len()`, `capacity()`, and `get(key, default)`. Lookup is explicit about
the missing-key case, so it needs neither exceptions nor an option-object
runtime:

```text
let table = {item: item * 10 for item in values}
let answer = table.get(3, 0)
```

A parenthesized generator is valid only as the iterable of a `for` loop. The
frontend fuses its mapping expression into that loop; it cannot escape and no
iterator object, resume table, `next` call, or heap allocation is emitted:

```text
for value in (item + 1 for item in doubled):
    total += value
```

The current minimal grammar permits one `for` clause and no comprehension
filter. Materialized comprehensions require a fixed-array source so their
maximum storage is statically known; generators can stay lazy without that
storage.

The same borrow syntax is used at a function boundary:

```text
fn translate(points: &mut [vec2i], delta: &vec2i) -> void:
    for point in &mut points:
        point.x += delta.x
        point.y += delta.y

translate(&mut bodies, &offset)
```

A borrowed parameter is one 32-bit real-mode far pointer—one 16-bit segment and
one 16-bit offset—so it can refer uniformly to stack, static, or far storage.
For `[T]` it addresses the scoped view described above; for `T` it addresses
the value directly. The borrow is explicit at the call, mutable access requires `&mut`,
and an immutable binding cannot be mutably borrowed. A call may not give the
same named object to two parameters when either access is mutable. References
are non-owning and confined to the call or loop scope: they cannot be stored,
returned, or outlive the referenced local. These rules require no reference
counting, garbage collector, lifetime table, stack unwinder, or runtime borrow
check.

`is` and `is not` compare the identity of scoped views, while `==` and `!=`
remain value comparisons (struct value equality is not in this slice).
`for index in start..end` evaluates both bounds once and visits `start` through
`end - 1`. Its immutable induction variable has the bounds' integer type, so
`for step_no in 0..step_count` runs exactly `step_count` iterations when
`step_count` is nonnegative.
Literal indices are checked by the frontend. The
`fixtures/nbody.mod` fixed-point integrator—using a named Q23.9 `scalar` and
an array of six `body` structs—is
the current end-to-end feature gate for structs, arrays, nested loops,
strings, f-strings, and printing.

In the implemented slice, primitive and struct expressions have value
semantics; `let` creates an immutable place and `var` a mutable place. Struct
copies are explicit in HIR as leaf loads and stores, while arrays remain
non-copyable aggregates. Borrows are explicit, non-owning views represented
by one far pointer. Escaping references, owning
moves, and explicit cloning remain future work.

Canonical language code uses lowercase `snake_case` for functions, variables,
parameters, fields, and user-defined types. This is the language and standard
library convention, not a lexical restriction; interop and generated code may
retain another system's spelling when needed.

The frontend performs name resolution and strict type checking, constructs an
explicit control-flow graph, and writes qbopt's versioned common-HIR JSON. It
does not emit bytecode, p-code, machine instructions, or target registers.

Run it with:

```text
cargo run --manifest-path frontends/modern/Cargo.toml -- program.mod
```

`--tokens` and `--syntax` expose the two earlier stages without performing
semantic analysis.

The common-HIR reference executor provides an executable semantic oracle:

```text
uv run python tools/modernrun.py frontends/modern/fixtures/nbody.mod --entry nbody --show-return 1
```

This runs source through lexing, parsing, strict semantic analysis, common-HIR
verification, and HIR execution. It is deliberately not called target
code-generation: it emits no OMF or executable and supplies no real-mode ABI.
Its captured output is the known answer that the freestanding real-mode backend
must reproduce exactly.

The frontend document is accepted by `qbopt.hir.decode`, `qbopt.hir.verify`,
and `qbopt.hir.lower`, then follows qbopt's shared optimization, lowering,
allocation, and OMF object-writing path. The minimal real-mode bootstrap and
freestanding runtime can link that object into a DOS executable.

Not implemented in this slice are panics, `checked_to[T]()`, imports,
general resizable collections, escaping or owning references, patterns,
lambdas, comprehension filters, or multiple comprehension clauses. General string construction is also
absent: f-strings are currently a print facility, not heap values. Those
features should extend semantic analysis and elaborate to the same small HIR
rather than adding surface-language HIR operations.
