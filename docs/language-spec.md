# Language specification — draft 0.1

This document specifies the small, statically typed systems language discussed
for 386 real-mode programs. The language has no final name.

The design rule is:

> Every feature must reduce to values, direct control flow, or explicit memory.

There is one mechanism for each job:

| Job | Mechanism |
|---|---|
| Data | `struct` and `enum` |
| Behavior | `fn`, methods, and lambdas |
| Abstraction | Generics and structural protocols |
| Lifetime | Ownership, borrowing, and scoped destruction |
| Failure | `Option`, `Result`, and `?` |
| Iteration | Iterators, `for`, and `yield` |
| Foreign code | Explicit representation, address space, and ABI |

The language has no garbage collector, exception unwinding, runtime reflection,
classes, inheritance, implicit dynamic dispatch, or allocation caused by an
implicit conversion. An expression whose result is an owned dynamic collection
visibly allocates; fixed arrays, views, and generators do not.

## 1. Source form

Source files use indentation for blocks. A newline terminates a statement;
semicolons do not exist. Parentheses, brackets, and braces permit a logical
line to continue across physical lines. `#` begins a line comment.

Identifiers are ASCII. Source files may be UTF-8, but runtime text uses the
selected target's single-byte code page. A character that cannot be represented
in that code page is a compile-time error.

```text
fn abs(value: i16) -> i16:
    if value < 0:
        return -value
    return value
```

Conditions have type `bool`. Integers, pointers, collections, and `Option`
values have no implicit truth value.

## 2. Bindings and assignment

`let` creates a binding; values are immutable by default. `const` creates a
compile-time value. Use `mut` to make a binding mutable.

```text
let name = "Ada"
let mut score: i16 = 0
const screen_width: u16 = 320
```

A `const` may be declared at module level or in a function. Its value must
fold at compile time, from literals, operators, and other constants, and it
may size an array: `let row: u8[screen_width] = [0] * screen_width`.

A binding's type is inferred from its initializer unless written explicitly.
Assignment uses `=` and requires a mutable binding.

```text
score = score + 1
```

There are no implicit numeric conversions.

## 3. Built-in types

The scalar types are:

```text
bool
char
i8  u8
i16 u16
i32 u32
f32 f64
void
```

`char` is one code unit in the target code page. It occupies one byte.

Integer literals acquire a type from context and must fit that type. Integer
arithmetic is two's-complement and wraps to the operand width. Checked and
saturating operations are library methods of each integer type:
`checked_add`, `checked_sub` and `checked_mul` return `Option[T]`, `none` on
overflow; `saturating_add`, `saturating_sub` and `saturating_mul` clamp to the
type's bounds. There are no alternate arithmetic operators. Division by
zero and the unrepresentable signed division `min / -1` invoke the panic
handler. Panic terminates; it never unwinds.

The compound built-in types are:

```text
(T, U)          # tuple
T[N]            # fixed array
T[X, Y]         # fixed ranked array
vec[T]          # owned dynamic sequence
&[T]            # borrowed slice
&T[N]           # borrowed fixed array
&T[X, Y]        # borrowed ranked array
dict[K, V]      # owned hash table
string          # owned text
&string         # borrowed string view
fn(T) -> U      # noncapturing function value
```

There is no `null`. Absence is represented by `Option[T]`.

### Protocol requirements for built-in types

`dict[K, V]` requires K to satisfy the `Hashable` protocol:

```text
protocol Hashable[T]:
    fn hash(self: &T) -> u16
    fn eq(self: &T, other: &T) -> bool
```

Scalar types and `string` implement `Hashable`. User-defined structs and enums implement `Hashable` if all their
fields do.

Scalar types and `string` implement the `Ordered` protocol:

```text
protocol Ordered[T]:
    fn cmp(self: &T, other: &T) -> i8  # -1, 0, or 1
```

A type is printed, and formatted by `{value}` in an f-string, through the
`Display` protocol; scalars and `string` print without it:

```text
protocol Display[T]:
    fn display(self: &T) -> string
```

Collections implement the `Iterable` protocol:

```text
protocol Iterable[T]:
    fn iter(self: &Self) -> Iterator[T]
```

Iterators implement the `Iterator` protocol:

```text
protocol Iterator[T]:
    fn next(self: &mut Self) -> Option[T]
```

`string`, arrays, and vectors implement `Iterable`. Their iterators are
ordinary generators, compiled to plain loops by section 12's rule.

### Conversions

A conversion names its target type as a function:

```text
let wide = i32(narrow)
let byte = u8(code)
let ratio = f32(count)
```

| From | To | Result |
|---|---|---|
| integer | integer | Wider: sign- or zero-extended by the source's signedness. Narrower: the low bits, as arithmetic wraps. |
| integer | float | The nearest representable value. |
| float | integer | Truncated toward zero; a value outside the target's range invokes the panic handler. |
| float | float | The nearest representable value. |
| `bool` | integer | `0` or `1`. |
| `char` | integer | The code unit, zero-extended. |
| integer | `char` | The low byte, as integer to `u8`. |
| enum | integer | Its tag. |
| integer | fixed-point | Converted to the storage type, then scaled; the scaled value wraps as arithmetic does. |
| fixed-point | integer | Truncated toward zero, then as integer to integer. |
| fixed-point | fixed-point | Rescaled; lost fraction bits truncate toward zero. |

There is no conversion to `bool`; compare instead (`count != 0`). A
conversion that must not lose information is the library method
`checked_to[T]()` of each number type, returning `Option[T]` for an integer
`T`: `none` when the value is out of `T`'s range, or a NaN. A float in range
truncates toward zero.

Between integer and float types, conversions are also implicit, as in C.
`bool`, `char`, and fixed-point types convert only explicitly.

- **Assignment.** A value bound, assigned, passed, or returned converts to
  the destination's type by the table above, narrowing included.
- **Promotion.** An operand narrower than the target's `int` converts to
  `int`. On this target `int` is `i16`, so `u16` is not promoted; a 32-bit
  target would make `int` `i32`.
- **Usual arithmetic conversions.** The operands of an arithmetic, bitwise, or
  comparison operator convert to a common type. With a float operand, it is
  the wider float. Otherwise, after promotion, it is the wider integer type.
  Where that leaves a signed and an unsigned operand of the same width, C
  picks the unsigned one; here it is a compile-time error, unless the signed
  operand was promoted from an unsigned type. So `i8 + u8` is `i16`,
  `u8 + u16` is `u16`, and `i8 < u16` is rejected.
