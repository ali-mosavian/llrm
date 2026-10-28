//! `runIPSCCP` over one-module programs. Each expected text is what the pin's
//! `opt -passes=ipsccp` makes of the same module.

use std::collections::BTreeMap;
use std::rc::Rc;

use llrm_mir::constantrange::ConstantRange;
use llrm_mir::module::Module;
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::program::Program;

use super::run_ipsccp;
use crate::testing::{parsed, printed};

/// `text` after `run_ipsccp`.
fn ipsccp(text: &str) -> String {
    let mut module = parsed(text);
    Program::lend(&mut module, Rc::new(llrm_mir::target::Neutral), run_ipsccp).expect("a program");
    printed(&module)
}

/// The line of `text` that defines `@name`.
fn header<'t>(
    text: &'t str,
    name: &str,
) -> &'t str {
    let at = format!("@{name}(");
    text.lines().find(|line| line.starts_with("define") && line.contains(&at)).unwrap_or_default()
}

/// The body of `@name` in `text`.
fn body<'t>(
    text: &'t str,
    name: &str,
) -> &'t str {
    let start = text.find(header(text, name)).expect("the function");
    let end = text[start..].find("\n}").map_or(text.len(), |one| start + one);
    &text[start..end]
}

#[test]
fn test_a_callee_called_with_1_and_3_gets_the_range_1_to_4() {
    let text = ipsccp(
        "define internal i32 @g(i32 %a) {
b0:
  %m = mul i32 %a, 5
  ret i32 %m
}

define i32 @f() {
b0:
  %x = call i32 @g(i32 1)
  %y = call i32 @g(i32 3)
  %s = add i32 %x, %y
  ret i32 %s
}
",
    );
    let g = header(&text, "g");
    assert!(g.contains("range(i32 1, 4) %a"), "{text}");
    assert!(g.contains("range(i32 5, 16) i32 @g"), "{text}");
    assert!(header(&text, "f").contains("range(i32 10, 31)"), "{text}");
    // A range is not a constant: the returns stay.
    assert!(body(&text, "g").contains("ret i32 %m"), "{text}");
}

#[test]
fn test_a_constant_every_caller_passes_replaces_the_argument() {
    let text = ipsccp(
        "declare void @use(i32)

define internal void @g(i32 %a, i32 %b) {
b0:
  %m = mul i32 %a, %b
  call void @use(i32 %m)
  ret void
}

define void @f(i32 %v) {
b0:
  call void @g(i32 5, i32 %v)
  call void @g(i32 5, i32 2)
  ret void
}
",
    );
    assert!(body(&text, "g").contains("%m = mul i32 5, %b"), "{text}");
}

/// Outside code may call an exported function, and an address-taken one
/// through its address: neither argument is the join of the calls seen.
#[test]
fn test_an_exported_or_address_taken_functions_arguments_are_untouched() {
    let text = ipsccp(
        "declare void @use(i32)

@slot = global ptr @h

define void @g(i32 %a) {
b0:
  call void @use(i32 %a)
  ret void
}

define internal void @h(i32 %a) {
b0:
  call void @use(i32 %a)
  ret void
}

define void @f() {
b0:
  call void @g(i32 1)
  call void @h(i32 1)
  ret void
}
",
    );
    for name in ["g", "h"] {
        assert!(body(&text, name).contains("call void @use(i32 %a)"), "@{name}: {text}");
        assert!(!header(&text, name).contains("range"), "@{name}: {text}");
    }
}

/// `@g` returns 7 on every path: the caller reads 7 and `@g`'s returns are
/// zapped to `poison`; `@g`'s calls keep their arguments.
#[test]
fn test_a_tracked_constant_return_reaches_the_caller_and_is_zapped() {
    let text = ipsccp(
        "define internal noundef i32 @g(i32 %a) {
b0:
  %c = icmp eq i32 %a, 0
  br i1 %c, label %b1, label %b2

b1:
  ret i32 7

b2:
  ret i32 7
}

define i32 @f(i32 %v) {
b0:
  %x = call noundef i32 @g(i32 %v)
  %y = add i32 %x, 1
  ret i32 %y
}
",
    );
    assert!(body(&text, "f").contains("ret i32 8"), "{text}");
    let g = body(&text, "g");
    assert_eq!(g.matches("ret i32 poison").count(), 2, "{text}");
    assert!(!header(&text, "g").contains("noundef"), "{text}");
    assert!(body(&text, "f").contains("call i32 @g(i32 %v)"), "{text}");
}

