//! Hoisting decisions, in the loops of `@f`: what `Licm` would move, by the
//! name of its result (`store` for a store).

use super::{DECIDED, Decisions, Licm};
use crate::testing::parsed;

/// The names of what `Licm` hoists in `text`, in the order it decides.
fn decided(text: &str) -> Vec<String> {
    named(text, |all| all.hoisted.clone())
}

/// The names of what it sinks, and of what it erases as dead.
fn sunk_and_erased(text: &str) -> (Vec<String>, Vec<String>) {
    (named(text, |all| all.sunk.clone()), named(text, |all| all.erased.clone()))
}

fn named(
    text: &str,
    pick: impl Fn(&Decisions) -> Vec<super::Hoisted>,
) -> Vec<String> {
    DECIDED.with(|all| *all.borrow_mut() = Decisions::default());
    let mut module = parsed(text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: false });
    passes
        .run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral))
        .unwrap_or_else(|error| panic!("{error}\n{text}"));
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let hoisted = DECIDED.with(|all| pick(&all.borrow()));
    hoisted
        .iter()
        .map(|one| {
            let instruction = function.instruction(one.inst);
            instruction
                .result
                .and_then(|value| function.value(value).name.clone())
                .unwrap_or_else(|| instruction.opcode.mnemonic().to_owned())
        })
        .collect()
}

/// The pointers `Licm` promotes out of `@f`'s loops (by name) and whether their
/// stores leave too.
fn promoted(text: &str) -> Vec<(Vec<String>, bool)> {
    DECIDED.with(|all| *all.borrow_mut() = Decisions::default());
    let mut module = parsed(text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: false });
    passes
        .run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral))
        .unwrap_or_else(|error| panic!("{error}\n{text}"));
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    DECIDED.with(|all| {
        all.borrow()
            .promoted
            .iter()
            .map(|one| {
                let names = one
                    .pointers
                    .iter()
                    .map(|pointer| match pointer {
                        llrm_mir::module::Operand::Value(value) => {
                            function.value(*value).name.clone().unwrap_or_default()
                        }
                        _ => "constant".to_owned(),
                    })
                    .collect();
                (names, one.store_safe)
            })
            .collect()
    })
}

/// `s += v` over `i < %n`, `v` computed by `body` in the loop's second block.
fn looped(
    declared: &str,
    parameters: &str,
    head: &str,
    body: &str,
) -> String {
    format!(
        "{declared}define i16 @f(i16 %n{parameters}) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s1, %b2 ]
{head}  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
{body}  %s1 = add i16 %s, %v
  %i1 = add i16 %i, 1
  br label %b1

b3:
  ret i16 %s
}}
"
    )
}

const GLOBALS: &str = "@g = global i16 5\n@h = global i16 0\n\n";

/// Arithmetic on values defined outside the loop is hoisted, and what reads it
/// with it. With `hasLoopInvariantOperands` answering no, nothing moved.
#[test]
fn test_invariant_arithmetic_is_hoisted_with_what_reads_it() {
    let text = looped("", ", i16 %a", "", "  %k = mul i16 %a, 3\n  %v = add i16 %k, 1\n");
    assert_eq!(decided(&text), ["k", "v"]);
}

/// A load of a cell the loop stores to stays, and one of a cell nothing in it
/// writes goes: the walker's clobber is inside the loop, or it is not.
#[test]
fn test_a_load_the_loop_writes_stays_and_one_it_does_not_goes() {
    let body = |load: &str, store: &str| format!("  %v = load i16, ptr {load}\n  store i16 %i, ptr {store}\n");
    assert_eq!(decided(&looped(GLOBALS, "", "", &body("@g", "@g"))), Vec::<String>::new());
    assert_eq!(decided(&looped(GLOBALS, "", "", &body("@g", "@h"))), ["v"]);
}

/// A division by a variable faults where the loop would not have run it: it
/// goes only from where the loop certainly runs it. The same division by a
/// constant goes from anywhere.
#[test]
fn test_a_division_that_may_fault_goes_only_from_where_it_certainly_runs() {
    let in_header = looped("", ", i16 %a", "  %d = sdiv i16 100, %a\n", "  %v = add i16 %d, 1\n");
    assert_eq!(decided(&in_header), ["d", "v"]);
    let in_body = looped("", ", i16 %a", "", "  %d = sdiv i16 100, %a\n  %v = add i16 %d, 1\n");
    assert_eq!(decided(&in_body), Vec::<String>::new());
    let by_constant = looped("", "", "", "  %d = sdiv i16 100, 7\n  %v = add i16 %d, 1\n");
    assert_eq!(decided(&by_constant), ["d", "v"]);
}

