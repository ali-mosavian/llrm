//! The LazyValueInfo cache is judged by replaying what it was asked and told
//! (`LLRM_CHECK_CAPTURES`), not against one lone fresh query. Its ranges depend
//! on which question came first: asking the counter's successor before the
//! counter holds [302, 301) for `%i` of a loop counting 1..300, where a lone
//! query gives [1, 0); both contain {1..300}. Judged against the lone query
//! that was `lvi held but stale: [(BlockId(6), Value(ValueId(84)))]` on
//! bench/bintree (nib, -O2), the cache of a body a rolled-back edit left as it
//! was. A bound changed under a cache that was not told must still fail.
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::lazyvalueinfo::{Env, LazyValueInfo, LviCache};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PassManager, PreservedAnalyses, Unit};
use llrm_mir::valuetracking::AssumptionCache;

const TEXT: &str = "declare void @insert(i16, i16)
define i32 @f() {
b1:
  br label %b7

b7:
  %i = phi i16 [ 1, %b1 ], [ %n, %b14 ]
  %lcg = phi i16 [ 1, %b1 ], [ %a, %b14 ]
  %m = mul i16 %lcg, 25173
  %a = add i16 %m, 13849
  %mask = and i16 %a, 4095
  %c = icmp ugt i16 %i, 1
  br i1 %c, label %b12, label %b14

b9:
  ret i32 0

b12:
  call void @insert(i16 %mask, i16 %i)
  br label %b14

b14:
  %n = add nuw nsw i16 %i, 1
  %exitcond = icmp eq i16 %n, 301
  br i1 %exitcond, label %b9, label %b7
}
";

fn nth(
    unit: &Unit,
    wanted: impl Fn(&Opcode) -> bool,
    n: usize,
) -> InstId {
    unit.function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| wanted(&unit.function.instruction(inst).opcode))
        .nth(n)
        .expect("the instruction")
}

/// Asks `%n` at the call, then `%i` at the compare: the counter's range is
/// then the one the exit edge gives.
struct Builds;

impl FunctionPass for Builds {
    fn name(&self) -> &'static str {
        "builds"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let shared = analyses.get::<LviCache>(unit.context, unit.layout, unit.function);
        let mut lvi = shared.take();
        let call = nth(unit, |op| matches!(op, Opcode::Call(_)), 0);
        let compare = nth(unit, |op| matches!(op, Opcode::ICmp(_)), 0);
        let successor = nth(unit, |op| matches!(op, Opcode::Binary(BinaryOp::Add)), 1);
        let counter = nth(unit, |op| matches!(op, Opcode::Phi), 0);
        let (successor, counter) = (
            Operand::Value(unit.function.instruction(successor).result.expect("a result")),
            Operand::Value(unit.function.instruction(counter).result.expect("a result")),
        );
        let assumptions = AssumptionCache::of(unit.context, &[], unit.metadata, unit.function);
        let mut env = Env::new(unit.context, &[], unit.metadata, unit.function, unit.layout, &assumptions);
        lvi.get_constant_range(&mut env, successor, call, false);
        let held = lvi.get_constant_range(&mut env, counter, compare, false);
        let lone = LazyValueInfo::new().get_constant_range(&mut env, counter, compare, false);
        assert_ne!(held, lone, "the premise: the order the questions came in changes the range");
        shared.put(lvi);
        PreservedAnalyses::all()
    }
}

/// An edit made and undone, as SCCP's `PredicateInfo` does: the body is as it
/// was.
struct RollsBack;

impl FunctionPass for RollsBack {
    fn name(&self) -> &'static str {
        "rolls-back"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let shared = analyses.get::<LviCache>(unit.context, unit.layout, unit.function);
        let lvi = shared.take();
        let call = nth(unit, |op| matches!(op, Opcode::Call(_)), 0);
        let counter = nth(unit, |op| matches!(op, Opcode::Phi), 0);
        let other = nth(unit, |op| matches!(op, Opcode::Phi), 1);
        let value = |inst: InstId| Operand::Value(unit.function.instruction(inst).result.expect("a result"));
        let (counter, other) = (value(counter), value(other));
        let at = unit
            .function
            .instruction(call)
            .operands
            .iter()
            .position(|&op| op == counter)
            .expect("the counter is an argument");
        unit.function.set_operand(call, at, other);
        unit.function.set_operand(call, at, counter);
        shared.put(lvi);
        PreservedAnalyses::all()
    }
}

/// The loop exits at 101, not 301, and the cache is not told. (A start of 0 for
/// 1 changes no range here: [302, 301) and [1, 0) each already hold it.)
struct ChangesBound;

impl FunctionPass for ChangesBound {
    fn name(&self) -> &'static str {
        "changes-bound"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let shared = analyses.get::<LviCache>(unit.context, unit.layout, unit.function);
        let lvi = shared.take();
        let exit = nth(unit, |op| matches!(op, Opcode::ICmp(_)), 1);
        let ty = unit.function.operand_type(unit.context, unit.function.instruction(exit).operands[1]).expect("a type");
        let bound = Operand::Constant(unit.context.constant(Constant { ty, kind: ConstantKind::Int(101) }));
        unit.function.set_operand(exit, 1, bound);
        shared.put(lvi);
        PreservedAnalyses::all()
    }
}

struct AsksLvi;

impl FunctionPass for AsksLvi {
    fn name(&self) -> &'static str {
        "asks-lvi"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        analyses.get::<LviCache>(unit.context, unit.layout, unit.function);
        PreservedAnalyses::all()
    }
}

fn run(between: impl FunctionPass + 'static) {
    // SAFETY: both tests set the same value, before any thread reads the flag.
    unsafe {
        std::env::set_var("LLRM_CHECK_CAPTURES", "1");
    }
    let mut module = llrm_mir::parse::module(TEXT).expect("parses");
    let mut manager = PassManager::default();
    manager.add(Builds);
    manager.add(between);
    manager.add(AsksLvi);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
}

#[test]
fn test_a_range_that_depends_on_the_order_asked_passes_the_oracle_after_a_rolled_back_edit() {
    run(RollsBack);
}

#[test]
#[should_panic(expected = "lvi held but stale")]
fn test_a_bound_changed_under_a_cache_not_told_fails_the_oracle() {
    run(ChangesBound);
}
