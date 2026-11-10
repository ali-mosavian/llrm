//! The count-down a loop gets once an idiom has made its trip count known.

use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};

use super::common::{defining_instruction, emit, int};
use super::process::Run;

/// The count-down a loop gets once its trip count is known (the tail of
/// `transformLoopToPopcount` and of `transformLoopToCountable`): a phi
/// `tcphi = [initial, preheader], [tcdec, body]` at the top of the body, `tcdec
/// = tcphi - 1` before the loop's compare, and the compare made `tcdec 0`: the
/// loop stays on `stays` where its true edge is the body, else on `leaves`.
pub(super) fn count_down_loop(
    run: &mut Run,
    preheader: BlockId,
    body: BlockId,
    initial: Operand,
    stays: IntPredicate,
    leaves: IntPredicate,
) {
    let lb_br = run.function.terminator(body).expect("a loop branch");
    let lb_cond =
        defining_instruction(run.function, run.function.instruction(lb_br).operands[0]).expect("matched as a compare");
    let ty = run.function.operand_type(run.context, initial).expect("a typed count");

    let first = run.function.iter_block(body).next().expect("a loop body");
    let tc_phi = run
        .function
        .create_instruction(Opcode::Phi, ty, vec![initial, Operand::Block(preheader)], Flags::default(), Some("tcphi"));
    run.function.insert(tc_phi, Position::Before(first)).expect("a placed phi");
    let tc_phi_value = Operand::Value(run.function.instruction(tc_phi).result.expect("a phi value"));
    let tc_dec = emit(
        run.function,
        lb_cond,
        Opcode::Binary(BinaryOp::Sub),
        ty,
        vec![tc_phi_value, int(run.context, ty, 1)],
        Flags::NSW,
        "tcdec",
    );
    run.function.set_operands(tc_phi, vec![initial, Operand::Block(preheader), tc_dec, Operand::Block(body)]);

    let predicate = if run.function.instruction(lb_br).operands[1] == Operand::Block(body) { stays } else { leaves };
    run.function.set_opcode(lb_cond, Opcode::ICmp(predicate));
    run.function.set_operand(lb_cond, 0, tc_dec);
    run.function.set_operand(lb_cond, 1, int(run.context, ty, 0));
}
