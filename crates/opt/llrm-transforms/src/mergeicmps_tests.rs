//! Adjacent-field compares of two structs, as one `memcmp` call.

use llrm_mir::libfunc::Routine;

use super::MergeICmps;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"\n\n";

/// `text` through the pass on real-mode x86 with `memcmp` stated.
fn run(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let routines = vec![Routine { standard: "memcmp", symbol: "memcmp".into(), calling_convention: 0 }];
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(MergeICmps::with_routines(routines));
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// `a.x == b.x && a.y == b.y` stayed two loads-and-branches; it is one
/// `memcmp(a, b, 4) == 0`.
#[test]
fn test_two_adjacent_field_compares_are_one_memcmp() {
    let after = run("define i1 @f(ptr dereferenceable(4) %a, ptr dereferenceable(4) %b) {
b0:
  %ax = load i16, ptr %a
  %bx = load i16, ptr %b
  %c0 = icmp eq i16 %ax, %bx
  br i1 %c0, label %b1, label %done

b1:
  %pa = getelementptr i8, ptr %a, i16 2
  %pb = getelementptr i8, ptr %b, i16 2
  %ay = load i16, ptr %pa
  %by = load i16, ptr %pb
  %c1 = icmp eq i16 %ay, %by
  br label %done

done:
  %r = phi i1 [ false, %b0 ], [ %c1, %b1 ]
  ret i1 %r
}
");
    assert_eq!(after.matches("@memcmp(ptr %a, ptr %b, i16 4)").count(), 1, "{after}");
    assert!(!after.contains("load i16"), "{after}");
}