- A range's bounds meet at their common type, as operands do.

An integer literal takes the type of the other operand when it fits, and its
own type otherwise: `int`, or `i32` if it does not fit `int`.

### Operators

From tightest to loosest binding:

| Operators | Meaning |
|---|---|
| `f(x)` `a[i]` `a.b` `x?` | call, index or slice, field or method, propagate failure |
| `-x` `~x` `&x` `&mut x` | negate, bitwise not, borrow |
| `*` `/` `%` | multiply, divide, remainder |
| `+` `-` | add, subtract |
| `<<` `>>` | shift |
| `&` | bitwise and |
| `^` | bitwise exclusive or |
| `\|` | bitwise or |
| `==` `!=` `<` `<=` `>` `>=` `is` `is not` | compare |
| `cond ? a : b` | ternary conditional |
| `!` | logical not |
| `&&` | logical and |
| `\|\|` | logical or |

Binary operators group left to right. Comparisons chain: `a < b < c` evaluates
as `(a < b) && (b < c)`. Bitwise operators bind tighter than comparisons,
so `flags & mask == 0` means `(flags & mask) == 0`.

The operands of an arithmetic, bitwise, or comparison operator convert to
their common type; the result has that type, and a comparison's is `bool`. A
shift's operands are promoted separately; the result has the left one's type,
and the count may be any integer type. `&&`, `||`, and `!` take and give `bool`;
`&&` and `||` evaluate their right operand only when it decides the result.
`a is b` is true when `a` and `b` name the same struct: one variable, or one
element of one array. A scalar has no identity; compare it with `==`.

Integer `/` is an error; use `//` for integer division (floor division on
signed operands). `%` takes the sign of the dividend. `>>` is arithmetic on
a signed operand and logical on an unsigned one. A shift count that is
negative, or not less than the operand's width, is a compile-time error when
constant and invokes the panic handler otherwise.

Every binary arithmetic and bitwise operator has a compound assignment:
`+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, and `>>=`. There is
no `++`, `--`, or `**`.

## 4. Functions, methods, and lambdas

A named function has one declaration form and an explicit return type:

```text
fn add(a: i16, b: i16) -> i16:
    return a + b
```

Parameters may have default values and be called by name:

```text
fn draw(color: u8, x: i16, y: i16, thickness: u8 = 1) -> void:
    # ...

draw(7, 100, 200)           # positional: uses default thickness=1
draw(color=7, x=100, y=200) # named, any order
draw(x=100, y=200, color=7) # order doesn't matter with named args
```

Named arguments improve readability when many parameters exist and are especially
valuable for FFI calls.

A method is a function declared in a type's namespace. Its first parameter is
`self`, and may be immutable `&self`, mutable `&mut self`, or owned `self`.
Methods follow the same parameter rules as functions: defaults and named arguments.

```text
struct Point:
    mut x: i16
    mut y: i16

fn Point.move(self: &mut Point, dx: i16, dy: i16 = 0) -> void:
    self.x = self.x + dx
    self.y = self.y + dy

let mut point = Point(x=0, y=0)
point.move(dx=2, dy=3)
point.move(2)  # dy defaults to 0
```

Methods are called with dot syntax. They are not also callable as free
functions. The language has no UFCS and no second method-call spelling.
Only the module that defines a type may define its methods. Another module uses
a free function or defines a wrapper type; it cannot attach competing behavior
to someone else's type.

A lambda has one expression as its body:

```text
let double = |x: i16| x * 2
```

Parameter and result types may be inferred from context. A multi-statement
anonymous operation is written as a local named function rather than a second
lambda syntax:

```text
fn main() -> i16:
    fn fact(n: i16) -> i16:
        if n <= 1:
            return 1
        return n * fact(n - 1)
    print(fact(5))
    return 0
```

A local function is seen from its declaration to the end of its block, and in
its own body. It sees the constants and functions around it but not the
bindings; it is otherwise a function like any other.

Lambdas may borrow values from their enclosing scope and may not outlive those
values. A function's name, or a noncapturing lambda, converts to a function
value of type `fn(A) -> R`:

```text
fn apply(f: fn(i16) -> i16, x: i16) -> i16:
    return f(x)

let inc: fn(i16) -> i16 = |x| x + 1
apply(inc, 4)      # 5
apply(fact, 4)     # 24
```

A function value is a small integer naming one of the functions converted to
its type. The program is compiled whole, so a call through it is a `match` over
those functions and a direct call of each. The language has no
implicit heap allocation for closures and no special currying; partial
application is written with a lambda.

Functions are not overloaded. Variadic native functions do not exist. APIs with
many optional parameters use defaults and named arguments.

## 5. User-defined data

A `struct` is a product of named fields in declaration order. Fields are
immutable unless declared with `mut`.

```text
struct Point:
    x: i16
    y: i16

let origin = Point(x=0, y=0)
let mut p = Point(0, 0)
```

An `enum` is a tagged union. A variant may carry values.

```text
enum Shape:
    point(Point)
    circle(center: Point, radius: u16)
    rectangle(min: Point, max: Point)
```

Variants are constructed by name:

```text
let s: Shape = Shape.circle(center=Point(0, 0), radius=5)
```

There are no classes, base types, constructors, properties, or implementation
inheritance.

Structs and enums may be generic:

```text
struct Pair[A, B]:
    first: A
    second: B
```

### Bit-packed structs

A `bits` struct is stored in one backing integer (`u8`, `u16`, or `u32`).
Its fields take bits from the lowest upward, in declaration order, with no
padding. A field is `bool` (1 bit), `uN` or `iN` (N bits, 1 to 32), an enum
with a declared width, or another `bits` struct, as wide as its backing
integer. The field widths must add up to at most the backing width; unused
high bits read as zero.

```text
bits struct Attr: u8          # the VGA text attribute byte
    fg: u4                    # bits 0-3
    mut bg: u3                # bits 4-6
    blink: bool               # bit 7

enum Mode: u2
    text
    cga
    ega
    vga