/// A call that may throw stays whatever it reads; one that cannot, and writes
/// nothing, goes from the header.
#[test]
fn test_a_call_that_may_throw_stays() {
    let call = |attrs: &str| {
        looped(&format!("declare i16 @pure(i16){attrs}\n\n"), ", i16 %a", "  %v = call i16 @pure(i16 %a)\n", "")
    };
    assert_eq!(decided(&call(" readnone")), Vec::<String>::new());
    assert_eq!(decided(&call(" nounwind readnone")), ["v"]);
}

/// An exit's LCSSA phi reading `body`'s `%v` out of a loop that counts `%i` up
/// to `%n`: the value is read only after the loop.
fn sinking(body: &str) -> String {
    format!(
        "@g = global i16 5
@h = global i16 0

define i16 @f(i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
{body}  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %i1 = add i16 %i, 1
  br label %b1

b3:
  %r = phi i16 [ %v, %b1 ]
  ret i16 %r
}}
"
    )
}

/// A value only the code after the loop reads is computed there: sunk from the
/// loop, as the exit's phi reads it. Read in the loop too, it stays.
#[test]
fn test_a_value_only_read_after_the_loop_is_sunk() {
    let (sunk, erased) = sunk_and_erased(&sinking("  %v = mul i16 %i, 3\n"));
    assert_eq!((sunk, erased), (vec!["v".to_owned()], Vec::<String>::new()));
    let used = sinking("  %v = mul i16 %i, 3\n  %w = add i16 %v, 1\n  store i16 %w, ptr @h\n");
    assert_eq!(sunk_and_erased(&used).0, Vec::<String>::new());
}

/// A value nothing reads is erased, not sunk.
#[test]
fn test_a_value_nothing_reads_is_erased() {
    let (sunk, erased) = sunk_and_erased(&sinking("  %v = mul i16 %i, 3\n  %dead = add i16 %i, 7\n"));
    assert_eq!((sunk, erased), (vec!["v".to_owned()], vec!["dead".to_owned()]));
}

/// A load goes after the loop only where the loop has no def before it: a store
/// anywhere in the loop (here to another global) keeps it
/// (`pointerInvalidatedByLoop` for sinking checks every def, the walker is not
/// asked).
#[test]
fn test_a_load_is_sunk_only_from_a_loop_with_no_defs() {
    assert_eq!(sunk_and_erased(&sinking("  %v = load i16, ptr @g\n")).0, ["v"]);
    assert_eq!(sunk_and_erased(&sinking("  %v = load i16, ptr @g\n  store i16 %i, ptr @h\n")).0, Vec::<String>::new());
}

/// A loop that reads and writes a local slot: `extra` goes in the loop before
/// the read of the slot.
fn slot_loop(
    declared: &str,
    extra: &str,
    load: &str,
) -> String {
    format!(
        "{declared}@g = global i16 5

define i16 @f(i16 %n, ptr %p) {{
b0:
  %s = alloca i16
  store i16 0, ptr %s
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
{extra}  %x = {load}
  %y = add i16 %x, %i
  store i16 %y, ptr %s
  %i1 = add i16 %i, 1
  br label %b1

b3:
  %r = load i16, ptr %s
  ret i16 %r
}}
"
    )
}

/// The loads and stores of a slot only the loop touches are one value in the
/// loop: promoted, stores and all.
#[test]
fn test_a_slot_only_the_loop_touches_is_promoted_with_its_stores() {
    assert_eq!(promoted(&slot_loop("", "", "load i16, ptr %s")), [(vec!["s".to_owned()], true)]);
}

/// A call that is handed the slot's address may read or write it: no promotion.
#[test]
fn test_a_slot_a_call_may_touch_is_not_promoted() {
    let text = slot_loop("declare void @peek(ptr)\n\n", "  call void @peek(ptr %s)\n", "load i16, ptr %s");
    assert_eq!(promoted(&text), Vec::<(Vec<String>, bool)>::new());
}

