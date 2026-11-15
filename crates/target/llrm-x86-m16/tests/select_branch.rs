//! `if (b == 10) { lines += hit; hit = 0; }` in a byte loop: SimplifyCFG made
//! two selects of its phis, and with no conditional move each is a branch and a
//! move run on every byte beside the add (m16 grep -O1/-O2: the scan loop's
//! state spilled, 34.89M clk against bcc's 33.86M). Each select is priced as
//! that pair, so the add stays behind its branch.

use std::rc::Rc;

use llrm_mir::program::Program;
use llrm_mir::target::Machine;
use llrm_transforms::pipeline::{Applied, Options, applied};
use llrm_x86_m16::target::Dos;

const LOOP: &str = "define i32 @f(ptr %p, i16 %n, i16 %h0) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i2, %latch ]
  %lines = phi i32 [ 0, %entry ], [ %l2, %latch ]
  %hit = phi i16 [ %h0, %entry ], [ %h2, %latch ]
  %q = getelementptr i8, ptr %p, i16 %i
  %b = load i8, ptr %q
  %c = icmp eq i8 %b, 10
  br i1 %c, label %then, label %latch
then:
  %z = zext i16 %hit to i32
  %sum = add i32 %lines, %z
  br label %latch
latch:
  %l2 = phi i32 [ %sum, %then ], [ %lines, %loop ]
  %h2 = phi i16 [ 0, %then ], [ %hit, %loop ]
  %i2 = add i16 %i, 1
  %d = icmp eq i16 %i2, %n
  br i1 %d, label %exit, label %loop
exit:
  ret i32 %l2
}
";

fn selects() -> usize {
    let layout = llrm_x86_m16::layout();
    let module =
        llrm_mir::parse::module(&format!("target datalayout = \"{}\"\n{LOOP}", layout.datalayout)).expect("parses");
    let machine: Rc<dyn Machine> = Rc::new(Dos::pin_tuned());
    let mut program = Program::new(vec![module], machine).expect("program");
    applied(&mut program, &Applied { options: Options::speed(), only: None, dump: None }).expect("compiles");
    llrm_mir::print::module(&program.modules[0]).lines().filter(|line| line.contains(" = select ")).count()
}

#[test]
fn a_rare_arm_of_a_byte_loop_stays_a_branch_where_a_select_is_one() {
    assert_eq!(selects(), 0, "the add and the move run on every byte");
}
