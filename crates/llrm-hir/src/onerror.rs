//! ON ERROR in MIR, as both frontends mark it for the backend.
//!
//! A function with an error handler has the personality `PERSONALITY`. Each
//! call in it that may raise an error is an invoke to its one landing pad,
//! where the handler starts, and the pad's selector is ERR. `ONERROR`,
//! `void (i1)`, is ON ERROR GOTO: true registers the handler, false none.
//! RESUME is the frontend's own control flow from the handler back to a
//! statement. The backend's prepare step makes this what the runtime runs.

use llrm_mir::opcode::{Attribute, BASIC};
use llrm_mir::{ConstantId, GlobalId, GlobalKind, Linkage, Module, Type, TypeId};

use crate::mir::FAR;

pub const PERSONALITY: &str = "llrm.qb.personality";
pub const ONERROR: &str = "llrm.qb.onerror";
/// ERR outside the handler: the runtime's error number.
pub const ERR: &str = "llrm.qb.B$FERR";

/// What a handled function's code refers to.
#[derive(Clone, Copy, Debug)]
pub struct Handled {
    pub onerror: ConstantId,
    pub onerror_type: TypeId,
    /// The landing pad's type, `{ptr, i32}`.
    pub pad: TypeId,
}

/// Gives `function` the personality, and declares ON ERROR GOTO.
pub fn handled(module: &mut Module, function: GlobalId) -> Result<Handled, String> {
    let types = &mut module.context.types;
    let (void, flag, i32, ptr) = (types.void(), types.int(1), types.int(32), types.ptr(0));
    let personality_type = types.intern(Type::Function { returns: i32, parameters: Vec::new(), variadic: true });
    let onerror_type = types.intern(Type::Function { returns: void, parameters: vec![flag], variadic: false });
    let pad = types.intern(Type::Struct { fields: vec![ptr, i32], packed: false });
    let personality = declared(module, PERSONALITY, personality_type, 0, &[])?;
    let onerror = declared(module, ONERROR, onerror_type, BASIC, &["nounwind"])?;
    let GlobalKind::Function(handled) = &mut module.globals[function.0 as usize].kind else { return Err("a handler outside a function".to_owned()) };
    handled.personality = Some(personality);
    Ok(Handled { onerror, onerror_type, pad })
}

/// ERR, `i16 ()`.
pub fn err(module: &mut Module) -> Result<(ConstantId, TypeId), String> {
    let types = &mut module.context.types;
    let i16 = types.int(16);
    let ty = types.intern(Type::Function { returns: i16, parameters: Vec::new(), variadic: false });
    Ok((declared(module, ERR, ty, BASIC, &["nounwind"])?, ty))
}

fn declared(module: &mut Module, name: &str, ty: TypeId, convention: u32, flags: &[&str]) -> Result<ConstantId, String> {
    if let Some(one) = module.named(name) {
        return Ok(module.reference(one));
    }
    let id = module.add_function(name, ty, Linkage::External)?;
    let global = &mut module.globals[id.0 as usize];
    global.address_space = FAR;
    let GlobalKind::Function(function) = &mut global.kind else { unreachable!("a function") };
    function.calling_convention = convention;
    function.attrs.extend(flags.iter().map(|one| Attribute::Flag((*one).to_owned())));
    Ok(module.reference(id))
}
