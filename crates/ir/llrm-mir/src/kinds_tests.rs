use crate::context::Context;
use crate::depends::Depends;
use crate::kinds::{self, Edits, Kinds, Reads};
use crate::module::{Function, InstId, Module, Operand};
use crate::opcode::{BinaryOp, Flags, Opcode};
use crate::{Position, parse};

const TEXT: &str = "define i16 @f(i16 %a, i1 %c, ptr %p, ptr %q) {
entry:
  %x = add i16 %a, 1
  %l = load i16, ptr %p
  %lp = load ptr, ptr %q
  store i16 %x, ptr %p
  store ptr %p, ptr %q
  %r = call i16 @g(i16 %l)
  br i1 %c, label %yes, label %no
yes:
  %y = mul i16 %l, 2
  br label %join
no:
  %n = sub i16 %x, 3
  br label %join
join:
  %m = phi i16 [ %y, %yes ], [ %n, %no ]
  %s = add i16 %m, %r
  ret i16 %s
}

declare i16 @g(i16)
";

fn module() -> Module {
    parse::module(TEXT).unwrap_or_else(|error| panic!("{error}"))
}

fn named(
    function: &Function,
    name: &str,
) -> InstId {
    function
        .walk()
        .find_map(|(_, inst)| {
            let result = function.instruction(inst).result?;
            (function.value(result).name.as_deref() == Some(name)).then_some(inst)
        })
        .unwrap_or_else(|| panic!("%{name}"))
}

fn stores(function: &Function) -> Vec<InstId> {
    function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(function.instruction(inst).opcode, Opcode::Store { .. }))
        .collect()
}

/// The class of an integer load told from a pointer load, a store of an integer
/// from a store of a pointer: the split a reader of pointers needs so that an
/// integer store does not reach it.
#[test]
fn a_load_and_a_store_are_told_by_the_class_of_the_value_they_carry() {
    let mut module = module();
    let (context, f) = module.function_mut("f").expect("@f");
    let (int_load, ptr_load) = (named(f, "l"), named(f, "lp"));
    let st = stores(f);
    assert_eq!(kinds::of(context, f, int_load), Kinds::LOAD_INT);
    assert_eq!(kinds::of(context, f, ptr_load), Kinds::LOAD_PTR);
    assert_eq!(kinds::of(context, f, st[0]), Kinds::STORE_INT);
    assert_eq!(kinds::of(context, f, st[1]), Kinds::STORE_PTR);
    assert!(Kinds::LOAD.contains(Kinds::LOAD_PTR) && !Kinds::INT.meets(Kinds::LOAD_PTR));
}

/// An edit of an integer store leaves a reader of pointer stores as it was, and
/// reaches one of integer stores; an edit that feeds the store reaches it
/// through the value, unless the flow stops at stores.
#[test]
fn an_integer_store_does_not_reach_a_reader_of_pointer_stores() {
    let mut first = module();
    let (context, f) = first.function_mut("f").expect("@f");
    let st = stores(f)[0];
    let a = Operand::Value(f.parameters()[0]);
    let mark = f.mark();
    f.set_operand(st, 0, a);
    let touched = f.touched_since(mark).expect("history");
    let ptr_stores = Reads::analysis(Kinds::STORE_PTR, Kinds::NONE, None);
    let int_stores = Reads::analysis(Kinds::STORE_INT, Kinds::NONE, None);
    assert!(touched.leaves(context, f, &ptr_stores));
    assert!(!touched.leaves(context, f, &int_stores));
    // The add that feeds the integer store: reached only through its value.
    let mut fresh = module();
    let (context, f) = fresh.function_mut("f").expect("@f");
    let x = named(f, "x");
    let mark = f.mark();
    f.set_operand(x, 1, a);
    let touched = f.touched_since(mark).expect("history");
    let flowing = Reads::analysis(Kinds::STORE_INT, Kinds::NONE, Some(Kinds::BRANCH));
    let stopping = Reads::analysis(Kinds::STORE_INT, Kinds::NONE, Some(Kinds::STORE));
    let none = Reads::analysis(Kinds::STORE_INT, Kinds::NONE, None);
    assert!(!touched.leaves(context, f, &flowing), "the add's value reaches the store");
    assert!(touched.leaves(context, f, &stopping), "the flow stops at stores");
    assert!(touched.leaves(context, f, &none), "no flow: the add alone changed");
}