```

Reading a field gives the smallest standard type that holds it: `u4` reads as
`u8`, `i12` as `i16`. Writing a value that does not fit is a compile-time
error when constant and wraps to the field width otherwise. `uN` and `iN` of
nonstandard width exist only as field types.

A `bits` struct converts to and from its backing integer, which is how it is
read from and written to memory, ports, or registers:

```text
let a = Attr(fg=15, bg=1, blink=false)
let raw = u8(a)               # 0x1F
let back = Attr(raw)
```

A field has no address, so it cannot be borrowed. The bit order is part of
the language, not of the implementation, so the layout is the same with every
compiler and matches the hardware formats it is meant for.

Reads and writes compile to shifts and masks on the backing integer, and a
field's width is a range fact for the optimizer:

| Source | Code |
|---|---|
| `x = a.bg` | `mov al, [a]` / `shr al, 4` / `and al, 7` |
| `if a.blink:` | `test byte ptr [a], 80h` / `jz ...` |
| `a.bg = 2` | `and byte ptr [a], 8Fh` / `or byte ptr [a], 20h` |
| `a.bg = x` | `and x, 7` / `shl x, 4` / `and byte ptr [a], 8Fh` / `or [a], x` |
| a signed `i4` field | `shl` to the top, then `sar` down |

## 6. Patterns

One pattern language is used by `let`, `match`, `for`, and comprehensions.
Patterns include:

```text
name                     # binding
_                        # ignored value
42                       # literal
(a, b)                   # tuple
Point(x, y)              # struct
.circle(center, radius)  # enum variant
[head, *tail]            # sequence
```

`match` is exhaustive for enums and booleans.

```text
match shape:
    .point(Point(x, y)):
        plot(x, y)
    .circle(center, radius):
        draw_circle(center, radius)
    .rectangle(min, max):
        draw_rectangle(min, max)
```

A refutable binding uses `else`:

```text
let [head, *tail] = values else:
    return .err(.empty)
```

Sequence patterns accept at most one starred binding:

```text
[]
[a]
[a, b]
[head, *tail]
[*head, tail]
[first, *middle, last]
```

Sequence patterns do not remove elements. Single-element bindings are borrowed
elements and a starred binding is a borrowed slice. Reading a borrowed scalar
automatically copies it. Creating an owned remainder is explicit:

```text
let owned_tail = [item for item in tail]
```

Sequence patterns require a sized, sliceable sequence. They do not consume an
arbitrary iterator or generator.

## 7. Control flow

The control forms are:

```text
if / else
match
while
for
break
continue
return
with
? (ternary)
```

There are no `switch`, `do`, `goto`, exceptions, or labeled loop variants.
`else if` is an `else` whose block is one `if`.
Conditions may chain: `a < b < c` is valid and evaluates as `(a < b) && (b < c)`.

`with` creates a nested ownership scope:

```text
with file = File.open("LEVEL.DAT")?:
    load_level(&file)?
```

The value is destroyed at the end of the block; `with mut` binds it mutably,
as `let mut` does. `with` has no enter/exit
protocol and cannot suppress failure. There is no separate `defer` mechanism.

## 8. Ownership and borrowing

Every value has one owner. Passing, returning, or assigning an owned value
moves it. The source cannot be used after the move. Compound types move by
default; primitives copy, and so does a struct or enum whose fields all copy.

```text
T           # owned (move)
&T          # shared borrow (read-only)
&mut T      # exclusive borrow (mutable)
```

A parameter's type says how it is passed: `fn f(x: Point)` takes ownership,
`fn f(x: &Point)` borrows, and `fn f(x: &mut Point)` borrows exclusively.
Call sites write the argument alone, without `&`. A borrow cannot outlive its
owner. While an exclusive borrow exists, no other borrow may access the same
value. While shared borrows exist, the value may not be mutated or moved.

A returned borrow is conservatively tied to every borrowed input from which it
could have been derived. Version 0.1 has no named lifetime syntax.

Primitives (integers, booleans, floats) always copy. A function taking an `i16`
receives a copy, not a reference.

A resource type defines the reserved `drop` method:

```text
fn File.drop(self: &mut File) -> void:
    unsafe:
        dos_close(self.handle)