/// An exported function's constant return reaches its callers in the
/// module, but outside code calls it too: its returns stay.
#[test]
fn test_an_exported_functions_return_is_read_but_not_zapped() {
    let text = ipsccp(
        "define i32 @g() {
b0:
  ret i32 7
}

define i32 @f() {
b0:
  %x = call i32 @g()
  %y = add i32 %x, 1
  ret i32 %y
}
",
    );
    assert!(body(&text, "f").contains("ret i32 8"), "{text}");
    assert!(body(&text, "g").contains("ret i32 7"), "{text}");
}

/// Every store to `@gv` writes its initializer 4: the load reads 4, and the
/// stores go.
#[test]
fn test_a_global_every_store_writes_one_value_folds_its_loads() {
    let text = ipsccp(
        "@gv = internal global i32 4

define void @set() {
b0:
  store i32 4, ptr @gv
  ret void
}

define i32 @get() {
b0:
  %v = load i32, ptr @gv
  %w = add i32 %v, 1
  ret i32 %w
}
",
    );
    assert!(body(&text, "get").contains("ret i32 5"), "{text}");
    assert!(!body(&text, "set").contains("store"), "{text}");
    assert!(!body(&text, "get").contains("load"), "{text}");
}

/// A global stored two values stays as it is.
#[test]
fn test_a_global_stored_two_values_keeps_its_loads_and_stores() {
    let text = ipsccp(
        "@gv = internal global i32 4

define void @set(i1 %c) {
b0:
  store i32 5, ptr @gv
  ret void
}

define i32 @get() {
b0:
  %v = load i32, ptr @gv
  ret i32 %v
}
",
    );
    assert!(body(&text, "set").contains("store i32 5, ptr @gv"), "{text}");
    assert!(body(&text, "get").contains("load i32, ptr @gv"), "{text}");
}

/// AM-BD-6: the counted loop's `%r` reaches `@sink` through the phi and the
/// edge `%r < %row`, `%row` in [0, 8): the pin states `range(i16 -32768, 6)`,
/// the stand-in `parameter_ranges` stated [-32768, 32767).
pub(crate) const AM_BD_6: &str = "define internal i16 @sink(i16 %x) {
b:
  %y = add i16 %x, 1
  ret i16 %y
}

define internal i16 @place(i16 %row, i16 %n) {
b:
  %done = icmp eq i16 %row, %n
  br i1 %done, label %leaf, label %scan
scan:
  br label %outer
outer:
  %c = phi i16 [ 0, %scan ], [ %c1, %latch ]
  %more = icmp slt i16 %c, %n
  br i1 %more, label %pre, label %next
pre:
  br label %head
head:
  %r = phi i16 [ 0, %pre ], [ %r1, %body ]
  %go = icmp slt i16 %r, %row
  br i1 %go, label %body, label %latch
body:
  %s = call i16 @sink(i16 %r)
  %r1 = add nsw i16 %r, 1
  br label %head
latch:
  %c1 = add nsw i16 %c, 1
  br label %outer
next:
  %up = add nsw i16 %row, 1
  %p = call i16 @place(i16 %up, i16 %n)
  ret i16 %p
leaf:
  ret i16 1
}

define i16 @f(i16 %a) {
b:
  %x = call i16 @place(i16 0, i16 7)
  ret i16 %x
}
";

#[test]
fn test_a_counted_loops_phi_range_reaches_the_callee_argument() {
    let text = ipsccp(AM_BD_6);
    assert!(header(&text, "sink").contains("range(i16 -32768, 6) %x"), "{text}");
    assert!(header(&text, "sink").contains("range(i16 -32767, 7) i16 @sink"), "{text}");
    assert!(header(&text, "place").contains("range(i16 0, 8) %row"), "{text}");
    let place = body(&text, "place");
    assert!(place.contains("icmp eq i16 %row, 7") && place.contains("call i16 @place(i16 %up, i16 7)"), "{text}");
    assert_eq!(place.matches("ret i16 poison").count(), 2, "{text}");
    assert!(body(&text, "f").contains("ret i16 1"), "{text}");
}

/// What a pass states of one parameter that has users.
#[derive(Clone, Debug, PartialEq)]
enum Stated {
    Nothing,
    /// Replaced by a constant.
    Constant,
    /// A `range` attribute it added or changed.
    Range(ConstantRange),
    /// The body never runs (the pin's `unreachable` entry).
    Dead,
}

