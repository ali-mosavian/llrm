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
    named_on(text, llrm_mir::target::Neutral, pick)
}

/// [`named`] on `machine`.
fn named_on(
    text: &str,
    machine: impl llrm_mir::target::Machine + 'static,
    pick: impl Fn(&Decisions) -> Vec<super::Hoisted>,
) -> Vec<String> {
    DECIDED.with(|all| *all.borrow_mut() = Decisions::default());
    let mut module = parsed(text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: false, speculate: true });
    passes.run_module(&mut module, std::rc::Rc::new(machine)).unwrap_or_else(|error| panic!("{error}\n{text}"));
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
    promoted_on(text, llrm_mir::target::Neutral)
}

/// [`promoted`] on `machine`.
fn promoted_on(
    text: &str,
    machine: impl llrm_mir::target::Machine + 'static,
) -> Vec<(Vec<String>, bool)> {
    DECIDED.with(|all| *all.borrow_mut() = Decisions::default());
    let mut module = parsed(text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: false, speculate: true });
    passes.run_module(&mut module, std::rc::Rc::new(machine)).unwrap_or_else(|error| panic!("{error}\n{text}"));
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

/// A libm call `InferFunctionAttrs` marked `memory(errnomem: write)` is hoisted
/// from the header of a loop that touches no other memory, as the pin does
/// (`onlyWritesMemory`, then `noConflictingReadWrites`).
#[test]
fn test_a_call_that_only_writes_errno_is_hoisted() {
    let text = looped(
        "declare i16 @lib(i16) nofree nosync nounwind willreturn memory(errnomem: write)\n\n",
        ", i16 %a",
        "  %v = call i16 @lib(i16 %a)\n",
        "",
    );
    assert_eq!(decided(&text), ["v"]);
}

/// A speculatable call that is handed a `noundef` argument stayed in the loop:
/// `isSafeToSpeculativelyExecute` ignores UB-implying attributes by default,
/// and LICM strips them from a call it moves that was not certain to run.
#[test]
fn test_a_call_with_a_noundef_argument_is_hoisted_without_it() {
    use crate::testing::printed;
    let text = looped(
        "declare i16 @pure(i16 noundef) speculatable nounwind readnone\n\n",
        ", i16 %a",
        "",
        "  %v = call i16 @pure(i16 noundef %a)\n",
    );
    assert_eq!(decided(&text), ["v"]);
    let mut module = parsed(&text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: true, speculate: true });
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("licm");
    let after = printed(&module);
    assert_eq!(block_of(&after, "b0"), ["%v = call i16 @pure(i16 %a)", "br label %b1"], "{after}");
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

/// The loop of a global it only stores to, on one path.
fn conditional_store_loop(test: &str) -> String {
    format!(
        "@g = internal global i32 0

define i32 @f(i32 %n) {{
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %i1, %b4 ]
  %c = icmp slt i32 %i, %n
  br i1 %c, label %b2, label %b5

b2:
{test}  br i1 %t, label %b3, label %b4

b3:
  store i32 %i, ptr @g
  br label %b4

b4:
  %i1 = add i32 %i, 1
  br label %b1

b5:
  ret i32 %n
}}
"
    )
}

/// Promoting a global the loop only stores to, on some paths, loads it in the
/// preheader and stores it after the loop for no load saved, and keeps the
/// global from being removed as write-only: nbody.bas promoted DELTAY&, DIST2&
/// and DELTAX& once its division guard was gone, +43 B at -Os m16.
#[test]
fn test_a_global_stored_on_some_paths_and_never_loaded_is_not_promoted() {
    assert_eq!(promoted(&conditional_store_loop("  %t = icmp eq i32 %i, 3\n")), Vec::<(Vec<String>, bool)>::new());
}

/// With a load in the loop the promotion saves it: the load is promoted.
/// Changed meaning (the thread-local deviation is gone): the stores are not
/// sunk (the pin's `StoreSafety` stays unknown for a global whose store may not
/// run), so the second field, `store_safe`, is false.
#[test]
fn test_a_global_stored_on_some_paths_and_loaded_is_promoted() {
    assert_eq!(
        promoted(&conditional_store_loop("  %x = load i32, ptr @g\n  %t = icmp eq i32 %x, 3\n")),
        [(vec!["constant".to_owned()], false)]
    );
}

/// A machine whose module is single-threaded, as wasm's is.
struct SingleThreaded(llrm_mir::target::Neutral);

impl llrm_mir::target::Machine for SingleThreaded {
    fn single_threaded(&self) -> bool {
        true
    }

    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        self.0.foreign_span(selectors, offsets, width)
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        self.0.spaces()
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        self.0.costs()
    }

    fn registers(&self) -> i64 {
        self.0.registers()
    }

    fn call_registers(&self) -> i64 {
        self.0.call_registers()
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        self.0.address_forms()
    }
}

/// A slot whose address a call took before the loop, stored on one path only.
fn captured_slot_loop() -> String {
    "declare void @use(ptr)

define i32 @f(i32 %n) {
b0:
  %s = alloca i32
  call void @use(ptr %s)
  store i32 0, ptr %s
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %i1, %b4 ]
  %c = icmp slt i32 %i, %n
  br i1 %c, label %b2, label %b5

b2:
  %x = load i32, ptr %s
  %t = icmp eq i32 %x, 3
  br i1 %t, label %b3, label %b4

b3:
  store i32 %i, ptr %s
  br label %b4

b4:
  %i1 = add i32 %i, 1
  br label %b1

b5:
  ret i32 %n
}
"
    .to_owned()
}

/// The pin's `isThreadLocalObject` takes every object of a single-threaded
/// module, so a slot a call captured before the loop still has its store sunk
/// with its load; in a `POSIX` module a captured slot is not thread-local.
/// long_double_variadic -Os wasm32: `sum`'s `va_list` slot, handed to
/// `va_start`, kept its store and was read back, +12 B.
#[test]
fn test_a_captured_slot_stored_on_some_paths_is_sunk_only_in_a_single_threaded_module() {
    let text = captured_slot_loop();
    assert_eq!(promoted(&text), [(vec!["s".to_owned()], false)]);
    assert_eq!(promoted_on(&text, SingleThreaded(llrm_mir::target::Neutral)), [(vec!["s".to_owned()], true)]);
}

/// The pin's `isWritableObject` takes no global ("we may not have provenance
/// to perform the write"), so single-threaded or not a global stored on some
/// paths keeps its store in the loop: jpeg_fixed `emit` -O1 wasm32 stored
/// `adler_a` and `adler_b` on the path where the loop does not run, +28 B.
#[test]
fn test_a_global_stored_on_some_paths_keeps_its_store_in_a_single_threaded_module() {
    let text = conditional_store_loop("  %x = load i32, ptr @g\n  %t = icmp eq i32 %x, 3\n");
    assert_eq!(promoted_on(&text, SingleThreaded(llrm_mir::target::Neutral)), [(vec!["constant".to_owned()], false)]);
}

/// `isWritableObject` takes a `byval` argument and a `noalias` call's result,
/// not only an alloca: with the object thread-local (single-threaded module)
/// the store made on some paths is sunk. A plain pointer argument stays
/// unwritable, so its store stayed in the loop and the other two did too.
#[test]
fn test_a_byval_argument_stored_on_some_paths_is_sunk_but_a_plain_pointer_is_not() {
    let loop_over = |attrs: &str| {
        format!(
            "define i32 @f(i32 %n, ptr {attrs} %g) {{
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %i1, %b4 ]
  %c = icmp slt i32 %i, %n
  br i1 %c, label %b2, label %b5

b2:
  %x = load i32, ptr %g
  %t = icmp eq i32 %x, 3
  br i1 %t, label %b3, label %b4

b3:
  store i32 %i, ptr %g
  br label %b4

b4:
  %i1 = add i32 %i, 1
  br label %b1

b5:
  ret i32 %n
}}
"
        )
    };
    let sunk = |attrs: &str| promoted_on(&loop_over(attrs), SingleThreaded(llrm_mir::target::Neutral));
    assert_eq!(sunk("byval(i32) align 4 dereferenceable(4)"), [(vec!["g".to_owned()], true)]);
    assert_eq!(sunk("dereferenceable(4) align 4"), [(vec!["g".to_owned()], false)]);
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
    applied_speculating(text, inputs, true)
}

/// As `applied`, `licm<no-allowspeculation>` where `speculate` is false.
fn applied_speculating(
    text: &str,
    inputs: &[Vec<i128>],
    speculate: bool,
) -> (String, String) {
    use crate::testing::{printed, results};
    let before = parsed(text);
    let mut after = before.clone();
    let mut passes = llrm_mir::passes::PassManager::default();
    (passes.verify_each, passes.verify_invalidation) = (true, true);
    passes.add(Licm { apply: true, speculate });
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
    // Any return type: the function is the one named `@f`.
    let f = &text[text.find(" @f(").expect("@f")..];
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

/// As `global_loop`, rotated: the body runs at least once, so its stores are
/// guaranteed to execute.
fn rotated_global_loop(body: &str) -> String {
    format!(
        "@g = global i16 5
@h = global i16 7

define i16 @f(i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
{body}  %i1 = add i16 %i, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %b1, label %b3

b3:
  %r = load i16, ptr @g
  ret i16 %r
}}
"
    )
}

