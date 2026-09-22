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

`let` creates an immutable binding. `var` creates a mutable binding. `const`
creates a compile-time value.

```text
let name = "Ada"
var score: i16 = 0
const screen_width: u16 = 320
```

A binding's type is inferred from its initializer unless written explicitly.
Assignment uses `=` and requires a mutable destination.

```text
score = score + 1
```

There are no implicit declarations by assignment and no implicit numeric
conversions.

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
saturating operations are named library methods such as `checked_add` and
`saturating_add`; there are no alternate arithmetic operators. Division by
zero and the unrepresentable signed division `min / -1` invoke the panic
handler. Panic terminates; it never unwinds.

The compound built-in types are:

```text
(T, U)          # tuple
[T; N]          # fixed array
[T; X, Y]       # fixed ranked array
vec[T]          # owned dynamic sequence
vec[T, N]       # owned dynamic sequence of rank N
&[T]            # borrowed slice
&[T, N]         # borrowed ranked view
dict[K, V]      # owned hash table
string          # owned text
str             # borrowed length-counted text
cstr            # borrowed NUL-terminated text
fn(T) -> U      # noncapturing function value
```

There is no `null`. Absence is represented by `Option[T]`.

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
| `char` | `u8`, and back | The code unit unchanged. |
| integer | fixed-point | Converted to the storage type, then scaled; the scaled value wraps as arithmetic does. |
| fixed-point | integer | Truncated toward zero, then as integer to integer. |
| fixed-point | fixed-point | Rescaled; lost fraction bits truncate toward zero. |

There is no conversion to `bool`; compare instead (`count != 0`). A
conversion that must not lose information is a named library method,
`checked_to[T]()`, returning `Option[T]`.

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
| `not` | logical not |
| `and` | logical and |
| `or` | logical or |

Binary operators group left to right. Comparisons do not chain: `a < b < c`
is a compile-time error. As in Python, bitwise operators bind tighter than
comparisons, so `flags & mask == 0` means `(flags & mask) == 0`.

The operands of an arithmetic, bitwise, or comparison operator convert to
their common type; the result has that type, and a comparison's is `bool`. A
shift's operands are promoted separately; the result has the left one's type,
and the count may be any integer type. `and`, `or`, and `not` take and give `bool`;
`and` and `or` evaluate their right operand only when it decides the result.

Integer `/` truncates toward zero and `%` takes the sign of the dividend.
`>>` is arithmetic on a signed operand and logical on an unsigned one. A shift
count that is negative, or not less than the operand's width, is a
compile-time error when constant and invokes the panic handler otherwise.

Every binary arithmetic and bitwise operator has a compound assignment:
`+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, and `>>=`. There is
no `++`, `--`, or `**`.

## 4. Functions, methods, and lambdas

A named function has one declaration form and an explicit return type:

```text
fn add(a: i16, b: i16) -> i16:
    return a + b
```

A method is a function declared in a type's namespace. Its first parameter is
named `self`.

```text
fn Point.move(self: &mut Point, dx: i16, dy: i16) -> void:
    self.x = self.x + dx
    self.y = self.y + dy

point.move(2, 3)
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
lambda syntax.

Lambdas may borrow values from their enclosing scope and may not outlive those
values. A noncapturing lambda converts to a function value. The language has no
implicit heap allocation for closures and no special currying; partial
application is written with a lambda.

Functions are not overloaded. Default parameters and variadic native functions
do not exist. An API with many optional inputs accepts a struct.

## 5. User-defined data

A `struct` is a product of named fields in declaration order.

```text
struct Point:
    x: i16
    y: i16

let origin = Point(x=0, y=0)
```

An `enum` is a tagged union. A variant may carry values.

```text
enum Shape:
    point(Point)
    circle(center: Point, radius: u16)
    rectangle(min: Point, max: Point)
```

Variant names are contextual and begin with a dot:

```text
let result: Result[i16, Error] = .ok(42)
```

There are no classes, base types, constructors, properties, or implementation
inheritance.

Structs and enums may be generic:

```text
struct Pair[A, B]:
    first: A
    second: B
```

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
```

There are no `switch`, `do`, `goto`, ternary operator, exceptions, or labeled
loop variants.

`with` creates a nested ownership scope:

```text
with file = File.open("LEVEL.DAT")?:
    load_level(&file)?
```

The value is destroyed at the end of the block. `with` has no enter/exit
protocol and cannot suppress failure. There is no separate `defer` mechanism.

## 8. Ownership and borrowing

Every value has one owner. Passing, returning, or assigning an owned value
moves it. The source cannot be used after the move.

Scalars, references, raw pointers, function pointers, and aggregates composed
only of copyable fields are copied. A type with scoped destruction is not
copyable. Cloning is always explicit.

```text
&T          # shared borrow
&mut T      # exclusive borrow
```

A borrow cannot outlive its owner. While an exclusive borrow exists, no other
borrow may access the same value. While shared borrows exist, the value may not
be mutated or moved.

A returned borrow is conservatively tied to every borrowed input from which it
could have been derived. Version 0.1 has no named lifetime syntax.

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

## 9. Failure

Recoverable failure uses the ordinary enums:

```text
Option[T]       # .some(T) or .none
Result[T, E]    # .ok(T) or .err(E)
```

`?` unwraps success or returns the failure from the enclosing function.

```text
fn load(path: str) -> Result[Image, LoadError]:
    with file = File.open(path)?:
        let header = file.read_header()?
        return file.read_image(header)