/// The blocks an edit reached that a reader models, for a regional run.
#[test]
fn the_blocks_meeting_a_reader_are_those_of_the_instructions_it_models() {
    let mut module = module();
    let (context, f) = module.function_mut("f").expect("@f");
    let (y, join) = (named(f, "y"), f.layout()[3]);
    let a = Operand::Value(f.parameters()[0]);
    let mark = f.mark();
    f.set_operand(y, 1, a);
    let touched = f.touched_since(mark).expect("history");
    let phis = Reads::analysis(Kinds::PHI, Kinds::NONE, Some(Kinds::NONE));
    let found = touched.blocks_meeting(context, f, &phis).expect("a region");
    assert_eq!(found.iter().collect::<Vec<_>>(), [join], "the phi the mul feeds");
    let arith = Reads { kinds: Kinds::ARITH, keyed: Kinds::NONE, stops: None };
    assert_eq!(
        touched.blocks_meeting(context, f, &arith).expect("a region").iter().collect::<Vec<_>>(),
        [f.layout()[1]]
    );
}

/// A block made is a change of the CFG: a reader that sees `CFG` is reached, a
/// region cannot be named, and one that does not see it is not.
#[test]
fn a_block_made_reaches_a_reader_that_sees_the_cfg_and_names_no_region() {
    let mut module = module();
    let (context, f) = module.function_mut("f").expect("@f");
    let mark = f.mark();
    let made = f.create_block(Some("extra"));
    f.insert_block(made, None).expect("placed");
    let touched = f.touched_since(mark).expect("history");
    let sees = Reads::analysis(Kinds::LOAD, Kinds::NONE, None);
    let blind = Reads { kinds: Kinds::LOAD, keyed: Kinds::NONE, stops: None };
    assert!(!touched.leaves(context, f, &sees));
    assert!(touched.leaves(context, f, &blind));
    assert!(touched.blocks_meeting(context, f, &sees).is_none());
    assert!(touched.kinds(context, f).meets(Kinds::CFG));
}

thread_local! {
    /// The declaration the predicate form below is asked about.
    static DECLARED: std::cell::Cell<(Kinds, Kinds, Kinds)> = const { std::cell::Cell::new((Kinds::NONE, Kinds::NONE, Kinds::NONE)) };
}

fn models(
    context: &Context,
    f: &Function,
    inst: InstId,
) -> bool {
    DECLARED.with(|one| one.get().0).meets(kinds::of(context, f, inst))
}

fn keyed(
    context: &Context,
    f: &Function,
    inst: InstId,
) -> bool {
    DECLARED.with(|one| one.get().1).meets(kinds::of(context, f, inst))
}

/// The class of an opcode alone (every type of it), for a flow that stops
/// there.
fn stops_at(opcode: &Opcode) -> bool {
    let class = match opcode {
        Opcode::Store { .. } => Kinds::STORE,
        Opcode::Load { .. } => Kinds::LOAD,
        Opcode::Phi => Kinds::PHI,
        Opcode::Select => Kinds::SELECT,
        Opcode::Br | Opcode::Switch => Kinds::BRANCH,
        Opcode::Ret => Kinds::RET,
        Opcode::Call(_) => Kinds::CALL,
        Opcode::Invoke(_) => Kinds::INVOKE,
        _ => Kinds::NONE,
    };
    DECLARED.with(|one| one.get().2).meets(class)
}

/// The class form answers as `Depends` does, for each edit of a script and each
/// declaration of a few readers: one vocabulary in place of two.
#[test]
fn the_class_form_answers_as_the_predicate_form_does() {
    let declarations: Vec<(&str, Kinds, Kinds, Option<Kinds>)> = vec![
        ("pointer memory", Kinds::STORE_PTR | Kinds::LOAD_PTR, Kinds::NONE, Some(Kinds::STORE | Kinds::TERMINATOR)),
        ("all memory", Kinds::LOAD | Kinds::STORE | Kinds::CALL, Kinds::CALL, Some(Kinds::NONE)),
        ("integer values", Kinds::INT | Kinds::ARITH, Kinds::NONE, None),
        ("phis", Kinds::PHI, Kinds::PHI, Some(Kinds::BRANCH)),
        ("arithmetic and branches", Kinds::ARITH | Kinds::BRANCH, Kinds::NONE, Some(Kinds::RET)),
    ];
    let edits: Vec<(&str, Box<dyn Fn(&mut Function)>)> = vec![
        (
            "operand of the add",
            Box::new(|f| {
                let a = Operand::Value(f.parameters()[0]);
                f.set_operand(named(f, "x"), 1, a)
            }),
        ),
        (
            "erase the pointer store",
            Box::new(|f| {
                let st = stores(f)[1];
                f.erase(st).expect("erased")
            }),
        ),
        (
            "move the add",
            Box::new(|f| {
                let (l, x) = (named(f, "l"), named(f, "x"));
                f.move_to(x, Position::Before(l)).expect("moved");
            }),
        ),
        (
            "make an add",
            Box::new(|f| {
                let l = named(f, "l");
                let ty = f.instruction(l).ty;
                let a = Operand::Value(f.parameters()[0]);
                let made = f.create_instruction(Opcode::Binary(BinaryOp::Add), ty, vec![a, a], Flags::default(), None);
                f.insert(made, Position::Before(l)).expect("placed");
            }),
        ),
        ("flags of the mul", Box::new(|f| f.set_flags(named(f, "y"), Flags::default()))),
        (
            "retarget the branch",
            Box::new(|f| {
                let (entry, join) = (f.entry().unwrap(), f.layout()[3]);
                let br = f.terminator(entry).unwrap();
                f.set_operand(br, 1, Operand::Block(join));
            }),
        ),
    ];
    for (edit, apply) in &edits {
        for (name, models_of, keyed_of, stops_of) in &declarations {
            let mut module = module();
            let (context, f) = module.function_mut("f").expect("@f");
            let mark = f.mark();
            apply(f);
            let changes = f.changes_since(mark).expect("history").to_vec();
            let touched = f.touched_since(mark).expect("history");
            DECLARED.with(|one| one.set((*models_of, *keyed_of, stops_of.unwrap_or(Kinds::NONE))));
            let depends = Depends { models, keyed, flows: stops_of.map(|_| stops_at as fn(&Opcode) -> bool) };
            let by_predicates = depends.unaffected(&changes, context, f);
            let by_classes = touched.leaves(context, f, &Reads::analysis(*models_of, *keyed_of, *stops_of));
            assert_eq!(by_classes, by_predicates, "{edit} / {name}");
        }
    }
}

