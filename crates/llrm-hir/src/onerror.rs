//! ON ERROR in MIR, as both frontends mark it for the backend.
//!
//! A function with an error handler has the personality `PERSONALITY`. Each
//! call in it that may raise an error while trapping is on is an invoke to
//! its one landing pad, where the handler starts, and the pad's selector is
//! ERR. `ONERROR`, `void (i1)`, turns trapping on or off for the module's
//! handler, `ONLOCALERROR` for the procedure's own, which the runtime keeps
//! in its frame and forgets when it returns. The handler runs
//! with it off, so an error it raises ends the program, as the runtime ends
//! one raised inside its own handler: the pad turns it off first, and each
//! RESUME back on for the handler ON ERROR GOTO last named.
//! RESUME is the frontend's own control flow from the handler back to a
//! statement, and ERL the faulting statement's line, read from a table by
//! the same number. The backend's prepare step makes this what the runtime
//! runs.

use llrm_mir::build::Builder;
use llrm_mir::opcode::{Attribute, CastOp, Flags, BASIC};
use llrm_mir::{Constant, ConstantId, ConstantKind, GlobalId, GlobalKind, GlobalVariable, Linkage, Module, Operand, Type, TypeId};

use crate::mir::FAR;

pub const PERSONALITY: &str = "llrm.qb.personality";
pub const ONERROR: &str = "llrm.qb.onerror";
pub const ONLOCALERROR: &str = "llrm.qb.onlocalerror";
/// ERR outside the handler: the runtime's error number.
pub const ERR: &str = "llrm.qb.B$FERR";

/// What a handled function's code refers to.
#[derive(Clone, Copy, Debug)]
pub struct Handled {
    /// `ONERROR`, or `ONLOCALERROR` for a procedure's own handler.
    pub onerror: ConstantId,
    pub onerror_type: TypeId,
    /// The landing pad's type, `{ptr, i32}`.
    pub pad: TypeId,
    /// Each statement's line, by its number: `[n x i16]`, and its type.
    pub lines: (ConstantId, TypeId),
}

/// Gives `function` the personality, declares ON ERROR GOTO -- ON LOCAL
/// ERROR GOTO where `local` -- and keeps `lines`, each statement's BASIC
/// line in the order a site numbers them.
pub fn handled(module: &mut Module, function: GlobalId, lines: &[i64], local: bool) -> Result<Handled, String> {
    let name = module.global(function).name.clone().unwrap_or_default();
    let i16 = module.context.types.int(16);
    let table = module.context.types.intern(Type::Array { element: i16, count: lines.len() as u64 });
    let members = lines.iter().map(|&line| module.context.int(i16, i128::from(line as u16 as i16))).collect();
    let initializer = module.context.constant(Constant { ty: table, kind: ConstantKind::Aggregate(members) });
    let variable = GlobalVariable { ty: table, constant: true, initializer: Some(initializer), align: None };
    let lines = module.add_variable(&format!("$QB$ERL${name}"), variable, Linkage::Internal)?;
    let lines = (module.reference(lines), table);
    let types = &mut module.context.types;
    let (void, flag, i32, ptr) = (types.void(), types.int(1), types.int(32), types.ptr(0));
    let personality_type = types.intern(Type::Function { returns: i32, parameters: Vec::new(), variadic: true });
    let onerror_type = types.intern(Type::Function { returns: void, parameters: vec![flag], variadic: false });
    let pad = types.intern(Type::Struct { fields: vec![ptr, i32], packed: false });
    let personality = declared(module, PERSONALITY, personality_type, 0, &[])?;
    let onerror = declared(module, if local { ONLOCALERROR } else { ONERROR }, onerror_type, BASIC, &["nounwind"])?;
    let GlobalKind::Function(handled) = &mut module.globals[function.0 as usize].kind else { return Err("a handler outside a function".to_owned()) };
    handled.personality = Some(personality);
    Ok(Handled { onerror, onerror_type, pad, lines })
}

/// The slot keeping the number of the handler ON ERROR GOTO last named, 0
/// for none; made in the entry block, which is current.
pub fn active(b: &mut Builder) -> Operand {
    let i16 = b.context.types.int(16);
    let slot = b.alloca(i16, "active");
    let none = b.int(16, 0);
    b.store(none, slot, false);
    slot
}

/// The module's slot keeping the number of the handler ON ERROR GOTO last
/// named in its body, 0 for none.
pub fn active_global(module: &mut Module) -> Result<ConstantId, String> {
    let i16 = module.context.types.int(16);
    let none = module.context.int(i16, 0);
    let global = module.add_variable("$QB$ACTIVE", GlobalVariable { ty: i16, constant: false, initializer: Some(none), align: None }, Linkage::Internal)?;
    Ok(module.reference(global))
}

/// ON ERROR GOTO the handler numbered `handler`, 0 for none. Inside the
/// handler trapping stays off until RESUME turns on what this names.
pub fn goto(b: &mut Builder, handled: &Handled, active: Operand, handler: u16, inside: bool) -> Result<(), String> {
    let number = b.int(16, i128::from(handler));
    b.store(number, active, false);
    if inside {
        return if handler == 0 { Err("ON ERROR GOTO 0 in the handler, which ends the program with the error it handles: not selected yet".to_owned()) } else { Ok(()) };
    }
    let on = b.int(1, i128::from(handler != 0));
    b.call_as(BASIC, handled.onerror_type, Operand::Constant(handled.onerror), &[on], "");
    Ok(())
}

/// The pad's first act: trapping off while the handler runs.
pub fn landed(b: &mut Builder, handled: &Handled) {
    let off = b.int(1, 0);
    b.call_as(BASIC, handled.onerror_type, Operand::Constant(handled.onerror), &[off], "");
}

/// Before RESUME: trapping on again. A handler is named there: trapping was
/// on to land, and `goto` refuses ON ERROR GOTO 0 inside the handler.
pub fn resuming(b: &mut Builder, handled: &Handled) {
    let on = b.int(1, 1);
    b.call_as(BASIC, handled.onerror_type, Operand::Constant(handled.onerror), &[on], "");
}

/// ERL in the handler: the line of the statement numbered at `site`, as
/// `returns`.
pub fn erl(b: &mut Builder, handled: &Handled, site: Operand, returns: TypeId) -> Operand {
    let i16 = b.context.types.int(16);
    let number = b.load(i16, site, false, "");
    let zero = b.int(16, 0);
    let (table, ty) = handled.lines;
    let at = b.gep(ty, Operand::Constant(table), &[zero, number], Flags::default(), "");
    let line = b.load(i16, at, false, "erl");
    if returns == i16 { line } else { b.cast(CastOp::ZExt, line, returns, "") }
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
