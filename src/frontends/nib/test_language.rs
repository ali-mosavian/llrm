//! The draft's features, run on the host HIR interpreter.

use crate::hir::codec;
use crate::hir::execute;

/// What `main` prints.
pub(crate) fn output(source: &str) -> String {
    let hir = super::compile(source, "t").unwrap_or_else(|error| {
        panic!(
            "{}:{}: {}",
            error.span.line, error.span.column, error.message
        )
    });
    let program = codec::decode(&hir).expect("decodes");
    execute::run(&program, "main", &[]).expect("runs").output
}

pub(crate) fn refused(source: &str) -> String {
    super::compile(source, "t").expect_err("refused").message
}

#[test]
fn a_conditional_evaluates_only_its_chosen_arm_and_nests_to_the_right() {
    let source = "\
fn clamp(value: i16, low: i16 = 0, high: i16 = 100) -> i16:
    return value < low ? low : value > high ? high : value

fn main() -> i16:
    print(f\"{clamp(-5)} {clamp(50)} {clamp(500)}\")
    return 0
";
    assert_eq!(output(source), "0 50 100\n");
}

#[test]
fn named_arguments_bind_in_any_order_and_defaults_fill_the_rest() {
    let source = "\
fn clamp(value: i16, low: i16 = 0, high: i16 = 100) -> i16:
    return value < low ? low : value > high ? high : value

fn main() -> i16:
    print(f\"{clamp(500, high=200)} {clamp(high=10, value=11)} {clamp(-3, -2)}\")
    return 0
";
    assert_eq!(output(source), "200 10 -2\n");
    assert!(
        refused(&source.replace("clamp(-3, -2)", "clamp(low=1)")).contains("requires \"value\"")
    );
    assert!(
        refused(&source.replace("clamp(-3, -2)", "clamp(low=1, 2)"))
            .contains("positional argument cannot follow")
    );
}

#[test]
fn comparisons_chain_and_logical_operators_are_symbols() {
    let source = "\
fn main() -> i16:
    let a: i16 = 3
    print(f\"{0 < a < 5} {0 < a < 2} {!(a == 3) || a > 2 && a < 4}\")
    return 0
";
    assert_eq!(output(source), "true false true\n");
}

#[test]
fn integer_division_is_floor_division_and_slash_is_refused() {
    let source = "\
fn main() -> i16:
    let a: i16 = -7
    let b: u16 = 7
    print(f\"{a // 2} {a % 2} {b // 2}\")
    return 0
";
    assert_eq!(output(source), "-4 -1 3\n");
    assert!(refused(&source.replace("a // 2", "a / 2")).contains("use '//'"));
}

#[test]
fn a_struct_is_built_by_calling_its_name_and_returned_through_the_callers_slot() {
    let source = "\
struct Rect:
    x: i16
    y: i16
    w: i16
    h: i16

fn square(side: i16, at: i16 = 0) -> Rect:
    return Rect(x=at, y=at, w=side, h=side)

fn area(r: Rect) -> i16:
    return r.w * r.h

fn main() -> i16:
    let r = square(5)
    let s = square(side=3, at=2)
    print(f\"{r.x} {r.w} {s.x} {s.h} {area(r)} {area(square(4))} {area(Rect(0, 0, 2, 3))}\")
    return 0
";
    assert_eq!(output(source), "0 5 2 3 25 16 6\n");
}

#[test]
fn repeat_literals_and_length_fields() {
    let source = "\
fn main() -> i16:
    let grid: u8[3, 4] = [[7] * 4] * 3
    let values: i16[5] = [2] * 5
    print(f\"{values.len} {grid.dim[0]} {grid.dim[1]} {grid[2, 3]}\")
    return 0
";
    assert_eq!(output(source), "5 3 4 7\n");
}

#[test]
fn unchecked_bounds_drops_index_and_slice_checks_but_still_refuses_constant_ones() {
    // Before the switch, `--unchecked-bounds` could not be asked of the
    // frontend, and every index and slice of a sort's inner loop was checked.
    let source = "\
fn pick(items: &[i16], at: u16) -> i16:
    let part = &items[1:at]
    return items[at] + i16(part.len)

fn main() -> i16:
    return pick([4, 5, 6], 2)
";
    // The runtime routine stays declared either way; only its calls go.
    let checks = |unchecked_bounds| {
        let module = super::parse(super::lex(source).unwrap()).unwrap();
        let hir = super::compile_module(module, "t", &super::Frontend { unchecked_bounds }).unwrap();
        hir.replace([' ', '\n'], "").matches("\"callee\":\"N$EBND\"").count()
    };
    assert_eq!(checks(false), 3);
    assert_eq!(checks(true), 0);
    let constant = "fn main() -> i16:\n    let values: i16[3] = [1, 2, 3]\n    return values[3]\n";
    let module = super::parse(super::lex(constant).unwrap()).unwrap();
    let refused = super::compile_module(module, "t", &super::Frontend { unchecked_bounds: true }).expect_err("refused");
    assert!(refused.message.contains("3 is outside 0..3"), "{}", refused.message);
}

#[test]
fn std_sort_orders_runs_duplicates_and_arrays() {
    let source = "\
import std.sort as sort

fn main() -> i16:
    let mut xs: vec[i16] = []
    let mut seed: u16 = 7
    for _ in 0..300:
        seed = seed * 25173 + 13849
        xs.push(i16(seed % 50) - 25)
    sort.sort(xs)
    let mut bad: u16 = 0
    for at in 1..xs.len:
        if xs[at] < xs[at - 1]:
            bad += 1
    let mut few: u8[5] = [3, 3, 1, 3, 1]
    sort.sort(few)
    print(f\"{xs[0]} {xs[299]} {bad} {few[0]}{few[1]}{few[2]}{few[3]}{few[4]}\")
    return 0
";
    let directory = tempfile::tempdir().expect("a directory");
    let main = directory.path().join("main.nib");
    std::fs::write(&main, source).expect("written");
    let hir = super::compile_file(&main, &Default::default()).unwrap_or_else(|(_, error)| panic!("{}", error.message));
    let program = codec::decode(&hir).expect("decodes");
    let executed = execute::run(&program, "main", &[]).expect("runs");
    assert_eq!(executed.output, "-24 24 0 11333\n");
}

#[test]
fn enums_match_exhaustively_on_tags_and_payloads() {
    let source = include_str!("../../../docs/examples/shapes.nib");
    assert_eq!(
        output(source),
        "area 87\nmode 19 has 320 columns\ntext is 80\n"
    );
    let partial = source.replace("        .cga:\n            return 40\n", "");
    assert!(refused(&partial).contains("does not cover .cga"));
}

#[test]
fn question_mark_returns_the_failure_and_nested_patterns_cover_every_error() {
    let source = include_str!("../../../docs/examples/digits.nib");
    assert_eq!(
        output(source),
        "1234\nempty\nnot a digit\ntoo big\nfirst even 8\n"
    );
    let partial = source.replace(
        "        .err(.too_big):\n            print(\"too big\")\n",
        "",
    );
    assert!(refused(&partial).contains("does not cover .err(.too_big)"));
}

/// What `main` prints, having checked every heap buffer was dropped.
fn output_without_leaks(source: &str) -> String {
    let hir = super::compile(source, "t").unwrap_or_else(|error| {
        panic!(
            "{}:{}: {}",
            error.span.line, error.span.column, error.message
        )
    });
    let executed = execute::run(&codec::decode(&hir).expect("decodes"), "main", &[]).expect("runs");
    assert_eq!(executed.leaked, 0, "heap buffers leaked");
    executed.output
}

#[test]
fn strings_join_append_copy_on_write_and_every_owner_drops_its_buffer() {
    let source = include_str!("../../../docs/examples/greeting.nib");
    assert_eq!(
        output_without_leaks(source),
        "hello, world!\nHELLO, WORLD!\nhello\n[ada: 42] has 7 chars\nada sorts first\nreset\n"
    );
}

#[test]
fn a_moved_string_is_not_dropped_twice_and_a_borrowed_one_cannot_move() {
    let source = "\
fn keep(text: string) -> string:
    return text

fn main() -> i16:
    let a: string = \"x\" + \"y\"
    let b = a
    let c = keep(b)
    let mut d: string = c + \"z\"
    d = d + \"!\"
    with e = d + \"?\":
        print(e)
    print(d)
    return 0
";
    assert_eq!(output_without_leaks(source), "xyz!?\nxyz!\n");
    let borrowed = "\
struct Named:
    name: string

fn main() -> i16:
    let n = Named(name=\"a\" + \"b\")
    let taken = n.name
    return 0
";
    assert!(refused(borrowed).contains("cannot move"));
}

#[test]
fn structs_and_enums_drop_the_strings_they_own() {
    // Before aggregates owned their fields, every string stored in one leaked.
    let source = "\
struct Named:
    name: string
    rank: i16

enum Slot:
    empty
    held(Named)

fn describe(slot: Slot) -> string:
    match slot:
        .empty:
            return \"-\"
        .held(named):
            return named.name.copy()

fn main() -> i16:
    let a = Named(name=\"a\" + \"b\", rank=1)
    let mut b = a
    b = Named(name=\"c\" + \"d\", rank=2)
    print(b.rank)
    let s = Slot.held(b)
    print(describe(s))
    return 0
";
    assert_eq!(output_without_leaks(source), "2\ncd\n");
}

#[test]
fn a_conditional_string_owns_the_arm_it_took() {
    // A string arm's value had no owner: a heap arm leaked, and after
    // unknown values became borrows, a literal arm could not move.
    let source = "\
fn label(n: i16) -> string:
    return n > 1 ? f\"{n} items\" : \"one item\"

fn main() -> i16:
    let a = label(1)
    let b = label(3)
    print(a)
    print(b)
    return 0
";
    assert_eq!(output_without_leaks(source), "one item\n3 items\n");
}

#[test]
fn a_vec_grows_shrinks_copies_and_drops_what_it_owns() {
    let source = "\
struct Item:
    name: string
    count: i16

fn total(items: &vec[Item]) -> i16:
    let mut sum: i16 = 0
    for item in items:
        sum += item.count
    return sum

fn main() -> i16:
    let mut values: vec[i16] = [3, 1, 4]
    values.push(1)
    values.push(5)
    values[0] = 9
    let mut sum: i16 = 0
    for v in values:
        sum += v
    print(f\"{values.len} {sum} {values.pop()} {values.len}\")
    let squares = [v * v for v in values]
    print(f\"{squares[3]} {squares.len}\")
    let zeros = [0] * 5
    print(zeros.len)
    let mut names: vec[string] = []
    names.push(\"a\" + \"b\")
    names.push(\"cd\")
    let copied = names.copy()
    print(copied[0])
    let mut items: vec[Item] = []
    items.push(Item(name=\"x\" + \"y\", count=2))
    items.push(Item(name=\"z\", count=5))
    print(items[1].name)
    print(total(items))
    return 0
";
    assert_eq!(output_without_leaks(source), "5 20 5 4\n1 4\n5\nab\nz\n7\n");
}

#[test]
fn bits_structs_pack_read_write_and_nest() {
    let source = "\
bits struct Attr: u8
    mut fg: u4
    mut bg: u3
    mut blink: bool

enum Mode: u2
    text
    cga
    ega
    vga

bits struct Status: u16
    mut attr: Attr
    mode: Mode
    mut level: i4
    ready: bool

fn main() -> i16:
    let mut a = Attr(fg=15, bg=1, blink=false)
    print(u8(a))
    a.bg = 6
    a.blink = true
    a.fg -= 3
    print(f\"{a.fg} {a.bg} {a.blink} {u8(a)}\")
    let back = Attr(0x9C)
    print(f\"{back.fg} {back.bg} {back.blink}\")
    let mut s = Status(attr=a, mode=Mode.ega, level=-3, ready=true)
    s.attr.fg = 1
    s.level += 1
    print(f\"{s.attr.fg} {s.attr.bg} {u8(s.mode)} {s.level} {s.ready} {u16(s)}\")
    return 0
";
    assert_eq!(
        output(source),
        "31\n12 6 true 236\n12 1 true\n1 6 2 -2 true 31457\n"
    );
    let too_wide = "\
bits struct Attr: u8
    fg: u4
    mut bg: u4

fn main() -> i16:
    let a = Attr(fg=16, bg=0)
    return 0
";
    assert!(refused(too_wide).contains("does not fit in 4 bits"));
    let too_many = "\
bits struct Wide: u8
    low: u5
    high: u4

fn main() -> i16:
    return 0
";
    assert!(refused(too_many).contains("take 9 bits"));
}

#[test]
fn tuples_are_built_returned_matched_and_taken_apart() {
    let source = "\
struct Point:
    mut x: i16
    mut y: i16

fn divmod(a: i16, b: i16) -> (i16, i16):
    return (a // b, a % b)

fn bounds(points: &Point[3]) -> (Point, Point):
    let mut low = Point(x=points[0].x, y=points[0].y)
    let mut high = Point(x=points[0].x, y=points[0].y)
    for p in points:
        low.x = p.x < low.x ? p.x : low.x
        low.y = p.y < low.y ? p.y : low.y
        high.x = p.x > high.x ? p.x : high.x
        high.y = p.y > high.y ? p.y : high.y
    return (low, high)

fn named(n: i16) -> (string, i16):
    return (f\"item{n}\", n * 10)

fn main() -> i16:
    let (q, r) = divmod(17, 5)
    print(f\"{q} {r}\")
    let points: Point[3] = [Point(x=3, y=-1), Point(x=-2, y=4), Point(x=0, y=0)]
    let (low, high) = bounds(points)
    print(f\"{low.x},{low.y} {high.x},{high.y}\")
    let pair = (7, 'z')
    match pair:
        (7, c):
            print(f\"seven and {c}\")
        (_, _):
            print(\"other\")
    let (label, value) = named(4)
    print(f\"{label}={value}\")
    return 0
";
    assert_eq!(
        output_without_leaks(source),
        "3 2\n-2,-1 3,4\nseven and z\nitem4=40\n"
    );
    let refutable = "\
fn main() -> i16:
    let (1, b) = (1, 2)
    return b
";
    assert!(refused(refutable).contains("needs 'else:'"));
}

#[test]
fn a_borrowed_struct_is_not_dropped_by_the_callee() {
    // A callee dropped the strings of every struct it borrowed, so the
    // caller's own drop freed them twice.
    let source = "\
struct Named:
    name: string
    rank: i16

fn make(n: i16) -> Named:
    return Named(name=f\"n{n}\", rank=n)

fn Named.label(self: &Named) -> string:
    return f\"{self.name}#{self.rank}\"

fn main() -> i16:
    print(make(3).rank)
    print(make(4).label())
    return 0
";
    assert_eq!(output_without_leaks(source), "3\nn4#4\n");
}

#[test]
fn methods_are_called_with_dot_syntax_and_defaults() {
    let source = "\
struct Point:
    mut x: i16
    mut y: i16

fn Point.move(self: &mut Point, dx: i16, dy: i16 = 0) -> void:
    self.x = self.x + dx
    self.y = self.y + dy

fn Point.length2(self: &Point) -> i16:
    return self.x * self.x + self.y * self.y

fn Point.mirrored(self: &Point) -> Point:
    return Point(x=-self.x, y=self.y)

enum Mode: u8
    text
    graphics

fn Mode.columns(self: Mode) -> i16:
    return self == Mode.text ? 80 : 320

fn main() -> i16:
    let mut point = Point(x=0, y=0)
    point.move(dx=2, dy=3)
    point.move(2)
    print(f\"{point.x},{point.y} {point.length2()}\")
    let other = point.mirrored()
    print(f\"{other.x},{other.y} {other.mirrored().x}\")
    let mode = Mode.graphics
    print(mode.columns())
    return 0
";
    assert_eq!(output(source), "4,3 25\n-4,3 4\n320\n");
    let free = "\
struct Point:
    mut x: i16

fn Point.get(self: &Point) -> i16:
    return self.x

fn main() -> i16:
    let p = Point(x=1)
    return get(p)
";
    assert!(refused(free).contains("get"));
}

#[test]
fn generic_functions_are_instantiated_per_argument_type() {
    let source = "\
struct Meter:
    mut total: i32
    mut ticks: i16

fn Meter.write(self: &mut Meter, amount: i16) -> void:
    self.total += i32(amount)
    self.ticks += 1

protocol Sink:
    fn write(self: &mut Self, amount: i16) -> void

fn largest[T](values: &[T]) -> T:
    let mut best = values[0]
    for value in values:
        if value > best:
            best = value
    return best

fn swap[A, B](pair: (A, B)) -> (B, A):
    let (a, b) = pair
    return (b, a)

fn feed[S: Sink](out: &mut S, values: &[i16]) -> void:
    for value in values:
        out.write(value)

fn main() -> i16:
    let small: i16[4] = [3, 9, -2, 7]
    let wide: u32[3] = [70000, 5, 123456]
    print(f\"{largest(small)} {largest(wide)}\")
    let (letter, number) = swap((42, 'q'))
    print(f\"{letter} {number}\")
    let mut meter = Meter(total=0, ticks=0)
    feed(meter, small)
    print(f\"{meter.total} over {meter.ticks}\")
    return 0
";
    assert_eq!(output(source), "9 123456\nq 42\n17 over 4\n");
    let unsatisfied = "\
struct Quiet:
    x: i16

protocol Sink:
    fn write(self: &mut Self, amount: i16) -> void

fn feed[S: Sink](out: &mut S) -> void:
    out.write(1)

fn main() -> i16:
    let mut q = Quiet(x=0)
    feed(q)
    return 0
";
    assert!(refused(unsatisfied).contains("Quiet is not a Sink"));
}

#[test]
fn lambdas_are_inlined_where_called_and_built_into_generic_callees() {
    let source = "\
struct Point:
    mut x: i16
    y: i16

fn apply[F](f: F, value: i16) -> i16:
    return f(value)

fn count_if[F](values: &[i16], keep: F) -> i16:
    let mut count = 0
    for value in values:
        if keep(value):
            count += 1
    return count

fn main() -> i16:
    let scale = 3
    let scaled = |x: i16| x * scale
    let add = |a, b| a + b
    let now = || 7
    let far = |p: Point| p.x + p.y
    print(f\"{scaled(5)} {add(2, scaled(1))} {now()} {far(Point(x=4, y=6))}\")
    let even = |v: i16| v % 2 == 0
    let values: i16[5] = [1, 2, 4, 5, 8]
    print(f\"{apply(|x: i16| x * x, 9)} {count_if(values, even)} {count_if(values, |v: i16| v > 3)}\")
    return 0
";
    assert_eq!(output(source), "15 5 7 10\n81 3 3\n");
    let capturing = "\
fn apply[F](f: F) -> i16:
    return f(1)

fn main() -> i16:
    let k = 2
    return apply(|x: i16| x + k)
";
    assert!(refused(capturing).contains("cannot capture \"k\""));
}

/// A struct literal passed to a `&T` parameter was "not an addressable struct".
#[test]
fn a_struct_literal_is_borrowed_from_a_temporary() {
    let source = "\
struct Point:
    mut x: i16
    y: i16

fn sum(p: &Point) -> i16:
    return p.x + p.y

fn main() -> i16:
    print(sum(Point(x=4, y=6)))
    return 0
";
    assert_eq!(output(source), "10\n");
}

#[test]
fn generators_are_inlined_where_a_for_consumes_them() {
    let source = "\
struct Reading:
    sensor: u8
    value: i16

fn evens(limit: i16) -> iter[i16]:
    let mut n = 0
    while n < limit:
        yield n
        n += 2

fn enumerate[T](items: &[T]) -> iter[(u16, T)]:
    let mut i: u16 = 0
    for item in items:
        yield (i, item)
        i += 1

fn matching[T, F](items: &[T], keep: F) -> iter[T]:
    for item in items:
        if keep(item):
            yield item

fn countdown(start: i16) -> iter[i16]:
    let mut n = start
    while true:
        if n == 0:
            return
        yield n
        n -= 1

fn main() -> i16:
    let mut total = 0
    for n in evens(10):
        total += n
    print(f\"evens {total}\")
    let values: i16[5] = [4, -2, 9, 0, 7]
    for (i, v) in enumerate(values):
        if v == 0:
            continue
        if v == 7:
            break
        print(f\"{i}: {v}\")
    let floor = 3
    for v in matching(values, |x: i16| x > floor):
        print(f\"over {v}\")
    let squares = [x * x for x in values if x > 0]
    print(f\"{squares.len} {squares[0]} {squares[2]}\")
    let pairs = [a * 10 + b for a in 0..3 for b in 0..3 if a != b]
    print(f\"{pairs.len} {pairs[0]} {pairs[5]}\")
    for d in countdown(3):
        print(f\"t-{d}\")
    let mut found = 0
    for x in (v * 2 for v in values if v < 5):
        found += x
    print(f\"doubled {found}\")
    return 0
";
    assert_eq!(
        output_without_leaks(source),
        "evens 20\n0: 4\n1: -2\n2: 9\nover 4\nover 9\nover 7\n3 16 49\n6 1 21\nt-3\nt-2\nt-1\ndoubled 4\n"
    );
}

#[test]
fn a_generator_owns_what_it_yields_and_a_break_drops_its_locals() {
    let source = "\
fn labels(prefix: string, count: i16) -> iter[string]:
    let mut i = 0
    let suffix = f\"/{count}\"
    while i < count:
        yield f\"{prefix}{i}{suffix}\"
        i += 1

fn main() -> i16:
    for label in labels(\"row\", 5):
        if label == \"row3/5\":
            break
        print(label)
    let keys: i16[4] = [1, 2, 3, 4]
    let codes = {x: x * 3 for x in keys if x != 2}
    print(f\"{codes.get(4, 0)} {codes.get(2, -1)} {codes.len}\")
    let found: Option[i16][3] = [maybe(1), maybe(-1), maybe(5)]
    for case .some(v) in found:
        print(v)
    return 0

fn maybe(x: i16) -> Option[i16]:
    return x > 0 ? .some(x) : .none
";
    assert_eq!(
        output_without_leaks(source),
        "row0/5\nrow1/5\nrow2/5\n12 -1 3\n1\n5\n"
    );
}

/// `x > 0 ? .some(x) : .none` returned from a function was "not an addressable struct".
#[test]
fn a_conditional_builds_an_enum_in_either_arm() {
    let source = "\
fn maybe(x: i16) -> Option[i16]:
    return x > 0 ? .some(x) : .none

fn main() -> i16:
    match maybe(4):
        .some(v):
            print(v)
        .none:
            print(0)
    let other: Option[i16] = maybe(-1)
    match other:
        .some(v):
            print(v)
        .none:
            print(\"none\")
    return 0
";
    assert_eq!(output(source), "4\nnone\n");
}

#[test]
fn the_league_example_ranks_teams_with_methods_generics_lambdas_and_generators() {
    let source = "\
# A small league table: methods on a struct, a generic function, lambdas,
# generators consumed by for loops, and tuples returned and taken apart.

struct Team:
    name: string
    mut won: i16
    mut drawn: i16
    mut lost: i16

fn Team.points(self: &Team) -> i16:
    return self.won * 3 + self.drawn

fn Team.played(self: &Team) -> i16:
    return self.won + self.drawn + self.lost

fn Team.record(self: &mut Team, scored: i16, conceded: i16) -> void:
    if scored > conceded:
        self.won += 1
    else:
        if scored == conceded:
            self.drawn += 1
        else:
            self.lost += 1

# The index and value of the largest of `values`.
fn best[T](values: &[T]) -> (u16, T):
    let mut at: u16 = 0
    let mut top = values[0]
    let mut i: u16 = 0
    for value in values:
        if value > top:
            top = value
            at = i
        i += 1
    return (at, top)

# Each index whose value `keep` accepts.
fn where[T, F](values: &[T], keep: F) -> iter[u16]:
    let mut i: u16 = 0
    for value in values:
        if keep(value):
            yield i
        i += 1

fn main() -> i16:
    let mut teams: vec[Team] = []
    for name in [\"Rovers\", \"United\", \"Athletic\"]:
        teams.push(Team(name=name.copy(), won=0, drawn=0, lost=0))
    teams[0].record(2, 1)
    teams[0].record(0, 0)
    teams[1].record(3, 0)
    teams[1].record(1, 2)
    teams[2].record(1, 1)
    teams[2].record(0, 4)

    let points = [team.points() for team in teams]
    let (leader, top) = best(points)
    print(f\"{teams[leader].name} lead on {top}\")

    let needed = 2
    for i in where(points, |p: i16| p >= needed):
        let team = teams[i].copy()
        print(f\"{team.name}: {team.points()} from {team.played()}\")
    return 0
";
    assert_eq!(
        output_without_leaks(source),
        "Rovers lead on 4\nRovers: 4 from 2\nUnited: 3 from 2\n"
    );
}

/// `for name in [...]` was refused: "for currently iterates a named sequence".
#[test]
fn a_for_iterates_any_sequence_expression() {
    let source = "\
fn main() -> i16:
    for word in [\"a\", \"bc\"]:
        print(word)
    for n in [x * 2 for x in [1, 2, 3]]:
        print(n)
    return 0
";
    assert_eq!(output_without_leaks(source), "a\nbc\n2\n4\n6\n");
}

/// `v[0].bump()` on a vec of structs was "array methods currently require a named array",
/// and `v[0].copy()` could not be taken out of the vec.
#[test]
fn a_vec_element_takes_methods_and_copies() {
    let source = "\
struct Tally:
    mut label: string
    mut n: i16

fn Tally.bump(self: &mut Tally) -> void:
    self.n += 1

fn main() -> i16:
    let mut tallies: vec[Tally] = [Tally(label=\"a\", n=0)]
    tallies[0].bump()
    tallies[0].bump()
    let kept = tallies[0].copy()
    tallies[0].label = \"b\"
    print(f\"{kept.label} {kept.n} {tallies[0].label}\")
    return 0
";
    assert_eq!(output_without_leaks(source), "a 2 b\n");
}

/// A `vec[T]` passed to `&[T]` was "borrow of ... has the wrong type".
#[test]
fn a_vec_is_borrowed_as_a_view() {
    let source = "\
fn total(values: &[i16]) -> i16:
    let mut sum = 0
    for value in values:
        sum += value
    return sum

fn main() -> i16:
    let values = [x * x for x in [1, 2, 3, 4]]
    print(f\"{total(values)} {total(values[1:3])} {total(values[2:])}\")
    return 0
";
    assert_eq!(output_without_leaks(source), "30 13 25\n");
}

/// The output of the program whose main module is `main`, its imports read from `files`.
fn linked_output(main: &str, files: &[(&str, &str)]) -> Result<String, String> {
    let mut read = |name: &str| {
        files
            .iter()
            .find(|(one, _)| *one == name)
            .map(|(_, source)| (*source).to_owned())
            .ok_or_else(|| "no such module".to_owned())
    };
    let module = super::modules::load(main, &mut read)
        .map_err(|(module, error)| format!("{module}: {}", error.message))?;
    let hir = super::compile_module(module, "t", &Default::default()).map_err(|error| error.message)?;
    let executed = execute::run(&codec::decode(&hir).expect("decodes"), "main", &[]).expect("runs");
    assert_eq!(executed.leaked, 0, "heap buffers leaked");
    Ok(executed.output)
}

#[test]
fn modules_import_qualified_public_declarations() {
    let geometry = "\
pub struct Point:
    mut x: i16
    y: i16

pub enum Side:
    left
    right

pub fn Point.shifted(self: &Point, dx: i16) -> Point:
    return Point(x=self.x + dx, y=self.y)

pub fn side(p: &Point) -> Side:
    return p.x < 0 ? Side.left : Side.right

fn secret() -> i16:
    return 7
";
    let report = "\
import shapes.geometry as geo

pub fn describe(p: &geo.Point) -> string:
    match geo.side(p):
        geo.Side.left:
            return f\"{p.x},{p.y} left\"
        geo.Side.right:
            return f\"{p.x},{p.y} right\"
";
    let main = "\
import shapes.geometry
import report

fn main() -> i16:
    let p = shapes.geometry.Point(x=-3, y=4)
    print(report.describe(p))
    let q = p.shifted(5)
    print(report.describe(q))
    return 0
";
    let files = [("shapes.geometry", geometry), ("report", report)];
    assert_eq!(
        linked_output(main, &files).unwrap(),
        "-3,4 left\n2,4 right\n"
    );

    let private = "import shapes.geometry as geo\n\nfn main() -> i16:\n    return geo.secret()\n";
    assert_eq!(
        linked_output(private, &files).unwrap_err(),
        ": secret is private to module shapes.geometry"
    );

    let cycle = [
        ("a", "import b\n\npub fn f() -> i16:\n    return 1\n"),
        ("b", "import a\n"),
    ];
    let main = "import a\n\nfn main() -> i16:\n    return a.f()\n";
    assert_eq!(
        linked_output(main, &cycle).unwrap_err(),
        "b: import cycle: a -> b -> a"
    );
}

#[test]
fn constants_fold_and_size_arrays() {
    let source = "\
const WIDTH: u16 = 8
const HEIGHT = 3
const CELLS = WIDTH * HEIGHT
const TITLE = \"grid\"
const LIMIT: i32 = -(1 << 20)

fn main() -> i16:
    let mut grid: u8[HEIGHT, WIDTH] = [[0] * WIDTH] * HEIGHT
    grid[1, 2] = 5
    let const scale = 10
    const offset: i16 = scale + 1
    print(f\"{TITLE} {WIDTH}x{HEIGHT} = {CELLS}, {grid[1, 2]} {offset} {LIMIT}\")
    return 0
";
    assert_eq!(output(source), "grid 8x3 = 24, 5 11 -1048576\n");
    assert!(
        refused("fn f() -> i16:\n    return 1\n\nconst X = f()\n")
            .contains("X is not a compile-time value")
    );
}

#[test]
fn foreign_calls_and_raw_pointers_are_unsafe_and_only_abi_safe_types_cross() {
    let source = "@repr(\"c16\", pack=1)
struct Packet:
    kind: u8
    length: i16

struct Loose:
    kind: u8
    length: i16

@extern(\"cdecl16\")
fn send(packet: *far Packet) -> i16

fn main() -> i16:
    let packet = Packet(kind=1, length=2)
    unsafe:
        return send(&packet)
";
    let hir = super::compile(source, "t").expect("compiles");
    let program = codec::decode(&hir).expect("decodes");
    let width = |name: &str| {
        program.modules[0]
            .types
            .iter()
            .find(|one| one.name == name)
            .expect("a type")
            .width
    };
    assert_eq!(
        (width("Packet"), width("Loose")),
        (3, 4),
        "pack=1 aligns no field"
    );
    assert!(
        refused(&source.replace("    unsafe:\n        return", "    return")).contains("unsafe")
    );
    assert!(
        refused(
            &source
                .replace("*far Packet", "*far Loose")
                .replace("Packet(", "Loose(")
        )
        .contains("cross a foreign ABI")
    );
    assert!(refused(&source.replace("*far Packet", "*far mut Packet")).contains("'&mut'"));
    assert!(refused(&source.replace("*far Packet", "*near Packet")).contains("near pointer"));
    assert!(
        refused(
            &source
                .replace("packet: *far Packet) -> i16", "name: string) -> i16")
                .replace("&packet", "\"x\"")
        )
        .contains("cross a foreign ABI")
    );
}

#[test]
fn format_codes_pick_a_base_and_fill_a_field() {
    let source = "\
fn main() -> i16:
    let score: u16 = 3054
    let delta: i16 = -42
    let name = \"Ada\"
    let label = f\"{name:-5}|{score:06x}\"
    print(f\"{label} [{delta:5}] [{delta:05}] [{7:b}] [{8:o}] [{delta > 0 ? 1 : 2}]\")
    return 0
";
    assert_eq!(
        output(source),
        "Ada  |000bee [  -42] [-0042] [111] [10] [2]\n"
    );
    assert!(refused(&source.replace("{name:-5}", "{name:x}")).contains("only an integer"));
}

/// Two literal arms had no type: "the arms of '?:' need a known type".
#[test]
fn a_conditional_of_two_literals_takes_their_own_type() {
    let source = "\
fn main() -> i16:
    let big = true
    print(f\"{big ? 1 : -2} {big ? 70000 : 1}\")
    return 0
";
    assert_eq!(output(source), "1 70000\n");
}

#[test]
fn string_views_borrow_ranges_and_temporaries_and_print_compare_and_copy() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/examples/settings.nib"
    ))
    .expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "name    = Ada\nrole    = pilot\nlevel   = 7\n          twice that is 14\n3 settings, kept Ada\n"
    );
}

#[test]
fn a_moved_binding_is_unusable_on_every_path_until_assigned_again() {
    let source = "\
fn keep(text: string) -> u16:
    return text.len

fn main() -> i16:
    let mut s = \"ab\" + \"c\"
    let quit = false
    if quit:
        keep(s)
        return 1
    print(s)
    keep(s)
    s = \"again\"
    print(s)
    return 0
";
    assert_eq!(output_without_leaks(source), "abc\nagain\n");
    let moved = |from: &str, to: &str| refused(&source.replace(from, to));
    assert!(moved("    s = \"again\"\n", "").contains("\"s\" was moved"));
    assert!(moved("        return 1\n", "").contains("\"s\" was moved"), "a move on one branch reaches the join");
    let looped = source.replace("    keep(s)\n    s = ", "    while quit:\n        keep(s)\n    s = ");
    assert!(refused(&looped).contains("moved in one loop iteration"));
}

#[test]
fn drop_runs_once_per_owner_in_reverse_order_and_skips_a_moved_value() {
    let source = "\
struct Log:
    name: string
    lines: i16

fn Log.drop(self: &mut Log) -> void:
    print(f\"closing {self.name}\")

fn take(log: Log) -> i16:
    return log.lines

fn main() -> i16:
    let first = Log(name=\"first\", lines=1)
    let second = Log(name=\"second\", lines=2)
    let third = Log(name=\"third\", lines=3)
    with inner = Log(name=\"inner\", lines=4):
        print(\"in with\")
    if second.lines > 1:
        take(second)
    let mut last = Log(name=\"old\", lines=0)
    last = Log(name=\"new\", lines=5)
    print(\"end\")
    return 0
";
    assert_eq!(
        output_without_leaks(source),
        "in with\nclosing inner\nclosing second\nclosing old\nend\nclosing new\nclosing third\nclosing first\n"
    );
    assert!(refused(&source.replace("print(\"end\")", "last.drop()")).contains("cannot be called"));
    assert!(refused(&source.replace("print(\"end\")", "let again = first.copy()")).contains("cannot be copied"));
    assert!(refused(&source.replace("self: &mut Log) -> void", "self: &Log) -> void")).contains("a drop method is"));
}

#[test]
fn an_index_past_its_dimension_panics_unless_unsafe_vouches_for_it() {
    let source = "\
fn at(values: &[i16], i: u16) -> i16:
    return values[i]

fn main() -> i16:
    let table: i16[3] = [4, 5, 6]
    let mut stack: vec[i16] = [7]
    print(f\"{at(&table, 2)} {stack[0]}\")
    print(f\"{at(&table, 3)}\")
    return 0
";
    let hir = super::compile(source, "t").expect("compiles");
    let program = codec::decode(&hir).expect("decodes");
    let executed = execute::run(&program, "main", &[]).expect("runs");
    assert_eq!((executed.output.as_str(), executed.panic.as_deref()), ("6 7\n", Some("index out of bounds")));
    assert!(refused(&source.replace("stack[0]", "table[3]")).contains("outside 0..3"));
    let unchecked = source.replace("    return values[i]", "    unsafe:\n        return values[i]");
    let checks = |hir: &str| hir.matches(&format!("\"callee\":\"{}\"", crate::abi::nib::ERROR_BOUNDS)).count();
    assert_eq!(checks(&hir) - checks(&super::compile(&unchecked, "t").expect("compiles")), 1);
}

#[test]
fn a_for_takes_an_iterator_from_iter_and_calls_next_until_none() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/dice.nib")).expect("the example");
    assert_eq!(output_without_leaks(&source), "1: 10\n2: 14\n3:  8\n4:  8\n5: 13\n6:  7\nfirst six\n");
    assert!(refused(&source.replace("for face in dice:", "for face in &mut dice:")).contains("an iterator yields values"));
}

#[test]
fn a_view_result_borrows_what_the_caller_lent_and_never_a_local() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/csv.nib")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "Ada   | pilot at level 7\nGrace | admiral at level 9\nLinus | (no role)\ntrue Grace\n"
    );
    let dangling = source.replace("    return &row[0:0]", "    let empty = \"\"\n    return &empty[0:0]");
    assert!(refused(&dangling).contains("would dangle"));
    // Row 13 of section 9.3: a borrowed string is copied, never returned as owned.
    let owned = "fn pick(x: &string) -> string:\n    return x\n\nfn main() -> i16:\n    return 0\n";
    assert!(refused(owned).contains("borrowed view of a string"), "{}", refused(owned));
}

#[test]
fn a_shift_past_the_width_and_a_wide_index_past_the_dimension_panic() {
    let run = |source: &str| {
        let hir = super::compile(source, "t").expect("compiles");
        let executed = execute::run(&codec::decode(&hir).expect("decodes"), "main", &[]).expect("runs");
        (executed.output, executed.panic)
    };
    let shift = "fn main() -> i16:\n    let x: u16 = 1\n    let mut n: u16 = 15\n    print(f\"{x << n}\")\n    n = 16\n    print(f\"{x << n}\")\n    return 0\n";
    assert_eq!(run(shift).1.as_deref(), Some("shift count out of range"));
    assert_eq!(run(&shift.replace("n = 16", "n = 15")).1, None);
    // A 32-bit index is compared whole, not as its low word: 65536 is not 0.
    let wide = "fn main() -> i16:\n    let table: i16[3] = [1, 2, 3]\n    let at: i32 = 65536\n    print(f\"{table[at]}\")\n    return 0\n";
    assert_eq!(run(wide).1.as_deref(), Some("index out of bounds"));
}

#[test]
fn a_float_outside_the_integer_it_converts_to_panics() {
    let run = |value: &str, target: &str| {
        let source = format!("fn main() -> i16:\n    let x: f32 = {value}\n    print(f\"{{{target}(x)}}\")\n    return 0\n");
        let hir = super::compile(&source, "t").expect("compiles");
        let executed = execute::run(&codec::decode(&hir).expect("decodes"), "main", &[]).expect("runs");
        (executed.output, executed.panic)
    };
    assert_eq!(run("-32768.5", "i16"), ("-32768\n".into(), None));
    assert_eq!(run("32767.9", "i16"), ("32767\n".into(), None));
    assert_eq!(run("-0.5", "u8"), ("0\n".into(), None));
    for (value, target) in [("32768.0", "i16"), ("-1.0", "u8"), ("256.0", "u8")] {
        assert_eq!(run(value, target).1.as_deref(), Some("float outside the integer type"), "{target}({value})");
    }
}

#[test]
fn exports_are_declared_for_c_basic_and_assembler_callers() {
    use super::declarations::{Language, declarations};
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/pascal/levels.nib")).expect("the example");
    let module = super::parse(super::lex(&source).expect("lexes")).expect("parses");
    let declared = |language| declarations(&module, "levels", language).expect("declares");
    let c = declared(Language::C);
    assert!(c.contains("#pragma pack(1)\ntypedef struct {\n    short low;\n    short high;\n} Range;"), "{c}");
    assert!(c.contains("extern short __far __pascal clamp(short value, short low, short high);"), "{c}");
    let basic = declared(Language::Basic);
    assert!(basic.contains("DECLARE FUNCTION clamp% (BYVAL value AS INTEGER, BYVAL low AS INTEGER, BYVAL high AS INTEGER)"), "{basic}");
    let assembler = declared(Language::Assembler);
    assert!(assembler.contains("Range struct\n    low dw ?\n    high dw ?\nRange ends"), "{assembler}");
    assert!(assembler.contains("extrn CLAMP:far    ; pascal16(value: i16, low: i16, high: i16) -> i16, retf 6"), "{assembler}");
}

#[test]
fn sequence_patterns_match_lengths_and_view_the_rest() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/sequences.nib")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "empty\none 7\n1..4 around 2\n4 -1\nfirst at 1,2\nstarts 1, 2 then 2\ntrue false\n"
    );
    let falls = source.replace("        return -1\n", "        print(\"none\")\n");
    assert!(refused(&falls).contains("must leave"), "{}", refused(&falls));
    let unguarded = source.replace("    let [first, *rest] = values else:\n        return -1\n", "    let [first, *rest] = values\n");
    assert!(refused(&unguarded).contains("needs 'else:'"), "{}", refused(&unguarded));
}

#[test]
fn a_question_mark_moves_an_owned_payload_out() {
    // `let s = make(n)?` of a Result[string, E]: "cannot move out of a borrow, field, or element".
    let source = "\
enum E:
    bad

fn make(n: i16) -> Result[string, E]:
    if n < 0:
        return .err(.bad)
    return .ok(f\"n{n}\")

fn show(n: i16) -> Result[i16, E]:
    let s = make(n)?
    print(s)
    return .ok(1)

fn main() -> i16:
    match show(3):
        .ok(v):
            print(v)
        .err(e):
            print(\"bad\")
    match show(-1):
        .ok(v):
            print(v)
        .err(e):
            print(\"bad\")
    return 0
";
    assert_eq!(output_without_leaks(source), "n3\n1\nbad\n");
}

#[test]
fn locals_of_loop_bodies_and_match_arms_drop_each_pass() {
    // They were bound in the loop's or arm's own scope, which never dropped: one leak per pass.
    let source = "\
fn main() -> i16:
    let words = [\"a\", \"b\", \"c\"]
    for word in words:
        let loud = word + \"!\"
        if loud == \"b!\":
            continue
        print(loud)
    for i in 0..3:
        let tag = \"#\" + \"x\"
        if i == 2:
            break
        print(tag)
    match 2:
        2:
            let two = \"t\" + \"wo\"
            print(two)
        _:
            print(\"other\")
    return 0
";
    assert_eq!(output_without_leaks(source), "a!\nc!\n#x\n#x\ntwo\n");
}

#[test]
fn a_question_mark_ending_a_with_header_propagates() {
    // The suite's ':' was read as a conditional's: "expected an expression".
    let source = "\
enum E:
    bad

fn make(n: i16) -> Result[string, E]:
    if n < 0:
        return .err(.bad)
    return .ok(f\"n{n}\")

fn show(n: i16) -> Result[i16, E]:
    with s = make(n)?:
        print(s)
    return .ok(n > 0 ? 1 : 0)

fn main() -> i16:
    match show(3):
        .ok(v):
            print(v)
        .err(e):
            print(\"bad\")
    match show(-1):
        .ok(v):
            print(v)
        .err(e):
            print(\"bad\")
    return 0
";
    assert_eq!(output_without_leaks(source), "n3\n1\nbad\n");
}

#[test]
fn constant_division_floors_like_the_runtime() {
    // Folded Euclidean: 7 // -2 was -3 and -7 % 2 was 1.
    let source = "\
const Q: i16 = 7 // -2
const R: i16 = -7 % 2

fn main() -> i16:
    let a: i16 = 7
    let b: i16 = -2
    let c: i16 = -7
    let d: i16 = 2
    print(f\"{Q} {R} {a // b} {c % d}\")
    return 0
";
    assert_eq!(output(source), "-4 -1 -4 -1\n");
}

#[test]
fn a_quotient_too_wide_for_its_type_panics_as_the_divide_fault_does() {
    // The host executor wrapped i16 -32768 // -1 to -32768; idiv faults.
    let run = |divisor: &str| {
        let source = format!("fn main() -> i16:\n    let m: i16 = -32768\n    let d: i16 = {divisor}\n    print(\"a\")\n    print(f\"{{m // d}}\")\n    return 0\n");
        let hir = super::compile(&source, "t").expect("compiles");
        let executed = execute::run(&codec::decode(&hir).expect("decodes"), "main", &[]).expect("runs");
        (executed.output, executed.panic)
    };
    let fault = ("a\n".to_owned(), Some("division by zero or overflow".to_owned()));
    assert_eq!(run("-1"), fault);
    assert_eq!(run("0"), fault);
    assert_eq!(run("-2").1, None);
}

#[test]
fn a_bool_bit_field_reads_as_the_all_ones_true() {
    // It read as 1, so `a.blink == true` compared 1 with all ones: false.
    let source = "\
bits struct Attr: u8
    fg: u7
    blink: bool

fn main() -> i16:
    let a = Attr(0x80)
    print(f\"{a.blink == true} {i16(a.blink)}\")
    return 0
";
    assert_eq!(output(source), "true 1\n");
}

#[test]
fn checked_and_saturating_arithmetic_are_library_methods() {
    let source = "\
fn show(value: Option[i16]) -> string:
    match value:
        .some(v):
            return f\"{v}\"
        .none:
            return \"none\"

fn main() -> i16:
    let big: i16 = 30000
    let low: i16 = -30000
    let zero: i16 = 0
    let minus: i16 = -1
    let min: i16 = -32768
    print(f\"{show(big.checked_add(2767))} {show(big.checked_add(2768))} {show(low.checked_sub(2768))} {show(low.checked_sub(2769))}\")
    print(f\"{show(zero.checked_mul(min))} {show(minus.checked_mul(min))} {show(minus.checked_mul(32767))}\")
    print(f\"{big.saturating_add(big)} {low.saturating_add(low)} {low.saturating_sub(big)} {big.saturating_mul(minus)} {big.saturating_mul(low)}\")
    let level: u8 = 250
    let wide: u32 = 4000000000
    print(f\"{level.saturating_add(10)} {level.saturating_sub(251)} {wide.saturating_mul(2)} {wide.saturating_add(294967295)} {wide.saturating_add(294967296)}\")
    return 0
";
    assert_eq!(
        output_without_leaks(source),
        "32767 none -32768 none\n0 none -32767\n32767 -32768 -32768 -30000 -32768\n255 0 4294967295 4294967295 4294967295\n"
    );
    assert_eq!(
        output_without_leaks(include_str!("../../../docs/examples/meter.nib")),
        "health 255\nhealth 0\ntrue 2100000000\nfalse 2100000000\nfalse 2100000000\n"
    );
    assert!(refused("fn i16.twice(self: i16) -> i16:\n    return self * 2\n\nfn main() -> i16:\n    return 0\n").contains("only the language defines i16's methods"));
}

#[test]
fn type_arguments_can_be_given_and_checked_to_converts_without_loss() {
    let source = "\
fn narrowed[T, W](wide: W, low: W, high: W) -> Option[T]:
    if wide < low || wide > high:
        return .none
    return .some(T(wide))

fn show[T](value: Option[T]) -> string:
    match value:
        .some(v):
            return f\"{v}\"
        .none:
            return \"none\"

fn main() -> i16:
    let a: i16 = 30000
    print(show[i16](narrowed[i16, i32](i32(a) + 2767, -32768, 32767)))
    print(show[i16](narrowed[i16](i32(a) + 2768, -32768, 32767)))
    let big: i32 = 70000
    let small: i32 = -5
    let ratio: f64 = 199.9
    print(f\"{show(big.checked_to[u16]())} {show(small.checked_to[u16]())} {show(small.checked_to[i8]())} {show(ratio.checked_to[u8]())} {show(ratio.checked_to[i8]())}\")
    return 0
";
    assert_eq!(output_without_leaks(source), "32767\nnone\nnone none -5 199 none\n");
    assert!(refused("fn f() -> i16:\n    return 1\n\nfn main() -> i16:\n    return f[i16]()\n").contains("f takes no type arguments"));
}


#[test]
fn enumerate_zip_and_range_are_library_generators_yielding_references() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/laps.nib")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "lap 1: 62s\nlap 2: 58s\nlap 3: 60s\nlap 4: 57s\nfastest lap 4\nada made 10\nbob is 1 short\n\
         58s pace: 696s for 12 laps\n59s pace: 708s for 12 laps\n60s pace: 720s for 12 laps\n"
    );
    let text = "fn main() -> i16:\n    for (i, c) in enumerate(\"hi\"):\n        print(f\"{i}{c}\")\n    return 0\n";
    assert_eq!(output_without_leaks(text), "0h\n1i\n");
    // A reference is taken, never made from a value.
    assert!(refused("fn f() -> (i16, &i16):\n    return (1, 2)\n\nfn main() -> i16:\n    return 0\n").contains("a reference is taken with '&'"));
}

#[test]
fn a_method_is_called_on_a_value_never_as_a_free_function() {
    // `Point.sum(p)` resolved to the method and was accepted (section 5).
    let source = "\
struct Point:
    mut x: i16
    y: i16

fn Point.sum(self: &Point) -> i16:
    return self.x + self.y

fn main() -> i16:
    let p = Point(x=1, y=2)
    return Point.sum(p)
";
    assert!(refused(source).contains("Point.sum is a method"), "{}", refused(source));
    assert_eq!(output(&source.replace("Point.sum(p)", "p.sum()")), "");
}

#[test]
fn let_takes_any_pattern_and_a_sequence_match_can_be_complete() {
    // `let Point(x, y) = ...` was "a binding requires an initializer"; a
    // match on `[]` and `[head, *tail]` "can reach its end without returning".
    let source = "\
struct Point:
    mut x: i16
    y: i16

fn first(values: &[i16]) -> i16:
    match values:
        []:
            return 0
        [head, *tail]:
            return head

fn main() -> i16:
    let Point(x, y) = Point(x=1, y=2)
    let values = [4, 5]
    let none: vec[i16] = []
    print(f\"{x + y} {first(&values)} {first(&none)}\")
    return 0
";
    assert_eq!(output_without_leaks(source), "3 4 0\n");
}

#[test]
fn source_text_outside_ascii_is_the_target_code_page() {
    // "string literals contain target-code-page bytes; use a byte escape".
    let source = "fn main() -> i16:\n    let s = \"caf\u{e9} \u{bd}\"\n    print(f\"{s} {s.len} {i16('\u{e9}')}\")\n    return 0\n";
    assert_eq!(output(source), "caf\u{e9} \u{bd} 6 130\n");
    assert!(refused("fn main() -> i16:\n    print(\"\u{20ac}\")\n    return 0\n").contains("is not in the target code page"));
}

#[test]
fn fields_elements_and_views_are_places_like_names() {
    // `s.v.push`, `s.v[i]`, `for x in s.v`, `f(s.v)`, `let r = &v[1:]` and
    // `outer[0][1]` each failed: "array base must be a named binding",
    // "a borrow is valid only as a borrowed function argument", or a move.
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/gradebook.nib")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "ada: 3 marks, first 71\nrecent 3, average 53\nbest 85\nweek 2 day 2: 5, week 1 has 3\n"
    );
    let immutable = source.replace("let mut ada", "let ada");
    assert!(refused(&immutable).contains("immutable"), "{}", refused(&immutable));
}

#[test]
fn generic_structs_enums_and_methods_take_their_types_from_use() {
    // `Pair(first=1, ...)` was "unknown struct", `Maybe.yes(4)` "unknown
    // enum", `Result.ok()` of `Result[void, E]` wanted a payload, and a
    // generic type's method named an unknown `T`.
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/generic_types.nib")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "ada 42 7 true\ntop 9\nstored 11\nfull at 12\nfull at 13\n200 -1\nfirst roll 4\nnothing held\n"
    );
}

#[test]
fn a_borrow_never_outlives_what_it_borrows() {
    // `.some(copy)` of a local compiled, and DOS read a stale stack word
    // through the returned pointer; storing one through a `&mut` parameter
    // or into an outer local compiled too.
    let returned = "\
fn pick(values: &[i16]) -> Option[&i16]:
    let copy: i16 = values[0]
    return .some(copy)

fn main() -> i16:
    return 0
";
    assert!(refused(returned).contains("would dangle"), "{}", refused(returned));
    let stored = "\
fn leak(out: &mut Option[&i16]) -> void:
    let x: i16 = 5
    out = .some(x)

fn main() -> i16:
    return 0
";
    assert!(refused(stored).contains("\"out\" would outlive \"x\""), "{}", refused(stored));
    let inner = "\
fn main() -> i16:
    let mut o: Option[&i16] = .none
    if true:
        let y: i16 = 1
        o = .some(y)
    return 0
";
    assert!(refused(inner).contains("\"o\" would outlive \"y\""), "{}", refused(inner));
    let walked = "fn main() -> i16:\n    let mut v: vec[i16] = [1, 2]\n    for x in &v:\n        v.push(x)\n    return 0\n";
    assert!(refused(walked).contains("borrowed here"), "{}", refused(walked));
    // What may be done: return `&T` of a parameter, rename a reference,
    // and reseat a `let mut` view.
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/borrows.nib")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "leader bob\nbest di with 58\nfirst cy\n[move]\n[north]\n[then]\n[east]\n"
    );
}

#[test]
fn dicts_hash_their_keys_grow_and_lend_a_looked_up_key() {
    // `dict[K, V]` was "unknown type"; a lookup by a borrowed string moved
    // it, and `&s` had no `.hash()`: "array has no method".
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/tally.nib")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "6 words, the x3, owl x0\nhearts 2, distinct 2, queen true\n40 squares, 37 -> 1369\n"
    );
    let missing = source.replace("counts.get(\"owl\", 0)", "counts[\"owl\"]");
    let hir = super::compile(&missing, "t").expect("compiles");
    let executed = execute::run(&codec::decode(&hir).expect("decodes"), "main", &[]).expect("runs");
    assert_eq!(executed.panic.as_deref(), Some("key not found"));
}

#[test]
fn a_main_returning_result_exits_0_on_ok_and_1_on_err() {
    // It was run with no result slot: "expected 1 arguments, received 0".
    let exit = |outcome: &str| {
        let source = format!("enum E:\n    bad(code: u16)\n\nfn main() -> Result[void, E]:\n    print(\"hi\")\n    return {outcome}\n");
        let hir = super::compile(&source, "t").expect("compiles");
        let executed = execute::run(&codec::decode(&hir).expect("decodes"), "main", &[]).expect("runs");
        (executed.output, executed.value)
    };
    assert_eq!(exit(".ok()"), ("hi\n".into(), Some(crate::hir::model::Number::Int(0))));
    assert_eq!(exit(".err(.bad(3))"), ("hi\n".into(), Some(crate::hir::model::Number::Int(1))));
}

#[test]
fn a_field_is_written_only_when_declared_mut() {
    // Section 5: every field was writable, `mut` on one did not parse.
    let source = "struct Point:\n    x: i16\n    mut y: i16\n\nfn Point.lift(self: &mut Point) -> void:\n    self.y += 1\n\nfn main() -> i16:\n    let mut p = Point(x=3, y=4)\n    p.y = 7\n    p.lift()\n    print(p.y)\n    return 0\n";
    assert_eq!(output(source), "8\n");
    for write in ["p.x = 1", "p.x += 1"] {
        let refused_write = refused(&source.replace("p.y = 7", write));
        assert!(refused_write.contains("field \"x\" of Point is not declared 'mut'"), "{refused_write}");
    }
    let through_self = refused(&source.replace("self.y += 1", "self.x += 1"));
    assert!(through_self.contains("not declared 'mut'"), "{through_self}");
}

#[test]
fn an_else_if_is_an_else_holding_one_if() {
    // It was "expected ':' before block".
    let source = "fn name(x: i16) -> string:\n    if x == 1:\n        return \"one\"\n    else if x == 2:\n        return \"two\"\n    else:\n        return \"many\"\n\nfn main() -> i16:\n    print(f\"{name(1)} {name(2)} {name(3)}\")\n    return 0\n";
    assert_eq!(output(source), "one two many\n");
}

#[test]
fn a_local_const_is_folded_where_read_and_a_repeat_count_must_be_one() {
    // A local `const` was a `let`: "a repeat count must be a compile-time
    // integer"; and a vec took a runtime count.
    let source = "fn main() -> i16:\n    const w: u16 = 4\n    const h: u16 = w * 2 + 1\n    let row: u8[h] = [0] * h\n    if row.len == 9:\n        let w = 1\n        print(w)\n    print(f\"{row.len} {w}\")\n    return 0\n";
    assert_eq!(output(source), "1\n9 4\n");
    let runtime = "fn main() -> i16:\n    let n: u16 = 3\n    let v: vec[i16] = [0] * n\n    return 0\n";
    assert!(refused(runtime).contains("compile-time integer"), "{}", refused(runtime));
}

#[test]
fn a_comprehension_is_a_vec_typed_by_its_patterns_and_tuples() {
    // Nested clauses, `case` patterns and tuple elements were "an empty list
    // needs a vec type"; one over a fixed array became a fixed array of
    // scalars only, refusing `(x, x)`.
    let source = "enum Rec:\n    named(id: i16, name: string)\n    blank\n\nfn main() -> i16:\n    let xs: i16[2] = [1, 2]\n    let pairs = [(x, y) for x in xs for y in [2, 3] if x != y]\n    let (a, b) = pairs[2]\n    let mut twins = [(x, x) for x in xs]\n    twins.push((5, 5))\n    let records: vec[Rec] = [Rec.named(1, \"a\"), Rec.blank, Rec.named(2, \"b\")]\n    let names = [name.copy() for case .named(id, name) in records]\n    let rows: vec[vec[i16]] = [[1, 2], [3], [4, 5]]\n    let seconds = [b for case [_, b] in rows]\n    print(f\"{pairs.len} {a}{b} {twins.len} {names[1]} {seconds[1]}\")\n    return 0\n";
    assert_eq!(output_without_leaks(source), "3 23 3 b 5\n");
}

#[test]
fn a_function_without_self_is_called_on_its_type() {
    // `R.open(n)?` was "R.open is a method; call it on a value".
    let source = "struct R:\n    n: i16\n\nenum E:\n    bad\n\nfn R.drop(self: &mut R) -> void:\n    print(f\"drop {self.n}\")\n\nfn R.open(n: i16) -> Result[R, E]:\n    if n < 0:\n        return .err(.bad)\n    return .ok(R(n=n))\n\nfn R.get(self: &R) -> i16:\n    return self.n\n\nfn run(n: i16) -> Result[i16, E]:\n    with file = R.open(n)?:\n        print(file.get())\n    return .ok(0)\n\nfn main() -> i16:\n    match run(-1):\n        .ok(_):\n            print(\"ok\")\n        .err(_):\n            print(\"err\")\n    let _ = run(5)\n    return 0\n";
    assert_eq!(output(source), "err\n5\ndrop 5\n");
    let on_type = refused(&source.replace("file.get()", "R.get()"));
    assert!(on_type.contains("is a method; call it on a value"), "{on_type}");
}

#[test]
fn is_compares_any_two_struct_places_and_refuses_scalars() {
    // Only two loop views compared; `ra is &a` was "struct value has no
    // reference identity".
    let source = "struct P:\n    x: i16\n\nfn position(ps: &vec[P], p: &P) -> i16:\n    let mut i: i16 = 0\n    for q in ps:\n        if q is p:\n            return i\n        i += 1\n    return -1\n\nfn main() -> i16:\n    let a = P(x=1)\n    let b = P(x=1)\n    let ra = &a\n    let ps: vec[P] = [P(x=1), P(x=1), P(x=1)]\n    print(f\"{ra is &a} {&a is not &b} {a is b} {position(ps, ps[2])} {position(ps, a)}\")\n    return 0\n";
    assert_eq!(output(source), "true true false 2 -1\n");
    let scalar = "fn main() -> i16:\n    let n: i16 = 1\n    print(n is n)\n    return 0\n";
    assert!(refused(scalar).contains("a scalar has no identity"), "{}", refused(scalar));
}

#[test]
fn a_type_with_display_prints_and_formats_through_it() {
    // `print(f"{p}")` was "aggregate "p" requires an index or field".
    let source = "struct P:\n    x: i16\n\nfn P.display(self: &P) -> string:\n    return f\"P{self.x}\"\n\nfn main() -> i16:\n    let p = P(x=3)\n    print(p)\n    let label = f\"<{p}>\"\n    print(f\"{label} {p}\")\n    return 0\n";
    assert_eq!(output_without_leaks(source), "P3\n<P3> P3\n");
    // A call inside an f-string that prints or builds one ran while the
    // outer one was building: "N$PEND without N$PBEG".
    let calls = "fn g(n: i16) -> i16:\n    print(f\"[g{n}]\")\n    return n * 2\n\nfn first(s: &string) -> &string:\n    return &s[0:1]\n\nfn main() -> i16:\n    let w: string = \"xy\"\n    print(f\"{g(1)} {g(2)} {first(w)}\")\n    let t = f\"{g(3)}-{g(4)}\"\n    print(t)\n    return 0\n";
    assert_eq!(output_without_leaks(calls), "[g1]\n[g2]\n2 4 x\n[g3]\n[g4]\n6-8\n");
}

#[test]
fn a_matched_call_result_is_dropped_when_the_match_ends() {
    // `match all(a):` never dropped the vec its `.ok` held: "1 heap buffers leaked".
    let source = "enum E:\n    bad\n\nfn all(xs: &[i16]) -> Result[vec[i16], E]:\n    return .ok([x * 2 for x in xs])\n\nfn main() -> i16:\n    let a: i16[2] = [1, 2]\n    match all(a):\n        .ok(v):\n            print(v[1])\n        .err(_):\n            print(\"err\")\n    return 0\n";
    assert_eq!(output_without_leaks(source), "4\n");
}

#[test]
fn a_question_mark_returns_before_its_statement_makes_a_place() {
    // `v.push(make(n)?)` grew `v` first, so a failure left a garbage
    // element counted; `d[k] = make(n)?` claimed the key the same way.
    let source = "enum E:\n    bad\n\nfn make(n: i16) -> Result[i16, E]:\n    if n < 0:\n        return .err(.bad)\n    return .ok(n)\n\nfn fill(v: &mut vec[i16], d: &mut dict[i16, i16], n: i16) -> Result[void, E]:\n    v.push(make(n)?)\n    d[n] = make(n)?\n    return .ok()\n\nfn main() -> i16:\n    let mut v: vec[i16] = []\n    let mut d: dict[i16, i16] = {}\n    let _ = fill(v, d, 1)\n    let _ = fill(v, d, -1)\n    print(f\"{v.len} {d.len}\")\n    return 0\n";
    assert_eq!(output_without_leaks(source), "1 1\n");
}

#[test]
fn the_roster_example_prints_compares_and_builds_players() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/roster.nib")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "refused rating -3\nada (2150) master *\nbob (1505) expert\ncy (1620) expert\n3 above 1500, first ada at 2150\n"
    );
}

#[test]
fn a_function_value_names_a_function_and_calls_through_its_type() {
    // `let g = dbl` was "unknown name", and a generic parameter could not
    // be bound to a function passed by name.
    let source = "fn dbl(x: i16) -> i16:\n    return x * 2\n\nfn neg(x: i16) -> i16:\n    return -x\n\nfn apply[F](f: F, x: i16) -> i16:\n    return f(x)\n\nfn main() -> i16:\n    let g = dbl\n    let mut h: fn(i16) -> i16 = |x| x + 1\n    print(f\"{g(4)} {h(4)} {apply(neg, 4)} {apply(dbl, 5)}\")\n    h = neg\n    print(h(7))\n    return 0\n";
    assert_eq!(output(source), "8 5 -4 10\n-7\n");
    let captures = "fn main() -> i16:\n    let k: i16 = 1\n    let h: fn(i16) -> i16 = |x| x + k\n    print(h(1))\n    return 0\n";
    assert!(refused(captures).contains("cannot capture \"k\""), "{}", refused(captures));
}

#[test]
fn a_local_fn_is_a_function_seen_to_the_end_of_its_block() {
    // A `fn` in a body was "expected expression".
    let source = "struct C:\n    mut n: i16\n\nfn C.bump(self: &mut C, by: i16) -> void:\n    fn twice(x: i16) -> i16:\n        return x + x\n    self.n = self.n + twice(by)\n\nfn apply(f: fn(i16) -> i16, x: i16) -> i16:\n    return f(x)\n\nfn main() -> i16:\n    fn fact(n: i16) -> i16:\n        if n <= 1:\n            return 1\n        return n * fact(n - 1)\n    let mut c = C(n=0)\n    c.bump(3)\n    print(f\"{fact(5)} {apply(fact, 4)} {c.n}\")\n    return 0\n";
    assert_eq!(output(source), "120 24 6\n");
    let captures = "fn main() -> i16:\n    let k: i16 = 3\n    fn add(x: i16) -> i16:\n        return x + k\n    print(add(1))\n    return 0\n";
    assert!(refused(captures).contains("unknown name \"k\""), "{}", refused(captures));
}

#[test]
fn the_easing_example_slides_through_a_table_of_function_values() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/easing.nib")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "linear    0   8  16  24  32  40\nin        0   1   6  14  25  40\nout       0  14  25  33  38  40\nsmooth    0   3  12  27  36  40\nstep      0   0   0  20  20  40\n"
    );
}

#[test]
fn a_generator_that_escapes_is_a_state_its_next_resumes() {
    // `let g = count(3)` was "consumed by a 'for', not called", and an
    // `iter[T]` parameter was "iter is not a generic type".
    let shapes = "struct P:\n    x: i16\n    y: i16\n\nfn grid(w: i16, h: i16) -> iter[P]:\n    for y in 0..h:\n        let mut x: i16 = 0\n        loop:\n            if x == w:\n                break\n            if x == 1:\n                x += 1\n                continue\n            yield P(x=x, y=y)\n            x += 1\n\nfn evens(limit: u16) -> iter[u16]:\n    for v in 0..limit:\n        match v % 2:\n            0:\n                yield v\n            _:\n                if v > 6:\n                    return\n\nfn total(g: iter[u16]) -> u16:\n    let mut t: u16 = 0\n    for x in g:\n        t += x\n    return t\n\nfn upto(start: i16, end: i16) -> iter[i16]:\n    for i in start..end:\n        yield i\n\nfn main() -> i16:\n    let mut g = grid(3, 2)\n    let mut n: i16 = 0\n    for p in g:\n        print(f\"{p.x},{p.y}\")\n        n += 1\n    print(n)\n    print(total(evens(20)))\n    let mut r = upto(5, 8)\n    let _ = r.next()\n    for v in r:\n        print(v)\n    return 0\n";
    assert_eq!(output(shapes), "0,0\n2,0\n0,1\n2,1\n4\n12\n6\n7\n");
    let spec = "struct Point:\n    mut x: i16\n    mut y: i16\n\nfn countdown(start: u16) -> iter[u16]:\n    let mut n = start\n    while n > 0:\n        yield n\n        n -= 1\n\nfn total(values: iter[u16]) -> u16:\n    let mut sum: u16 = 0\n    for v in values:\n        sum += v\n    return sum\n\nfn main() -> i16:\n    let mut c = countdown(3)\n    c.next()\n    print(total(c))\n    let mut count: i16 = 5\n    let mut corner = Point(x=0, y=0)\n    unsafe:\n        let p: *far mut i16 = &mut count\n        *p = *p + 1\n        let q: *far mut Point = &mut corner\n        (*q).x = 3\n    print(f\"{count} {corner.x}\")\n    return 0\n";
    assert_eq!(output(spec), "3\n6 3\n");
}

#[test]
fn a_raw_pointer_reads_and_writes_its_place_only_in_unsafe() {
    // `*p` was "expected expression", `*huge` "a raw pointer is '*far' or
    // '*near'", and `&mut s` as `*far mut S` stored a `&S` in it.
    let source = "struct S:\n    mut x: i16\n\nfn main() -> i16:\n    let mut v: i16 = 5\n    let mut s = S(x=1)\n    unsafe:\n        let p: *far mut i16 = &mut v\n        let h: *huge i16 = &v\n        *p = *p + *h\n        let q: *far mut S = &mut s\n        (*q).x = (*q).x + 10\n    print(f\"{v} {s.x}\")\n    return 0\n";
    assert_eq!(output(source), "10 11\n");
    let outside = "fn read(p: *far i16) -> i16:\n    return *p\n\nfn main() -> i16:\n    return 0\n";
    assert!(refused(outside).contains("is unsafe"), "{}", refused(outside));
    let reads = "fn main() -> i16:\n    let v: i16 = 5\n    unsafe:\n        let p: *far i16 = &v\n        *p = 3\n    return 0\n";
    assert!(refused(reads).contains("binding \"*p\" is immutable"), "{}", refused(reads));
}

#[test]
fn a_function_ending_in_a_loop_only_break_leaves_needs_no_return() {
    // Was "can reach its end without returning": `loop:` had an exit edge.
    let source = "fn first_even(n: i16) -> i16:\n    let mut i = n\n    loop:\n        if i % 2 == 0:\n            return i\n        i += 1\n\nfn main() -> i16:\n    print(first_even(3))\n    return 0\n";
    assert_eq!(output(source), "4\n");
}

#[test]
fn a_mutable_name_for_a_struct_parameter_is_a_copy_of_its_own() {
    // `let mut q = p` aliased the parameter immutably: "cannot borrow "q" mutably".
    let source = "struct C:\n    mut n: i16\n\nfn C.bump(self: &mut C) -> void:\n    self.n += 1\n\nfn bumped(p: C) -> i16:\n    let mut q = p\n    q.bump()\n    return q.n\n\nfn main() -> i16:\n    let c = C(n=1)\n    print(f\"{bumped(c)} {c.n}\")\n    return 0\n";
    assert_eq!(output(source), "2 1\n");
}

#[test]
fn iter_on_a_sequence_is_an_iterator_that_keeps_its_view_borrowed() {
    // `a.iter()` was "array has no method iter", and a generator taking
    // `&[T]` could not escape: its state had no place for the view.
    let source = "fn total(values: iter[i16]) -> i16:\n    let mut sum: i16 = 0\n    for v in values:\n        sum += v\n    return sum\n\nfn main() -> i16:\n    let a: i16[3] = [1, 2, 3]\n    let mut v: vec[i16] = [10, 20]\n    v.push(30)\n    let mut it = v.iter()\n    let _ = it.next()\n    print(total(it))\n    print(total(a.iter()))\n    let mut e = enumerate(a)\n    match e.next():\n        .some((i, x)):\n            print(f\"{i}:{x}\")\n        .none:\n            print(\"none\")\n    return 0\n";
    assert_eq!(output_without_leaks(source), "50\n6\n0:1\n");
    let pushed = "fn main() -> i16:\n    let mut v: vec[i16] = [10, 20]\n    let mut it = v.iter()\n    v.push(30)\n    let _ = it.next()\n    return 0\n";
    assert!(refused(pushed).contains("\"v\" is borrowed here"), "{}", refused(pushed));
}

#[test]
fn the_readings_example_smooths_peeks_and_reports_generators() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/readings.nib")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "raw       12  15  11  30  28  27   9  10\navg3      12  18  23  28  21  15\nfirst pair averages 13\npeak of the rest: 29\n"
    );
}

#[test]
fn a_raw_pointer_to_a_view_has_the_pointer_type_it_was_given() {
    // The view's own data-pointer type leaked into the binding: "store value
    // type does not match its place".
    let source = "fn first(text: &string) -> char:\n    unsafe:\n        let data: *far char = &text\n        return *data.offset(1)\n\nfn main() -> i16:\n    let w: string = \"hi\"\n    print(first(w))\n    return 0\n";
    assert_eq!(output(source), "i\n");
}

#[test]
fn a_near_pointer_widens_to_a_far_one_to_the_same_place() {
    let source = "var codes: u8[2] = [7, 9]\n\nfn main() -> i16:\n    unsafe:\n        let p: *near mut u8 = &mut codes\n        let f = p.far()\n        *f.offset(1) = 4\n        print(f\"{codes[1]} {*p.offset(1)}\")\n    return 0\n";
    assert_eq!(output(source), "4 4\n");
}

#[test]
fn an_imported_modules_variables_are_its_own_when_assigned_or_shadowed() {
    // Assigning `count` or `table[i]` in an imported module was "unknown
    // name", and a local named as a module value read the module's.
    let counter = "var count: u16 = 0\nvar table: i16[2] = [0] * 2\n\npub fn record(value: i16) -> u16:\n    table[count] = value\n    count += 1\n    return count\n\npub fn shadowed() -> u16:\n    let count: u16 = 9\n    return count\n";
    let main = "import counter\n\nfn main() -> i16:\n    counter.record(5)\n    print(f\"{counter.record(6)} {counter.shadowed()}\")\n    return 0\n";
    assert_eq!(linked_output(main, &[("counter", counter)]).unwrap(), "2 9\n");
}

#[test]
fn a_local_hides_a_constant_of_its_name() {
    // The constant's literal replaced the local's every use: this printed 5.
    let source = "const LIMIT: i16 = 5\n\nfn main() -> i16:\n    let LIMIT: i16 = 2\n    print(LIMIT)\n    return 0\n";
    assert_eq!(output(source), "2\n");
}

#[test]
fn a_mut_pointer_passes_where_a_read_only_one_is_expected() {
    let source = "fn first(p: *far i16) -> i16:\n    unsafe:\n        return *p\n\nfn main() -> i16:\n    let mut v: i16 = 7\n    unsafe:\n        let p: *far mut i16 = &mut v\n        print(first(p))\n    return 0\n";
    assert_eq!(output(source), "7\n");
}

#[test]
fn a_literal_argument_takes_the_type_its_generic_gets_from_the_others() {
    // `upto(0, n)` bound T to the literal's i16: "i16 and u16 have no common type".
    let source = "fn upto[T](start: T, end: T) -> iter[T]:\n    let mut i = start\n    while i < end:\n        yield i\n        i += 1\n\nfn f(n: u16) -> u16:\n    let mut t: u16 = 0\n    for i in upto(0, n):\n        t += i * n\n    return t\n\nfn main() -> i16:\n    print(f(3))\n    return 0\n";
    assert_eq!(output(source), "9\n");
}

#[test]
fn module_variables_are_shared_by_every_function_and_a_local_hides_one() {
    assert_eq!(output("const SLOTS = 4\nvar count: u16 = 0\nvar table: i16[SLOTS] = [0] * SLOTS\nvar grid: u8[2, 3] = [[1, 2, 3], [4, 5, 6]]\nvar ready: bool = false\n\nfn record(value: i16) -> void:\n    table[count] = value\n    count += 1\n\nfn main() -> i16:\n    record(7)\n    record(9)\n    ready = true\n    print(f\"{count} {table[0]} {table[1]} {table[2]} {grid[1, 2]} {ready}\")\n    let count: u16 = 99\n    print(count)\n    return 0\n"), "2 7 9 0 6 true\n99\n");
}

#[test]
fn a_raw_pointer_steps_indexes_casts_and_compares() {
    assert_eq!(output("var bytes: u8[8] = [1, 2, 3, 4, 5, 6, 7, 8]\n\nfn main() -> i16:\n    unsafe:\n        let base: *near mut u8 = &mut bytes\n        let words = base.cast[u16]()\n        let third = base.offset(2)\n        print(f\"{base[0]} {third[0]} {third[-1]} {words[1]}\")\n        third[1] = 40\n        words[3] = 0x0102\n        print(f\"{bytes[3]} {bytes[6]} {bytes[7]}\")\n        print(f\"{third > base} {third.offset(-2) == base} {base.is_null()}\")\n    return 0\n"), "1 3 2 1027\n40 2 1\ntrue true false\n");
}

#[test]
fn an_f32_prints_the_shortest_text_that_reads_back_as_it() {
    // An f32 printed as the f64 it widens to: 0.10000000149011612.
    let source = "fn main() -> i16:\n    let a: f32 = 0.1\n    let b: f64 = 0.1\n    print(f\"{a} {b} {a * 3.0}\")\n    return 0\n";
    assert_eq!(output(source), "0.1 0.1 0.3\n");
}

#[test]
fn the_planets_example_prints_floats_as_the_shortest_text_that_reads_back() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/planets.nib")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "Mercury: g 3.7013390935421295 m/s2, orbit 47877.7475278498 m/s, year 87.96015390047978 days\n\
         Earth: g 9.819532032815959 m/s2, orbit 29788.22982930735 m/s, year 365.21871445326343 days\n\
         Jupiter: g 25.917397035788568 m/s2, orbit 13058.13590428579 m/s, year 4335.543604886578 days\n\
         G 6.674e-11, sun 1.989e+30 kg, one part in 5.0276520864756165e-31\n\
         thermometer mean 10.037499 C, first 21.5, zero 0.0\n"
    );
}

#[test]
fn a_default_may_name_a_constant() {
    // Constants were not substituted in defaults: "a default must be a literal".
    let source = "const STEP: i16 = 3\n\nfn next(at: i16, step: i16 = STEP) -> i16:\n    return at + step\n\nfn main() -> i16:\n    print(next(1))\n    return 0\n";
    assert_eq!(output(source), "4\n");
}

#[test]
fn arithmetic_on_literals_takes_the_type_of_the_other_operand() {
    // `80 - 1` was typed int: "u16 and i16 have no common type".
    let source = "const LIMIT = 80\n\nfn main() -> i16:\n    let at: u16 = 3\n    if at < LIMIT - 1:\n        print(at * (2 + 1))\n    return 0\n";
    assert_eq!(output(source), "9\n");
}

#[test]
fn a_raw_pointer_reaches_a_fields_place_and_a_sequence_fields_data() {
    // Only a named place had a raw address: a struct's buffer could not be filled.
    let source = "struct Reader:\n    mut count: i16\n    mut buffer: vec[u8]\n    mut cells: u8[3]\n\nfn fill(reader: &mut Reader) -> void:\n    unsafe:\n        let data: *far mut u8 = &mut reader.buffer\n        *data.offset(1) = 7\n        let count: *far mut i16 = &mut reader.count\n        *count = 2\n        let cells: *far mut u8 = &mut reader.cells\n        *cells.offset(2) = 9\n\nfn main() -> i16:\n    let mut reader = Reader(count=0, buffer=[0] * 3, cells=[0] * 3)\n    fill(&mut reader)\n    print(f\"{reader.count} {reader.buffer[1]} {reader.cells[2]}\")\n    return 0\n";
    assert_eq!(output(source), "2 7 9\n");
}

#[test]
fn a_string_pushes_and_pops_chars_and_a_nul_follows_its_last() {
    // A string had no push: "array has no method \"push\"".
    let source = "fn main() -> i16:\n    let mut s: string = \"ab\"\n    s.push('c')\n    s.push('d')\n    let last = s.pop()\n    unsafe:\n        let p: *far char = &s\n        print(f\"{s} {last} {u8(*p.offset(3))}\")\n    return 0\n";
    assert_eq!(output(source), "abc d 0\n");
}

#[test]
fn a_loop_walks_a_generator_method() {
    // Only a plain call was a generator: "\"counter\" is not an array or string".
    let source = "struct Counter:\n    limit: i16\n\nfn Counter.upto(self: &Counter) -> iter[i16]:\n    for i in 0..self.limit:\n        yield i\n\nfn main() -> i16:\n    let counter = Counter(limit=3)\n    for i in counter.upto():\n        print(i)\n    return 0\n";
    assert_eq!(output(source), "0\n1\n2\n");
}

#[test]
fn a_match_on_a_temporary_moves_what_its_arm_binds_and_drops_the_rest() {
    // Bindings borrowed from the temporary: "cannot move out of a borrow, field, or element".
    let source = "enum Found:\n    none\n    both(first: string, second: string)\n\nfn find(n: i16) -> Found:\n    if n == 0:\n        return .none\n    return .both(f\"a{n}\", f\"b{n}\")\n\nfn main() -> i16:\n    let mut kept: vec[string] = []\n    for i in 0..3:\n        match find(i):\n            .both(first, _):\n                kept.push(first)\n            .none:\n                print(\"none\")\n    for one in kept:\n        print(one)\n    return 0\n";
    assert_eq!(output(source), "none\na1\na2\n");
}

#[test]
fn an_array_field_is_laid_out_in_place_indexed_assigned_borrowed_and_iterated() {
    // A field could not be an array: "a field cannot be an array yet".
    let source = "\
@repr(\"c16\", pack=1)
struct Node:
    id: i16
    mut bound: u8[6]
    spare: u8[2]

struct Grid:
    mut cells: i16[2, 3]
    n: u8

fn total(values: &u8[6]) -> u16:
    let mut t: u16 = 0
    for v in values:
        t += u16(v)
    return t

fn bump(values: &mut u8[6]) -> void:
    values[0] += 1

fn main() -> i16:
    let mut n = Node(id=7, bound=[0] * 6, spare=[8, 9])
    n.bound[2] = 5
    n.bound[5] = n.bound[2] + 1
    bump(&mut n.bound)
    let mut s: u16 = 0
    for b in n.bound:
        s += u16(b)
    print(f\"{n.id} {n.bound[0]} {n.bound[5]} {n.bound.len} {s} {total(&n.bound)} {n.spare[1]}\")
    let mut g = Grid(cells=[[1, 2, 3], [4, 5, 6]], n=3)
    g.cells[1, 2] = 60
    print(f\"{g.cells[0, 1]} {g.cells[1, 2]} {g.cells.len} {g.cells.dim[1]} {g.n}\")
    let fresh: u8[6] = [1, 2, 3, 4, 5, 6]
    n.bound = fresh
    let kept = n
    n.bound = [9] * 6
    let mut back: u8[6] = [0] * 6
    back = n.bound
    print(f\"{kept.bound[5]} {n.bound[5]} {kept.id} {back[5]}\")
    return 0
";
    assert_eq!(output(source), "7 1 6 6 12 12 9\n2 60 6 3 3\n6 9 7 9\n");
    let hir = super::compile(source, "t").expect("compiles");
    let program = codec::decode(&hir).expect("decodes");
    let width = |name: &str| program.modules[0].types.iter().find(|one| one.name == name).expect("a type").width;
    assert_eq!((width("Node"), width("Grid")), (10, 14));
    assert!(refused(&source.replace("n.bound[2] = 5", "n.spare[0] = 5")).contains("\"spare\" of Node is not declared 'mut'"));
    assert!(refused(&source.replace("n.bound[2] = 5", "n.bound[6] = 5")).contains("6 is outside 0..6"));
    assert!(refused(&source.replace("let mut n = Node", "let n = Node")).contains("immutable"));
    assert!(refused(&source.replace("n.bound = fresh", "n.bound = [1, 2]")).contains("array expects 6 elements, got 2"));
    assert!(refused("bits struct B: u8\n    low: u4[2]\n").contains("a bits struct field cannot be an array"));
    let past = source.replace("n.bound[2] = 5", "let i: u16 = 6\n    n.bound[i] = 5");
    let executed = execute::run(&codec::decode(&super::compile(&past, "t").expect("compiles")).expect("decodes"), "main", &[]).expect("runs");
    assert_eq!(executed.panic.as_deref(), Some("index out of bounds"));
}

#[test]
fn array_fields_nest_in_arrays_vecs_and_raw_pointers() {
    let source = "\
struct Face:
    mut bound: u8[4]
    mut side: u8

struct Mesh:
    mut faces: Face[3]
    mut normal: f32[3]

fn main() -> i16:
    let mut m = Mesh(faces=[Face(bound=[1, 2, 3, 4], side=0)] * 3, normal=[0.0, 0.5, 1.0])
    m.faces[1].bound[2] = 30
    m.faces[2].side = 2
    let mut list: vec[Face] = [Face(bound=[5, 6, 7, 8], side=1)]
    list.push(Face(bound=[0] * 4, side=9))
    list[1].bound[3] = 44
    let arr: Face[2] = [Face(bound=[1, 1, 1, 1], side=0), Face(bound=[2, 2, 2, 2], side=1)]
    let mut sides: u8 = 0
    for f in m.faces:
        sides += f.side + f.bound[3]
    print(f\"{m.faces[1].bound[2]} {m.faces[0].bound[2]} {m.normal[1]} {list[0].bound[1]} {list[1].bound[3]} {arr[1].bound[0]} {sides}\")
    unsafe:
        let p: *far mut Mesh = &mut m
        (*p).normal[0] = 2.5
        (*p).faces[0].bound[0] = 11
        print(f\"{(*p).normal[0]} {m.faces[0].bound[0]} {(*p).faces[1].bound[2]}\")
    return 0
";
    assert_eq!(output(source), "30 3 0.5 6 44 2 14\n2.5 11 30\n");
}

#[test]
fn an_array_field_of_owned_values_drops_copies_and_reassigns_each_element() {
    let source = "\
struct Roster:
    mut names: string[2]
    count: u8

fn main() -> i16:
    let mut r = Roster(names=[\"ada\", \"bob\"], count=2)
    r.names[1] = \"cy\"
    let c = r.copy()
    r.names = [\"x\", \"y\"]
    print(f\"{r.names[0]} {r.names[1]} {c.names[0]} {c.names[1]}\")
    return 0
";
    assert_eq!(output_without_leaks(source), "x y ada cy\n");
    assert!(refused(&source.replace("r.names = [\"x\", \"y\"]", "r.names = c.names")).contains("cannot move out of a borrow, field, or element"));
}

#[test]
fn a_constant_folds_from_constants_declared_in_any_order() {
    // `const GEOM_MAXREC: i32 = 20 + GEOM_MAXVTX * 6` before GEOM_MAXVTX: "GEOM_MAXREC is not a compile-time value".
    let source = "\
struct Row:
    cells: u8[WIDE]

const TOTAL: i32 = 20 + WIDE * 6
const WIDE = BASE + 1
const BASE = 2

fn main() -> i16:
    let r = Row(cells=[7] * WIDE)
    print(f\"{TOTAL} {r.cells.len}\")
    return 0
";
    assert_eq!(output(source), "38 3\n");
    assert!(refused(&source.replace("const BASE = 2", "const BASE = TOTAL")).contains("depends on itself"));
    assert!(refused(&source.replace("const BASE = 2", "const BASE = MISSING")).contains("BASE is not a compile-time value: MISSING is not a constant"));
}

#[test]
fn an_array_field_is_declared_for_c_and_assembler_and_refused_for_basic() {
    use super::declarations::{Language, declarations};
    let source = "@repr(\"c16\", pack=1)\nstruct Node:\n    id: i16\n    bound: u8[6]\n    grid: i16[2, 3]\n\nfn main() -> i16:\n    return 0\n";
    let module = super::parse(super::lex(source).expect("lexes")).expect("parses");
    let c = declarations(&module, "t", Language::C).expect("declares");
    assert!(c.contains("    unsigned char bound[6];\n    short grid[2][3];\n"), "{c}");
    let assembler = declarations(&module, "t", Language::Assembler).expect("declares");
    assert!(assembler.contains("    bound db 6 dup (?)\n    grid dw 6 dup (?)\n"), "{assembler}");
    assert!(declarations(&module, "t", Language::Basic).expect_err("refused").message.contains("\"bound\" has no declaration in BASIC"));
}

/// `source`'s refusal, its imports supplied by the compiler.
fn refused_with_imports(source: &str) -> String {
    let module = super::modules::load(source, &mut |name| Err(format!("{name} is not supplied"))).expect("loads");
    super::compile_module(module, "t", &Default::default()).expect_err("refused").message
}

#[test]
fn a_library_for_basic_refuses_the_nib_runtime_and_misplaced_adapters() {
    let library = "import abi.qb45 as qb\n\n@export(\"qb45\")\nfn Show(value: qb.Ref[i16]) -> void:\n    BODY\n";
    let printing = refused_with_imports(&library.replace("BODY", "print(value)"));
    assert!(printing.contains("Nib runtime (N$PI2)") && printing.contains("qb45"), "{printing}");
    // An index outside `unsafe:` is checked, and the check panics through the runtime.
    let checked = "import abi.qb45 as qb\n\n@export(\"qb45\")\nfn First(values: qb.ArrayRef[i16]) -> i16:\n    return values[0]\n";
    assert!(refused_with_imports(checked).contains("N$EBND"), "{}", refused_with_imports(checked));
    let other = library.replace("\"qb45\"", "\"pds71\"").replace("BODY", "return");
    assert!(refused_with_imports(&other).contains("qb45.Ref, which only a qb45 export or extern takes"), "{}", refused_with_imports(&other));
    let plain = "import abi.qb45 as qb\n\nfn show(value: qb.Ref[i16]) -> void:\n    return\n";
    assert!(refused_with_imports(plain).contains("only a qb45 export or extern"), "{}", refused_with_imports(plain));
    let text = "import abi.qb45 as qb\n\n@export(\"qb45\")\nfn Name(text: qb.StringRef) -> string:\n    return \"\"\n";
    assert!(refused_with_imports(text).contains("cannot cross to qb45"), "{}", refused_with_imports(text));
}

#[test]
fn qb45_exports_are_declared_as_basic_procedures() {
    use super::declarations::{Language, declarations};
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/basic/sortlib.nib")).expect("the example");
    let module = super::modules::load(&source, &mut |name| Err(format!("{name} is not supplied"))).expect("loads");
    let basic = declarations(&module, "sortlib", Language::Basic).expect("declares");
    assert!(basic.contains("DECLARE SUB SortScores (scores() AS INTEGER, count AS INTEGER)\nDECLARE SUB Upper (text AS STRING)\n"), "{basic}");
    assert!(basic.contains("DECLARE FUNCTION Average# (scores() AS INTEGER, count AS INTEGER)\n"), "{basic}");
    assert!(basic.contains("DECLARE FUNCTION Initials$ (person AS STRING)\n"), "{basic}");
    assert!(!basic.contains("TYPE"), "{basic}");
}

#[test]
fn a_module_variable_may_be_a_struct_its_literal_laid_out_in_the_executable() {
    // "a module variable cannot be a struct yet".
    let source = "struct Point:\n    mut x: i16\n    y: i16\n\nstruct Frame:\n    mut corner: Point\n    mut sizes: u8[3]\n\nvar home: Frame = Frame(corner=Point(x=1, y=-2), sizes=[4, 5, 6])\n\nfn grow() -> void:\n    home.corner.x += 10\n    home.sizes[1] = 9\n\nfn main() -> i16:\n    grow()\n    print(f\"{home.corner.x} {home.corner.y} {home.sizes[1]} {home.sizes[2]}\")\n    return 0\n";
    assert_eq!(output(source), "11 -2 9 6\n");
}

#[test]
fn a_struct_assigned_from_its_own_array_fields_reads_them_first() {
    // An array copy reads its cells when stored: `t = T(a=t.b, b=t.a)` printed "2 2".
    let source = "\
struct T:
    mut a: u8[3]
    mut b: u8[3]

fn main() -> i16:
    let mut t = T(a=[1, 1, 1], b=[2, 2, 2])
    t = T(a=t.b, b=t.a)
    print(f\"{t.a[0]} {t.b[0]}\")
    return 0
";
    assert_eq!(output(source), "2 1\n");
}

#[test]
fn inline_assembly_is_unsafe_names_what_it_touches_and_does_not_run_on_the_host() {
    let source = "\
fn main() -> i16:
    unsafe:
        asm(ax=1, out=(dx=let low), clobbers=[flags]):
            mov dx, ax
        return i16(low)
";
    let program = codec::decode(&super::compile(source, "t").expect("compiles")).expect("decodes");
    let error = execute::run(&program, "main", &[]).expect_err("machine code");
    assert!(error.0.contains("does not run on the host"), "{}", error.0);
    let refusals = [
        ("    unsafe:\n        asm", "    if true:\n        asm", "inline assembly is unsafe"),
        ("ax=1", "eax=1", "eax is not a register an input"),
        ("dx=let", "bp=let", "bp is not a register an input or output"),
        ("clobbers=[flags]", "clobbers=[ds]", "ds is not a register a block may clobber"),
        ("ax=1", "ax=1, ah=2", "ah overlaps another input"),
        ("ax=1", "ax=main", "ax takes a 16-bit integer"),
        ("mov dx, ax", "hlt", "'hlt' is not an instruction inline assembly supports"),
        ("mov dx, ax", "mov edx, eax", "register edx is not available"),
        ("mov dx, ax", "jmp nowhere", "unknown label 'nowhere'"),
    ];
    for (from, to, expected) in refusals {
        let refused = refused(&source.replace(from, to));
        assert!(refused.contains(expected), "{to}: {refused}");
    }
    let wrong = super::compile(&source.replace("mov dx, ax", "hlt"), "t").expect_err("refused");
    assert_eq!((wrong.span.line, wrong.span.column), (4, 13));
}

#[test]
fn a_refutable_let_on_a_temporary_moves_what_it_binds() {
    // Its binding borrowed from the temporary: "cannot move out of a borrow, field, or element".
    let source = "fn find(n: i16) -> Option[string]:\n    if n == 0:\n        return .none\n    return .some(f\"n{n}\")\n\nfn first(n: i16) -> string:\n    let .some(text) = find(n) else:\n        return \"none\"\n    return text\n\nfn main() -> i16:\n    print(first(0))\n    print(first(4))\n    return 0\n";
    assert_eq!(output(source), "none\nn4\n");
}

#[test]
fn a_public_fixed_point_type_is_named_by_its_importers() {
    // "a fixed-point type cannot be 'pub' yet".
    let geometry = "pub type Q16 = fixed i32, fraction=16\n\npub fn half(x: Q16) -> Q16:\n    return x / Q16(2)\n";
    let main = "import geometry\n\ntype Q8 = fixed i16, fraction=8\n\nfn main() -> i16:\n    let x = geometry.Q16(3.5)\n    let y: geometry.Q16 = geometry.half(x)\n    let z = Q8(y)\n    print(f\"{y} {z}\")\n    return 0\n";
    assert_eq!(linked_output(main, &[("geometry", geometry)]).expect("runs"), "1.75 1.75\n");
}

#[test]
fn size_of_is_a_types_bytes_as_laid_out_generic_or_not() {
    // Nothing gave a type's size, so a record could not be read or written as bytes.
    let source = "@repr(\"c16\", pack=1)\nstruct Node:\n    id: i16\n    bound: u8[6]\n\nfn bytes[T](count: u16) -> u16:\n    return size_of[T]() * count\n\nfn main() -> i16:\n    print(f\"{size_of[Node]()} {size_of[i32]()} {bytes[Node](3)}\")\n    return 0\n";
    assert_eq!(output(source), "8 4 24\n");
}

#[test]
fn a_variant_carries_a_fixed_array_that_matches_copies_and_drops() {
    // A variant field could not be an array: "a variant field cannot be an array yet".
    let source = "\
enum Message:
    quit
    packet(kind: u8, bytes: u8[4])

enum Entry:
    empty
    pair(names: string[2])

fn checksum(m: &Message) -> u16:
    match m:
        .packet(kind, bytes):
            let mut total: u16 = u16(kind)
            for b in bytes:
                total += u16(b)
            return total
        .quit:
            return 0

fn main() -> i16:
    let data: u8[4] = [1, 2, 3, 4]
    let m = Message.packet(kind=9, bytes=data)
    let copy = m
    print(f\"{checksum(&m)} {checksum(&copy)} {checksum(&Message.quit)}\")
    match copy:
        .packet(_, bytes):
            print(bytes[3])
        .quit:
            print(\"quit\")
    let e = Entry.pair(names=[\"a\" + \"b\", \"c\" + \"d\"])
    match e:
        .pair(names):
            print(names[1])
        .empty:
            print(\"empty\")
    return 0
";
    assert_eq!(output_without_leaks(source), "19 19 0\n4\ncd\n");
}

#[test]
fn a_tuple_holds_a_fixed_array_built_indexed_taken_apart_and_returned() {
    // A tuple element could not be an array: "a tuple element cannot be an array yet".
    let source = "\
fn extremes(values: &i16[4]) -> (i16[2], u16):
    let mut low = values[0]
    let mut high = values[0]
    for v in values:
        low = v < low ? v : low
        high = v > high ? v : high
    return ([low, high], values.len)

fn main() -> i16:
    let v: i16[4] = [5, -2, 9, 3]
    let t = extremes(&v)
    print(f\"{t[0][0]} {t[0][1]} {t[1]}\")
    let (range, count) = extremes(&v)
    print(f\"{range[1] - range[0]} {count}\")
    let pair: (u8[3], i16) = ([7, 8, 9], 1)
    let (bytes, n) = pair
    print(f\"{bytes[2]} {n} {pair[0].len}\")
    return 0
";
    assert_eq!(output(source), "-2 9 4\n11 4\n9 1 3\n");
}

#[test]
fn a_type_argument_may_be_a_fixed_array() {
    // `Option[u8[4]]` was "a type argument cannot be an array yet".
    let source = "\
struct Pair[T]:
    first: T
    second: T

fn head(o: Option[u8[4]]) -> u8:
    match o:
        .some(bytes):
            return bytes[0]
        .none:
            return 0

fn main() -> i16:
    let p: Pair[i16[2]] = Pair(first=[1, 2], second=[3, 4])
    print(f\"{p.first[1] + p.second[0]} {p.second.len}\")
    let o: Option[u8[4]] = .some([7, 0, 0, 0])
    print(f\"{head(o)} {head(.none)}\")
    return 0
";
    assert_eq!(output(source), "5 2\n7 0\n");
}

#[test]
fn patterns_bind_and_take_apart_array_fields() {
    // A struct pattern over an array field was "a pattern cannot take array field \"body\" yet".
    let source = "\
struct Frame:
    id: u8
    body: u8[3]

fn tag(frame: &Frame) -> u8:
    let Frame(id, body) = frame
    return id + body[2]

fn main() -> i16:
    let f = Frame(id=1, body=[4, 5, 6])
    let Frame(id, body) = f
    match f.body:
        [4, *rest]:
            print(f\"{rest.len} {rest[0]}\")
        _:
            print(\"other\")
    match f:
        Frame(_, [a, b, c]):
            print(a + b + c)
    print(f\"{id} {body[0]} {tag(&f)}\")
    return 0
";
    assert_eq!(output(source), "2 5\n15\n1 4 7\n");
}

#[test]
fn pop_moves_a_struct_out_of_its_vec() {
    // `v.pop()` on a `vec[Point]` was "pop() of a struct element is not supported yet".
    let source = "\
struct Point:
    x: i16
    y: i16

struct Named:
    name: string
    rank: i16

fn main() -> i16:
    let mut points: vec[Point] = [Point(x=1, y=2), Point(x=3, y=4)]
    let last = points.pop()
    print(f\"{last.x},{last.y} {points.len} {points.pop().x} {points.len}\")
    let mut names: vec[Named] = [Named(name=\"a\" + \"b\", rank=1), Named(name=\"c\" + \"d\", rank=2)]
    let n = names.pop()
    print(f\"{n.name} {n.rank} {names.len}\")
    return 0
";
    assert_eq!(output_without_leaks(source), "3,4 1 1 0\ncd 2 1\n");
}

#[test]
fn a_failure_is_returned_wrapped_in_the_one_variant_that_holds_its_type() {
    // "'?' cannot return this failure": each other module's error needed a match to wrap it.
    let source = "\
enum ReadError:
    missing
    failed(code: u16)

enum LoadError:
    read(error: ReadError)
    short(missing: u16)

fn fetch(n: u16) -> Result[u16, ReadError]:
    if n == 0:
        return .err(.failed(5))
    return .ok(n)

fn load(n: u16) -> Result[u16, LoadError]:
    let got = fetch(n)?
    if got < 4:
        return .err(.short(4 - got))
    return .ok(got)

fn describe(n: u16) -> string:
    match load(n):
        .ok(got):
            return f\"ok {got}\"
        .err(.read(.failed(code))):
            return f\"failed {code}\"
        .err(.read(.missing)):
            return \"missing\"
        .err(.short(missing)):
            return f\"short {missing}\"

fn main() -> i16:
    print(f\"{describe(0)}, {describe(1)}, {describe(9)}\")
    return 0
";
    assert_eq!(output(source), "failed 5, short 3, ok 9\n");
}

#[test]
fn a_local_array_owns_its_elements_and_moves_them_whole() {
    // Its elements were statement temporaries: "use of a dropped buffer".
    let source = "\
struct Named:
    name: string
    id: u8

fn wrap(n: u8) -> (string[2], u8):
    let names: string[2] = [\"w\" + \"x\", \"y\"]
    return (names, n)

fn main() -> i16:
    let a: string[2] = [\"a\" + \"b\", \"c\"]
    let x = a
    print(f\"{x[0]} {x[1]}\")
    let s = \"q\" + \"r\"
    let b: string[2] = [s, \"t\"]
    print(b[0])
    let t = wrap(3)
    print(f\"{t[0][0]} {t[1]}\")
    let people: Named[2] = [Named(name=\"p\" + \"q\", id=1), Named(name=\"z\", id=2)]
    let moved: Named[2] = people
    print(f\"{moved[0].name} {moved[1].id}\")
    return 0
";
    assert_eq!(output_without_leaks(source), "ab c\nqr\nwx 3\npq 2\n");
}

#[test]
fn a_pattern_on_a_temporary_moves_out_an_array_of_owned_values() {
    // It was refused: "an array of owned values moves only inside its owner; match it by reference".
    let source = "\
struct Entry:
    names: string[2]
    id: u8

enum Slot:
    empty
    pair(names: string[2])

fn entry(id: u8) -> Entry:
    return Entry(names=[\"e\" + \"f\", \"g\"], id=id)

fn slot() -> Slot:
    return Slot.pair(names=[\"h\" + \"i\", \"j\" + \"k\"])

fn main() -> i16:
    let Entry(names, id) = entry(5)
    print(f\"{names[0]} {names[1]} {id}\")
    match slot():
        .pair(kept):
            print(kept[1])
        .empty:
            print(\"empty\")
    match entry(6):
        Entry(_, n):
            print(n)
    return 0
";
    assert_eq!(output_without_leaks(source), "ef g 5\njk\n6\n");
}

#[test]
fn a_moved_array_of_values_with_drop_methods_drops_each_once_where_it_ends() {
    // Zeroing the moved one-word array projected its place whole: "projection exceeds place".
    let source = "\
struct R:
    id: u8

fn R.drop(self: &mut R) -> void:
    print(self.id)

fn keep(flag: bool) -> i16:
    let a: R[2] = [R(id=1), R(id=2)]
    if flag:
        let b = a
        print(\"moved\")
    print(\"end\")
    return 0

fn main() -> i16:
    keep(true)
    keep(false)
    return 0
";
    assert_eq!(output(source), "moved\n1\n2\nend\nend\n1\n2\n");
}

#[test]
fn a_slice_past_the_end_or_reversed_panics() {
    // Slice bounds were never checked: `&v[1:5]` of four elements gave a view of 4, `&v[3:1]` one of 65534.
    let run = |source: &str| {
        let hir = super::compile(source, "t").expect("compiles");
        let executed = execute::run(&codec::decode(&hir).expect("decodes"), "main", &[]).expect("runs");
        (executed.output, executed.panic)
    };
    let source = |slice: &str, a: u16, b: u16| {
        format!("fn head(v: &[i16], n: u16) -> &[i16]:\n    return &v[0:n]\n\nfn main() -> i16:\n    let table: i16[4] = [4, 5, 6, 7]\n    let vals: vec[i16] = [4, 5, 6, 7]\n    let s: string = \"abcd\"\n    let a: u16 = {a}\n    let b: u16 = {b}\n    let h = {slice}\n    print(h.len)\n    return 0\n")
    };
    for slice in ["head(&table, b)", "&vals[a:b]", "&s[a:b]", "&table[a:b]"] {
        let length = if slice.starts_with("head") { "4\n" } else { "3\n" };
        assert_eq!(run(&source(slice, 1, 4)), (length.to_owned(), None), "{slice}");
        assert_eq!(run(&source(slice, 1, 5)).1.as_deref(), Some("index out of bounds"), "{slice} past the end");
    }
    assert_eq!(run(&source("&vals[a:b]", 3, 1)).1.as_deref(), Some("index out of bounds"), "reversed");
    let checks = |source: &str| super::compile(source, "t").expect("compiles").matches(&format!("\"callee\":\"{}\"", crate::abi::nib::ERROR_BOUNDS)).count();
    let constant = "fn main() -> i16:\n    let table: i16[4] = [4, 5, 6, 7]\n    let h = &table[1:3]\n    print(h.len)\n    return 0\n";
    assert_eq!(checks(constant), 0);
    assert!(refused(&constant.replace("[1:3]", "[1:5]")).contains("outside"));
}

#[test]
fn a_fixed_array_slices_with_runtime_bounds() {
    // `&vals[0:n]` of a fixed array was refused: "this slice requires compile-time integer bounds".
    let source = "fn main() -> i16:\n    let vals: i16[4] = [4, 5, 6, 7]\n    let n: u16 = 3\n    let h = &vals[1:n]\n    print(f\"{h.len} {h[1]}\")\n    return 0\n";
    assert_eq!(output(source), "2 6\n");
}

#[test]
fn a_for_walks_a_slice_of_a_vec_or_string() {
    // `for x in &v[1:3]` over a vec or string was refused: "string and vec ranges are not in this slice".
    let source = "\
fn main() -> i16:
    let v: vec[i16] = [1, 2, 3, 4]
    let s: string = \"abcd\"
    let mut t: i16 = 0
    for x in &v[1:3]:
        t += x
    for ch in &s[1:3]:
        print(ch)
    print(t)
    return 0
";
    assert_eq!(output(source), "b\nc\n5\n");
}

#[test]
fn let_on_a_named_value_borrows_its_parts() {
    // `let (a, b) = pair` moved `pair`: a second `let` of it was "was moved".
    let source = "fn main() -> i16:\n    let pair = (\"a\" + \"b\", \"c\" + \"d\")\n    let (a, b) = pair\n    print(a)\n    let (c, d) = pair\n    print(d)\n    let (e, f) = (\"e\" + \"\", \"f\" + \"\")\n    print(e + f)\n    return 0\n";
    assert_eq!(output_without_leaks(source), "ab\ncd\nef\n");
    let moved = "fn main() -> i16:\n    let pair = (\"a\" + \"b\", 1)\n    let (a, n) = pair\n    let q = pair\n    print(a)\n    return 0\n";
    assert!(refused(moved).contains("\"pair\" is borrowed here"), "{}", refused(moved));
}

#[test]
fn a_borrowed_value_cannot_move() {
    // Moving a borrowed struct or viewed string compiled, and the borrow read freed memory.
    let borrowed = "struct P:\n    name: string\n\nfn main() -> i16:\n    let p = P(name=\"a\" + \"b\")\n    let r = &p\n    let q = p\n    print(r.name)\n    return 0\n";
    assert!(refused(borrowed).contains("\"p\" is borrowed here"), "{}", refused(borrowed));
    let viewed = "fn main() -> i16:\n    let s: string = \"abc\" + \"d\"\n    let v = &s[0:2]\n    let t = s\n    print(v)\n    print(t)\n    return 0\n";
    assert!(refused(viewed).contains("\"s\" is borrowed here"), "{}", refused(viewed));
}

#[test]
fn a_near_pointer_reaches_a_module_variable_its_fields_and_arrays() {
    // A field or array field was "near pointer reaches only static data", the whole struct a bad copy.
    let source = "struct Point:\n    mut x: i16\n    mut y: i16\n    mut bound: u8[4]\n\nvar origin: Point = Point(x=1, y=2, bound=[5, 6, 7, 8])\n\nfn main() -> i16:\n    unsafe:\n        let y: *near mut i16 = &mut origin.y\n        *y = 9\n        let b: *near u8 = &origin.bound\n        print(b[2])\n        let p: *near mut Point = &mut origin\n        (*p).x = 4\n    print(f\"{origin.x} {origin.y}\")\n    return 0\n";
    assert_eq!(output(source), "7\n4 9\n");
}

#[test]
fn a_type_parameter_is_inferred_through_a_borrowed_argument() {
    // "cannot infer T": a `&v` or `&l.xs` argument had no type to unify with `&vec[T]`.
    let source = "struct L:\n    xs: vec[i16]\n\nfn count[T](items: &vec[T]) -> u16:\n    return items.len\n\nfn main() -> i16:\n    let l = L(xs=[1, 2, 3])\n    let v: vec[i16] = [4]\n    print(f\"{count(&v)} {count(&l.xs)}\")\n    return 0\n";
    assert_eq!(output(source), "1 3\n");
}

#[test]
fn a_range_loop_may_ignore_its_counter() {
    // "a range binds one name": `for _ in 0..n` was refused.
    let source = "fn main() -> i16:\n    let mut dots: string = \"\"\n    for _ in 0..3:\n        dots.push('.')\n    print(dots)\n    return 0\n";
    assert_eq!(output(source), "...\n");
}

#[test]
fn logical_not_binds_looser_than_a_conditional() {
    // `!c ? a : b` grouped as `(!c) ? a : b`, printing true.
    let source = "\
fn main() -> i16:
    let c = true
    let a = true
    let b = true
    print(!c ? a : b)
    return 0
";
    assert_eq!(output(source), "false\n");
}

#[test]
fn a_chained_comparison_evaluates_any_middle_operand_once() {
    // A middle operand other than a name, literal, or field was refused.
    let source = "\
var calls: i16 = 0

fn middle(n: i16) -> i16:
    calls += 1
    return n

fn main() -> i16:
    let a: i16 = 1
    print(f\"{a < middle(2) < 3} {a < a + 1 <= 2} {calls}\")
    return 0
";
    assert_eq!(output(source), "true true 1\n");
}

#[test]
fn a_raw_pointer_compares_with_its_read_only_kind_and_with_zero() {
    // `*near mut T` beside `*near T`, or `0` beside any pointer, had no common type.
    let source = "\
var n: i16 = 4

fn main() -> i16:
    unsafe:
        let q: *near mut i16 = &mut n
        let r: *near i16 = q
        let far: *far u8 = 0
        print(f\"{r == q} {q != 0} {0 == far}\")
    return 0
";
    assert_eq!(output(source), "true true true\n");
}

#[test]
fn a_float_is_a_dictionary_key() {
    // A float had no `hash`: \"$key4\" is not an array or string.
    let source = "\
fn main() -> i16:
    let d: dict[f32, i16] = {1.5: 2, 0.0: 5}
    print(f\"{d[1.5]} {d[-0.0]}\")
    return 0
";
    assert_eq!(output(source), "2 5\n");
}

#[test]
fn an_enum_with_payloads_is_a_dictionary_key() {
    // An enum with payloads had no `hash`: \"$key4\" is not an array or string.
    let source = "\
enum K:
    a
    b(n: u8)

fn main() -> i16:
    let mut d: dict[K, i16] = {}
    d[K.a] = 1
    d[K.b(2)] = 2
    d[K.b(3)] = 3
    d[K.b(2)] = 4
    print(f\"{d.len} {d[K.b(2)]} {d[K.a]}\")
    return 0
";
    assert_eq!(output(source), "3 4 1\n");
}

#[test]
fn an_imported_constant_sizes_an_array_and_folds_like_a_local_one() {
    // `u8[sh.MAX]` and `const N = sh.MAX` were refused: the parser folded only its own module's constants.
    let shapes = "pub const MAX: u16 = 40\n";
    let main = "\
import geo.shapes as sh

const TWICE = sh.MAX * 2

fn main() -> i16:
    let row: u8[sh.MAX] = [0] * sh.MAX
    const N: u16 = sh.MAX + 1
    let wide: u8[N] = [0] * N
    let widest: u8[TWICE] = [0] * TWICE
    print(f\"{row.len} {wide.len} {widest.len}\")
    return 0
";
    assert_eq!(linked_output(main, &[("geo.shapes", shapes)]).unwrap(), "40 41 80\n");
}

#[test]
fn a_method_without_pub_is_private_to_its_module() {
    // `b.secret()` on another module's type ran: `pub` was checked on paths, not on a method called on a value.
    let shapes = "\
pub struct Box:
    w: u16

pub fn Box.area(self: &Box) -> u16:
    return self.secret() * 2

fn Box.secret(self: &Box) -> u16:
    return self.w
";
    let main = "import geo.shapes as sh\n\nfn main() -> i16:\n    let b = sh.Box(w=5)\n    print(b.area())\n    return 0\n";
    let files = [("geo.shapes", shapes)];
    assert_eq!(linked_output(main, &files).unwrap(), "10\n");
    let private = main.replace("b.area()", "b.secret()");
    assert!(linked_output(&private, &files).unwrap_err().contains("Box.secret is private to its module"));
}

#[test]
fn the_built_in_protocols_bound_a_type_parameter() {
    // `T: Ordered` was refused as an unknown protocol: the built-in protocols were never declared.
    let source = "\
struct Point:
    x: i16
    y: i16

fn Point.display(self: &Point) -> string:
    return f\"({self.x}, {self.y})\"

fn largest[T: Ordered](values: &[T]) -> T:
    let mut best = values[0]
    for value in values:
        if value.cmp(best) > 0:
            best = value
    return best

fn same[T: Hashable](a: &T, b: &T) -> bool:
    return a.eq(b) && a.hash() == b.hash()

fn shown[T: Display](value: &T) -> string:
    return value.display()

fn main() -> i16:
    let values: i16[3] = [3, 9, 2]
    let p = Point(x=1, y=2)
    print(f\"{largest(values)} {same(p, Point(x=1, y=2))} {shown(p)}\")
    return 0
";
    assert_eq!(output(source), "9 true (1, 2)\n");
    assert!(refused(&source.replace("shown(p)", "shown(values[0])")).contains("i16 is not a Display"));
}

#[test]
fn a_protocol_takes_type_parameters_that_its_bound_supplies() {
    // `protocol Source[T]:` did not parse, and no bound could name a protocol's type argument.
    let source = "\
protocol Source[T]:
    fn get(self: &Self) -> T

struct Seven:
    k: u8

fn Seven.get(self: &Seven) -> i16:
    return 7

fn read[S: Source[i16]](s: &S) -> i16:
    return s.get() + 1

fn main() -> i16:
    print(read(Seven(k=0)))
    return 0
";
    assert_eq!(output(source), "8\n");
    assert!(refused(&source.replace("Source[i16]]", "Source[u8]]")).contains("Seven is not a Source"));
    assert!(refused(&source.replace("Source[i16]]", "Source]")).contains("Source takes 1 type argument"));
}

#[test]
fn an_import_alias_cannot_be_shadowed() {
    // A local named like an import alias was accepted, hiding the module.
    let shapes = "pub fn area(w: i16) -> i16:\n    return w * w\n";
    for main in [
        "import shapes as geo\n\nfn main() -> i16:\n    let geo: i16 = 1\n    return geo\n",
        "import shapes as geo\n\nfn twice(geo: i16) -> i16:\n    return geo * 2\n\nfn main() -> i16:\n    return twice(2)\n",
        "import shapes\n\nfn main() -> i16:\n    for shapes in 0..2:\n        print(shapes)\n    return 0\n",
    ] {
        let error = linked_output(main, &[("shapes", shapes)]).expect_err("refused");
        assert!(error.contains("shadow"), "{error}");
    }
}

#[test]
fn a_float_divided_by_zero_is_inf_or_nan_as_on_the_x87() {
    // The host interpreter stopped with "float division by zero" where compiled code gets inf or nan.
    let source = "fn main() -> i16:\n    let z: f64 = 0.0\n    print(f\"{1.0 / z} {-1.0 / z} {z / z}\")\n    return 0\n";
    assert_eq!(output(source), "inf -inf nan\n");
}

#[test]
fn a_name_moved_before_continue_is_out_of_scope_in_the_next_iteration() {
    // Its own scope ends at the jump, yet it was "moved in one loop iteration and used in the next".
    let source = "fn take(s: string) -> void:\n    print(s)\n\nfn main() -> i16:\n    let mut n: i16 = 0\n    while n < 2:\n        n += 1\n        match n:\n            1:\n                let s: string = f\"x{n}\"\n                take(s)\n                continue\n            _:\n                print(\"other\")\n    return 0\n";
    assert_eq!(output_without_leaks(source), "x1\nother\n");
    let used = "fn take(s: string) -> void:\n    print(s)\n\nfn main() -> i16:\n    let s: string = f\"x{1}\"\n    let mut n: i16 = 0\n    while n < 2:\n        n += 1\n        take(s)\n        continue\n    return 0\n";
    assert!(refused(used).contains("moved in one loop iteration"), "{}", refused(used));
}

#[test]
fn an_escaping_generator_keeps_owned_strings_arrays_and_the_iterators_it_is_given() {
    // Was "cannot keep the parameter "text" yet", "cannot keep "word", a
    // string, yet", "cannot keep the array "window" yet", "cannot assign an
    // element of "window" yet", and a stage over an `iter[T]` parameter
    // "iterates a generator or a sequence".
    let source = "\
fn words(text: &string) -> iter[string]:
    let mut word: string = \"\"
    for c in text:
        if c == ' ':
            if word.len > 0:
                yield word
                word = \"\"
        else:
            word.push(c)
    if word.len > 0:
        yield word

fn longer(words: iter[string], least: u16) -> iter[string]:
    for word in words:
        if word.len >= least:
            yield word

fn sums(values: iter[u16]) -> iter[u16]:
    let mut window: u16[2] = [0, 0]
    let mut seen: u16 = 0
    for value in values:
        window[seen % 2] = value
        seen += 1
        yield window[0] + window[1]

fn lengths(words: iter[string]) -> iter[u16]:
    for word in words:
        yield word.len

fn main() -> i16:
    let text: string = \"a bb ccc dd e\"
    let mut long = longer(words(text), 2)
    for word in long:
        print(word)
    let mut totals = sums(lengths(words(text)))
    for total in totals:
        print(total)
    return 0
";
    assert_eq!(output_without_leaks(source), "bb\nccc\ndd\n1\n3\n5\n5\n3\n");
}

#[test]
fn an_escaping_generator_drops_what_it_holds_at_its_scope_or_its_own_end() {
    // A `with` was "cannot yield inside 'unsafe', 'with' or a destructuring
    // 'let' yet". A resource the frame holds is dropped once: where its scope
    // ends, or with the iterator; a generator given one holds it from the start.
    let source = "\
struct Log:
    name: string

fn Log.drop(self: &mut Log) -> void:
    print(f\"close {self.name}\")

fn lines(name: string, n: i16) -> iter[i16]:
    with log = Log(name=name):
        for i in 0..n:
            yield i
    print(\"after\")

fn doubled(values: iter[i16]) -> iter[i16]:
    for v in values:
        yield v * 2

fn main() -> i16:
    let mut all = lines(\"all\", 2)
    for x in all:
        print(x)
    let mut part = lines(\"part\", 5)
    let _ = part.next()
    let mut early = doubled(lines(\"early\", 5))
    for x in early:
        if x == 2:
            break
    print(\"end\")
    return 0
";
    assert_eq!(output_without_leaks(source), "0\n1\nclose all\nafter\nclose early\nend\nclose part\n");
}

#[test]
fn an_escaping_generator_resumes_inside_unsafe_and_destructuring() {
    // Was "cannot yield inside 'unsafe', 'with' or a destructuring 'let'
    // yet"; a destructuring `let` before a yield lost its names.
    let source = "\
struct P:
    x: i16
    y: i16

fn pairs(ps: &[P]) -> iter[i16]:
    for p in ps:
        let P(x, y) = p
        yield x
        yield y

fn first(values: &[i16]) -> iter[i16]:
    let [head, *rest] = values else:
        return
    yield head
    yield i16(rest.len)

fn bumped(n: i16) -> iter[i16]:
    let mut count: i16 = 0
    for i in 0..n:
        unsafe:
            let p: *far mut i16 = &mut count
            *p = *p + i
            yield *p

fn main() -> i16:
    let ps: P[2] = [P(x=1, y=2), P(x=3, y=4)]
    let mut g = pairs(ps)
    for v in g:
        print(v)
    let a: i16[3] = [7, 8, 9]
    let mut f = first(a)
    for v in f:
        print(v)
    let mut b = bumped(4)
    for v in b:
        print(v)
    return 0
";
    assert_eq!(output_without_leaks(source), "1\n2\n3\n4\n7\n2\n0\n1\n3\n6\n");
}

#[test]
fn an_escaping_generator_keeps_borrows_only_of_what_its_caller_lent_it() {
    // A `&mut` parameter was "cannot keep the parameter yet", a view bound
    // in a match arm "cannot keep "rest" yet", and a `return` ending an arm
    // "statement is unreachable".
    let source = "\
struct C:
    mut n: i16

fn ticks(c: &mut C, n: i16) -> iter[i16]:
    for i in 0..n:
        c.n += 1
        yield c.n

fn scaled(xs: &mut [i16]) -> iter[i16]:
    for i in 0..xs.len:
        xs[i] = xs[i] * 2
        yield xs[i]

fn tails(xs: &[i16]) -> iter[u16]:
    let mut rest = xs
    loop:
        match rest:
            [_, *tail]:
                yield tail.len
                rest = tail
            []:
                return

fn tagged(xs: &[i16]) -> iter[string]:
    for (i, x) in enumerate(xs):
        yield f\"{i}={x}\"

fn main() -> i16:
    let mut c = C(n=0)
    let mut t = ticks(c, 2)
    for v in t:
        print(v)
    print(c.n)
    let mut a: i16[3] = [1, 2, 3]
    let mut s = scaled(a)
    for v in s:
        print(v)
    let mut r = tails(a)
    for n in r:
        print(n)
    let mut g = tagged(a)
    for text in g:
        print(text)
    return 0
";
    assert_eq!(output_without_leaks(source), "1\n2\n2\n2\n4\n6\n2\n1\n0\n0=2\n1=4\n2=6\n");
    let own = "fn window() -> iter[i16]:\n    let a: i16[3] = [1, 2, 3]\n    let w = &a[0:2]\n    yield w[0]\n\nfn main() -> i16:\n    let mut g = window()\n    return 0\n";
    assert!(refused(own).contains("keeps only borrows of what its caller lent it; \"w\" borrows its own \"a\""), "{}", refused(own));
    let changed = "fn chars(text: &string) -> iter[char]:\n    for c in text:\n        yield c\n\nfn main() -> i16:\n    let mut t: string = \"ab\"\n    let mut g = chars(t)\n    t = \"zz\"\n    for c in g:\n        print(c)\n    return 0\n";
    assert!(refused(changed).contains("\"t\" is borrowed here"), "{}", refused(changed));
}

#[test]
fn a_generator_expression_that_escapes_is_a_generator_of_its_own() {
    // Stored, it was "a generator is non-escaping and must be consumed by a
    // for loop"; passed to an `iter[T]` parameter, "cannot infer $Iterator0".
    let stored = "fn main() -> i16:\n    let v: vec[i16] = [1, 2, 3]\n    let g = (x * 2 for x in v)\n    for y in g:\n        print(y)\n    return 0\n";
    assert_eq!(output_without_leaks(stored), "2\n4\n6\n");
    let passed = "fn total(values: iter[i16]) -> i16:\n    let mut t: i16 = 0\n    for v in values:\n        t += v\n    return t\n\nfn main() -> i16:\n    let v: vec[i16] = [1, 2]\n    let step: i16 = 1\n    print(total((x + step for x in v if x > 1)))\n    return 0\n";
    assert_eq!(output_without_leaks(passed), "3\n");
}

#[test]
fn a_split_for_evaluates_what_it_iterates_once() {
    // `make(n)` was called again at every step.
    let source = "\
fn make(n: i16) -> vec[i16]:
    print(\"make\")
    let mut v: vec[i16] = []
    for i in 0..n:
        v.push(i * 3)
    return v

fn each(n: i16) -> iter[i16]:
    for x in make(n):
        yield x

fn main() -> i16:
    let mut g = each(3)
    for x in g:
        print(x)
    return 0
";
    assert_eq!(output_without_leaks(source), "make\n0\n3\n6\n");
}

#[test]
fn a_reference_field_read_as_a_value_reads_what_it_refers_to() {
    // `s.r + 1` was "expected *far pointer, found i16".
    let source = "struct S:\n    r: &i16\n\nfn main() -> i16:\n    let x: i16 = 4\n    let s = S(r=&x)\n    print(s.r + 1)\n    return 0\n";
    assert_eq!(output(source), "5\n");
}

#[test]
fn the_pipeline_example_streams_words_through_escaping_stages() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/pipeline.nib")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "skipped x\nmean 12\nmean 18\nmean 23\nskipped err\nmean 28\nmean 21\nmean 15\nclosed feed\nskipped x\nalarm at 23\nclosed alarm\ndone\n"
    );
}

#[test]
fn a_function_returns_a_fixed_array_in_registers_or_through_the_slot() {
    // It was refused: "a function cannot return an array".
    let source = "\
struct Point:
    x: i16
    y: i16

struct Holder:
    values: i16[4]
    tag: u8

fn corners(k: i16) -> i16[4]:
    return [k, -2 * k, 3 * k, -4 * k]

fn word(first: u8) -> char[3]:
    return [char(first), char(first + 1), char(first + 2)]

fn path(dx: i16) -> Point[3]:
    let ps: Point[3] = [Point(x=0, y=0), Point(x=dx, y=1), Point(x=2 * dx, y=2)]
    return ps

fn names(n: u8) -> string[2]:
    return [f\"n{n}\", \"fixed\"]

fn twice[T](value: T) -> T[2]:
    return [value, value]

fn sum(values: &i16[4]) -> i16:
    let mut total: i16 = 0
    for v in values:
        total += v
    return total

fn main() -> i16:
    let c = corners(1)
    let mut w = word(97)
    print(f\"{c[1]} {w[0]}{w[2]}\")
    w = word(120)
    let h = Holder(values=corners(2), tag=1)
    print(f\"{w[1]} {h.values[2]} {sum(corners(3))}\")
    let p = path(5)
    print(f\"{p[2].x} {p[1].y}\")
    let ns = names(7)
    let mut kept: string[2] = names(8)
    kept = names(9)
    print(f\"{ns[0]} {ns[1]} {kept[0]}\")
    let b: u8 = 6
    let pair = twice(b)
    print(pair[0] + pair[1])
    return 0
";
    assert_eq!(output_without_leaks(source), "-2 ac\ny 6 -6\n10 1\nn7 fixed n9\n12\n");
}

#[test]
fn an_aggregate_result_called_for_its_effect_drops_what_it_owns() {
    // The result's temporary was never dropped: "1 heap buffers leaked", per call.
    let source = "\
struct Named:
    name: string

fn named(n: u8) -> Named:
    return Named(name=f\"n{n}\")

fn names(n: u8) -> string[2]:
    return [f\"n{n}\", \"fixed\"]

fn main() -> i16:
    named(3)
    names(4)
    print(\"done\")
    return 0
";
    assert_eq!(output_without_leaks(source), "done\n");
}

#[test]
fn a_sequence_pattern_on_a_temporary_array_moves_its_elements_out() {
    // The temporary was borrowed and never dropped: "2 heap buffers leaked", and `let [a, b]` needed 'else:'.
    let source = "\
struct Tag:
    label: string
    id: u8

fn names(n: u8) -> string[3]:
    return [f\"n{n}\", \"x\" + \"y\", f\"z{n}\"]

fn tags() -> Tag[2]:
    return [Tag(label=\"a\" + \"b\", id=1), Tag(label=\"c\" + \"d\", id=2)]

fn corners(k: i16) -> i16[4]:
    return [k, 2 * k, 3 * k, 4 * k]

fn main() -> i16:
    let [first, *_, last] = names(1)
    print(f\"{first} {last}\")
    let [a, b, c] = names(2)
    print(f\"{a} {b} {c}\")
    match tags():
        [Tag(label, _), second]:
            print(f\"{label} {second.id}\")
    let [low, *rest] = corners(1)
    print(f\"{low} {rest[2]} {rest.len}\")
    return 0
";
    assert_eq!(output_without_leaks(source), "n1 z1\nn2 xy z2\nab 2\n1 4 3\n");
    let starred = source.replace("[first, *_, last]", "[first, *more]").replace("{first} {last}", "{first}");
    assert!(refused(&starred).contains("section 6"));
}

#[test]
fn a_borrowed_fixed_array_parameter_keeps_its_length() {
    // `&T[N]` was a slice view that lost N: `return values` as `-> i16[4]` gave "expected an array of dimensions [4]".
    let source = "\
fn total(values: &i16[4]) -> i16:
    let mut sum: i16 = 0
    for v in values:
        sum += v
    return sum

fn passed(values: &i16[4]) -> i16:
    return total(values) + values[3] + i16(values.len)

fn same(values: &i16[4]) -> &i16[4]:
    return values

fn copied(values: &i16[4]) -> i16[4]:
    return values

fn bump(values: &mut i16[4]) -> void:
    values[0] += 10

fn head(values: &[i16]) -> i16:
    return values[0]

fn main() -> i16:
    let mut v: i16[4] = [1, 2, 3, 4]
    let c = copied(&v)
    bump(&mut v)
    let r = same(&v)
    print(f\"{total(&v)} {passed(&v)} {r[0]} {c[0]} {head(&v)}\")
    return 0
";
    assert_eq!(output(source), "20 28 11 1 11\n");
    assert!(refused(&source.replace("values[3] +", "values[4] +")).contains("array index 4 is outside 0..4"));
    let wider = source.replace("let mut v: i16[4] = [1, 2, 3, 4]", "let mut v: i16[5] = [1, 2, 3, 4, 5]");
    assert!(refused(&wider).contains("wrong type"));
}

#[test]
fn a_borrowed_fixed_array_from_any_expression_is_used_as_the_parameter_is() {
    // `same(x)[2]` on a `-> &i16[3]` result: "a *far pointer is not a sequence".
    let source = "\
fn same(a: &i16[3]) -> &i16[3]:
    return a

fn paired(a: &i16[3]) -> (&i16[3], u8):
    return (a, 7)

fn head(values: &[i16]) -> i16:
    return values[0]

fn main() -> i16:
    let x: i16[3] = [1, 2, 3]
    print(same(x)[2])
    let r = same(x)
    let t = paired(x)
    let mut sum: i16 = 0
    for v in same(x):
        sum += v
    print(f\"{r[0]} {r.len} {same(x).len} {t[0][1]} {t[0].len} {sum} {head(&same(x)[1:3])}\")
    return 0
";
    assert_eq!(output(source), "3\n1 3 3 2 3 6 2\n");
}

#[test]
fn a_function_returning_an_iterator_hands_over_the_one_it_returns() {
    // `return upto(n)` from an `iter[T]` function was "a generator returns no value".
    let source = "\
fn upto(n: i16) -> iter[i16]:
    for i in 0..n:
        yield i

fn make(n: i16) -> iter[i16]:
    return upto(n)

fn evens(v: &vec[i16]) -> iter[i16]:
    return (x for x in v if x % 2 == 0)

fn after_first(values: iter[i16]) -> iter[i16]:
    let mut rest = values
    let _ = rest.next()
    return rest

fn either(n: i16, low: bool) -> iter[i16]:
    yield -1
    if low:
        return upto(n)
    return (x * 10 for x in 0..n)

fn main() -> i16:
    let mut kept = make(2)
    for x in kept:
        print(x)
    let v: vec[i16] = [1, 2, 4]
    for x in evens(v):
        print(x)
    for x in after_first(make(3)):
        print(x)
    let mut high = either(2, false)
    for x in high:
        print(x)
    return 0
";
    assert_eq!(output_without_leaks(source), "0\n1\n2\n4\n1\n2\n-1\n0\n10\n");
}

#[test]
fn a_move_in_a_generator_body_is_checked_however_the_body_is_compiled() {
    // Stored, not consumed in place, the moved "s" was read as empty: the
    // host run failed with "N$PS of null".
    let generator = "fn take(s: string) -> void:\n    print(s)\n\nfn bad() -> iter[i16]:\n    let s: string = \"a\"\n    take(s)\n    yield 1\n    print(s)\n\n";
    let stored = format!("{generator}fn main() -> i16:\n    let mut g = bad()\n    let _ = g.next()\n    return 0\n");
    let consumed = format!("{generator}fn main() -> i16:\n    for x in bad():\n        print(x)\n    return 0\n");
    for source in [stored, consumed] {
        assert!(refused(&source).contains("\"s\" was moved"), "{}", refused(&source));
    }
}

#[test]
fn an_escaping_generator_keeps_each_binding_of_a_name_apart() {
    // Two loops binding "x", one over strings, was "keeps one "x"; rename this one".
    let source = "\
fn both(words: &vec[string], n: i16) -> iter[string]:
    for x in words:
        yield x.copy()
    for x in 0..n:
        let n = x * 10
        yield f\"{n}\"
    match n:
        1:
            let x: i16 = 7
            yield f\"{x}\"
        _:
            let x: string = \"other\"
            yield x

fn main() -> i16:
    let words: vec[string] = [\"a\", \"b\"]
    let mut g = both(words, 1)
    for s in g:
        print(s)
    return 0
";
    assert_eq!(output_without_leaks(source), "a\nb\n0\n7\n");
}

#[test]
fn an_escaping_generator_keeps_a_local_whose_type_its_value_gives() {
    // An f-string, a reference and a vec literal were each "write the type of
    // ...: a generator that escapes keeps it".
    let source = "\
struct C:
    mut n: i16

fn counts(c: &mut C, n: i16) -> iter[string]:
    let label = f\"n{n}\"
    let total = &mut c.n
    let steps = [1, 2]
    for step in steps:
        total += step
        yield f\"{label} {total}\"

fn main() -> i16:
    let mut c = C(n=0)
    let mut g = counts(c, 5)
    for s in g:
        print(s)
    print(c.n)
    return 0
";
    assert_eq!(output_without_leaks(source), "n5 1\nn5 3\n3\n");
    let own = "fn gen(x: i16) -> iter[i16]:\n    let mut y: i16 = x\n    let r = &mut y\n    yield 1\n    r += 1\n    yield y\n\nfn main() -> i16:\n    let mut g = gen(1)\n    return 0\n";
    assert!(refused(own).contains("keeps only borrows of what its caller lent it; \"r\" borrows its own \"y\""), "{}", refused(own));
}
