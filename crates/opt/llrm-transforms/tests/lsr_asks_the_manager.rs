//! lsr looks at the loops of a function one after another, and a loop it leaves alone leaves the function as it was: what the manager
//! holds of it (the registers) is asked of again only after a loop is changed.
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_mir::passes::{Observer, PassManager};

static REGISTERS: AtomicUsize = AtomicUsize::new(0);

fn counting(what: &'static str, hit: bool) {
    if what == "registers" && !hit {
        REGISTERS.fetch_add(1, Ordering::Relaxed);
    }
}

/// `depth` loops one inside the other, counting and doing nothing lsr can change.
fn nest(depth: usize) -> String {
    let mut text = String::from("define i32 @f(i32 %n) {\nb0:\n  br label %h0\n\n");
    for k in 0..depth {
        let (inner, exit) = (if k + 1 == depth { format!("l{k}") } else { format!("h{}", k + 1) }, if k == 0 { "end".to_owned() } else { format!("l{}", k - 1) });
        let from = if k == 0 { "b0".to_owned() } else { format!("h{}", k - 1) };
        text += &format!("h{k}:\n  %i{k} = phi i32 [ 0, %{from} ], [ %n{k}, %l{k} ]\n  %c{k} = icmp slt i32 %i{k}, %n\n  br i1 %c{k}, label %{inner}, label %{exit}\n\n");
    }
    for k in (0..depth).rev() {
        text += &format!("l{k}:\n  %n{k} = add nsw i32 %i{k}, 1\n  br label %h{k}\n\n");
    }
    text + "end:\n  ret i32 0\n}\n"
}

#[test]
fn test_loops_lsr_leaves_alone_share_the_registers_the_manager_holds() {
    llrm_mir::passes::observe(Observer { span: |_, _, run| run(), function: |_, run| run(), count: counting });
    let mut module = llrm_mir::parse::module(&nest(8)).unwrap_or_else(|error| panic!("{error}"));
    let mut manager = PassManager::default();
    manager.add(llrm_transforms::lsr::Lsr::default());
    REGISTERS.store(0, Ordering::Relaxed);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let found = REGISTERS.load(Ordering::Relaxed);
    assert!(found <= 2, "the registers were found {found} times for 8 loops that change nothing");
}