/// A global read and written in the loop is one value there: loaded before it,
/// stored once after, the loop's own loads and stores gone. Changed meaning
/// (the thread-local deviation is gone): this holds for a loop whose store is
/// guaranteed to execute, the rotated shape; the loop that may run no trip
/// keeps its stores
/// (`test_a_global_store_the_loop_may_skip_stays_in_the_loop`).
#[test]
fn test_a_global_read_and_written_in_a_loop_lives_in_a_register() {
    let text = rotated_global_loop("  %x = load i16, ptr @g\n  %y = add i16 %x, %i\n  store i16 %y, ptr @g\n");
    let (_, after) = applied(&text, &trips());
    let body = block_of(&after, "b1");
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

/// A global the loop stores to, in a loop that may run no trip, is not
/// thread-local (`isThreadLocalObject`): the stores stay in the loop and only
/// the load leaves it. Sunk, a loop that ran no trip still stored (jpeg_fixed
/// -O1 wasm: `emit` +28 B over the pin, four phis and identity copies).
#[test]
fn test_a_global_store_the_loop_may_skip_stays_in_the_loop() {
    let text = global_loop("  %x = load i16, ptr @g\n  %y = add i16 %x, %i\n  store i16 %y, ptr @g\n");
    let (_, after) = applied_speculating(&text, &trips(), false);
    assert!(block_of(&after, "b2").iter().any(|line| line.starts_with("store i16")), "{after}");
    assert!(!block_of(&after, "b3").iter().any(|line| line.starts_with("store i16")), "{after}");
    assert!(block_of(&after, "b0").iter().any(|line| line.contains("load i16, ptr @g")), "{after}");
}

/// A store made only on some trips: the value before the loop is loaded first,
/// so a loop that never stores leaves it as it was. Changed meaning (the
/// thread-local deviation is gone): the load leaves for the preheader and the
/// store stays, as the global is not thread-local and the store is not
/// guaranteed; sunk, it would run on the no-trip path.
#[test]
fn test_a_store_made_on_some_trips_is_preceded_by_a_load_before_the_loop() {
    let text = global_loop("  %x = load i16, ptr @g\n  store i16 %x, ptr @h\n  store i16 %i, ptr @g\n");
    let (_, after) = applied(&text, &trips());
    assert!(!block_of(&after, "b2").iter().any(|line| line.contains("load i16, ptr @g")), "{after}");
    assert!(
        block_of(&after, "b2").iter().any(|line| line.starts_with("store i16") && line.contains("ptr @g")),
        "{after}"
    );
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

/// LPM1's first LICM is `licm<no-allowspeculation>`: a multiply the loop may
/// skip stays where it is, and the second LICM (speculating) hoists it. Without
/// the flag the first one hoisted it too and the guard the rotate puts before
/// the loop (#2194) had nothing left to protect.
#[test]
fn test_without_allow_speculation_a_conditional_multiply_stays_in_the_loop() {
    let text = "define i16 @f(i16 %n, i16 %a) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b4 ]
  %t = and i16 %i, 1
  %c = icmp eq i16 %t, 0
  br i1 %c, label %b2, label %b3

b2:
  %k = mul i16 %a, 3
  br label %b4

b3:
  br label %b4

b4:
  %p = phi i16 [ %k, %b2 ], [ 0, %b3 ]
  %j = add i16 %i, 1
  %d = icmp slt i16 %j, %n
  br i1 %d, label %b1, label %b5

b5:
  ret i16 %p
}
";
    let inputs: Vec<Vec<i128>> = [0, 1, 3].iter().flat_map(|&n| [1, 2].map(|a| vec![n, a])).collect();
    let (_, speculated) = applied_speculating(text, &inputs, true);
    assert_eq!(block_of(&speculated, "b0"), ["%k = mul i16 %a, 3", "br label %b1"], "{speculated}");
    let (_, kept) = applied_speculating(text, &inputs, false);
    assert_eq!(block_of(&kept, "b2"), ["%k = mul i16 %a, 3", "br label %b4"], "{kept}");
}

/// A store after the loop's exit test, of the loop's own counter: the value
/// the global holds at the exit is the counter of the trip before, not the
/// counter the header holds then (`fill` stored 8 where the loop's last store
/// was 7).
///
/// Changed meaning (the thread-local deviation is gone): the sunk store now
/// happens for a slot that is thread-local, an alloca not captured; a global
/// keeps the store in the loop.
#[test]
fn test_a_store_after_the_exit_test_leaves_the_value_of_the_last_trip() {
    let text = "define i16 @f(i16 %n) {
b0:
  %s = alloca i16
  store i16 5, ptr %s
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %x = load i16, ptr %s
  store i16 %i, ptr %s
  %i1 = add i16 %i, 1
  br label %b1

b3:
  %r = load i16, ptr %s
  ret i16 %r
}
";
    let (_, after) = applied(text, &trips());
    assert!(block_of(&after, "b3").iter().any(|line| line.starts_with("store i16")), "{after}");
}

/// A slot stored in an outer loop and read in an inner one through a pointer
/// that joins its address with a pointer loaded from memory: the read may be of
/// the slot, so it is not promoted. LICM took `*s = 1` out of the loop and the
/// read saw the slot unwritten (`phi_slot_unknown_arm`, bench bintree bq32
/// printed 2158 for 5736113, once LoopRotate put the inlined loop in this
/// form).
#[test]
fn test_a_slot_read_through_a_join_with_a_loaded_pointer_is_not_promoted() {
    let text = "@tbl = internal global [64 x i8] zeroinitializer
@t = internal constant ptr @tbl

define i16 @f(i16 %n) {
b1:
  %a = alloca i16
  %t = load volatile ptr, ptr @t
  br label %outer

outer:
  %i = phi i16 [ 0, %b1 ], [ %i1, %latch ]
  %c = icmp sgt i16 %i, 1
  br i1 %c, label %set, label %latch

set:
  store i16 1, ptr %a
  br label %inner

inner:
  %p = phi ptr [ %a, %set ], [ %q, %inner ]
  %v = load i16, ptr %p
  %q = getelementptr i8, ptr %t, i16 %v
  %z = load i16, ptr %q
  %d = icmp eq i16 %z, 0
  br i1 %d, label %done, label %inner

done:
  %w = phi ptr [ %q, %inner ]
  store i16 %i, ptr %w
  br label %latch

latch:
  %i1 = add i16 %i, 1
  %e = icmp slt i16 %i1, %n
  br i1 %e, label %outer, label %exit

exit:
  ret i16 0
}
";
    let mut module = parsed(text);
    let after = crate::testing::managed(&mut module, Licm { apply: true, speculate: false });
    assert!(after.contains("store i16 1, ptr %a"), "the slot's store left the loop: {after}");
}

/// A promoted store with no load before the loop takes `UndefValue` on the
/// preheader edge, as LLVM's `SSA.AddAvailableValue(Preheader, UndefValue)`;
/// with poison a later fold was free to pick the arm (loopidioms'
/// `phi_slot_unknown_arm` printed 2158 where 5736113 was wanted).
#[test]
fn test_a_promoted_store_without_a_load_starts_from_undef_not_poison() {
    let text = "define void @fill(ptr %p) {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %3, %b3 ]
  %1 = icmp slt i16 %0, 8
  br i1 %1, label %b4, label %b5

b4:
  %2 = icmp ult i16 %0, 8
  br i1 %2, label %b3, label %b6

b3:
  store i16 %0, ptr %p
  %3 = add i16 %0, 1
  br label %b2

b5:
  ret void

b6:
  unreachable
}
";
    let mut after = parsed(text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: true, speculate: true });
    passes.run_module(&mut after, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let after = crate::testing::printed(&after);
    assert!(after.contains("[ undef, %b1 ]") && !after.contains("poison"), "{after}");
}

/// Asks the accesses' annotations, as the next pass that reads memory does.
struct AsksAnnotated;

impl llrm_mir::passes::FunctionPass for AsksAnnotated {
    fn name(&self) -> &'static str {
        "asks-annotated"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        analyses.get::<llrm_analysis::manager::Annotated>(unit.context, unit.layout, unit.function);
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// A LICM run that only moved instructions kept the accesses' annotations: the
/// next run (or any pass asking MemorySSA) found them held. They were derived
/// again after every hoist, 81% of the cost of a MemorySSA miss in d_faces.
#[test]
fn test_a_hoist_only_run_keeps_the_annotations_for_the_next_ask() {
    let text = "define i16 @f(i16 %n, i16 %a) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b2 ]
  %k = mul i16 %a, 3
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %j = add i16 %i, %k
  br label %b1

b3:
  ret i16 %i
}
";
    let mut module = parsed(text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: true, speculate: true });
    passes.add(AsksAnnotated);
    let (_, misses_before) = llrm_mir::counts::analysed("annotated");
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let (_, misses) = llrm_mir::counts::analysed("annotated");
    assert!(crate::testing::printed(&module).contains("%k = mul"), "premise: the multiply is hoisted");
    assert_eq!(misses - misses_before, 1, "the next ask derived the annotations again");
}

/// A call that reads and writes `inaccessiblemem` only (a BIOS poll: the
/// speaker example's `key_pressed`) is not one that `doesNotAccessMemory`: the
/// bottom-tested loop runs it every trip, and LICM took it out of the loop, so
/// `wait` never saw a key (examples/speaker.nib never ended, once LoopRotate
/// made the poll's block one the loop certainly runs).
#[test]
fn test_a_call_that_touches_only_inaccessible_memory_stays_in_the_loop() {
    let text = "declare cc 1005 i32 @now() addrspace(1) nearcode memory(inaccessiblemem: readwrite) nounwind
declare cc 1005 i8 @key_pressed() addrspace(1) nearcode memory(inaccessiblemem: readwrite) nounwind

define i8 @f(i16 %0) {
b1:
  %1 = call cc 1005 addrspace(1) i32 @now()
  %2 = icmp ne i16 %0, 0
  br i1 %2, label %13, label %b4

b3:
  %3 = phi i32 [ %1, %13 ], [ %11, %b10 ]
  %4 = phi i16 [ %0, %13 ], [ %10, %b10 ]
  %5 = call cc 1005 addrspace(1) i8 @key_pressed()
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b5, label %b7

b4:
  ret i8 1

b5:
  ret i8 0

b7:
  %7 = call cc 1005 addrspace(1) i32 @now()
  %8 = icmp ne i32 %7, %3
  br i1 %8, label %b8, label %b10

b8:
  %9 = sub nuw i16 %4, 1
  br label %b10

b10:
  %10 = phi i16 [ %9, %b8 ], [ %4, %b7 ]
  %11 = phi i32 [ %7, %b8 ], [ %3, %b7 ]
  %12 = icmp ne i16 %10, 0
  br i1 %12, label %b3, label %14

13:
  br label %b3

14:
  br label %b4
}
";
    let mut module = parsed(text);
    let after = crate::testing::managed(&mut module, Licm { apply: true, speculate: true });
    let header = after.split("\nb3:").nth(1).expect("the loop header").split("\n\n").next().expect("its end");
    assert!(header.contains("@key_pressed()"), "the poll left the loop: {after}");
}

