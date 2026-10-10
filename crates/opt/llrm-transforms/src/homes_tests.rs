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

/// A running sum kept in `%cell` as well: nbody's `x[0]`, promoted to a phi and
/// stored on every trip.
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

/// A write to the cell while the phi is live: `%s` is read after it, so the
/// cell no longer holds it.
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

/// `Homes`' question asked both ways of each function: the phis it finds are
/// the same.
struct Same;

impl llrm_mir::passes::ModulePass for Same {
    fn name(&self) -> &'static str {
        "same-homes"
    }

    fn run(
        &mut self,
        module: &mut llrm_mir::module::Module,
        analyses: &mut llrm_mir::passes::ModuleAnalyses,
    ) -> Vec<llrm_mir::context::GlobalId> {
        let layout = analyses.program().layout.clone();
        let outer = analyses.outer(module);
        for (id, name, function) in module.functions().filter(|(_, _, function)| !function.is_declaration()) {
            let manager = analyses.manager(id, &outer);
            let Ok(accesses) = llrm_analysis::memoryssa::Accesses::managed(&module.context, &layout, function, manager)
            else {
                continue;
            };
            let program = Some(analyses.program().as_ref());
            assert_eq!(
                super::homed(function, &accesses, program),
                super::homed_scanning(function, &accesses, program),
                "@{:?}",
                name.name
            );
        }
        Vec::new()
    }
}

/// N cells each set by a store and N joins that store one and read another:
/// `mir homes` walked the live sets of every block for each candidate phi (9.3
/// G at N=128, cubic).
fn joined(n: usize) -> String {
    let mut text = String::from("define double @f(i32 %n, double %d) {\npre:\n");
    for k in 0..n {
        text += &format!("  %c{k} = alloca double\n  store double {k}.0, ptr %c{k}\n");
    }
    text += "  br label %loop\n\nloop:\n  %i = phi i32 [ 0, %pre ], [ %j, %loop ]\n";
    for k in 0..n {
        text += &format!("  %s{k} = phi double [ 0.0, %pre ], [ %t{k}, %loop ]\n");
    }
    for k in 0..n {
        let before = if k == 0 { "0.0".to_owned() } else { format!("%t{}", k - 1) };
        text += &format!("  %t{k} = fadd double %s{k}, {before}\n  store double %t{k}, ptr %c{k}\n");
    }
    text + &format!(
        "  %j = add i32 %i, 1\n  %c = icmp slt i32 %j, %n\n  br i1 %c, label %loop, label %done\n\ndone:\n  ret double %t{}\n}}\n",
        n - 1
    )
}

#[test]
fn test_the_walk_of_the_live_sets_finds_the_homes_the_scan_of_each_candidate_found() {
    assert!(homed(&joined(5)).contains("!llrm.home"), "premise: `joined` has a phi in its cell");
    for text in [SUM.to_owned(), joined(1), joined(5), joined(9)] {
        let mut module = parsed(&text);
        let mut manager = PassManager::default();
        manager.verify_each = true;
        manager.require::<Summaries>();
        manager.add_module(Same);
        manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    }
}