```

The compiler calls `drop` once, in reverse construction order, at normal scope
exit and along `return`, `break`, `continue`, and `?` paths. Cleanup is direct
control flow; there is no unwinder or runtime cleanup table. `drop` cannot be
called directly. Early destruction is expressed with a smaller `with` scope.

### Who allocates and who frees

Whoever owns a value when its scope ends drops it. Dropping a `string`,
`vec`, or fixed array drops its elements, then frees the buffer if its
`heap` flag is set (section 13). A function's result is owned by its caller.
Section 9 gives every case down to machine code.

Raw memory is available only through `unsafe`:

```text
*near T
*near mut T
*far T
*far mut T
*huge T
*huge mut T
```

Raw pointers carry no lifetime, validity, or aliasing guarantee.

## 9. Calls, results, and ownership at the boundary

This section fixes what crosses a call: how arguments and results are passed,
who allocates, who frees, and the machine code of each case. The compiler
emits all of it except 9.4's carry flag: until then an `Option` or `Result`
returns as any other enum does.

### 9.1 Calling convention

- Every call is `call far`. Arguments are pushed right to left, each at least
  one word, and the caller removes them. With a frame, the first argument is
  at `[bp+6]`.
- An `f32` or `f64` is passed in its stored form, 4 or 8 bytes. The x87
  evaluates in extended precision and rounds only when it stores.
- An owned `string` or `vec` is passed as its near pointer, one word. The
  caller emits no drop for it afterwards; the callee now owns it.
- An owned fixed-size value (struct, enum, `T[N]`, and the proposed `string[N]`) is passed as
  its bytes, pushed last field first.
- `&T` and `&T[N]` are passed as a far pointer to the value. `&[T]` and
  `&string` are passed as a far pointer to an 8-byte view descriptor
  (`length`, `capacity`, data offset, data segment) on the caller's stack.
- A result that goes to a slot (9.2) adds a hidden far pointer, pushed last,
  so it is at `[bp+6]` and the first source argument moves to `[bp+10]`.

### 9.2 Where a result goes

| Result | Returned in |
|---|---|
| `void` | nothing |
| `bool`, `char`, 8-bit integer | `al` (`bool`: `0` or `0FFh`) |
| 16-bit integer, owned `string`, owned `vec` | `ax` (a `string` or `vec` is its near pointer) |
| 32-bit integer, `&T`, `&T[N]` | `dx:ax` (a borrow is `segment:offset`) |
| `f32`, `f64` | `st(0)` |
| struct, enum, `T[N]`, proposed `string[N]`, of 4 bytes or less and holding no pointer | `al`, `ax` or `dx:ax`, as the integer its bytes spell |
| the same, larger or holding a pointer | the slot |
| `&[T]`, `&string` | the slot, as an 8-byte descriptor |
| tuple `(A, B, ...)` | as a struct of its elements; 4 bytes or less in registers, first element lowest (`ax`, then `dx`) |
| `Option[T]`, `Result[T, E]` | 9.4 |

The slot is uninitialized storage in the caller's frame, reached through the
hidden far pointer; the callee only writes it. The caller passes a fresh
variable or a temporary there, never storage that holds a live value, so a
failing or partial write can never damage a value that is still visible.

### 9.3 Every case

"Slot" is the destination of 9.2. "Drop" is the scope-exit code of 9.5.

| # | Declared result | Data comes from | Callee does | Caller does | Allocates | Frees |
|---|---|---|---|---|---|---|
| 1 | scalar | anything | leaves it in `al`/`ax`/`dx:ax` | reads the register | nothing | nothing |
| 2 | fixed size, 4 bytes or less, no pointer | anything | loads it into `al`/`ax`/`dx:ax` | stores the registers | nothing | nothing |
| 3 | fixed size, larger or holding a pointer | built in the callee | builds it directly in the slot | passes the slot | nothing | caller's drop, for owned fields |
| 4 | fixed size, larger or holding a pointer | a callee local built before it is known to be the result | copies it to the slot | passes the slot | nothing | caller's drop, for owned fields |
| 5 | fixed size, larger or holding a pointer | an owned parameter | copies it from its argument words to the slot | passes the slot | nothing | caller's drop, for owned fields |
| 6 | fixed size, larger or holding a pointer | a borrowed parameter, all fields copyable | copies it through the far pointer to the slot | passes the slot | nothing | nothing |
| 7 | fixed size | a borrowed parameter with owned fields | compile error: copy it explicitly | | | |
| 8 | `string`, `vec` | a literal | returns the literal's address | stores `ax` | nothing | nothing; drop sees no `heap` bit |
| 9 | `string`, `vec` | built in the callee (`+`, f-string, `.copy()`) | allocates, fills, returns the pointer | stores `ax` | callee, program heap | caller's drop |
| 10 | `string`, `vec` | an owned parameter or local | returns the pointer and cancels its own drop | stores `ax` | nothing | caller's drop |
| 11 | `string`, `vec` | foreign memory with our descriptor | returns the pointer | stores `ax` | C or DOS | nothing here |
| 12 | `string`, `vec` | a bare C pointer | compile error: take a view or copy | | | |
| 13 | `string`, `vec` | a borrowed parameter | compile error: copy it explicitly | | | |
| 14 | `string`, `vec` | a buffer in the callee's frame | compile error: return `string[N]` or copy | | | |
| 15 | `&T`, `&T[N]` | a borrowed parameter or static data | returns the far pointer | stores `dx:ax` | nothing | nothing |
| 16 | `&[T]`, `&string` | a borrowed parameter or static data | writes the descriptor to the slot | passes the slot | nothing | nothing |
| 17 | any view | a callee local, an owned parameter, or a buffer the callee frees | compile error: dangling | | | |
| 18 | `Option`, `Result`, both sides 4 bytes or less | as rows 1-17 per side | payload in registers, carry flag set on `err`/`none` | branches on carry | per side | per side |
| 19 | `Option`, `Result`, one side larger | as rows 1-17 per side | the large side in the slot, the small in registers, carry flag | passes the slot, branches | per side | per side |
| 20 | `Option`, `Result`, both sides larger | as rows 1-17 per side | both share one slot, carry flag | passes the slot, branches | per side | per side |
| 21 | `Result` propagated by `?` | the callee's failure | as 18-20 | drops live locals, sets carry, returns | nothing | the drops |

### 9.4 Option and Result

A returned `Option[T]` or `Result[T, E]` carries its tag in the carry flag:
clear is `some`/`ok`, set is `none`/`err`. This is the convention of DOS's
own `int 21h`, so a DOS service can be declared as returning `Result`. Each
side is returned by 9.2; when both need the slot, they share one of the
larger size.

The callee executes `clc` or `stc` last, before an epilogue of only
`pop bp`/`leave` and `retf`, none of which changes flags. The caller
branches on carry before it removes the arguments, since `add sp, n` would
change it; on the failure path `leave` removes them. `?` is therefore one
`jc`. A function that propagates can pass its own slot down as the callee's
slot when it is large enough, so a large error is written once, where it will
finally be returned.

Stored in memory, as a field or `vec` element, `Option` and `Result` are
ordinary enums with a tag byte, except that `Option` of a pointer uses `0`
for `none`.

`main` returns an integer, the exit code, or `Result[void, E]`, which exits
with 0 for `ok` and 1 for `err`.

### 9.5 Ownership at the boundary

- **Passing an owned value** moves it; the caller emits no drop for it. The
  callee drops it at exit unless it moves it on.
- **Passing a borrow** changes no ownership and emits no drop.
- **A result** is owned by the caller, whatever its storage.
- **Drop** tests the `heap` bit and calls the runtime's `free` only if it is
  set, after dropping the elements of a `vec` or array whose element type
  needs it.
- **A conditional move**, where a value moves on some paths only, keeps a
  one-byte drop flag in the frame; drop tests it first. An unconditional move
  needs no flag: the compiler omits the drop.
- **Assignment** `x = foo()` evaluates the right side first, then drops `x`'s
  old value, then stores; `foo` may still borrow the old `x`.
- **A temporary**, as in `print(foo())`, is dropped at the end of its
  statement.
- **`?`** drops the live locals, then returns the failure.
- **A write to a string or `vec` that may be `readonly`** copies it to the
  heap first. The test is omitted where the compiler knows the value was
  built writable.

### 9.6 Machine code

Rows refer to 9.3. `_salloc`, `_sfree`, and `_sown` are the runtime's
allocate, free, and copy-to-heap routines.

**Row 1: scalar** (current compiler output).

```text
fn add(x: i16, y: i16) -> i16:
    return x + y
```

```asm
_add proc far                      ; caller:
    push bp                        ;     push 4
    mov bp, sp                     ;     push 3
    mov ax, word ptr [bp+6]        ;     call far ptr _add
    add ax, word ptr [bp+8]        ;     add sp, 4
    pop bp                         ;     ; ax = 7
    retf