/// `text` through `Licm` on the Dos target, printed.
fn licm_printed(text: &str) -> String {
    let mut module = parsed(&format!("{}{text}", llrm_analysis::testing::DOS));
    let mut passes = llrm_mir::passes::PassManager::default();
    (passes.verify_each, passes.verify_invalidation) = (true, true);
    passes.add(Licm { apply: true, speculate: true });
    passes.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    crate::testing::printed(&module)
}

/// A slice descriptor is a `noalias readonly` parameter and its elements are
/// written through a pointer loaded from it: the length and the data pointer
/// it holds are invariant whatever the stores write. Overlap with an
/// unknown pointer kept both in Nib's loops, and with them every bounds
/// check out of reach of the counting passes.
#[test]
fn test_a_load_from_a_readonly_noalias_parameter_leaves_past_a_store() {
    let text = "define i16 @f(i16 %n, ptr noalias readonly dereferenceable(16) %d) {
b0:
  %data = getelementptr i8, ptr %d, i16 2
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %p = load ptr, ptr %data
  %q = getelementptr i8, ptr %p, i16 %i
  store i8 1, ptr %q
  %v = load i16, ptr %d
  %i1 = add i16 %i, %v
  br label %b1

b3:
  ret i16 %i
}
";
    let after = licm_printed(text);
    let body = block_of(&after, "b2");
    assert!(
        body.iter().all(|line| !line.contains("load i16, ptr %d") && !line.contains("load ptr, ptr %data")),
        "{after}"
    );
}

/// The language says a load reads what is written once and never again: a store
/// the loop cannot rule out as that cell (any pointer) does not keep it in the
/// loop.
#[test]
fn test_a_load_the_language_says_is_invariant_leaves_past_a_store() {
    let with = |load: &str| {
        format!(
            "@g = global i16 5

define i16 @f(i16 %n, ptr %p) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %v = {load}
  store i16 %i, ptr %p
  %w = add i16 %i, %v
  %i1 = add i16 %w, 1
  br label %b1

b3:
  ret i16 %i
}}

!0 = !{{}}
"
        )
    };
    let plain = licm_printed(&with("load i16, ptr @g"));
    assert!(block_of(&plain, "b2").iter().any(|line| line.contains("load i16, ptr @g")), "a plain load stays: {plain}");
    let stated = licm_printed(&with("load i16, ptr @g, !invariant.load !0"));
    assert!(block_of(&stated, "b2").iter().all(|line| !line.contains("load i16, ptr @g")), "{stated}");
    assert!(block_of(&stated, "b0").iter().any(|line| line.contains("load i16, ptr @g")), "{stated}");
}

/// A loop around a load of `%q` that a store through `%p` cannot be ruled out
/// of, with `start` in the entry block and `loop_start` in the loop (the
/// printed function).
fn invariant_start_loop(
    start: &str,
    loop_start: &str,
) -> String {
    licm_printed(&format!(
        "declare ptr @llvm.invariant.start.p0(i64 immarg, ptr nocapture)
declare void @llvm.invariant.end.p0(ptr, i64 immarg, ptr nocapture)

define i16 @f(i16 %n, ptr %p, ptr dereferenceable(2) %q) {{
b0:
  {start}
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  {loop_start}
  %v = load i16, ptr %q
  store i16 %i, ptr %p
  %w = add i16 %i, %v
  %i1 = add i16 %w, 1
  br label %b1

b3:
  ret i16 %i
}}
"
    ))
}

fn load_of_q_in(
    printed: &str,
    block: &str,
) -> bool {
    block_of(printed, block).iter().any(|line| line.contains("load i16, ptr %q"))
}

/// LICM.cpp#L1053 `isLoadInvariantInLoop`: an `invariant.start` over the
/// load's address, in a block that properly dominates the loop's header and
/// with no use of its own, lets the load leave past a store the loop cannot
/// rule out. The load stayed (MIR had no `invariant.start`).
#[test]
fn test_a_load_under_a_dominating_invariant_start_leaves_past_a_store() {
    let plain = invariant_start_loop("", "");
    assert!(load_of_q_in(&plain, "b2"), "premise: without a scope the load stays: {plain}");
    let scoped = invariant_start_loop("%s = call ptr @llvm.invariant.start.p0(i64 2, ptr %q)", "");
    assert!(!load_of_q_in(&scoped, "b2") && load_of_q_in(&scoped, "b0"), "{scoped}");
}

/// The scope must cover the load (`LocSizeInBits <= InvariantSizeInBits`) and
/// end nowhere (`II->use_empty()`), and must stand outside the loop
/// (`properlyDominates(II->getParent(), CurLoop->getHeader())`); -1, an object
/// of unknown size, covers nothing (`InvariantSize->isNegative()`).
#[test]
fn test_an_invariant_start_that_does_not_cover_or_ends_or_sits_in_the_loop_keeps_the_load() {
    let narrow = invariant_start_loop("%s = call ptr @llvm.invariant.start.p0(i64 1, ptr %q)", "");
    assert!(load_of_q_in(&narrow, "b2"), "a one byte scope does not cover an i16: {narrow}");
    let ended = invariant_start_loop(
        "%s = call ptr @llvm.invariant.start.p0(i64 2, ptr %q)\n  call void @llvm.invariant.end.p0(ptr %s, i64 2, ptr %q)",
        "",
    );
    assert!(load_of_q_in(&ended, "b2"), "a scope that something ends is not one: {ended}");
    let inside = invariant_start_loop("", "%s = call ptr @llvm.invariant.start.p0(i64 2, ptr %q)");
    assert!(load_of_q_in(&inside, "b2"), "a scope inside the loop does not dominate its header: {inside}");
    let unknown = invariant_start_loop("%s = call ptr @llvm.invariant.start.p0(i64 -1, ptr %q)", "");
    assert!(load_of_q_in(&unknown, "b2"), "-1 is skipped: {unknown}");
}

const SCOPE_DECLS: &str = "declare ptr @llvm.invariant.start.p0(i64 immarg, ptr nocapture)
declare void @llvm.invariant.end.p0(ptr, i64 immarg, ptr nocapture)
";
const START: &str = "%s = call ptr @llvm.invariant.start.p0(i64 2, ptr %q)";
const END: &str = "call void @llvm.invariant.end.p0(ptr %s, i64 2, ptr %q)";

/// A loop `b1`/`b2` around the load of `%q` and a store the load cannot be
/// ruled out of, entered from `b0` and left to `b3`, with `{b0}` ahead of the
/// branch, `{b1}` and `{b2}` in the loop and `{b3}` after it.
fn scoped_loop(
    b0: &str,
    b1: &str,
    b2: &str,
    b3: &str,
) -> String {
    licm_printed(&format!(
        "{SCOPE_DECLS}
define i16 @f(i16 %n, ptr %p, ptr dereferenceable(2) %q) {{
b0:
  {b0}
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  {b1}
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  {b2}
  %v = load i16, ptr %q
  store i16 %i, ptr %p
  %w = add i16 %i, %v
  %i1 = add i16 %w, 1
  br label %b1

b3:
  {b3}
  ret i16 %i
}}
"
    ))
}

/// DEVIATE from LICM.cpp#L1086 (`II->use_empty()`): an `invariant.end` that
/// cannot run between the start and the loop leaves the scope open at the
/// header. The load stayed, the pin's answer for any end at all.
#[test]
fn test_an_invariant_end_after_the_loop_keeps_the_scope_open_for_the_load() {
    let after = scoped_loop(START, "", "", END);
    assert!(!load_of_q_in(&after, "b2") && load_of_q_in(&after, "b0"), "{after}");
}

/// The ends that do run between the start and the header close the scope the
/// load needs: right after the start, in the header, in the body, on the
/// entry path of a join, and when an outer loop runs the end again before the
/// inner loop. Each hoisted the load past the store.
#[test]
fn test_an_invariant_end_that_can_run_before_the_header_keeps_the_load() {
    let entry = scoped_loop(&format!("{START}\n  {END}"), "", "", "");
    assert!(load_of_q_in(&entry, "b2"), "end right after the start: {entry}");
    let header = scoped_loop(START, END, "", "");
    assert!(load_of_q_in(&header, "b2"), "end in the header: {header}");
    let body = scoped_loop(START, "", END, "");
    assert!(load_of_q_in(&body, "b2"), "end in the body: {body}");
}