fn ranges_of(module: &Module) -> BTreeMap<(String, usize), (bool, Option<ConstantRange>, bool)> {
    let mut out = BTreeMap::new();
    for (_, global, function) in module.functions() {
        if function.is_declaration() {
            continue;
        }
        let name = global.name.as_deref().unwrap_or_default().to_owned();
        let dead = function.layout().len() == 1
            && function.iter_block(function.layout()[0]).count() == 1
            && function.instruction(function.iter_block(function.layout()[0]).next().expect("one")).opcode
                == Opcode::Unreachable;
        for (index, &parameter) in function.parameters().iter().enumerate() {
            let range = function.parameter_attrs.get(index).and_then(|attrs| {
                attrs.iter().find_map(|one| match one {
                    Attribute::Range { ty, lower, upper } => {
                        Some(ConstantRange::new(module.context.types.int_bits(*ty).expect("an int"), *lower, *upper))
                    }
                    _ => None,
                })
            });
            out.insert((name.clone(), index), (!function.users(parameter).is_empty(), range, dead));
        }
    }
    out
}

fn stated(
    before: &BTreeMap<(String, usize), (bool, Option<ConstantRange>, bool)>,
    after: &BTreeMap<(String, usize), (bool, Option<ConstantRange>, bool)>,
) -> BTreeMap<(String, usize), Stated> {
    let mut out = BTreeMap::new();
    for (key, &(used, ref range, _)) in before {
        if !used {
            continue;
        }
        let Some((used_after, range_after, dead)) = after.get(key) else { continue };
        out.insert(
            key.clone(),
            if *dead {
                Stated::Dead
            } else if !used_after {
                Stated::Constant
            } else if range_after.is_some() && range_after != range {
                Stated::Range(range_after.expect("a range"))
            } else {
                Stated::Nothing
            },
        );
    }
    out
}

/// This port's statement of each used parameter over every `.ll` in
/// `$IPSCCP_DUMPS` (the module each corpus file has when `ipo-early` starts,
/// `LLRM_DUMP_INPUTS=1 llrm-c --dump`) against the pin's `opt -passes=ipsccp`
/// of the same files in `$IPSCCP_PIN`: each parameter where the two differ,
/// and the totals.
#[test]
#[ignore = "reads $IPSCCP_DUMPS and $IPSCCP_PIN"]
fn corpus_against_the_pin() {
    let directory = std::env::var("IPSCCP_DUMPS").expect("IPSCCP_DUMPS");
    let pin = std::env::var("IPSCCP_PIN").expect("IPSCCP_PIN");
    let mut against_pin: BTreeMap<bool, usize> = BTreeMap::new();
    let mut files: Vec<_> = std::fs::read_dir(&directory)
        .expect("a directory")
        .filter_map(|one| one.ok().map(|one| one.path()))
        .filter(|one| one.extension().is_some_and(|ext| ext == "ll"))
        .collect();
    files.sort();
    for file in files {
        let text = std::fs::read_to_string(&file).expect("a dump");
        let Ok(module) = llrm_mir::parse::module(&text) else {
            println!("{}: does not parse", file.display());
            continue;
        };
        let stem = file.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let path = std::path::Path::new(&pin).join(file.file_name().expect("a file"));
        let Some(theirs) = std::fs::read_to_string(&path).ok().and_then(|text| llrm_mir::parse::module(&text).ok())
        else {
            println!("{stem}: no pin output that parses");
            continue;
        };
        let before = ranges_of(&module);
        let mut ours = module.clone();
        Program::lend(&mut ours, Rc::new(llrm_mir::target::Neutral), run_ipsccp).expect("a program");
        let ours = stated(&before, &ranges_of(&ours));
        let real = stated(&before, &ranges_of(&theirs));
        for (key, ours) in &ours {
            let real = real.get(key).cloned().unwrap_or(Stated::Nothing);
            *against_pin.entry(*ours == real).or_default() += 1;
            if *ours != real {
                println!("{stem} @{} #{}: PORT {ours:?} PIN {real:?}", key.0, key.1);
            }
        }
    }
    println!("port equal to the pin's opt: {against_pin:?}");
}

