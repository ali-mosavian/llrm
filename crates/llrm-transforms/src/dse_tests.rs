//! `Dse` under the pass manager, each body MIR text run by llrm-mir's
//! interpreter before and after.

use llrm_analysis::manager::Summaries;
use llrm_mir::passes::PassManager;

use super::Dse;
use crate::promote::Promote;
use crate::testing::{parsed, printed, results};

const INPUTS: &[&[i128]] = &[&[0], &[3], &[-7], &[0x7fff]];

/// `text` through Promote then Dse: @f printed; what @f returns stays.
fn promoted(text: &str) -> String {
    let before = parsed(&format!("@g = global i16 0\n\ndefine void @h(ptr %s) {{\nb0:\n  store i16 9, ptr %s\n  ret void\n}}\n\n{text}"));
    let mut module = before.clone();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.verify_invalidation = true;
    manager.require::<Summaries>();
    manager.add(Promote);
    manager.add(Dse);
    manager.run(&mut module).unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, INPUTS), results(&before, INPUTS), "{after}");
    after[after.find("@f(").unwrap()..].to_owned()
}

/// Promote leaves the stores to a cell whose loads it replaced; nothing
/// reads the cell after, so they go.
#[test]
fn test_dse_removes_the_stores_promote_left() {
    let after = promoted(
        "define i16 @f(i16 %x) {
b0:
  %s = alloca i16
  store i16 %x, ptr %s
  %c = icmp slt i16 %x, 0
  br i1 %c, label %b1, label %b2

b1:
  store i16 0, ptr %s
  br label %b2

b2:
  %v = load i16, ptr %s
  ret i16 %v
}
",
    );
    assert!(!after.contains("store"), "{after}");
}

/// A store to a cell a callee reads, or that outlives the function, stays;
/// one overwritten before anything reads it goes.
#[test]
fn test_dse_keeps_an_observed_store() {
    for (body, kept) in [
        ("  %s = alloca i16\n  store i16 %x, ptr %s\n  call void @h(ptr %s)\n  %v = load i16, ptr %s\n", 1),
        ("  store i16 %x, ptr @g\n  %v = load i16, ptr @g\n", 1),
        ("  store i16 1, ptr @g\n  store i16 %x, ptr @g\n  %v = load i16, ptr @g\n", 1),
    ] {
        let after = promoted(&format!("define i16 @f(i16 %x) {{\nb0:\n{body}  ret i16 %v\n}}\n"));
        assert_eq!(after.matches("store").count(), kept, "{after}");
    }
}
