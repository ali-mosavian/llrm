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
    passes.add(Licm);
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