/// queens' `place(row, n)` recurses with `row + 1` and its own `n`: the one
/// outside call passes 7, so every call does, and `n` is 7 inside.
fn recursive(
    first: &str,
    second: &str,
) -> String {
    format!(
        "define internal i16 @place(i16 %row, i16 %n) {{
b:
  %done = icmp eq i16 %row, %n
  br i1 %done, label %leaf, label %more
more:
  %next = add i16 %row, 1
  %r = call i16 @place({first})
  %s = add i16 %r, %row
  ret i16 %s
leaf:
  ret i16 1
}}

define i16 @f(i16 %a) {{
b:
  %x = call i16 @place(i16 0, i16 7)
{second}  ret i16 %x
}}
"
    )
}

#[test]
fn test_a_recursive_call_passing_a_parameter_on_leaves_the_others_actuals_agreed() {
    let text = ipsccp(&recursive("i16 %next, i16 %n", ""));
    assert!(body(&text, "place").contains("icmp eq i16 %row, 7"), "{text}");
}

/// A second outside call with another value, or the recursion passing the
/// parameters swapped, keeps `n` unknown.
#[test]
fn test_another_actual_for_the_parameter_keeps_it_unknown() {
    for text in
        [recursive("i16 %next, i16 %n", "  %y = call i16 @place(i16 0, i16 %a)\n"), recursive("i16 %n, i16 %next", "")]
    {
        let after = ipsccp(&text);
        assert!(body(&after, "place").contains("icmp eq i16 %row, %n"), "{after}");
    }
}

/// queens' `place`: `row` from 0 and from `row + 1` below `n`, the calls those
/// of the module: `row` is in [0, 8), which the length's check `row <u 12` then
/// leaves nothing to decide.
fn bounded_recursion(extra: &str) -> String {
    format!(
        "define internal i16 @place(i16 %row, i16 %n) {{
b:
  %done = icmp eq i16 %row, %n
  br i1 %done, label %leaf, label %more
more:
  %fits = icmp ult i16 %row, 12
  br i1 %fits, label %next, label %crash
next:
  %up = add nsw i16 %row, 1
  %r = call i16 @place(i16 %up, i16 %n)
  ret i16 %r
leaf:
  ret i16 1
crash:
  ret i16 99
}}

define i16 @f(i16 %a) {{
b:
  %x = call i16 @place(i16 0, i16 7)
{extra}  ret i16 %x
}}
"
    )
}

#[test]
fn test_what_the_callers_pass_bounds_a_parameter_the_body_checks() {
    let text = ipsccp(&bounded_recursion(""));
    assert!(header(&text, "place").contains("range(i16 0, 8) %row"), "{text}");
    assert!(!body(&text, "place").contains("icmp ult i16 %row, 12"), "{text}");
}

/// A call from elsewhere with a value of its own leaves the parameter
/// unbounded.
#[test]
fn test_another_caller_leaves_the_parameter_unbounded() {
    let text = ipsccp(&bounded_recursion("  %y = call i16 @place(i16 %a, i16 7)\n"));
    assert!(header(&text, "place").contains("@place(i16 %row,"), "{text}");
    assert!(body(&text, "place").contains("icmp ult i16 %row, 12"), "{text}");
}

/// A pointer parameter replaced by the address of a global: what the body
/// wrote through it as `argmem` it now writes to a global, so `memory(argmem:
/// write)` no longer says where (`ReplacedPointerArg`).
#[test]
fn a_pointer_parameter_made_a_constant_address_unsplits_the_memory_attribute() {
    let text = ipsccp(
        "@g = global i16 0

define internal void @set(ptr %p) memory(argmem: write) {
b:
  store i16 1, ptr %p
  ret void
}

define void @top() {
b:
  call void @set(ptr @g)
  ret void
}
",
    );
    assert!(!header(&text, "set").contains("argmem: write"), "{text}");
}

/// An integer parameter made a constant moves no access between argument
/// memory and the rest: unsplitting the attribute anyway changed it, the next
/// stamp narrowed it back and re-ran the body's pipeline (three more runs and
/// 3.4% more instructions on QCport's `console` at -Os).
#[test]
fn an_integer_parameter_made_a_constant_leaves_the_memory_attribute_alone() {
    let text = ipsccp(
        "define internal void @set(ptr %p, i16 %n) memory(argmem: write) {
b:
  store i16 %n, ptr %p
  ret void
}

define void @top(ptr %q) {
b:
  call void @set(ptr %q, i16 7)
  ret void
}
",
    );
    assert!(header(&text, "set").contains("memory(argmem: write)"), "{text}");
}
