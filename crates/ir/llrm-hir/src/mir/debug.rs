//! `-g`: HIR's debug information as MIR's, through `llrm_mir::debuginfo`.
//! A variable in data is listed with the module; one in a frame is an
//! `llvm.dbg.declare` of its frame object, which [`Body::declare_variables`]
//! emits.

use std::collections::HashMap;

use llrm_mir::debuginfo as di;
use llrm_mir::{GlobalId, InstId, Linkage, MetadataId, Module};

use super::{frame_groups, function_type, Body, Emit, Tables};
use crate::model::{self, Storage};

pub(super) const DECLARE: &str = "llvm.dbg.declare.p0";

/// Each HIR debug type's node, made on first use.
struct Types<'h> {
    of: HashMap<i64, &'h model::DebugType>,
    made: HashMap<i64, MetadataId>,
}

impl Types<'_> {
    fn node(&mut self, module: &mut Module, id: i64) -> Emit<MetadataId> {
        if let Some(&made) = self.made.get(&id) {
            return Ok(made);
        }
        let one = *self.of.get(&id).ok_or_else(|| format!("no debug type {id}"))?;
        let target = one.target.map(|target| self.node(module, target)).transpose()?;
        let members = one
            .members
            .iter()
            .map(|member| Ok(di::Member { name: member.name.clone(), r#type: self.node(module, member.r#type)?, offset: member.offset }))
            .collect::<Emit<Vec<_>>>()?;
        let made = di::add_type(
            module,
            &di::Type { kind: one.kind, name: one.name.clone(), size: one.size, reach: one.reach, target, members },
        );
        self.made.insert(id, made);
        Ok(made)
    }
}

fn name(module: &Module, global: GlobalId) -> String {
    module.global(global).name.clone().unwrap_or_default()
}

/// Writes `hir`'s debug information into `module`, `data` and `functions`
/// each HIR data object's and function's global. Each frame variable's
/// node, by its function and place.
pub(super) fn emitted<'h>(
    module: &mut Module,
    tables: &Tables<'h>,
    hir: &'h model::Module,
    data: &HashMap<i64, GlobalId>,
    functions: &HashMap<i64, GlobalId>,
) -> Emit<HashMap<(i64, i64), MetadataId>> {
    let mut variables = HashMap::new();
    let Some(debug) = &hir.debug else { return Ok(variables) };
    let mut types = Types { of: debug.types.iter().map(|one| (one.id, one)).collect(), made: HashMap::new() };
    for global in &debug.globals {
        let Some(&object) = data.get(&global.object) else { continue };
        let r#type = types.node(module, global.r#type)?;
        let global = di::Global { global: name(module, object), offset: global.offset, name: global.name.clone(), r#type, scope: None };
        di::add_global(module, &global);
    }
    for procedure in &debug.functions {
        let (Some(&global), Some(function)) = (functions.get(&procedure.function), hir.functions.iter().find(|one| one.id == procedure.function)) else { continue };
        let scope = name(module, global);
        let parameters = procedure
            .parameters
            .iter()
            .map(|one| Ok((one.argument, one.name.clone(), types.node(module, one.r#type)?)))
            .collect::<Emit<Vec<_>>>()?;
        let r#type = types.node(module, procedure.r#type)?;
        di::add_function(module, &di::Function { function: scope.clone(), module: procedure.module, name: procedure.name.clone(), r#type, parameters });
        let groups = frame_groups(&mut module.context.types, tables, function)?;
        for variable in &procedure.variables {
            let place = function.places.iter().find(|one| one.id == variable.place).ok_or_else(|| format!("no place {}", variable.place))?;
            let r#type = types.node(module, variable.r#type)?;
            if place.storage == Storage::Local {
                let group = groups.iter().find(|group| group.places.iter().any(|one| one.id == place.id)).ok_or("a local outside the frame")?;
                let node = di::Variable { scope: scope.clone(), name: variable.name.clone(), r#type, offset: place.offset - group.start };
                variables.insert((function.id, place.id), di::add_variable(module, &node));
            } else if let Some(&object) = data.get(&place.symbol) {
                let global = di::Global { global: name(module, object), offset: place.offset, name: variable.name.clone(), r#type, scope: Some(scope.clone()) };
                di::add_global(module, &global);
            }
        }
    }
    Ok(variables)
}

/// Declares `llvm.dbg.declare` where a frame variable needs it.
pub(super) fn declared(module: &mut Module, tables: &mut Tables<'_>) -> Emit<()> {
    if tables.variables.is_empty() || tables.callees.contains_key(DECLARE) {
        return Ok(());
    }
    let types = &mut module.context.types;
    let (void, pointer) = (types.void(), types.ptr(0));
    let ty = function_type(types, void, vec![pointer]);
    let global = module.add_function(DECLARE, ty, Linkage::External)?;
    tables.callees.insert(DECLARE.to_owned(), module.reference(global));
    Ok(())
}

impl Body<'_, '_, '_> {
    /// Each frame variable declared where its frame object is.
    pub(super) fn declare_variables(&mut self) {
        for place in &self.function.places {
            let Some(&node) = self.tables.variables.get(&(self.function.id, place.id)) else { continue };
            let Some(&(object, _)) = self.frame.get(&place.id) else { continue };
            let pointer = self.b.context.types.ptr(0);
            let void = self.b.context.types.void();
            let ty = function_type(&mut self.b.context.types, void, vec![pointer]);
            let callee = llrm_mir::Operand::Constant(self.tables.callees[DECLARE]);
            let first = self.b.function.instruction_count();
            self.b.call(ty, callee, &[self.objects[object]], "");
            self.b.function.annotate(InstId(first as u32), di::VARIABLE, node);
        }
    }
}
