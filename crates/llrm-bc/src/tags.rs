//! What each access reaches, as the `!tbaa` types the HIR emitter uses
//! (`llrm_hir::mir::Tags`): a place -- a DGROUP object, a runtime cell, a
//! frame or the pushed bytes -- or a far allocation, an array's heap block
//! reached through the selector its descriptor holds. The two never meet,
//! which is what llrm-core's `allocation` said of a far access. An access
//! with neither provenance is left untagged.

use llrm_hir::mir::Tags;
use llrm_mir::context::{ConstantKind, Context, GlobalId};
use llrm_mir::module::{Function, GlobalKind, InstId, Module, Operand};
use llrm_mir::opcode::{CastOp, Opcode};

use crate::addresses::{made, offset_parts, pointer_parts};
use crate::arrays;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Class {
    Place,
    Allocation,
}

/// A descriptor: its root pointer and offset.
type Descriptor = (Operand, i64);

/// The descriptors of far arrays `function`'s DIM requests name: their
/// attributes, the high byte of the word pushed before the descriptor,
/// say far (1) or huge (2). A near array's data is in DGROUP.
fn descriptors(module: &Module, function: &Function) -> Vec<Descriptor> {
    let mut out = Vec::new();
    for (_, inst) in function.walk() {
        let one = function.instruction(inst);
        if !matches!(one.opcode, Opcode::Call(_)) {
            continue;
        }
        let callee = llrm_mir::memory::callee(&module.context, function, inst);
        let named = callee.and_then(|global| module.global(global).name.as_deref());
        if !named.is_some_and(arrays::allocates) || one.operands.len() < 3 {
            continue;
        }
        let (shape, descriptor) = (one.operands[one.operands.len() - 3], one.operands[one.operands.len() - 2]);
        let far = crate::addresses::constant(&module.context, shape).is_some_and(|shape| (1..=3).contains(&((shape >> 8) & 0xff)));
        if !far {
            continue;
        }
        if let Some(parts) = offset_parts(function, &module.context, descriptor, 0).filter(|parts| parts.terms.is_empty()) {
            out.push((parts.root, parts.constant));
        }
    }
    out
}

fn class(function: &Function, context: &Context, spaces: &[u32], hary: Option<Operand>, pointer: Operand, descriptors: &[Descriptor]) -> Option<Class> {
    let root = pointer_parts(function, context, pointer).root;
    if let Operand::Constant(id) = root
        && let ConstantKind::Global(global) = context.get(id).kind
    {
        return (spaces[global.0 as usize] == 0).then_some(Class::Place);
    }
    match made(function, context, root)? {
        (Opcode::Alloca { .. }, _) => Some(Class::Place),
        (Opcode::Cast(CastOp::AddrSpaceCast), operands) => {
            let from = function.operand_type(context, operands[0])?;
            match context.types.get(from) {
                llrm_mir::Type::Pointer(0) => class(function, context, spaces, hary, operands[0], descriptors),
                llrm_mir::Type::Pointer(crate::SEGMENT) => {
                    let (Opcode::Cast(CastOp::IntToPtr), selector) = made(function, context, operands[0])? else { return None };
                    let descriptor = match made(function, context, selector[0])? {
                        // The selector a descriptor holds, two bytes in.
                        (Opcode::Load { .. }, loaded) => (pointer_parts(function, context, loaded[0]), 2),
                        // The one B$HARY answers, of the descriptor in BX.
                        (Opcode::ExtractValue(indices), answer) if indices == [1] => {
                            let (Opcode::Call(_), arguments) = made(function, context, answer[0])? else { return None };
                            let [.., descriptor, callee] = arguments[..] else { return None };
                            if Some(callee) != hary {
                                return None;
                            }
                            (offset_parts(function, context, descriptor, 0)?, 0)
                        }
                        _ => return None,
                    };
                    let (field, at) = descriptor;
                    (field.terms.is_empty() && descriptors.contains(&(field.root, field.constant - at))).then_some(Class::Allocation)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Tags every access in `module` whose provenance says what it reaches.
pub fn tag(module: &mut Module) {
    let spaces: Vec<u32> = module.globals.iter().map(|one| one.address_space).collect();
    let hary = module.named(&crate::access::declared()).map(|one| Operand::Constant(module.reference(one)));
    let bodies: Vec<GlobalId> = module.functions().filter(|(_, _, function)| !function.is_declaration()).map(|(id, _, _)| id).collect();
    let mut global: Vec<Descriptor> = Vec::new();
    for &id in &bodies {
        let function = module.global(id).function().expect("a function");
        global.extend(descriptors(module, function).into_iter().filter(|(root, _)| matches!(root, Operand::Constant(_))));
    }
    let mut tags: Option<Tags> = None;
    for id in bodies {
        let function = module.global(id).function().expect("a function");
        let mut mine = global.clone();
        mine.extend(descriptors(module, function).into_iter().filter(|(root, _)| matches!(root, Operand::Value(_))));
        let classes: Vec<(InstId, Class)> = function
            .walk()
            .filter_map(|(_, inst)| {
                let one = function.instruction(inst);
                let pointer = match one.opcode {
                    Opcode::Load { .. } => one.operands[0],
                    Opcode::Store { .. } => one.operands[1],
                    _ => return None,
                };
                Some((inst, class(function, &module.context, &spaces, hary, pointer, &mine)?))
            })
            .collect();
        if classes.is_empty() {
            continue;
        }
        let tags = tags.get_or_insert_with(|| Tags::new(module));
        let (place, allocation) = (tags.place, tags.allocation);
        let GlobalKind::Function(function) = &mut module.globals[id.0 as usize].kind else { unreachable!("a function") };
        for (inst, class) in classes {
            function.annotate(inst, "tbaa", if class == Class::Place { place } else { allocation });
        }
    }
}
