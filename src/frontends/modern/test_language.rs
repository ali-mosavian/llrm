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
fn enums_match_exhaustively_on_tags_and_payloads() {
    let source = include_str!("../../../docs/examples/shapes.mod");
    assert_eq!(
        output(source),
        "area 87\nmode 19 has 320 columns\ntext is 80\n"
    );
    let partial = source.replace("        .cga:\n            return 40\n", "");
    assert!(refused(&partial).contains("does not cover .cga"));
}

#[test]
fn question_mark_returns_the_failure_and_nested_patterns_cover_every_error() {
    let source = include_str!("../../../docs/examples/digits.mod");
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
    let source = include_str!("../../../docs/examples/greeting.mod");
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
    let hir = super::compile_module(module, "t").map_err(|error| error.message)?;
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
    let source = "\
@repr(\"c16\", pack=1)
struct Packet:
    kind: u8
    length: i16

struct Loose:
    kind: u8
    length: i16

extern \"cdecl16\":
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
        "/docs/examples/settings.mod"
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
    let checks = |hir: &str| hir.matches("\"callee\":\"_rt_panic_bounds\"").count();
    assert_eq!(checks(&hir) - checks(&super::compile(&unchecked, "t").expect("compiles")), 1);
}

#[test]
fn a_for_takes_an_iterator_from_iter_and_calls_next_until_none() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/dice.mod")).expect("the example");
    assert_eq!(output_without_leaks(&source), "1: 10\n2: 14\n3:  8\n4:  8\n5: 13\n6:  7\nfirst six\n");
    assert!(refused(&source.replace("for face in dice:", "for face in &mut dice:")).contains("an iterator yields values"));
}

#[test]
fn a_view_result_borrows_what_the_caller_lent_and_never_a_local() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/csv.mod")).expect("the example");
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
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/pascal/levels.mod")).expect("the example");
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
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/sequences.mod")).expect("the example");
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
        output_without_leaks(include_str!("../../../docs/examples/meter.mod")),
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
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/laps.mod")).expect("the example");
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
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/gradebook.mod")).expect("the example");
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
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/generic_types.mod")).expect("the example");
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
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/borrows.mod")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "leader bob\nbest di with 58\nfirst cy\n[move]\n[north]\n[then]\n[east]\n"
    );
}

#[test]
fn dicts_hash_their_keys_grow_and_lend_a_looked_up_key() {
    // `dict[K, V]` was "unknown type"; a lookup by a borrowed string moved
    // it, and `&s` had no `.hash()`: "array has no method".
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/tally.mod")).expect("the example");
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
    // outer one was building: "_rt_end without _rt_begin".
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
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/roster.mod")).expect("the example");
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
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/examples/easing.mod")).expect("the example");
    assert_eq!(
        output_without_leaks(&source),
        "linear    0   8  16  24  32  40\nin        0   1   6  14  25  40\nout       0  14  25  33  38  40\nsmooth    0   3  12  27  36  40\nstep      0   0   0  20  20  40\n"
    );
}