/// A side path that ends the scope and rejoins the loop's entry runs the end
/// before the header; one that leaves the function does not.
#[test]
fn test_an_invariant_end_on_a_side_path_matters_only_if_it_rejoins_the_loop() {
    let side = |leave: &str| {
        licm_printed(&format!(
            "{SCOPE_DECLS}
define i16 @f(i16 %n, i1 %k, ptr %p, ptr dereferenceable(2) %q) {{
b0:
  {START}
  br i1 %k, label %e, label %pre

e:
  {END}
  {leave}

pre:
  br label %b1

b1:
  %i = phi i16 [ 0, %pre ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %v = load i16, ptr %q
  store i16 %i, ptr %p
  %w = add i16 %i, %v
  %i1 = add i16 %w, 1
  br label %b1

b3:
  ret i16 %i
}}
"
        ))
    };
    let leaves = side("ret i16 0");
    assert!(!load_of_q_in(&leaves, "b2"), "an end on a path that returns: {leaves}");
    let rejoins = side("br label %pre");
    assert!(load_of_q_in(&rejoins, "b2"), "an end on a path into the header: {rejoins}");
}

/// The start outside an outer loop and the end after the inner loop, in the
/// outer body, runs before the inner header on the next outer round; with the
/// start in the outer body too, it reopens the scope first.
#[test]
fn test_an_invariant_end_after_an_inner_loop_matters_if_the_outer_loop_repeats_it() {
    let nest = |outer_entry: &str, outer_head: &str| {
        licm_printed(&format!(
            "{SCOPE_DECLS}
define i16 @f(i16 %n, ptr %p, ptr dereferenceable(2) %q) {{
b0:
  {outer_entry}
  br label %o

o:
  %j = phi i16 [ 0, %b0 ], [ %j1, %t ]
  {outer_head}
  br label %b1

b1:
  %i = phi i16 [ 0, %o ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %t

b2:
  %v = load i16, ptr %q
  store i16 %i, ptr %p
  %w = add i16 %i, %v
  %i1 = add i16 %w, 1
  br label %b1

t:
  {END}
  %j1 = add i16 %j, 1
  %d = icmp slt i16 %j1, %n
  br i1 %d, label %o, label %x

x:
  ret i16 %j
}}
"
        ))
    };
    let outside = nest(START, "");
    assert!(load_of_q_in(&outside, "b2"), "the end runs again before the inner header: {outside}");
    let restarted = nest("", START);
    assert!(!load_of_q_in(&restarted, "b2"), "the start comes after the end each round: {restarted}");
}

/// Two starts on one pointer: the one ended in the body no longer covers the
/// load, the one never ended still does. Each is judged by its own ends.
#[test]
fn test_one_of_two_starts_on_a_pointer_ended_in_the_loop_leaves_the_other_to_cover() {
    let two = |second_ended: bool| {
        let end = if second_ended { "call void @llvm.invariant.end.p0(ptr %t, i64 2, ptr %q)" } else { "" };
        scoped_loop(
            &format!("{START}\n  %t = call ptr @llvm.invariant.start.p0(i64 2, ptr %q)"),
            "",
            &format!("call void @llvm.invariant.end.p0(ptr %s, i64 2, ptr %q)\n  {end}"),
            "",
        )
    };
    let one_open = two(false);
    assert!(!load_of_q_in(&one_open, "b2"), "the second start is still open: {one_open}");
    let both_ended = two(true);
    assert!(load_of_q_in(&both_ended, "b2"), "both ended in the body: {both_ended}");
}

/// A scope that two starts reach through a phi is ended through the phi, which
/// names neither start. The load stayed in the loop.
#[test]
fn test_an_invariant_end_through_a_phi_keeps_the_load() {
    let printed = licm_printed(&format!(
        "{SCOPE_DECLS}
define i16 @f(i16 %n, i1 %k, ptr %p, ptr dereferenceable(2) %q) {{
b0:
  br i1 %k, label %a, label %c

a:
  %s = call ptr @llvm.invariant.start.p0(i64 2, ptr %q)
  br label %j

c:
  %t = call ptr @llvm.invariant.start.p0(i64 2, ptr %q)
  br label %j

j:
  %u = phi ptr [ %s, %a ], [ %t, %c ]
  br label %b1

b1:
  %i = phi i16 [ 0, %j ], [ %i1, %b2 ]
  %cc = icmp slt i16 %i, %n
  br i1 %cc, label %b2, label %b3

b2:
  %v = load i16, ptr %q
  store i16 %i, ptr %p
  %w = add i16 %i, %v
  %i1 = add i16 %w, 1
  br label %b1

b3:
  call void @llvm.invariant.end.p0(ptr %u, i64 2, ptr %q)
  ret i16 %i
}}
"
    ));
    assert!(load_of_q_in(&printed, "b2"), "{printed}");
}

/// A GEP the target folds into the loop's load, also read by an exit phi: LLVM
/// copies it into the exit and keeps it in the loop. llrm left the exit phi
/// reading the loop's pointer, where `opt -passes=licm` re-forms the address
/// there from the index (five bodies at the i486 target).
#[test]
fn test_a_gep_folded_into_a_load_is_copied_into_the_exit() {
    struct I486(llrm_mir::target::Neutral);
    impl llrm_mir::target::Machine for I486 {
        fn tti(&self) -> Option<std::rc::Rc<dyn llrm_mir::tti::Tti>> {
            Some(std::rc::Rc::new(llrm_x86::tti::X86::of_cpu("i486").unwrap()))
        }

        fn foreign_span(
            &self,
            selectors: (i64, i64),
            offsets: (i64, i64),
            width: i64,
        ) -> Option<(i64, i64)> {
            self.0.foreign_span(selectors, offsets, width)
        }

        fn spaces(&self) -> llrm_mir::spaces::Spaces {
            self.0.spaces()
        }

        fn costs(&self) -> llrm_mir::target::OperationCosts {
            self.0.costs()
        }

        fn registers(&self) -> i64 {
            self.0.registers()
        }

        fn call_registers(&self) -> i64 {
            self.0.call_registers()
        }

        fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
            self.0.address_forms()
        }
    }
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"

define i32 @f(ptr %p, i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %i1, %b1 ]
  %a = getelementptr i32, ptr %p, i32 %i
  %v = load i32, ptr %a
  %i1 = add i32 %i, %v
  %c = icmp eq i32 %v, 0
  br i1 %c, label %b2, label %b1

b2:
  %e = phi ptr [ %a, %b1 ]
  store i32 %n, ptr %e
  ret i32 0
}
";
    let mut module = parsed(text);
    let mut passes = llrm_mir::passes::PassManager::default();
    (passes.verify_each, passes.verify_invalidation) = (true, true);
    passes.add(Licm { apply: true, speculate: true });
    passes.run_module(&mut module, std::rc::Rc::new(I486(llrm_mir::target::Neutral))).unwrap();
    let after = crate::testing::printed(&module);
    assert!(!after.contains("phi ptr"), "{after}");
    assert!(after.contains("%a = getelementptr"), "{after}");
}

/// `(LV + a) + 1` with `a` invariant: `LV + (a + 1)`, the sum made in the
/// preheader. llrm left the add in the loop, where `opt -passes=licm` hoists
/// it (matmul at the i486 target). Flags: `nuw` kept only if both had it, `nsw`
/// only with `nuw`; `bo0` with another user stays.
#[test]
fn test_an_invariant_operand_of_a_chained_add_is_reassociated_out() {
    let text = |first: &str, second: &str, extra: &str| {
        format!(
            "define i16 @f(i16 %n, i16 %a) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %x = add {first} i16 %i, %a
  %y = add {second} i16 %x, 1
  {extra}
  %i1 = add i16 %i, %y
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %i1
}}
"
        )
    };
    let after = licm_printed(&text("nsw", "nsw", ""));
    assert!(block_of(&after, "b0").iter().any(|line| line.contains("%invariant.op = add i16 %a, 1")), "{after}");
    let body = block_of(&after, "b1");
    assert!(body.iter().any(|line| line.contains("%y.reass = add i16 %i, %invariant.op")), "{after}");
    assert!(body.iter().all(|line| !line.contains("%x = ")), "{after}");
    let after = licm_printed(&text("nuw nsw", "nuw nsw", ""));
    assert!(after.contains("%y.reass = add nuw nsw i16 %i,"), "{after}");
    let after = licm_printed(&text("nuw", "", ""));
    assert!(after.contains("%y.reass = add i16 %i,"), "{after}");
    let after = licm_printed(&text("nsw", "nsw", "%z = add i16 %x, 2"));
    assert!(!after.contains("reass"), "{after}");
    // A chain: the reassociations of one loop read nothing an earlier one
    // erased. Reading it looped SCEV forever in `qb-pressx`.
    let chain = "define i16 @f(i16 %n, i16 %a, i16 %b, i16 %c, i16 %d) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %p = add i16 %i, %a
  %q = add i16 %p, %b
  %r = add i16 %q, %c
  %s = add i16 %r, %d
  %i1 = add i16 %i, %s
  %k = icmp slt i16 %i1, %n
  br i1 %k, label %b1, label %b2

b2:
  ret i16 %i1
}
";
    let after = licm_printed(chain);
    assert!(block_of(&after, "b1").iter().filter(|line| line.contains(".reass =")).count() <= 1, "{after}");
}

/// `hoistBOAssociation` edits in place, so each link of an add chain reads the
/// one before it as already reassociated and the chain collapses to one add in
/// one run. llrm skipped every link whose `bo0` an earlier link had replaced:
/// a chain of six invariants kept three adds in the loop (`dot` at -O3, where
/// the unroll decision then saw 26 instructions, not 6, and left the loop).
#[test]
fn test_an_add_chain_of_invariants_collapses_in_one_run() {
    let terms = ["a", "b", "c", "d", "e", "f"];
    let mut body = String::from("  %t0 = add i16 %i, %a\n");
    for at in 1..terms.len() {
        body += &format!("  %t{at} = add i16 %t{}, %{}\n", at - 1, terms[at]);
    }
    let text = format!(
        "define i16 @f(i16 %n, i16 %a, i16 %b, i16 %c, i16 %d, i16 %e, i16 %f) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
{body}  %i1 = add i16 %i, %t{last}
  %k = icmp slt i16 %i1, %n
  br i1 %k, label %b1, label %b2

b2:
  ret i16 %i1
}}
",
        last = terms.len() - 1
    );
    let after = licm_printed(&text);
    let looped = block_of(&after, "b1");
    assert_eq!(looped.iter().filter(|line| line.contains(".reass =")).count(), 1, "{after}");
    assert_eq!(looped.iter().filter(|line| line.contains(" = add ")).count(), 2, "{after}");
}

/// `collectChildrenInLoop` looked at every block of the function once per loop
/// block (and `inSubLoop` compared whole loop bodies): with the inliner's big
/// bodies `sink_and_hoist` went from 24 to 62 M on d_faces. Its work follows
/// the loop, not the function.
#[test]
fn test_the_children_walk_of_a_loop_does_not_look_at_the_whole_function() {
    let chain: String = (0..60).map(|at| format!("\nc{at}:\n  br label %c{}\n", at + 1)).collect();
    let text = format!(
        "define i16 @f(i16 %n, i16 %a) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %k = mul i16 %a, 3
  br label %b2

b2:
  %i1 = add i16 %i, %k
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %b1, label %c0
{chain}
c60:
  ret i16 %i1
}}
"
    );
    super::sink::CHILDREN_EXAMINED.with(|one| one.set(0));
    licm_printed(&text);
    let looked = super::sink::CHILDREN_EXAMINED.with(|one| one.get());
    assert!(looked <= 10, "{looked} blocks looked at for a loop of two blocks in a function of 64");
}