/// A volatile access cannot move to a register.
#[test]
fn test_a_volatile_slot_is_not_promoted() {
    assert_eq!(promoted(&slot_loop("", "", "load volatile i16, ptr %s")), Vec::<(Vec<String>, bool)>::new());
}

/// A read of the same memory through a pointer that changes in the loop (it may
/// be the global): the loads leave but the stores stay, as they must be seen in
/// memory.
#[test]
fn test_a_read_outside_the_set_promotes_the_loads_only() {
    let text = "@g = global i16 5

define i16 @f(i16 %n, ptr %p) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %x = load i16, ptr @g
  %q = getelementptr i16, ptr %p, i16 %i
  %z = load i16, ptr %q
  %y = add i16 %x, %z
  store i16 %y, ptr @g
  %i1 = add i16 %i, 1
  br label %b1

b3:
  ret i16 0
}
";
    assert_eq!(promoted(text), [(vec!["constant".to_owned()], false)]);
}

/// `text` through `Licm` with the edits made, under the verifier and the
/// preserved-analyses check, and the answers of @f before and after on
/// `inputs`.
fn applied(
    text: &str,
    inputs: &[Vec<i128>],
) -> (String, String) {
    use crate::testing::{printed, results};
    let before = parsed(text);
    let mut after = before.clone();
    let mut passes = llrm_mir::passes::PassManager::default();
    (passes.verify_each, passes.verify_invalidation) = (true, true);
    passes.add(Licm { apply: true });
    passes
        .run_module(&mut after, std::rc::Rc::new(llrm_mir::target::Neutral))
        .unwrap_or_else(|error| panic!("{error}\n{text}"));
    let inputs: Vec<&[i128]> = inputs.iter().map(Vec::as_slice).collect();
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{}", printed(&after));
    (printed(&before), printed(&after))
}

/// The instructions of @f's block `name`.
fn block_of(
    text: &str,
    name: &str,
) -> Vec<String> {
    let f = &text[text.find("define i16 @f(").expect("@f")..];
    let start = f.find(&format!("\n{name}:\n")).unwrap_or_else(|| panic!("no {name} in\n{text}")) + name.len() + 3;
    f[start..].lines().take_while(|line| line.starts_with("  ")).map(|line| line.trim().to_owned()).collect()
}

fn trips() -> Vec<Vec<i128>> {
    [0, 1, 5, -1].iter().map(|&n| vec![n]).collect()
}

/// Hoisting applied: the invariant leaves the loop for the preheader and the
/// function answers as it did.
#[test]
fn test_an_invariant_operation_is_moved_to_the_preheader() {
    let text = looped("", ", i16 %a", "", "  %k = mul i16 %a, 3\n  %v = add i16 %k, 1\n");
    let inputs: Vec<Vec<i128>> = [0, 1, 5].iter().flat_map(|&n| [0, 7, -3].map(|a| vec![n, a])).collect();
    let (_, after) = applied(&text, &inputs);
    assert_eq!(block_of(&after, "b0"), ["%k = mul i16 %a, 3", "%v = add i16 %k, 1", "br label %b1"]);
}

/// Sinking applied: the value only the code after the loop reads is computed
/// there, from an LCSSA phi of what it reads in the loop.
#[test]
fn test_a_value_read_after_the_loop_is_computed_there() {
    let text = sinking("  %a = mul i16 %i, 3\n  %v = add i16 %a, 1\n");
    let (_, after) = applied(&text, &trips());
    assert!(!block_of(&after, "b1").iter().any(|line| line.contains("mul") || line.contains("add i16 %a")), "{after}");
    let exit = block_of(&after, "b3");
    assert!(
        exit.iter().any(|line| line.contains("mul i16")) && exit.iter().any(|line| line.contains("add i16")),
        "{after}"
    );
}

