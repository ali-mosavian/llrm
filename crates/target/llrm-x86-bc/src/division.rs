//! `cwd / idiv`: a word dividend sign-extended into DX is a word division.
//! Adapted from llrm-core's `raising_division`. Where the quotient does not
//! fit a word the machine traps and `sdiv i16` is undefined, as the core's
//! 32-by-16 division leaves it.

use llrm_x86_bcmachine::model::ir::Operation;
use llrm_x86_bcmachine::model::ir::nodes::Node;
use llrm_mir::BinaryOp;

use crate::emit::{Emit, Emitter};
use crate::longs::extended;
use crate::sites::Recognizer;

pub struct Division;

impl Recognizer for Division {
    fn node(&self, e: &mut Emitter, node: &Node) -> Option<Emit<()>> {
        let what = node.semantics();
        if what.op != Operation::Divide || what.name.as_deref() != Some("idiv") || what.sources.len() != 3 || what.dests.len() != 2 {
            return None;
        }
        let (high, low) = (e.read(&what.sources[0]).ok()?, e.read(&what.sources[1]).ok()?);
        if e.bits_of(low) != 16 || !extended(e, low, high) {
            return None;
        }
        Some((|| {
            let divisor = e.read(&what.sources[2])?;
            let quotient = e.binary(BinaryOp::SDiv, low, divisor);
            let remainder = e.binary(BinaryOp::SRem, low, divisor);
            e.write(&what.dests[0], quotient)?;
            e.write(&what.dests[1], remainder)?;
            e.unknown_flags(node.effects(), "idiv");
            Ok(())
        })())
    }
}
