use llrm_analysis::testing::corpus;
use llrm_mir::interpret;

use crate::pipeline::{self, Applied};

// Enough for every corpus entry that finishes at all.
const FUEL: u64 = 2_000_000;

#[test]
fn the_pipeline_keeps_every_corpus_module_verifying_and_computing_the_same() {
    let applied = Applied { target: Some(std::rc::Rc::new(llrm_cycles::target::Dos::default())), ..Applied::default() };
    let mut ran = 0;
    for (name, mut module) in corpus() {
        let entry = module.named("main").filter(|&id| module.global(id).function().is_some_and(|one| !one.is_declaration() && one.parameters().is_empty()));
        let before = entry.map(|_| interpret::run(&module, "main", Vec::new(), FUEL));
        pipeline::applied(&mut module, &applied).unwrap_or_else(|error| panic!("{name}: {error}"));
        if let Some(Ok(before)) = before {
            assert_eq!(interpret::run(&module, "main", Vec::new(), FUEL), Ok(before), "{name}");
            ran += 1;
        }
    }
    assert!(ran > 0, "no corpus entry runs");
}

/// Commutes @f's add so that its constant comes first or last.
struct Commute {
    name: &'static str,
    constant_first: bool,
}

impl llrm_mir::passes::FunctionPass for Commute {
    fn name(&self) -> &'static str {
        self.name
    }

    fn run(&mut self, unit: &mut llrm_mir::passes::Unit, _: &mut llrm_mir::passes::Analyses) -> llrm_mir::passes::PreservedAnalyses {
        use llrm_mir::module::Operand;
        let (_, add) = unit.function.walk().find(|&(_, one)| unit.function.instruction(one).opcode == llrm_mir::opcode::Opcode::Binary(llrm_mir::opcode::BinaryOp::Add)).expect("an add");
        let operands = unit.function.instruction(add).operands.clone();
        if matches!(operands[0], Operand::Constant(_)) == self.constant_first {
            return llrm_mir::passes::PreservedAnalyses::all();
        }
        unit.function.set_operands(add, vec![operands[1], operands[0]]);
        llrm_mir::passes::PreservedAnalyses::none()
    }
}

/// Arenas never shrink, so before bodies were compared by their text two
/// passes undoing each other ran the whole size-scaled limit.
#[test]
#[should_panic(expected = "cycle after 1 rounds")]
fn passes_that_undo_each_other_stop_after_one_cycle() {
    let mut module = llrm_analysis::testing::parsed("define i16 @f(i16 %x) {\nb0:\n  %y = add i16 %x, 1\n  ret i16 %y\n}\n");
    let mut fixed = pipeline::Fixed::new(&Applied { only: Some("none".to_owned()), ..Applied::default() });
    fixed.only = false;
    fixed.passes = vec![Box::new(Commute { name: "first", constant_first: true }), Box::new(Commute { name: "last", constant_first: false })];
    crate::testing::managed(&mut module, fixed);
}

/// A loop counted in an internal global no code outside names, a
/// `nocallback` routine called each trip: the count is proven and the
/// loop copied out. GlobalsAA was never required, so every call wrote the
/// counter and FPDEEP's PRINT loop stayed rolled.
#[test]
fn a_call_keeps_no_global_it_cannot_name() {
    let mut module = llrm_analysis::testing::parsed(
        "@i = internal global i16 0

declare void @print(i16) nocallback

define void @main() {
b0:
  store i16 1, ptr @i
  br label %b1

b1:
  %v = load i16, ptr @i
  %go = icmp sle i16 %v, 3
  br i1 %go, label %b2, label %b3

b2:
  call void @print(i16 %v)
  %w = load i16, ptr @i
  %n = add i16 %w, 1
  store i16 %n, ptr @i
  br label %b1

b3:
  ret void
}
",
    );
    let applied = Applied { target: Some(std::rc::Rc::new(llrm_cycles::target::Dos::default())), ..Applied::default() };
    pipeline::applied(&mut module, &applied).unwrap();
    let text = llrm_mir::print::module(&module);
    for trip in 1..=3 {
        assert!(text.contains(&format!("call void @print(i16 {trip})")), "{text}");
    }
}

/// matmul8's init: an inner loop branching on `i == j` is peeled, then its
/// outer loop, which decides every branch. Peel ran once per body, so the
/// outer loop stayed rolled around eight undecided diamonds.
#[test]
fn a_loop_whose_inner_loop_was_peeled_is_peeled_in_turn() {
    let text = "define i16 @main() {
b0:
  %a = alloca [64 x i16]
  br label %outer

outer:
  %i = phi i16 [ 0, %b0 ], [ %in, %outerlatch ]
  %go = icmp slt i16 %i, 8
  br i1 %go, label %inner, label %done

inner:
  %j = phi i16 [ 0, %outer ], [ %jn, %latch ]
  %more = icmp slt i16 %j, 8
  br i1 %more, label %body, label %outerlatch

body:
  %row = shl i16 %i, 3
  %at = add i16 %row, %j
  %slot = getelementptr inbounds i16, ptr %a, i16 %at
  %same = icmp eq i16 %i, %j
  br i1 %same, label %diagonal, label %other

diagonal:
  store i16 2, ptr %slot
  br label %latch

other:
  %sum = add i16 %i, %j
  %r = srem i16 %sum, 3
  store i16 %r, ptr %slot
  br label %latch

latch:
  %jn = add i16 %j, 1
  br label %inner

outerlatch:
  %in = add i16 %i, 1
  br label %outer

done:
  %p = getelementptr inbounds i16, ptr %a, i16 13
  %v = load i16, ptr %p
  %q = getelementptr inbounds i16, ptr %a, i16 18
  %w = load i16, ptr %q
  %t = add i16 %v, %w
  ret i16 %t
}
";
    let mut module = llrm_analysis::testing::parsed(text);
    let before = interpret::run(&module, "main", Vec::new(), FUEL);
    let applied = Applied { target: Some(std::rc::Rc::new(llrm_cycles::target::Dos::default())), ..Applied::default() };
    pipeline::applied(&mut module, &applied).unwrap();
    let after = llrm_mir::print::module(&module);
    assert_eq!(interpret::run(&module, "main", Vec::new(), FUEL), before, "{after}");
    assert!(!after.contains(" phi "), "{after}");
}
