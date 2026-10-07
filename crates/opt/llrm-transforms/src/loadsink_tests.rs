//! `LoadSink`.

use llrm_analysis::manager::Summaries;
use llrm_mir::passes::PassManager;

use super::LoadSink;
use crate::testing::{parsed, printed};

fn sunk(text: &str) -> String {
    let mut module = parsed(text);
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add(LoadSink);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    printed(&module)
}

/// vx[i] += dx * scale: the load came first and the product after it, so the loaded value sat on the stack while the
/// product was made.
#[test]
fn test_a_load_moves_to_the_computation_that_reads_it() {
    let after = sunk("@g = global double 1.0
define double @f(double %dx, double %scale) {
b0:
  %v = load double, ptr @g
  %p = fmul double %dx, %scale
  %s = fadd double %v, %p
  ret double %s
}
");
    let lines: Vec<&str> = after.lines().map(str::trim).collect();
    let (load, product) = (lines.iter().position(|line| line.contains("load")).unwrap(), lines.iter().position(|line| line.contains("fmul")).unwrap());
    assert!(product < load, "{after}");
}

/// A store between may write what the load reads.
#[test]
fn test_a_load_does_not_move_past_a_store() {
    let after = sunk("@g = global double 1.0
@h = global double 1.0
define double @f(double %dx) {
b0:
  %v = load double, ptr @g
  store double %dx, ptr @h
  %s = fadd double %v, %dx
  ret double %s
}
");
    let lines: Vec<&str> = after.lines().map(str::trim).collect();
    let (load, store) = (lines.iter().position(|line| line.contains("load")).unwrap(), lines.iter().position(|line| line.contains("store")).unwrap());
    assert!(load < store, "{after}");
}
