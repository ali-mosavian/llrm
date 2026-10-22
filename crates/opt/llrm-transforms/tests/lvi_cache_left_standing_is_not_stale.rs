//! The LazyValueInfo cache is judged by its answers (`LLRM_CHECK_CAPTURES`).
//! A pass that asks for it and edits nothing leaves it standing: compared by
//! identity with a fresh, empty cache that was `fixednarrow left lvi held but
//! stale` (nbody_fixed, stft_fixed). One that edits without `forget_value`
//! leaves a stale answer, which must still fail.
use llrm_mir::lazyvalueinfo::{Env, LviCache};
use llrm_mir::module::Operand;
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PassManager, PreservedAnalyses, Unit};
use llrm_mir::valuetracking::AssumptionCache;

/// Asks the range of `%y`, then makes `%y` less known by replacing `%a`
/// with `%x`, forgetting `%y` only if `forgets`.
struct Edits {
    forgets: bool,
}

impl FunctionPass for Edits {
    fn name(&self) -> &'static str {
        "edits"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let shared = analyses.get::<LviCache>(unit.context, unit.layout, unit.function);
        let mut lvi = shared.take();
        let (_, ret) = unit.function.walk().last().expect("a ret");
        let by_name = |unit: &Unit, opcode: &dyn Fn(&Opcode) -> bool| {
            unit.function
                .walk()
                .map(|(_, inst)| inst)
                .find(|&inst| opcode(&unit.function.instruction(inst).opcode))
                .and_then(|inst| unit.function.instruction(inst).result)
                .expect("the instruction")
        };
        let a = by_name(unit, &|op| matches!(op, Opcode::Binary(BinaryOp::And)));
        let y = by_name(unit, &|op| matches!(op, Opcode::Binary(BinaryOp::Add)));
        let x = Operand::Value(unit.function.parameters()[0]);
        let assumptions = AssumptionCache::of(unit.context, &[], unit.metadata, unit.function);
        {
            let mut env = Env::new(unit.context, &[], unit.metadata, unit.function, unit.layout, &assumptions);
            let range = lvi.get_constant_range(&mut env, Operand::Value(y), ret, false);
            assert_eq!(range.get_unsigned_max().to_string(), "3", "%y is [0, 4) while %a is %x & 3");
        }
        unit.function.replace_all_uses_with(a, x);
        if self.forgets {
            lvi.forget_value(Operand::Value(y));
        }
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

const TEXT: &str = "define i16 @f(i16 %x) {
b0:
  %a = and i16 %x, 3
  %y = add i16 %a, 0
  ret i16 %y
}
";

fn run(first: impl FunctionPass + 'static) {
    // SAFETY: both tests set the same value, before any thread reads the flag.
    unsafe {
        std::env::set_var("LLRM_CHECK_CAPTURES", "1");
    }
    let mut module = llrm_mir::parse::module(TEXT).expect("parses");
    let mut manager = PassManager::default();
    manager.add(first);
    manager.add(AsksLvi);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
}

#[test]
fn test_a_cache_the_pass_leaves_standing_passes_the_oracle() {
    run(AsksLvi);
}

#[test]
fn test_an_edit_that_forgets_what_it_changed_passes_the_oracle() {
    run(Edits { forgets: true });
}

#[test]
#[should_panic(expected = "lvi held but stale")]
fn test_an_edit_that_does_not_forget_fails_the_oracle() {
    run(Edits { forgets: false });
}