/// The digest of a window answers as the `Touched` it comes from does, for the
/// two flows passes declare, and two windows add: what a step reads of the
/// edits since it last ran is the sum of the windows between, found without the
/// log.
#[test]
fn a_window_digest_answers_as_its_touched_does_and_windows_add() {
    let readers = [
        Reads { kinds: Kinds::PHI | Kinds::BRANCH, keyed: Kinds::NONE, stops: Some(Kinds::NONE) },
        Reads { kinds: Kinds::STORE_INT, keyed: Kinds::NONE, stops: Some(Kinds::NONE) },
        Reads { kinds: Kinds::ARITH, keyed: Kinds::NONE, stops: None },
        Reads::analysis(Kinds::LOAD | Kinds::CALL, Kinds::CALL, None),
        Reads::analysis(Kinds::CAST_PTR, Kinds::NONE, Some(Kinds::NONE)),
    ];
    let mut fresh = module();
    let (context, f) = fresh.function_mut("f").expect("@f");
    let (a, first) = (Operand::Value(f.parameters()[0]), f.mark());
    f.set_operand(named(f, "y"), 1, a);
    let second = f.mark();
    let st = stores(f)[1];
    f.erase(st).expect("erased");
    let (one, two) =
        (f.edits_since(context, first).expect("history"), f.edits_since(context, second).expect("history"));
    let (whole, early) = (f.touched_since(first).expect("history"), f.touched_since(first).expect("history"));
    let _ = early;
    let mut summed = Edits::of(&f.touched_since(first).expect("history"), context, f);
    // The first window alone: the mul's operand, up to the erase.
    let mut front = f.touched_since(first).expect("history");
    front.erased = Default::default();
    let front = Edits::of(&front, context, f);
    let mut added = front.clone();
    added.merge(&two);
    for reads in &readers {
        assert_eq!(one.meets(reads), Some(!whole.leaves(context, f, reads)), "{reads:?}");
        assert_eq!(one.blocks_meeting(reads), whole.blocks_meeting(context, f, reads), "{reads:?}");
        assert_eq!(added.meets(reads), one.meets(reads), "windows add: {reads:?}");
    }
    assert!(Edits::default().is_empty());
    summed.merge(&Edits::default());
    assert_eq!(summed.meets(&readers[0]), one.meets(&readers[0]));
    let bounded = Reads { kinds: Kinds::STORE, keyed: Kinds::NONE, stops: Some(Kinds::BRANCH) };
    assert_eq!(one.meets(&bounded), None, "a bounded flow is the Touched's to answer");
}

/// A window of a drained log is digested from the changes taken, not from a
/// mark the log no longer holds.
#[test]
fn a_window_taken_from_the_log_is_digested_from_the_changes() {
    let mut fresh = module();
    let (context, f) = fresh.function_mut("f").expect("@f");
    let a = Operand::Value(f.parameters()[0]);
    let _ = f.take_changes();
    f.set_operand(named(f, "y"), 1, a);
    let taken = f.take_changes();
    let window = f.edits_of(context, &taken);
    assert!(window.meets(&Reads { kinds: Kinds::ARITH, keyed: Kinds::NONE, stops: None }) == Some(true));
    assert!(window.meets(&Reads { kinds: Kinds::STORE, keyed: Kinds::NONE, stops: None }) == Some(false));
}