```

There are no thrown values, catch blocks, or stack unwinding.

## 10. Generics and protocols

Generics use square brackets and are monomorphized:

```text
fn first[T](values: &[T]) -> Option[&T]:
    match values:
        []:
            return .none
        [head, *tail]:
            return .some(head)
```

A protocol is a structural compile-time requirement:

```text
protocol Writer:
    fn write(self: &mut Self, data: &[u8]) -> Result[u16, IoError]

fn emit[W: Writer](out: &mut W, text: str) -> Result[void, IoError]:
    out.write(text.bytes())?
    return .ok()
```

A type satisfies a protocol when it has matching methods. There is no `impl`
declaration. Protocol calls are statically resolved. Version 0.1 has no dynamic
protocol objects or vtables.

Operators cannot be overloaded. Standard library protocols such as formatting,
iteration, equality, and hashing use named methods.

## 11. Iteration, generators, and comprehensions

An iterator has one operation:

```text
next() -> Option[T]
```

`for` obtains an iterator from its input and repeatedly calls `next`.

```text
for item in values:
    print(item)
```

An owned collection's normal iterator borrows it and yields shared element
references. `iter_mut()` yields exclusive references, and `into_iter()`
consumes the collection and yields owned elements. These are distinct methods;
`for` never guesses whether iteration should consume its input.

A function containing `yield` returns `iter[T]` and is compiled to an affine
state structure. It has no independent stack and performs no implicit heap
allocation.

```text
fn words(text: str) -> iter[str]:
    # ...
    yield word
```

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
    person.id: person.name
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
let zeroes: [i32; 64] = [0; 64]
let grid: [u8; 80, 25] = [32; 80, 25]
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

## 12. Arrays and text representation

An owned vector allocation places its descriptor immediately before its data:

```text
[dimensions][capacity][strides][data...]
                              ^ data pointer
```

Rank is part of the static type, so `vec[T, 2]` need not store a runtime rank.
A borrowed view carries a data pointer plus the required dimensions and
strides. Slicing aliases storage and never copies implicitly.

A slice is spelled as in Python and selects a half-open range:

```text
values[a:b]     # a up to, not including, b
values[a:]      # a to the end
values[:b]      # the start up to b
values[:]       # everything
```

`&values[a:b]` borrows the selection. There are no negative indices and no
step.

Nested vectors and ranked vectors are distinct:

```text
vec[vec[T]]    # potentially jagged
vec[T, 2]      # one contiguous allocation
```

An owned `string` has a prefix descriptor and a trailing NUL. A `str` is a
length-counted view and need not end in NUL. A `cstr` is a view whose endpoint
is NUL-terminated. Conversion from `str` to `cstr` may require an explicit
temporary copy.

Strings contain target-code-page bytes. The runtime has no Unicode tables,
normalization, locale collation, or variable-width indexing.

F-strings are compile-time-checked formatting expressions:

```text
print(f"{name}: {score:04x}")
```

Formatting uses named protocol methods and may be lowered directly into output
writes when that is observably equivalent.

## 13. Modules

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
    pixels: vec[u8, 2]

pub fn load(path: str) -> Result[Image, LoadError]:
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

## 14. Foreign interoperability

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
    far fn draw_points(points: *near CPoint, count: u16) -> void
```

`extern` imports a symbol; `export` exposes one. Foreign calls are unsafe by
default. Exported and imported signatures may contain only ABI-safe scalars,
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
exports. The generated files are tooling outputs, not additional language
constructs.

## 15. Deliberate omissions

Version 0.1 has no:

- garbage collection or reference counting;
- exceptions or stack unwinding;
- classes or inheritance;
- runtime reflection or RTTI;
- dynamic protocol dispatch;
- function or operator overloading;
- implicit conversions or implicit cloning;
- `null`;
- `defer`;
- UFCS or extension methods;
- macros or source rewriting;
- unrestricted compile-time execution;
- async runtime or scheduler;
- automatic currying;
- call splatting;
- C varargs or C++ ABI;
- Unicode runtime.

An omitted feature is added only if real programs cannot express it cleanly
with the existing mechanisms and its implementation does not impose a cost on
programs that do not use it.

## 16. Complete example

```text
import io

enum LoadError:
    not_found
    invalid(line: u16)

struct Entry:
    name: string
    value: i16

fn load_entries(path: str) -> Result[vec[Entry], LoadError]:
    with file = io.File.open(path)?:
        return [parse(line)? for line in file.lines() if not line.empty()]

fn main() -> Result[void, LoadError]:
    let entries = load_entries("VALUES.DAT")?

    let positive = {
        entry.name.as_str(): entry.value
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