/// `hoistGEP`: matmul's `gep i16, (gep [16 x i8], %a, %j), %i` read the
/// invariant index only after the variant one, so the row address was rebuilt
/// every trip. Not for constant indices, nor when the inner GEP has another
/// user.
#[test]
fn test_a_gep_of_a_variant_gep_is_reassociated_to_leave_the_loop() {
    let text = |outer: &str, extra: &str| {
        format!(
            "define i16 @f(ptr %a, i16 %n, i16 %k) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %p = getelementptr inbounds i16, ptr %a, i16 %i
  %q = getelementptr inbounds [16 x i8], ptr %p, i16 {outer}
  {extra}
  store i8 1, ptr %q
  %i1 = add nuw i16 %i, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %i1
}}
"
        )
    };
    let after = licm_printed(&text("%k", ""));
    assert!(
        block_of(&after, "b0")
            .iter()
            .any(|line| line.contains("%invariant.gep = getelementptr [16 x i8], ptr %a, i16 %k")),
        "{after}"
    );
    assert!(
        block_of(&after, "b1").iter().any(|line| line.contains("%gep = getelementptr i16, ptr %invariant.gep, i16 %i")),
        "{after}"
    );
    assert!(!after.contains("%p ="), "{after}");
    assert!(!licm_printed(&text("3", "")).contains("invariant.gep"));
    assert!(!licm_printed(&text("%k", "store ptr %p, ptr %a")).contains("invariant.gep"));
}

/// A reassociation reaches the preheader where the visit found it: matmul's
/// `invariant.op` came after the GEPs visited later, LLVM's before them.
#[test]
fn test_a_reassociation_keeps_its_place_among_the_hoists() {
    let after = licm_printed(
        "define i16 @f(ptr %a, i16 %n, i16 %k, i16 %m) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %x = add nsw i16 %i, %m
  %y = add nsw i16 %x, 1
  %g = getelementptr i16, ptr %a, i16 %k
  store i16 %y, ptr %g
  %i1 = add nuw i16 %i, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %i1
}
",
    );
    let entry = block_of(&after, "b0");
    let place = |what: &str| entry.iter().position(|line| line.contains(what)).unwrap_or_else(|| panic!("{after}"));
    assert!(place("%invariant.op =") < place("%g ="), "{after}");
}

/// Under `licm<no-allowspeculation>` a GEP the loop may skip stays in it:
/// `nbody`'s `gep @_pos_x, %i` in the conditional branch of the inner loop was
/// copied out to the inner preheader, which LLVM's first LICM does not do.
#[test]
fn test_without_allow_speculation_a_conditional_gep_stays_in_the_loop() {
    let text = "define i16 @f(i16 %n, ptr %a, i16 %k) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b4 ]
  %t = and i16 %i, 1
  %c = icmp eq i16 %t, 0
  br i1 %c, label %b2, label %b3

b2:
  %g = getelementptr i16, ptr %a, i16 %k
  %v = load i16, ptr %g
  br label %b4

b3:
  br label %b4

b4:
  %p = phi i16 [ %v, %b2 ], [ 0, %b3 ]
  %j = add i16 %i, 1
  %d = icmp slt i16 %j, %n
  br i1 %d, label %b1, label %b5

b5:
  ret i16 %p
}
";
    let mut module = parsed(text);
    let kept = crate::testing::managed(&mut module, Licm { apply: true, speculate: false });
    assert!(block_of(&kept, "b2").iter().any(|line| line.starts_with("%g =")), "{kept}");
}

/// `hoistGEP` asks `isKnownNonNegative` of the translated value tracking: the
/// matmul indices `phi [zext i16, 0]` are, so the re-formed GEPs are `inbounds`
/// as LLVM's are (the older query answered no, and they lost the flag).
#[test]
fn test_a_reassociated_gep_of_non_negative_indices_stays_inbounds() {
    let after = licm_printed(
        "define i16 @f(ptr %a, i16 %n, i32 %k) {
b0:
  %kk = and i32 %k, 255
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %i1, %b1 ]
  %c = phi i16 [ 0, %b0 ], [ %c1, %b1 ]
  %p = getelementptr inbounds i16, ptr %a, i32 %i
  %q = getelementptr inbounds [16 x i8], ptr %p, i32 %kk
  store i8 1, ptr %q
  %c1 = add i16 %c, 1
  %i1 = zext i16 %c1 to i32
  %d = icmp slt i16 %c1, %n
  br i1 %d, label %b1, label %b2

b2:
  ret i16 %c1
}
",
    );
    assert!(after.contains("%invariant.gep = getelementptr inbounds [16 x i8]"), "{after}");
    assert!(after.contains("%gep = getelementptr inbounds i16"), "{after}");
}

/// The exposed allocas were found on every LICM run that had a loop, a scan of
/// the function (3.1 M of d_faces -O2 in 17 runs), though only a loop that may
/// throw asks for them.
#[test]
fn test_the_exposed_allocas_are_not_scanned_for_a_loop_that_cannot_throw() {
    let text = "@g = global i16 0\n\ndefine i16 @f(i16 %n) {\nb0:\n  br label %h\nh:\n  %i = phi i16 [ 0, %b0 ], [ %j, %b ]\n  %c = icmp slt i16 %i, %n\n  br i1 %c, label %b, label %x\nb:\n  %v = load i16, ptr @g\n  %j = add i16 %i, %v\n  br label %h\nx:\n  ret i16 %i\n}\n";
    let before = llrm_mir::counts::get(llrm_mir::counts::Event::ExposedFrames);
    let mut module = parsed(text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: true, speculate: true });
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("licm runs");
    // The manager's own `ExposedFrames` is the one scan; LICM made a second.
    let scans = llrm_mir::counts::get(llrm_mir::counts::Event::ExposedFrames) - before;
    assert!(scans <= 1, "the function was scanned for exposed allocas {scans} times");
}

/// A machine that states the i486's register count and nothing else.
struct I486(llrm_mir::target::Neutral);

impl llrm_mir::target::Machine for I486 {
    fn tti(&self) -> Option<std::rc::Rc<dyn llrm_mir::tti::Tti>> {
        Some(std::rc::Rc::new(llrm_x86::tti::X86::of_cpu("i486").unwrap()))
    }

    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        self.0.foreign_span(selectors, offsets, width)
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        self.0.spaces()
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        self.0.costs()
    }

    fn registers(&self) -> i64 {
        self.0.registers()
    }

    fn call_registers(&self) -> i64 {
        self.0.call_registers()
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        self.0.address_forms()
    }
}

/// Ten globals the loop adds the index to, each a promotion candidate.
fn ten_counters() -> String {
    let globals: String = (1..=10).map(|k| format!("@g{k} = global i16 0\n")).collect();
    let body: String = (1..=10)
        .map(|k| format!("  %x{k} = load i16, ptr @g{k}\n  %y{k} = add i16 %x{k}, %i\n  store i16 %y{k}, ptr @g{k}\n"))
        .collect();
    format!(
        "{globals}
define i16 @f(i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
{body}  %i1 = add i16 %i, 1
  %c = icmp eq i16 %i1, %n
  br i1 %c, label %b2, label %b1

b2:
  ret i16 0
}}
"
    )
}

/// How many of the counters `Licm` promotes on `machine`.
fn counters_promoted(machine: impl llrm_mir::target::Machine + 'static) -> usize {
    DECIDED.with(|all| *all.borrow_mut() = Decisions::default());
    let mut module = parsed(&ten_counters());
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: false, speculate: true });
    passes.run_module(&mut module, std::rc::Rc::new(machine)).expect("runs");
    DECIDED.with(|all| all.borrow().promoted.len())
}

/// Promoting every global the nbody loop nest touches (17 at m16 -Os) held
/// them all across the inner loops, and they were spilled to the frame: the
/// m16 target has six registers, so only as many promotions as the loop's own
/// values leave room for are made.
#[test]
fn test_a_promotion_past_the_target_register_count_is_refused() {
    assert_eq!(counters_promoted(llrm_mir::target::Neutral), 10);
    assert_eq!(counters_promoted(llrm_x86_m16::Dos::default()), 3);
    assert_eq!(counters_promoted(I486(llrm_mir::target::Neutral)), 5);
}

/// Ten globals the loop only stores to, each a promotion candidate.
fn ten_stored() -> String {
    let globals: String = (1..=10).map(|k| format!("@g{k} = global i16 0\n")).collect();
    let body: String = (1..=10).map(|k| format!("  store i16 %i, ptr @g{k}\n")).collect();
    format!(
        "{globals}
define i16 @f(i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
{body}  %i1 = add i16 %i, 1
  %c = icmp eq i16 %i1, %n
  br i1 %c, label %b2, label %b1

b2:
  ret i16 0
}}
"
    )
}

/// A location the loop only stores to holds no register across the loop, so
/// the cap does not count it. Charged as a load-promoted one, the pass that
/// sinks the stores of locations the pass before it loaded found no room:
/// jpeg_single m32 -O2 kept two stores a byte in its checksum loop.
#[test]
fn test_a_store_only_promotion_is_not_charged_a_register() {
    let promoted = |machine: Box<dyn llrm_mir::target::Machine>| {
        DECIDED.with(|all| *all.borrow_mut() = Decisions::default());
        let mut module = parsed(&ten_stored());
        let mut passes = llrm_mir::passes::PassManager::default();
        passes.add(Licm { apply: false, speculate: true });
        passes.run_module(&mut module, std::rc::Rc::from(machine)).expect("runs");
        DECIDED.with(|all| all.borrow().promoted.len())
    };
    assert_eq!(promoted(Box::new(llrm_x86_m16::Dos::default())), 10);
    assert_eq!(promoted(Box::new(I486(llrm_mir::target::Neutral))), 10);
}

/// The dot loop of a Nib zip (`i < a && i < b`, both bounds invariant) tested
/// both bounds on every trip: the pin's LICM (`hoistMinMax`) makes one compare
/// against `umin(a, b)`, the `umin` in the preheader.
#[test]
fn test_two_invariant_upper_bounds_become_one_compare_against_their_min() {
    let after = licm_printed(
        "define i16 @f(i16 %a, i16 %b) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %i1 = add nuw i16 %i, 1
  %c1 = icmp ult i16 %i1, %a
  %c2 = icmp ult i16 %i1, %b
  %c = select i1 %c1, i1 %c2, i1 false
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %i1
}
",
    );
    let entry = block_of(&after, "b0");
    assert!(entry.iter().any(|line| line.contains("%invariant.umin = call i16 @llvm.umin.i16")), "{after}");
    let body = block_of(&after, "b1");
    assert_eq!(body.iter().filter(|line| line.contains("icmp")).count(), 1, "{after}");
    assert!(body.iter().any(|line| line.contains("icmp ult i16 %i1, %invariant.umin")), "{after}");
}

