//! The worklist's requeue: what a change reaches is tried again, inside the one
//! drain.

use llrm_analysis::testing::DOS;
use llrm_mir::module::InstId;
use llrm_mir::opcode::{BinaryOp, Opcode};

use super::Worklist;
use crate::testing::parsed;

/// A chain of adds, each read by the next, the last read by nothing.
fn chain(length: usize) -> String {
    let mut body = String::from("define i16 @f(i16 %x) {\nb0:\n  %a0 = add i16 %x, 1\n");
    for k in 1..length {
        body += &format!("  %a{k} = add i16 %a{}, 1\n", k - 1);
    }
    body + "  ret i16 %x\n}\n"
}

/// Forty adds, only the last unread. A rule that erases an unread add, offered
/// the instructions in order, finds only the last one: each other is read when
/// it is tried. The one before it is read no longer, but a walk does not go
/// back; it needed a pass for each add. The worklist queues what an erase
/// stopped being read, so one drain empties the chain.
#[test]
fn an_unread_chain_is_erased_in_one_drain_by_requeueing_what_a_change_unread() {
    let length = 40;
    let mut module = parsed(&format!("{DOS}{}", chain(length)));
    let (_, function) = module.function_mut("f").expect("@f");
    let mut work = Worklist::of(function);
    let mut tried = 0;
    let changed = work.drain(function, |function, inst: InstId| {
        tried += 1;
        let one = function.instruction(inst);
        let unread = one.result.is_some_and(|result| function.users(result).is_empty());
        if matches!(one.opcode, Opcode::Binary(BinaryOp::Add)) && unread {
            function.erase(inst).expect("unread");
            true
        } else {
            false
        }
    });
    assert!(changed);
    let left = function.walk().filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Binary(_))).count();
    assert_eq!(left, 0, "{left} of {length} adds left after one drain");
    assert!(tried <= 4 * length, "{tried} rule calls for {length} instructions");
}