/// A nest: what the inner loop hoists into the outer loop's body leaves that
/// loop too, the graph made again after the inner loop changed.
#[test]
fn test_what_an_inner_loop_hoists_leaves_the_outer_loop_too() {
    let text = "define i16 @f(i16 %n, i16 %a) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b4 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b5

b2:
  br label %b3

b3:
  %j = phi i16 [ 0, %b2 ], [ %j1, %b3 ]
  %k = mul i16 %a, 3
  %j1 = add i16 %j, %k
  %d = icmp slt i16 %j1, 20
  br i1 %d, label %b3, label %b4

b4:
  %i1 = add i16 %i, 1
  br label %b1

b5:
  ret i16 %i
}
";
    let inputs: Vec<Vec<i128>> = [0, 1, 3].iter().flat_map(|&n| [1, 2].map(|a| vec![n, a])).collect();
    DECIDED.with(|all| *all.borrow_mut() = Decisions::default());
    let (_, after) = applied(text, &inputs);
    assert_eq!(block_of(&after, "b0"), ["%k = mul i16 %a, 3", "br label %b1"], "{after}");
    assert_eq!(
        DECIDED.with(|all| all.borrow().rebuilds),
        0,
        "the manager's graph, kept right by the updater through the nest"
    );
}

/// A loop over a slot (a global) with the accesses `body` makes; the exit
/// returns the slot.
fn global_loop(body: &str) -> String {
    format!(
        "@g = global i16 5
@h = global i16 7

define i16 @f(i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
{body}  %i1 = add i16 %i, 1
  br label %b1

b3:
  %r = load i16, ptr @g
  ret i16 %r
}}
"
    )
}

/// A global read and written in the loop is one value there: loaded before it,
/// stored once after, the loop's own loads and stores gone.
#[test]
fn test_a_global_read_and_written_in_a_loop_lives_in_a_register() {
    let text = global_loop("  %x = load i16, ptr @g\n  %y = add i16 %x, %i\n  store i16 %y, ptr @g\n");
    let (_, after) = applied(&text, &trips());
    let body = block_of(&after, "b2");
    assert!(!body.iter().any(|line| line.contains("@g")), "{after}");
    assert!(block_of(&after, "b0").iter().any(|line| line.contains("load i16, ptr @g")), "{after}");
    assert!(
        block_of(&after, "b3").iter().any(|line| line.starts_with("store i16") && line.contains("ptr @g")),
        "{after}"
    );
}

/// A store the loop makes whenever it is entered, and no load: nothing is
/// loaded before the loop, the value stored last is stored once after it.
#[test]
fn test_a_global_only_written_in_a_loop_is_stored_once_after_it() {
    let text = "@g = global i16 5

define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  store i16 %i, ptr @g
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %i1 = add i16 %i, 1
  br label %b1

b3:
  %r = load i16, ptr @g
  ret i16 %r
}
";
    let (_, after) = applied(text, &trips());
    assert!(!block_of(&after, "b1").iter().any(|line| line.contains("@g")), "{after}");
    assert!(!block_of(&after, "b0").iter().any(|line| line.contains("load")), "{after}");
    assert!(block_of(&after, "b3").iter().any(|line| line.starts_with("store i16")), "{after}");
}

/// A store made only on some trips: the value before the loop is loaded first,
/// so a loop that never stores leaves it as it was.
#[test]
fn test_a_store_made_on_some_trips_is_preceded_by_a_load_before_the_loop() {
    let text = global_loop("  store i16 %i, ptr @g\n");
    let (_, after) = applied(&text, &trips());
    assert!(!block_of(&after, "b2").iter().any(|line| line.contains("@g")), "{after}");
    assert!(block_of(&after, "b0").iter().any(|line| line.contains("load i16, ptr @g")), "{after}");
}

/// Two globals, one a load whose address is another's value: promoted in
/// rounds. Stores to a global stay when the loop's other accesses may read it.
#[test]
fn test_a_slot_read_through_a_changing_pointer_keeps_its_stores() {
    let text = "@g = global [4 x i16] zeroinitializer

define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %x = load i16, ptr @g
  %q = getelementptr i16, ptr @g, i16 %i
  %z = load i16, ptr %q
  %y = add i16 %x, %z
  store i16 %y, ptr @g
  %i1 = add i16 %i, 1
  br label %b1

b3:
  %r = load i16, ptr @g
  ret i16 %r
}
";
    let (_, after) = applied(text, &[vec![0], vec![1], vec![3]]);
    assert!(block_of(&after, "b2").iter().any(|line| line.starts_with("store i16")), "{after}");
}