```

**Row 2: a tuple in registers.** `idiv` leaves the quotient in `ax` and the
remainder in `dx`, which is where `(i16, i16)` is returned.

```text
fn divmod(a: i16, b: i16) -> (i16, i16):
    return (a // b, a % b)

let (q, r) = divmod(17, 5)
```

```asm
_divmod proc far
    push bp
    mov bp, sp
    mov ax, word ptr [bp+6]
    cwd
    idiv word ptr [bp+8]           ; ax = quotient, dx = remainder
    pop bp
    retf
```

**Row 3: built in the slot.** Nothing is copied.

```text
struct rect:
    x: i16
    y: i16
    w: i16
    h: i16

fn square(side: i16) -> rect:
    return rect(x=0, y=0, w=side, h=side)
```

```asm
_square proc far                   ; caller, for let r = square(5):
    push bp                        ;     push 5
    mov bp, sp                     ;     lea ax, [bp-8]      ; r, not yet live
    les bx, dword ptr [bp+6]       ;     push ss
    mov ax, word ptr [bp+10]       ;     push ax
    mov word ptr es:[bx], 0        ;     call far ptr _square
    mov word ptr es:[bx+2], 0      ;     add sp, 6
    mov word ptr es:[bx+4], ax
    mov word ptr es:[bx+6], ax
    pop bp
    retf
```

**Rows 4 and 5: copied to the slot.** Word moves; `rep movsw` for large
values.

```asm
    les bx, dword ptr [bp+6]       ; slot
    mov ax, word ptr [bp-8]        ; the local (row 4) or argument words (row 5)
    mov word ptr es:[bx], ax
    mov ax, word ptr [bp-6]
    mov word ptr es:[bx+2], ax
    ; ... one pair per word
```

**Row 6: copied through the borrow.**

```asm
    les bx, dword ptr [bp+6]       ; slot
    lfs si, dword ptr [bp+10]      ; the borrowed value
    mov ax, word ptr fs:[si]
    mov word ptr es:[bx], ax
    ; ... one pair per word
```

**Row 8: a literal.** No frame, no allocation.

```asm
.data
            db 08h, 0              ; flags: static, readonly; pad
            dw 5, 5                ; length, capacity
L_hello     db 'hello', 0
.code
_greeting proc far
    mov ax, offset L_hello
    retf
```

**Row 9: built on the heap.**

```asm
    push cx                        ; bytes needed
    call far ptr _salloc           ; ax = new string: heap, length 0
    add sp, 2
    ; copy the pieces to [ax], set length at [ax-4], write the NUL
    retf                           ; ax = the string, owned by the caller
```

**Row 10: an owned parameter moved out.**

```asm
_pass proc far                     ; fn pass(s: string) -> string: return s
    push bp
    mov bp, sp
    mov ax, word ptr [bp+6]        ; s moves out; no drop of s
    pop bp
    retf
```

**Row 15: a borrow into an input.**

```asm
_second proc far                   ; fn second(p: &pair) -> &i16: return &p.b
    push bp
    mov bp, sp
    les bx, dword ptr [bp+6]
    lea ax, [bx+2]
    mov dx, es                     ; dx:ax = &p.b
    pop bp
    retf
```

**Row 16: a view descriptor into the slot.**

```text
fn head(v: &[i16], n: u16) -> &[i16]:
    return &v[0:n]
```

```asm
_head proc far
    push bp
    mov bp, sp
    les bx, dword ptr [bp+6]       ; slot
    lfs si, dword ptr [bp+10]      ; v's descriptor
    mov ax, word ptr [bp+14]       ; n, checked against fs:[si]
    mov word ptr es:[bx], ax       ; length
    mov word ptr es:[bx+2], ax     ; capacity = length
    mov ax, word ptr fs:[si+4]
    mov word ptr es:[bx+4], ax     ; data offset
    mov ax, word ptr fs:[si+6]
    mov word ptr es:[bx+6], ax     ; data segment
    pop bp
    retf
```

**Row 18: `Result` in registers.**

```text
fn digit(c: char) -> Result[u8, ParseError]:
    if c < '0' || c > '9':
        return .err(ParseError.not_digit)
    return .ok(u8(c) - u8('0'))
```

```asm
_digit proc far
    push bp
    mov bp, sp
    mov al, byte ptr [bp+6]
    sub al, '0'
    cmp al, 9
    ja L_bad
    clc                            ; ok: al = the digit
    pop bp
    retf
L_bad:
    mov al, NOT_DIGIT
    stc                            ; err: al = the error
    pop bp
    retf
```

**Row 21: `?`.**

```asm
    push word ptr [bp-2]           ; c
    call far ptr _digit
    jc L_fail                      ; before removing the argument
    add sp, 2
    ; ... al = the digit; this path does not fall through
L_fail:                            ; al = the error
    push ax                        ; kept across the drops
    ; drop the live locals
    pop ax
    stc                            ; last: the drops may have changed carry
    leave                          ; also removes the argument
    retf
```

**Drop** of an owned `string` or `vec` in `[bp-2]`:

```asm
    mov bx, word ptr [bp-2]
    test byte ptr [bx-6], 1        ; heap?
    jz L_kept
    push bx
    call far ptr _sfree
    add sp, 2
L_kept:
```

**Conditional move.** One flag byte; the drop above runs only if it is set.

```asm
    mov byte ptr [bp-3], 1         ; s is live
    ; ... on the path that moves s:
    mov byte ptr [bp-3], 0
    ; ... at scope exit:
    cmp byte ptr [bp-3], 0
    je L_done
    ; drop s
```

**Assignment** `s = foo()`:

```asm
    call far ptr _foo              ; ax = the new string
    mov bx, word ptr [bp-2]        ; the old one
    mov word ptr [bp-2], ax
    ; drop bx
```

**Write to a possibly read-only string** `s[0] = 'H'`:

```asm
    mov bx, word ptr [bp-2]
    test byte ptr [bx-6], 8        ; readonly?
    jz L_writable
    push bx
    call far ptr _sown             ; heap copy
    add sp, 2
    mov word ptr [bp-2], ax
    mov bx, ax
L_writable:
    mov byte ptr [bx], 'H'
```

## 10. Failure

Recoverable failure uses the ordinary enums:

```text
Option[T]       # .some(T) or .none
Result[T, E]    # .ok(T) or .err(E)
```

`?` unwraps success or returns the failure from the enclosing function.

```text
fn load(path: &string) -> Result[Image, LoadError]:
    with file = File.open(path)?:
        let header = file.read_header()?
        return file.read_image(header)
```

There are no thrown values, catch blocks, or stack unwinding.

## 11. Generics and protocols

Generics use square brackets and are monomorphized:

```text
fn first[T](values: &[T]) -> Option[&T]:
    match values:
        []:
            return .none
        [head, *tail]:
            return .some(head)
```

A call infers its type arguments from its arguments; those it cannot infer are
given in brackets, first to last: `parse[u16](text)`. Inside the body, `T(x)`
converts to what `T` is bound to.

A protocol is a structural compile-time requirement:

```text
protocol Writer:
    fn write(self: &mut Self, data: &[u8]) -> Result[u16, IoError]

fn emit[W: Writer](out: &mut W, text: &string) -> Result[void, IoError]:
    out.write(text.bytes())?
    return Result.ok()
```

A type satisfies a protocol when it has matching methods. There is no `impl`
declaration. Protocol calls are statically resolved. Version 0.1 has no dynamic
protocol objects or vtables.

Operators cannot be overloaded. Standard protocols such as `Iterable`,
`Iterator`, `Hashable`, `Ordered`, and `Display` use named methods.

## 12. Iteration, generators, and comprehensions

### Iterable and Iterator protocols

Two protocols enable `for` loops:

```text
protocol Iterable[T]:
    fn iter(self: &Self) -> Iterator[T]

protocol Iterator[T]:
    fn next(self: &mut Self) -> Option[T]
```

The `for` statement calls `.iter()` to obtain an iterator, then repeatedly calls
`.next()` until it returns `.none`:

```text
for item in values:
    print(item)

# expands to:
let iter = values.iter()
loop:
    match iter.next():
        .some(item):
            print(item)
        .none:
            break
```

`string`, `T[N]`, `vec[T]`, and `&[T]` implement `Iterable`. A string yields
its `char`s; the others yield their elements.

### Generators

A function containing `yield` returns `iter[T]`. It has no stack of its own
and never allocates.

```text
fn enumerate[T](items: &[T]) -> iter[(u16, &T)]:
    let mut i: u16 = 0
    for item in items:
        yield (i, item)
        i += 1
```

When a `for` consumes a generator whose body the compiler can see, the
compiler substitutes the loop body at each `yield`. No iterator, `Option`,
tuple, or `next` call remains. A `break` in the loop body leaves the generator
too, dropping its live locals on the way. The built-in iterators are ordinary
library generators, so arrays, strings, `enumerate`, `zip`, and `range` all
compile by this one rule; the compiler has no special case for any of them.

```text
let values: i16[8] = ...
for (i, x) in enumerate(values):
    print(f"{i}: {x}")
```

```asm
; values at [bp-16]
    xor si, si                     ; i
    lea di, [bp-16]                ; &values[i]
L_loop:
    push si
    call far ptr _pu2              ; {i}
    add sp, 2
    push offset L_colon            ; ": "
    call far ptr _pt
    add sp, 2
    push word ptr ss:[di]          ; {x}
    call far ptr _pi2
    add sp, 2
    call far ptr _pn
    add di, 2
    inc si
    cmp si, 8                      ; the view's length folded to 8
    jne L_loop
```

The tuple is never built: the pattern binds `i` and `x` to `si` and `[di]`.
The view of `values` is never written, because its length is known. This
listing assumes `si` and `di` survive calls, as in the DOS C convention;
otherwise `i` and the pointer live in the frame.

A generator that escapes, by being stored, passed on, or returned, becomes a
state struct holding its locals and a resume point. It has a fixed size, so it
is placed as section 9 places any such value. Each `next()` is a far call that
jumps to the saved resume point and returns its `Option` by section 9.4.

List comprehensions, dictionary comprehensions, and generator expressions use
one clause grammar:

```text
[value for pattern in input if condition]
{key: value for pattern in input if condition}
(value for pattern in input if condition)
```

The first creates an eager `vec`, the second an eager `dict`, and the third a
lazy generator.

```text
let squares = [x * x for x in values if x > 0]

let names = {
    person.id: person.name.copy()
    for person in people
    if person.active
}

let pairs = [
    (x, y)
    for x in xs
    for y in ys
    if x != y
]
```

Clauses evaluate left to right. Comprehension bindings are local. Duplicate
dictionary keys retain the last value. Dictionary iteration order is not part
of the language contract.

A refutable loop or comprehension pattern uses `case`; nonmatching items are
skipped.

```text
let names = [name for case [id, name] in records]
```

Without `case`, a refutable iteration pattern is a compile-time error.

The ordinary collection literals are:

```text
[one, two, three]
{"one": 1, "two": 2}
```

A repeat literal gives every element one value:

```text
let zeroes: i32[64] = [0] * 64
let grid: u8[80, 25] = [[32] * 25] * 80
```

The count is a compile-time constant. The value is evaluated once and copied
into each element, so its type must be copyable.

An unconstrained bracket literal, repeat or not, has type `vec[T]`. An
expected fixed-array type permits the same elements to initialize fixed
storage directly. Empty literals require an expected type. Collection spreading and call splatting do
not exist.

Dynamic collection construction uses the program allocator and invokes the
program's out-of-memory handler on failure. The allocator is linked only when
owned dynamic collections are used. Code requiring recoverable allocation uses
explicit fallible builders from the allocator library.

## 13. Arrays and text representation

An owned vector allocation places its descriptor immediately before its data:

```text
[flags][pad][dimensions][capacity][data...]
                                  ^ data pointer
```

`flags` is one byte of facts for the runtime:

| Bit | Field | Meaning |
|---|---|---|
| 0 | `heap` | the program allocator owns the buffer (`01h`) |
| 1-2 | origin, when not `heap` | `static` (`00h`), `stack` (`02h`), `foreign` (`04h`) |
| 3 | `readonly` | literals and read-only foreign data (`08h`) |
| 4-7 | reserved | |

`heap` has a bit of its own because drop, the most frequent reader, then
tests one bit: `test byte ptr [bx-6], 1`. `foreign` means memory obtained from
C or DOS into which our descriptor was written; a bare C `char *` has no
descriptor and enters only as a view or a copy (section 9).

`free` releases only `heap` buffers. Growth reallocates a `heap` buffer and
copies any other to the heap. A write to a `readonly` buffer copies it to the
heap first. The compiler writes the flags of static and stack descriptors,
together with the pad byte in one word store, and the allocator writes them
for heap buffers.

The pad byte keeps the data word-aligned. `length`/`dimensions` and
`capacity` keep fixed offsets from the data pointer, so only allocation,
growth, `free`, and writes to possibly read-only data read the flags.

Rank is part of the static type, so `vec[T]` does not store a runtime rank.
A borrowed view carries a data pointer plus the dimensions. Slicing aliases
storage and never copies implicitly.

Ranks are one to four. Arrays are row-major: the last index is contiguous, so
`a[i, j]` and `a[i, j + 1]` are neighbours. Every array and view is
contiguous, so no stride is stored: each follows from the dimensions after
it. A fixed array's descriptor has the same words as a vector's,
`[dimensions][capacity]`. At rank one these are `[length][capacity]`.

A borrowed `&T[N]` needs no descriptor: its dimensions are in its type, so it
is a far pointer to the array. An unsized view, `&[T]` or `&string`, is a
descriptor of its dimensions, capacity, and far data pointer. That descriptor
lives on the stack of the frame that creates it and has no allocator or
destructor; a call passes one far pointer to it, and a function returns it in
the caller's slot (section 9). A view is tied to the borrowed inputs it came
from (section 8). Neither copies the elements.

A ranked literal nests one bracket per dimension:

```text
let identity: f32[2, 2] = [[1, 0], [0, 1]]
```

A slice is spelled as in Python and selects a half-open range:

```text
values[a:b]     # a up to, not including, b
values[a:]      # a to the end
values[:b]      # the start up to b
values[:]       # everything
```

`&values[a:b]` borrows the selection. There are no negative indices and no
step.

An index at or past its dimension invokes the panic handler. A constant index
into a fixed dimension is checked at compile time. Inside `unsafe`, the
programmer vouches for every index and none is checked; iteration never
needs a check.

Dimensions and length are accessed with bracket notation:

```text
let n = a.len           # total length (field)
let d0 = a.dim[0]       # first dimension
let d1 = a.dim[1]       # second dimension
```

Nested vectors and ranked vectors are distinct:

```text
vec[vec[T]]    # potentially jagged
vec[T]         # always one contiguous allocation (rank determined at compile time)
```

A `vec` grows and shrinks at its end:

```text
let mut stack: vec[i16] = []
stack.push(4)          # may move the buffer
let top = stack.pop()  # panics when empty
let other = stack.copy()
```

`push` and `pop` need a mutable place: a mutable local, a field of one, or
a `&mut vec[T]`. `copy` copies the elements and everything they own. The
empty literal shares one static descriptor, so it allocates nothing until
the first `push`. A `vec` and a `string` share the runtime: grow, shrink,
copy, and drop take the element size.

### Strings

There is one text type, `string`. It holds target-code-page bytes, each a
`char`. There is no Unicode, normalization, or collation.

An owned string's value is a near pointer to its bytes. It has the array
descriptor, and a NUL follows the last byte, so C and DOS receive the value
unchanged. `length` is authoritative, so embedded NULs are allowed.

```text
[flags][pad][length][capacity][bytes...][NUL]
                              ^ value
```

A literal is a `static`, `readonly` string. Binding, passing, or returning it
copies only the pointer; the first write or `append` copies it to the heap. A
string is freed at scope exit only if its storage is `heap`.

```text
let greeting = "hello"          # static, no copy
let mut s: string = "hello"     # static until written
s[0] = 'H'                      # copies to the heap, then writes
s.append(" world")              # in place when capacity allows
let t = s + "!"                 # new string
```

Assignment moves the pointer; the target's old heap buffer is freed first.
`s = s + x` is `s.append(x)`.

`&s[a:b]` borrows bytes `a` up to `b` as a `&string` view. It is the same
stack descriptor as an array view (`length`, `capacity`, far data pointer),
with capacity equal to length, and is returned the same way. String views
are read-only; there is no `&mut` string view. A view is not NUL-terminated:
a foreign function that needs a terminator takes an owned string, and a view
passed there is copied explicitly first.

Returning an owned string moves its pointer. Returning a view copies its
descriptor to the caller's slot. Neither copies bytes; only `+`, `append`
past capacity, and an explicit copy do.

`s.len` is the length, `s[i]` a `char`, and `for c in s` iterates over the
chars. The compiler emits these inline, with no iterator object. Strings
implement `Hashable`, `Ordered`, `Iterable[char]`, `Display`, and
`Formattable`.

#### F-strings

An f-string is checked at compile time:

```text
print(f"{name}: {score:04x}")
let label = f"{name}: {score}"
```

`{expr}` uses the value's `Display` method and `{expr:code}` its
`Formattable` method with that code; `{{` and `}}` are literal braces. As a
direct `print` argument, an f-string streams each piece to output and
allocates nothing. Anywhere else it builds a new owned string.

| Code | Meaning |
|---|---|
| `x` `b` `o` | hex, binary, octal |
| `04x` | zero-padded to 4 |
| `5` / `-5` | padded to 5, right- / left-aligned |

#### Runtime

The compiler emits length, indexing, slicing, and iteration inline. The
runtime supplies:

| Routine | Used for |
|---|---|
| Near-heap `alloc`, `grow`, `free` | Owned strings. A string is a near pointer, so it lives in DGROUP; DOS allocates only whole segments. |
| Copy (`rep movsb`) | `append`, `+`, copying a literal or view |
| Compare (`rep cmpsb`) | `==` and ordering |
| Hash | `dict` keys |
| Formatters writing to a sink | `print` and f-strings; one set of digit routines serves both output and string buffers |
| Panic | Bounds violation and out of memory |

The allocator is linked only when a program builds an owned string on the
heap.

## 14. Modules

Each source file is one module. Its module name is its path relative to a
source root; source code does not repeat that name in a module declaration.

```text
src/main.<ext>                 # main
src/graphics.<ext>             # graphics
src/graphics/sprite.<ext>      # graphics.sprite
```

Directories provide namespacing only. They contain no executable initializer
and require no special index file.

Imports use absolute module names and remain qualified:

```text
import graphics.sprite

let image = graphics.sprite.load("PLAYER.SPR")?
```

An alias shortens the qualifier or resolves a collision:

```text
import graphics.sprite as sprite

let image = sprite.load("PLAYER.SPR")?
```

These are the only import forms. Selective imports, wildcard imports, relative
imports, re-exports, textual inclusion, and user-configurable preludes do not
exist.

An import introduces one module name or alias into module scope. It does not
copy declarations into the importing namespace, create a runtime module
object, or execute code. An import alias cannot be shadowed.

Declarations are private to their module unless marked `pub`:

```text
pub struct Image:
    width: u16
    height: u16
    pixels: vec[u8]  # flat 1D storage, indexed as [i*width + j]

pub fn load(path: &string) -> Result[Image, LoadError]:
    # ...
```

`pub` permits another source module to name a declaration. It does not create
an unmangled foreign symbol. `export` is the separate operation that exposes a
declaration through a foreign ABI.

Top level may contain declarations and constant initializers only. There are no
executable module bodies and no import-time side effects. A module-level `var`
must have a compile-time initializer; resource construction occurs explicitly
inside a function.

The import graph must be acyclic. A cycle is a compile-time error and is
reported as the complete chain of module names. Shared types belong in a lower
module imported by both participants.

The project manifest defines source roots and dependency names. Imports never
contain filesystem paths. The standard library is imported through the `std`
root and ABI support through the `abi` root:

```text
import std.io
import abi.qb45 as qb
```

Only scalar types, `Option`, `Result`, and the minimum protocols required by
the language are implicitly available. I/O, allocation policies, containers
beyond the built-ins, operating-system services, and foreign ABI adapters are
ordinary imports.

The compiler records the public typed interface of each module, compiles or
loads its dependencies, and gives the linker the object modules that supply
used symbols. Importing an otherwise unused module adds no initialization code
and no runtime data.

## 15. Foreign interoperability

Interop separates three independent properties:

1. `@repr` defines byte layout.
2. Pointer types define address space.
3. `extern` or `export` defines the calling ABI.

```text
@repr("c16", pack=1)
struct CPoint:
    x: i16
    y: i16

extern "cdecl16":
    @link_name("_draw_points")
    far fn draw_points(points: *far CPoint, count: u16) -> void

export "cdecl16":
    fn weight(value: i16) -> i16:
        return value * 2

fn main() -> i16:
    let corners: CPoint[2] = [CPoint(x=0, y=0), CPoint(x=8, y=4)]
    unsafe:
        draw_points(&corners, 2)
    return 0
```

`extern` imports a symbol; `export` exposes one under its own name, never
qualified by its module. Foreign calls and taking a raw pointer are unsafe:
they appear only in an `unsafe:` block. A `*mut` pointer is taken with `&mut`.
A `*near` pointer reaches only DGROUP's static data, so the program takes
`*far` ones; a `*near` one comes from foreign code.
[docs/examples/interop](examples/interop) links a C library both ways.
`pascal16` is the convention of QuickBASIC and Turbo Pascal libraries: its
symbols are upper case, arguments are pushed first to last, and the callee
removes them with `retf n`. [docs/examples/pascal](examples/pascal) calls an
assembly library that calls back into the program. Exported and imported signatures may contain only ABI-safe scalars,
represented structs and enums, raw pointers, foreign function pointers, and
compiler-provided foreign descriptor views.

Native `vec`, `dict`, `string`, closures, generators, protocols, and generic
functions never cross an ABI boundary implicitly.

The initial ABI profiles are:

```text
cdecl16
pascal16
interrupt16
qb45
pds71
vbdos
```

Assembly is not an ABI. An external assembly routine implements one of these
ABIs. Inline assembly declares all inputs, outputs, and clobbers and may appear
only in `unsafe` code.

Compiler-supplied BASIC ABI modules expose scoped views such as:

```text
qb.Ref[T]
qb.StringRef
qb.ArrayRef[T, N]
```

These adapt BASIC descriptors to native borrows without transferring ownership.
Retaining, resizing, or taking ownership requires an explicit copy or
ABI-specific owning operation.

The compiler can generate `.H`, `.BI`, and assembler `.INC` declarations from
exports: `modernfront --declare h|bi|inc SOURCE`. The generated files are
tooling outputs, not additional language constructs. The interop example's C
library includes the generated `main.h`.

## 16. Deliberate omissions

Version 0.1 has no:

- garbage collection or reference counting;
- exceptions or stack unwinding;
- classes or inheritance;
- runtime reflection or RTTI;
- dynamic protocol dispatch;
- function or operator overloading;
- implicit conversions beyond C's between integers and floats, or implicit cloning;
- `null` (use `Option[T]`);
- `defer` (use `with` scopes and `drop` methods);
- UFCS or extension methods;
- macros or source rewriting;
- unrestricted compile-time execution;
- async runtime or scheduler;
- automatic currying;
- call splatting;
- C varargs or C++ ABI;
- Unicode runtime;
- true division on integers (use `//` for integer division).

An omitted feature is added only if real programs cannot express it cleanly
with the existing mechanisms and its implementation does not impose a cost on
programs that do not use it.

## 17. Deferred design: pattern matching, pipes, and optimizations

The following features are under consideration but not yet accepted into the
language. They are documented here for future reference.

### Pattern matching in parameters

Allow destructuring enum and struct values in function parameter lists:

```text
fn process(Shape.circle(center, radius)) -> void:
    # center and radius are bound here
```

This is syntactic sugar for:

```text
fn process(s: Shape) -> void:
    let Shape.circle(center, radius) = s else return
    # body
```

Benefit: eliminates `match` boilerplate for single-pattern functions. No runtime
cost; compiles to identical machine code. Deferred because it requires no
overloading support (only one pattern per function) and `match` is already
sufficient.

### Pipe operator

Chain function calls with left-to-right data flow:

```text
values |> filter(|x| x > 0) |> map(|x| x * 2) |> sum()
```

Expands to:

```text
sum(map(filter(values, |x| x > 0), |x| x * 2))
```

Syntactic sugar for nested function calls. Deferred because current nesting is
sufficient and `|>` adds a new operator with parsing complexity.

### Enum tag remapping and sparse jump tables

For enums with non-contiguous tag values, the compiler remaps tags to dense
`[0, N)` internally and uses a dense jump table for `match` statements.

```text
enum Status:
    pending = 1
    active = 10
    done = 100
```

Internally becomes `[0, 1, 2]` for jump table generation. On FFI boundaries,
tags are translated back to original values.

**Machine cost on 486/P5:** Dense jump table = ~5-7 cycles per match.
Sparse jump tables with secondary lookups add 3-5 cycles. Tag remapping is
compile-time only, zero runtime overhead.

This is an implementation detail; users write sparse tag values and the compiler
optimizes transparently.

## 18. Complete example

```text
import io

enum LoadError:
    not_found
    invalid(line: u16)

struct Entry:
    name: string
    value: i16

fn load_entries(path: &string) -> Result[vec[Entry], LoadError]:
    with file = io.File.open(path, mode=0)?:
        return [parse(line)? for line in file.lines() if !line.empty()]

fn main() -> Result[void, LoadError]:
    let entries = load_entries("VALUES.DAT")?

    let positive = {
        entry.name.copy(): entry.value
        for entry in entries
        if entry.value > 0
    }

    match entries:
        []:
            io.print("no entries")
        [first, *rest]:
            io.print(f"first={first.name}, remaining={rest.len}")

    return .ok()
```
