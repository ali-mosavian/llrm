//! Array element addresses: llrm-core's `raising_array_access`.
//!
//! BC computes an element's address itself but for a huge array (/Ah) or
//! one whose subscripts it checks (/D): those call `B$HARY`, which takes the
//! descriptor in BX and the subscripts, then their count, pushed, pops
//! them, and answers the element's address in ES:BX -- or raises error 9.
//! It stays that call, answering BX and ES, as HIR's emitter keeps it:
//! computing the address here, as llrm-core did by default, drops the
//! check the program was compiled for, and a huge element is a 32-bit
//! offset from a far pointer, which MIR's 16-bit far index cannot hold.

use iced_x86::Register;
use llrm_mir::CastOp;
use llrm_x86_bcmachine::model::ir::nodes::Node;

use crate::emit::{Emit, Emitter, Var};
use crate::sites::Recognizer;

pub const ADDRESS: &str = "B$HARY";

/// The routine's name as the module declares it.
pub fn declared() -> String {
    format!("{}{ADDRESS}", crate::RUNTIME)
}

/// Declares `B$HARY` where the module calls it: `{bx, es} (...)`. Each call
/// is typed by its own words, the pushed ones in push order and then the
/// descriptor in BX, as HIR's emitter passes them.
pub fn declare(
    facts: &crate::machine::Facts,
    module: &mut llrm_mir::Module,
) -> Option<(llrm_mir::ConstantId, llrm_mir::TypeId)> {
    let called = facts
        .bodies
        .iter()
        .flat_map(|body| body.nodes.values())
        .any(|node| matches!(&**node, Node::Call(call) if call.name == ADDRESS));
    if !called || facts.bodies.iter().any(|body| body.body.name.as_deref() == Some(ADDRESS)) {
        return None;
    }
    let types = &mut module.context.types;
    let word = types.int(16);
    let pair = types.intern(llrm_mir::Type::Struct { fields: vec![word, word], packed: false });
    let ty = types.intern(llrm_mir::Type::Function { returns: pair, parameters: vec![], variadic: true });
    let global = module.add_function(&declared(), ty, llrm_mir::Linkage::External).ok()?;
    let one = &mut module.globals[global.0 as usize];
    one.address_space = facts.spaces.far;
    let llrm_mir::GlobalKind::Function(function) = &mut one.kind else { unreachable!("a function") };
    function.calling_convention = llrm_mir::opcode::BASIC;
    Some((module.reference(global), ty))
}

pub struct Element;

impl Recognizer for Element {
    fn node(
        &self,
        emitter: &mut Emitter,
        node: &Node,
    ) -> Option<Emit<()>> {
        let Node::Call(call) = node else { return None };
        if call.name != ADDRESS || emitter.unit.procedures.contains_key(ADDRESS) {
            return None;
        }
        Some(element(emitter))
    }
}

fn element(emitter: &mut Emitter) -> Emit<()> {
    let depth = emitter.depth();
    let count = emitter.stack_word(depth, 2)?;
    let rank = emitter
        .constant(count)
        .filter(|&rank| rank > 0)
        .ok_or_else(|| format!("{ADDRESS}'s subscript count is not a constant"))?;
    let mut arguments =
        (0..=rank).map(|index| emitter.stack_word(depth - 2 * (rank - index), 2)).collect::<Emit<Vec<_>>>()?;
    arguments.push(emitter.register(Register::BX)?);
    let &(callee, declared) = emitter.unit.intrinsics.get(&declared()).ok_or("B$HARY undeclared")?;
    let llrm_mir::Type::Function { returns, .. } = *emitter.b.context.types.get(declared) else {
        unreachable!("a function")
    };
    let word = emitter.b.context.types.int(16);
    let ty = emitter
        .b
        .context
        .types
        .intern(
            llrm_mir::Type::Function {
                returns,
                parameters: vec![word; arguments.len()],
                variadic: false,
            },
        );
    let answer = emitter.call_as(llrm_mir::opcode::BASIC, ty, callee, &arguments)?.expect("an answer");
    emitter.popped(2 * (rank + 1))?;
    let (address, selector) = (emitter.b.extract_value(answer, 0, ""), emitter.b.extract_value(answer, 1, ""));
    let segment = emitter.b.context.types.ptr(crate::segment(&emitter.unit.facts.spaces));
    let selector = emitter.cast(CastOp::IntToPtr, selector, segment);
    emitter.set_register(Register::BX, address)?;
    emitter.set(Var::Es, selector);
    // XOR SI,SI in its entry leaves the flags undefined.
    emitter.clobber(&[], ADDRESS);
    Ok(())
}