/// A select does not evaluate its second compare on every path, so the bound
/// the preheader now reads is frozen; the logical `and` form needs no freeze.
#[test]
fn test_a_select_freezes_the_second_bound_and_an_and_does_not() {
    let select = "  %c = select i1 %c1, i1 %c2, i1 false\n";
    let and = "  %c = and i1 %c1, %c2\n";
    for (logic, frozen) in [(select, true), (and, false)] {
        let text = format!(
            "define i16 @f(i16 %a, i16 %b) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %i1 = add nuw i16 %i, 1
  %c1 = icmp ult i16 %i1, %a
  %c2 = icmp ult i16 %i1, %b
{logic}  br i1 %c, label %b1, label %b2

b2:
  ret i16 %i1
}}
"
        );
        let after = licm_printed(&text);
        let entry = block_of(&after, "b0");
        assert_eq!(entry.iter().any(|line| line.contains("freeze")), frozen, "{after}");
        assert!(entry.iter().any(|line| line.contains("llvm.umin.i16")), "{after}");
    }
}

/// `||` of two upper-bound tests is the min's inverse: exit when `i >= min`.
/// Signed lower bounds take the max.
#[test]
fn test_or_of_bounds_and_signed_lower_bounds() {
    let or = licm_printed(
        "define i16 @f(i16 %a, i16 %b) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %i1 = add nuw i16 %i, 1
  %c1 = icmp uge i16 %i1, %a
  %c2 = icmp uge i16 %i1, %b
  %c = or i1 %c1, %c2
  br i1 %c, label %b2, label %b1

b2:
  ret i16 %i1
}
",
    );
    assert!(block_of(&or, "b0").iter().any(|line| line.contains("llvm.umin.i16")), "{or}");
    let max = licm_printed(
        "define i16 @f(i16 %a, i16 %b) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %i1 = add nuw i16 %i, 1
  %c1 = icmp sgt i16 %i1, %a
  %c2 = icmp sgt i16 %i1, %b
  %c = and i1 %c1, %c2
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %i1
}
",
    );
    assert!(block_of(&max, "b0").iter().any(|line| line.contains("llvm.smax.i16")), "{max}");
}

/// Mixed directions (`i < a && i > b`) have no single bound: untouched.
#[test]
fn test_opposite_directions_are_not_merged() {
    let after = licm_printed(
        "define i16 @f(i16 %a, i16 %b) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %i1 = add nuw i16 %i, 1
  %c1 = icmp ult i16 %i1, %a
  %c2 = icmp ugt i16 %i1, %b
  %c = and i1 %c1, %c2
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %i1
}
",
    );
    assert!(!after.contains("llvm.umin") && !after.contains("llvm.umax"), "{after}");
}

/// The loops of @f that `seeded` lets reach the memory graph.
fn seeded_loops(text: &str) -> Vec<bool> {
    use llrm_analysis::testing::{DOS, layout};
    use llrm_mir::passes::{Analyses, Outer};
    let module = parsed(&format!("{DOS}{text}"));
    let (layout, outer) = (layout(&module), Outer::of(&module, None));
    let mut module = module;
    let (context, function) = module.function_mut("f").expect("@f");
    let mut analyses = Analyses::new(std::rc::Rc::new(outer));
    let shape = analyses.get::<llrm_analysis::cfg::Shape>(context, &layout, function);
    let innermost = super::innermost_loops(&shape);
    shape.loops.iter().map(|one| super::seeded(function, &shape, &innermost, one)).collect()
}

/// #6706 widened the `seeded` look to any `and`, `or` or select of a compare:
/// nearly every loop then built MemorySSA and walked sink/hoist, licm 43.2M to
/// 198.9M and mssa 88.2M to 165.6M instructions on memory_expansion.ll. A
/// logical operation of two compares of variant values seeds nothing.
#[test]
fn a_logical_operation_of_variant_compares_does_not_seed_a_loop() {
    let text = "define i16 @f(i16 %n, i16 %m) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %inc, %loop ]
  %j = phi i16 [ 0, %entry ], [ %jnc, %loop ]
  %a = icmp ult i16 %i, %j
  %b = icmp slt i16 %j, %i
  %c = and i1 %a, %b
  %inc = add i16 %i, 1
  %jnc = add i16 %j, 2
  br i1 %c, label %loop, label %exit
exit:
  ret i16 %inc
}
";
    assert_eq!(seeded_loops(text), [false]);
}

/// The look #6706 added still seeds the loop it can act on: two compares of
/// one variant value against two invariant bounds.
#[test]
fn two_invariant_bounds_of_one_variant_value_seed_a_loop() {
    let text = "define i16 @f(i16 %n, i16 %m) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %inc, %loop ]
  %a = icmp ult i16 %i, %n
  %b = icmp ult i16 %i, %m
  %c = and i1 %a, %b
  %inc = add i16 %i, 1
  br i1 %c, label %loop, label %exit
exit:
  ret i16 %inc
}
";
    assert_eq!(seeded_loops(text), [true]);
}

/// Every alias query between two stack slots asked `exposes` for each slot's
/// uses afresh: LICM's alias queries took no `ExposedFrames`, and jpeg_fixed
/// spent 11 of the pass's 36 M Ir in those scans. LICM reads the manager's
/// set once.
#[test]
fn test_licm_alias_queries_do_not_rescan_each_slots_uses() {
    let text = "define void @f(i16 %n) {
b0:
  %a = alloca i16
  %b = alloca i16
  %c = alloca i16
  store i16 0, ptr %a
  br label %head
head:
  %i = phi i16 [ 0, %b0 ], [ %j, %head ]
  %x = load i16, ptr %a
  %y = load i16, ptr %c
  %s = add i16 %x, %y
  store i16 %s, ptr %b
  %j = add i16 %i, 1
  %t = icmp slt i16 %j, %n
  br i1 %t, label %head, label %exit
exit:
  ret void
}
";
    let before = llrm_analysis::frameescape::scans();
    decided(text);
    assert_eq!(llrm_analysis::frameescape::scans() - before, 0);
}

/// `hoistSubAddAssociation`'s three shapes, `a` and `b` invariant:
/// `(LV + a) - b` to `LV + (a - b)`, `(LV - a) - b` to `LV - (a + b)`,
/// `(LV - a) + b` (either order) to `LV + (b - a)`. llrm kept the invariant
/// sub or add in the loop, where `opt -passes=licm` hoists it.
#[test]
fn test_sub_and_add_chains_with_invariant_operands_are_reassociated_out() {
    let text = |inner: &str, outer: &str| {
        format!(
            "define i16 @f(i16 %n, i16 %a, i16 %b) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %x = {inner}
  %y = {outer}
  %i1 = add i16 %i, %y
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %i1
}}
"
        )
    };
    let cases = [
        ("add i16 %i, %a", "sub i16 %x, %b", "%invariant.op = sub i16 %a, %b", "%y.reass = add i16 %i, %invariant.op"),
        ("sub i16 %i, %a", "sub i16 %x, %b", "%invariant.op = add i16 %a, %b", "%y.reass = sub i16 %i, %invariant.op"),
        ("sub i16 %i, %a", "add i16 %x, %b", "%invariant.op = sub i16 %b, %a", "%y.reass = add i16 %i, %invariant.op"),
        ("sub i16 %i, %a", "add i16 %b, %x", "%invariant.op = sub i16 %b, %a", "%y.reass = add i16 %i, %invariant.op"),
    ];
    for (inner, outer, invariant, rebuilt) in cases {
        let after = licm_printed(&text(inner, outer));
        assert!(block_of(&after, "b0").iter().any(|line| line.contains(invariant)), "{inner} / {outer}\n{after}");
        let body = block_of(&after, "b1");
        assert!(body.iter().any(|line| line.contains(rebuilt)), "{inner} / {outer}\n{after}");
        assert!(body.iter().all(|line| !line.contains("%x = ")), "{inner} / {outer}\n{after}");
    }
}

/// `hoistAddSub`: `LV + C1 < C2` is `LV < C2 - C1` and `LV - C1 < C2` is
/// `LV < C1 + C2` when the move cannot overflow; `C1 - LV < C2` is `LV > C1 -
/// C2`. llrm kept the add in the loop, where `opt -passes=licm` hoists the
/// bound.
#[test]
fn test_a_compare_of_an_add_or_sub_with_an_invariant_operand_is_reassociated_out() {
    let text = |operation: &str, flags: &str, compare: &str, p: &str, q: &str| {
        format!(
            "define i16 @f(i16 %n, i16 %a, i16 %b) {{
b0:
  %p = {p}
  %q = {q}
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %x = {operation} {flags} i16 {compare}
  %go = icmp ult i16 %x, %q
  %gz = zext i1 %go to i16
  %j = add i16 %i, %gz
  %i1 = add i16 %j, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %i1
}}
"
        )
    };
    // add: LV + C1 u< C2.
    let after = licm_printed(&text("add", "nuw", "%i, %p", "and i16 %a, 255", "or i16 %b, 255"));
    assert!(block_of(&after, "b0").iter().any(|line| line.contains("%invariant.op = sub nuw i16 %q, %p")), "{after}");
    assert!(block_of(&after, "b1").iter().any(|line| line.contains("icmp ult i16 %i, %invariant.op")), "{after}");
    assert!(block_of(&after, "b1").iter().all(|line| !line.contains("%x = ")), "{after}");
    // sub: LV - C1 u< C2.
    let after = licm_printed(&text("sub", "nuw", "%i, %p", "and i16 %a, 255", "and i16 %b, 255"));
    assert!(block_of(&after, "b0").iter().any(|line| line.contains("%invariant.op = add nuw i16 %p, %q")), "{after}");
    // C1 - LV u< C2: LV u> C1 - C2.
    let after = licm_printed(&text("sub", "nuw", "%p, %i", "or i16 %a, 255", "and i16 %b, 255"));
    assert!(block_of(&after, "b0").iter().any(|line| line.contains("%invariant.op = sub nuw i16 %p, %q")), "{after}");
    assert!(block_of(&after, "b1").iter().any(|line| line.contains("icmp ugt i16 %i, %invariant.op")), "{after}");
    // No wrap flag: untouched.
    let after = licm_printed(&text("add", "", "%i, %p", "and i16 %a, 255", "or i16 %b, 255"));
    assert!(!after.contains("invariant.op"), "{after}");
}

