//! `Homes`: which phis are a cell's.

use llrm_analysis::manager::Summaries;
use llrm_mir::passes::PassManager;

use super::Homes;
use crate::testing::{parsed, printed};

/// `text` through `Homes`, printed.
fn homed(text: &str) -> String {
    let mut module = parsed(text);
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add_module(Homes);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    printed(&module)
}

/// A running sum kept in `%cell` as well: nbody's `x[0]`, promoted to a phi and stored on every trip.
const SUM: &str = "define double @f(i32 %n, double %d) {
pre:
  %cell = alloca double
  %other = alloca double
  store double 0.0, ptr %cell
  br label %loop

loop:
  %i = phi i32 [ 0, %pre ], [ %j, %loop ]
  %s = phi double [ 0.0, %pre ], [ %t, %loop ]
  %t = fadd double %s, %d
  store double %t, ptr %cell
  store double %d, ptr %other
  %j = add i32 %i, 1
  %c = icmp slt i32 %j, %n
  br i1 %c, label %loop, label %done

done:
  ret double %t
}
";

#[test]
fn test_a_stored_loop_carried_value_is_its_cells() {
    let after = homed(SUM);
    let phi = after.lines().find(|line| line.contains("%s = phi")).unwrap();
    assert!(phi.contains("!llrm.home"), "{after}");
    let counter = after.lines().find(|line| line.contains("%i = phi")).unwrap();
    assert!(!counter.contains("!llrm.home"), "{after}");
}

/// A write to the cell while the phi is live: `%s` is read after it, so the cell no longer holds it.
#[test]
fn test_a_cell_written_while_the_value_is_live_is_not_its_home() {
    let after = homed(
        &SUM.replace("  store double %d, ptr %other\n", "  store double %d, ptr %cell\n  %u = fadd double %s, %d\n"),
    );
    let phi = after.lines().find(|line| line.contains("%s = phi")).unwrap();
    assert!(!phi.contains("!llrm.home"), "{after}");
}

/// An input never stored to the cell: nothing is there to read.
#[test]
fn test_an_input_not_stored_is_no_home() {
    let after = homed(&SUM.replace("  store double %t, ptr %cell\n", "  store double %d, ptr %cell\n"));
    let phi = after.lines().find(|line| line.contains("%s = phi")).unwrap();
    assert!(!phi.contains("!llrm.home"), "{after}");
}
