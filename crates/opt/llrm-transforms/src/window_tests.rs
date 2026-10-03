//! `window` over huge pointer walks that stay in one window and one that does not.

use std::rc::Rc;

use llrm_analysis::testing::DOS;
use llrm_mir::passes::PassManager;

use super::Window;
use crate::testing::{Tuned, parsed, printed};

/// A loop of `trips` storing a dword through a huge pointer it steps by four.
fn walk(trips: i64) -> String {
    format!(
        "define i16 @f(ptr addrspace(3) %a) {{
start:
  br label %l

l:
  %i = phi i16 [ 0, %start ], [ %i.next, %l ]
  %p = phi ptr addrspace(3) [ %a, %start ], [ %p.next, %l ]
  store i32 7, ptr addrspace(3) %p
  %q = getelementptr i8, ptr addrspace(3) %p, i32 2
  store i16 9, ptr addrspace(3) %q
  %p.next = getelementptr i8, ptr addrspace(3) %p, i32 4
  %i.next = add nsw i16 %i, 1
  %c = icmp slt i16 %i.next, {trips}
  br i1 %c, label %l, label %d

d:
  ret i16 0
}}
"
    )
}

fn windowed(text: &str) -> String {
    let mut module = parsed(&format!("{DOS}{text}"));
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add(Window);
    manager.run_module(&mut module, Rc::new(Tuned { window: Some((1, 65521)), ..Tuned::default() })).unwrap();
    printed(&module)
}

/// A row of a 2D huge array, 201 dwords: every step carried into the
/// selector though no step of the row can (bench/huge's inner loops).
#[test]
fn test_a_walk_inside_one_window_runs_on_a_far_pointer() {
    let got = windowed(&walk(201));
    assert!(got.contains("call ptr addrspace(1) @llrm.ia16.window.p1.p3(ptr addrspace(3) %a)"), "{got}");
    assert!(got.contains("phi ptr addrspace(1)") && !got.contains("phi ptr addrspace(3)"), "{got}");
    assert!(got.contains("getelementptr i8, ptr addrspace(1)"), "{got}");
}

/// 30000 dwords reach past 64K: the walk keeps its carries.
#[test]
fn test_a_walk_past_one_window_keeps_its_huge_pointer() {
    let got = windowed(&walk(30000));
    assert!(!got.contains("llrm.ia16.window") && got.contains("phi ptr addrspace(3)"), "{got}");
}