/// `m_NUWAddLike`: an `or disjoint` is the add `hoistAdd` takes, so `i | 1 u<
/// 999` in a loop unrolled by two is `i u< 998` (wasm `floats` -O3 kept the
/// `or` and its constant: +1000 instructions).
#[test]
fn test_a_compare_of_a_disjoint_or_with_an_invariant_operand_is_reassociated_out() {
    let after = licm_printed(
        "define i16 @f(i16 %n, i16 %a) {
b0:
  %p = and i16 %a, 255
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %x = or disjoint i16 %i, %p
  %go = icmp ult i16 %x, 999
  %gz = zext i1 %go to i16
  %j = add i16 %i, %gz
  %i1 = add i16 %j, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %i1
}
",
    );
    assert!(block_of(&after, "b0").iter().any(|line| line.contains("%invariant.op = sub nuw i16 999, %p")), "{after}");
    assert!(block_of(&after, "b1").iter().all(|line| !line.contains("%x = ")), "{after}");
}

/// `hoistMulAddAssociation`: `(a * x + b * y) * c` with `a`, `b`, `c` invariant
/// is `(a * c) * x + (b * c) * y`, the two products made in the preheader.
/// llrm left the whole tree in the loop, where `opt -passes=licm` hoists them.
#[test]
fn test_a_product_of_a_sum_of_products_with_invariant_factors_is_reassociated_out() {
    let text = "define i16 @f(i16 %n, i16 %a, i16 %b, i16 %c) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %x = mul i16 %a, %i
  %y = mul i16 %i, %b
  %s = add nsw i16 %x, %y
  %t = mul i16 %s, %c
  %i1 = add i16 %i, %t
  %k = icmp slt i16 %i1, %n
  br i1 %k, label %b1, label %b2

b2:
  ret i16 %i1
}
";
    let after = licm_printed(text);
    let pre = block_of(&after, "b0");
    assert!(pre.iter().any(|line| line.contains("= mul i16 %a, %c")), "{after}");
    assert!(pre.iter().any(|line| line.contains("= mul i16 %b, %c")), "{after}");
    let body = block_of(&after, "b1");
    assert!(body.iter().all(|line| !line.contains("%t = ")), "{after}");
    assert!(body.iter().any(|line| line.contains("%x.reass = mul i16 %factor.op.mul1, %i")), "{after}");
}

/// mul_constants m64 -O2: `sum = sum * 31 + x * K` unrolled four times is a
/// chain of products of sums. The pin reassociates the instruction it has just
/// made, in the same walk, until one multiply of `sum` is left (31, 961, 29791,
/// 923521); llrm left alone whatever an earlier reassociation of the loop had
/// replaced, so two `mul 31` of the chain stayed in the body (+931 B over the
/// pin's whole file).
#[test]
fn test_a_chain_of_products_of_sums_is_reassociated_until_no_multiply_by_the_factor_is_left() {
    let text = "define i64 @f() {
b0:
  br label %b1

b1:
  %p = phi i64 [ 2654435769, %b0 ], [ %x4, %b1 ]
  %c = phi i64 [ 0, %b0 ], [ %n, %b1 ]
  %s = phi i64 [ 47235554646, %b0 ], [ %s4, %b1 ]
  %m1 = mul i64 %p, 1664525
  %x1 = add i64 %m1, 1013904223
  %t1 = mul i64 %s, 31
  %u1 = mul i64 %x1, 7
  %s1 = add i64 %u1, %t1
  %m2 = mul i64 %x1, 1664525
  %x2 = add i64 %m2, 1013904223
  %t2 = mul i64 %s1, 31
  %u2 = mul i64 %x2, 7
  %s2 = add i64 %u2, %t2
  %m3 = mul i64 %x2, 1664525
  %x3 = add i64 %m3, 1013904223
  %t3 = mul i64 %s2, 31
  %u3 = mul i64 %x3, 7
  %s3 = add i64 %u3, %t3
  %m4 = mul i64 %x3, 1664525
  %x4 = add i64 %m4, 1013904223
  %t4 = mul i64 %s3, 31
  %u4 = mul i64 %x4, 7
  %s4 = add i64 %u4, %t4
  %n = add nuw nsw i64 %c, 4
  %e = icmp eq i64 %n, 200
  br i1 %e, label %b2, label %b1

b2:
  ret i64 %s4
}
";
    let after = licm_printed(text);
    let body = block_of(&after, "b1");
    let left: Vec<_> =
        body.iter().filter(|line| line.contains("= mul i64 ") && line.trim_end().ends_with(", 31")).collect();
    assert!(left.is_empty(), "{left:?}\n{after}");
}

/// `hoistMulAddAssociation` rewrites in place, so the next product of a
/// 4x-unrolled `t = t * 31 + x * 6` chain sees the factor already folded in:
/// `t * 923521 + ...`, no `* 31` left (pin LICM after the unroll,
/// PassBuilderPipelines.cpp#L1508). llrm decided on the loop as it stood and
/// skipped every product that read a replaced one: the running sum was
/// multiplied twice (`* 31` each) where the pin multiplies it once
/// (`mul_constants` -O2 wasm32, +3..4 B in each of 31 functions).
#[test]
fn test_a_chain_of_products_of_sums_is_reassociated_through_every_link() {
    let text = "define i32 @f() {
b0:
  br label %b1

b1:
  %n = phi i32 [ 200, %b0 ], [ %n1, %b1 ]
  %x0 = phi i32 [ 7, %b0 ], [ %x4, %b1 ]
  %t0 = phi i32 [ 9, %b0 ], [ %t4, %b1 ]
  %a1 = mul i32 %x0, 1664525
  %x1 = add i32 %a1, 1013904223
  %m1 = mul i32 %t0, 31
  %c1 = mul i32 %x1, 6
  %t1 = add i32 %c1, %m1
  %a2 = mul i32 %x1, 1664525
  %x2 = add i32 %a2, 1013904223
  %m2 = mul i32 %t1, 31
  %c2 = mul i32 %x2, 6
  %t2 = add i32 %c2, %m2
  %a3 = mul i32 %x2, 1664525
  %x3 = add i32 %a3, 1013904223
  %m3 = mul i32 %t2, 31
  %c3 = mul i32 %x3, 6
  %t3 = add i32 %c3, %m3
  %a4 = mul i32 %x3, 1664525
  %x4 = add i32 %a4, 1013904223
  %m4 = mul i32 %t3, 31
  %c4 = mul i32 %x4, 6
  %t4 = add i32 %c4, %m4
  %n1 = add nsw i32 %n, -4
  %k = icmp eq i32 %n1, 0
  br i1 %k, label %b2, label %b1

b2:
  ret i32 %t4
}
";
    let after = licm_printed(text);
    let body = &after[after.find("\nb1:\n").expect("b1")..];
    // Only the phi `%t0` is multiplied in the loop; the sums `%t1..%t3` are
    // not.
    let sums = body.lines().filter(|line| line.contains("= mul i32 %t")).count();
    assert_eq!(sums, 1, "{sums} products of the running sum in the loop, want 1:\n{after}");
}

/// `Instruction::mayHaveSideEffects` of a call was the list of every location
/// it writes (a `Vec` of `MemRef`s, `memory::stated` read three times over)
/// asked for a yes: 1 M Ir of a jpeg_fixed -O2 compile in `sink_region` alone.
/// Whether it writes is derived from its attributes, with no list.
#[test]
fn test_whether_a_call_in_a_loop_has_side_effects_derives_no_list() {
    let text = "declare i16 @g(i16) nounwind willreturn
define void @f(i16 %n) {
b0:
  br label %head
head:
  %i = phi i16 [ 0, %b0 ], [ %j, %head ]
  %u = call i16 @g(i16 %i)
  %v = call i16 @g(i16 %n)
  %j = add i16 %i, 1
  %t = icmp slt i16 %j, %n
  br i1 %t, label %head, label %exit
exit:
  ret void
}
";
    let before = llrm_analysis::memoryssa::lazy_lists();
    let mut module = parsed(text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: true, speculate: true });
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("licm");
    let made = llrm_analysis::memoryssa::lazy_lists() - before;
    assert_eq!(made, 0, "{made} access lists made for a loop of calls");
}

/// A loop with nothing to promote priced its register pressure anyway: 1.6 M
/// Ir of a jpeg_fixed -O2 compile, in 121 loops. It is priced only for a loop
/// with a candidate.
#[test]
fn test_a_loop_with_nothing_to_promote_prices_no_register_pressure() {
    let text = "define i16 @f(i16 %n) {
b0:
  br label %head
head:
  %i = phi i16 [ 0, %b0 ], [ %j, %head ]
  %j = add i16 %i, 1
  %t = icmp slt i16 %j, %n
  br i1 %t, label %head, label %exit
exit:
  ret i16 %j
}
";
    let before = super::pressure::COUNTED.with(std::cell::Cell::get);
    let mut module = parsed(text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: true, speculate: true });
    passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("licm");
    let priced = super::pressure::COUNTED.with(std::cell::Cell::get) - before;
    assert_eq!(priced, 0, "pressure priced {priced} times for a loop with no memory access");
}

/// LICM hoists the load through `MemorySSAUpdater` and says it kept MemorySSA
/// (LICM.cpp `PA.preserve<MemorySSAAnalysis>()`): the graph asked for after it
/// is the one it updated. Before, the updater's graph was dropped with the
/// pass's change and the next pass computed MemorySSA again (2 computations,
/// not 1; 21 LICM misses against the pin's 11 on jpeg_fixed).
#[test]
fn test_the_memory_graph_licm_updated_is_the_one_the_next_pass_gets() {
    let text = looped(GLOBALS, "", "", "  %v = load i16, ptr @g\n");
    let (hits, misses) = crate::testing::mssa_misses_over(
        &text,
        vec![
            Box::new(crate::testing::AsksMssa { forgets: false }),
            Box::new(Licm { apply: true, speculate: true }),
            Box::new(crate::testing::AsksMssa { forgets: false }),
        ],
    );
    assert_eq!(misses, 1, "MemorySSA was computed again after LICM updated it");
    assert!(hits >= 2, "LICM and the pass after it read the one graph: {hits} hits");
}

/// How many times `@f`'s loops were scanned for a block that may throw
/// (`Decisions::safety_scans`).
fn safety_scans(
    text: &str,
    apply: bool,
) -> usize {
    DECIDED.with(|all| *all.borrow_mut() = Decisions::default());
    let mut module = parsed(text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply, speculate: true });
    passes
        .run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral))
        .unwrap_or_else(|error| panic!("{error}\n{text}"));
    DECIDED.with(|all| all.borrow().safety_scans)
}

/// How many predecessor lookups the loops' safety info made.
fn safety_pred_lookups(text: &str) -> usize {
    DECIDED.with(|all| *all.borrow_mut() = Decisions::default());
    let mut module = parsed(text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(Licm { apply: true, speculate: true });
    passes
        .run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral))
        .unwrap_or_else(|error| panic!("{error}\n{text}"));
    DECIDED.with(|all| all.borrow().safety_pred_lookups)
}

/// A loop of `n` diamonds, each arm holding a hoistable add.
fn diamond_chain(n: usize) -> String {
    let mut text = String::from(
        "@g = global i16 0\n\ndefine void @f(i16 %n, i16 %m) {\nb0:\n  br label %h\n\nh:\n  %i = phi i16 [ 0, %b0 ], [ %i1, %latch ]\n  br label %t0\n\n",
    );
    for k in 0..n {
        text += &format!(
            "t{k}:\n  %c{k} = icmp slt i16 %i, %m\n  br i1 %c{k}, label %l{k}, label %r{k}\n\nl{k}:\n  %x{k} = add i16 %n, {}\n  store i16 %x{k}, ptr @g\n  br label %t{}\n\nr{k}:\n  br label %t{}\n\n",
            k + 1,
            k + 1,
            k + 1
        );
    }
    text += &format!(
        "t{n}:\n  br label %latch\n\nlatch:\n  %i1 = add i16 %i, 1\n  %e = icmp slt i16 %i1, %m\n  br i1 %e, label %h, label %out\n\nout:\n  ret void\n}}\n"
    );
    text
}

/// A loop of 3n blocks asked for each arm whether it is guaranteed to execute
/// looked a block's predecessors up in the function once for every block the
/// walk back from each arm reached: 40 diamonds made 4,840 lookups, and
/// queens at -O3 (360-block loop, 53 runs of LICM) spent 2.1 G Ir there. The
/// lists of the loop's blocks are made once, so the lookups follow the blocks,
/// not their product.
#[test]
fn test_the_walk_back_from_each_block_reads_the_loops_predecessors_once() {
    let n = 40;
    let lookups = safety_pred_lookups(&diamond_chain(n));
    assert!(lookups <= 4 * 3 * n, "{lookups} predecessor lookups for a loop of {} blocks", 3 * n + 3);
}

/// The sinking, the hoisting and the promotion of one loop shared two
/// `LoopSafety` that each scanned the loop (153 loops of `jpeg_fixed` scanned
/// twice, 2 x 153 where the pin's one `ICFLoopSafetyInfo` per `runOnLoop`
/// scans 153 times). Decided without edits, the loop is scanned once.
#[test]
fn test_a_loop_decided_without_edits_is_scanned_for_a_throwing_block_once() {
    assert_eq!(safety_scans(&slot_loop("", "", "load i16, ptr %s"), false), 1);
}

/// With the edits made, what was found of the loop stands only until the next
/// edit: the sinking and hoisting found nothing to do and share their scan with
/// the first promotion round; the promotion edits the loop, so the second round
/// scans again. A scan kept across that edit would answer for a loop that is no
/// longer there.
#[test]
fn test_what_was_found_of_a_loop_is_dropped_by_the_next_edit() {
    assert_eq!(safety_scans(&slot_loop("", "", "load i16, ptr %s"), true), 2);
}

/// `Instruction::willReturn` is false for any volatile operation, a load
/// included: a loop with one may not reach its store, so the store to a global
/// is not guaranteed and stays in the loop. LICM sank it after the loop and
/// divided the loop's size by one unit the pin's did not: div_constants_long
/// peeled twice, +137 B per function on wasm32.
#[test]
fn a_volatile_load_before_a_global_store_keeps_the_store_in_the_loop() {
    for (volatile, stays) in [("", false), ("volatile ", true)] {
        let text = format!(
            "@g = global i16 5
@h = global i16 7

define i16 @f(i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, 12
  br i1 %c, label %b2, label %b3

b2:
  %x = load i16, ptr @g
  %v = load {volatile}i16, ptr @h
  %y = add i16 %x, %v
  store i16 %y, ptr @g
  %i1 = add i16 %i, 1
  br label %b1

b3:
  %r = load i16, ptr @g
  ret i16 %r
}}
"
        );
        let (_, after) = applied(&text, &trips());
        let kept = block_of(&after, "b2").iter().any(|line| line.starts_with("store i16") && line.contains("@g"));
        assert_eq!(kept, stays, "{volatile}: {after}");
    }
}

/// DEVIATE LM2: `@a[o]` read behind a guard of the inner loop is invariant
/// there and in bounds because the outer loop counts `o` below 6; with `o`
/// unbounded the guarded load stays. Cost before the fix: the load was redone
/// on every inner trip (nbody_fixed arm64 -O3: 723,134 instructions against
/// 645,136).
#[test]
fn test_a_guarded_load_indexed_by_an_outer_counter_is_hoisted_when_it_stays_in_the_object() {
    let nest = |bound: &str| {
        format!(
            "@a = global [6 x i64] zeroinitializer\n\
             define i64 @f(i64 %n, i64 %k) {{\n\
             entry:\n  br label %outer\n\
             outer:\n  %o = phi i64 [ 0, %entry ], [ %o.next, %outer.latch ]\n  %acc = phi i64 [ 0, %entry ], [ %acc.in, %outer.latch ]\n  br label %inner\n\
             inner:\n  %j = phi i64 [ 0, %outer ], [ %j.next, %inner.latch ]\n  %s = phi i64 [ %acc, %outer ], [ %s.next, %inner.latch ]\n  %guard = icmp ne i64 %j, %k\n  br i1 %guard, label %then, label %inner.latch\n\
             then:\n  %p = getelementptr i64, ptr @a, i64 %o\n  %v = load i64, ptr %p\n  %t = add i64 %s, %v\n  br label %inner.latch\n\
             inner.latch:\n  %s.next = phi i64 [ %s, %inner ], [ %t, %then ]\n  %j.next = add nuw i64 %j, 1\n  %again = icmp ult i64 %j.next, %n\n  br i1 %again, label %inner, label %outer.latch\n\
             outer.latch:\n  %acc.in = phi i64 [ %s.next, %inner.latch ]\n  %o.next = add nuw nsw i64 %o, 1\n  %more = icmp ult i64 %o.next, {bound}\n  br i1 %more, label %outer, label %exit\n\
             exit:\n  ret i64 %acc.in\n}}\n"
        )
    };
    let on = |bound: &str| named_on(&nest(bound), RangeDeref(llrm_mir::target::Neutral), |all| all.hoisted.clone());
    assert_eq!(on("6"), ["p", "v"]);
    assert_eq!(on("7"), ["p"]);
    // The pin's rule, the default: the load stays.
    assert_eq!(decided(&nest("6")), ["p"]);
}

/// A machine that opts in to `licm_range_deref`.
struct RangeDeref(llrm_mir::target::Neutral);

struct RangeTti;

use llrm_mir::tti::Tti as _;

impl llrm_mir::tti::Tti for RangeTti {
    fn licm_range_deref(&self) -> bool {
        true
    }

    fn cmp_sel_cost(
        &self,
        op: llrm_mir::tti::CmpSel,
        ty: llrm_mir::tti::Ty,
        kind: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.cmp_sel_cost(op, ty, kind)
    }

    fn memory_op_cost(
        &self,
        op: llrm_mir::tti::MemoryOp,
        ty: llrm_mir::tti::Ty,
        kind: llrm_mir::tti::CostKind,
        stored_constant: bool,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.memory_op_cost(op, ty, kind, stored_constant)
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.cf_instr_cost(phi, kind)
    }

    fn int_imm_cost(
        &self,
        imm: i128,
        bits: u32,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.int_imm_cost(imm, bits)
    }

    fn int_imm_cost_inst(
        &self,
        op: llrm_mir::tti::ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.int_imm_cost_inst(op, index, imm, bits)
    }

    fn is_truncate_free(
        &self,
        from: llrm_mir::tti::Ty,
        to: llrm_mir::tti::Ty,
    ) -> bool {
        llrm_mir::tti::Basic.is_truncate_free(from, to)
    }

    fn is_legal_addressing_mode(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        llrm_mir::tti::Basic.is_legal_addressing_mode(base_gv, base_offset, has_base_reg, scale)
    }

    fn cast_cost(
        &self,
        op: llrm_mir::tti::CastKind,
        dst: llrm_mir::tti::Ty,
        src: llrm_mir::tti::Ty,
        kind: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.cast_cost(op, dst, src, kind)
    }

    fn arithmetic_cost(
        &self,
        op: llrm_mir::tti::ArithOp,
        ty: llrm_mir::tti::Ty,
        kind: llrm_mir::tti::CostKind,
        first: llrm_mir::tti::OperandInfo,
        second: llrm_mir::tti::OperandInfo,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.arithmetic_cost(op, ty, kind, first, second)
    }
}

impl llrm_mir::target::Machine for RangeDeref {
    fn tti(&self) -> Option<std::rc::Rc<dyn llrm_mir::tti::Tti>> {
        Some(std::rc::Rc::new(RangeTti))
    }

    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        self.0.foreign_span(selectors, offsets, width)
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        self.0.spaces()
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        self.0.costs()
    }

    fn registers(&self) -> i64 {
        self.0.registers()
    }

    fn call_registers(&self) -> i64 {
        self.0.call_registers()
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        self.0.address_forms()
    }
}
