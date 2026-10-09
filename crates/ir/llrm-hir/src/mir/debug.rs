//! `-g`: HIR's debug information as MIR's, through `llrm_mir::debuginfo`.
//! A variable in data is listed with the module; one in a frame is an
//! a record that it lives where its frame object is, which
//! [`Body::declare_variables`] says: `llvm.dbg.declare`, as a
//! [`DebugRecord`](llrm_mir::DebugRecord), no instruction.

use llrm_mir::debuginfo as di;
use llrm_mir::{GlobalId, MetadataId, Module};
use llrm_support::hash::HashMap;

use super::{Body, Emit, Tables, frame_groups};
use crate::model::{self, Storage};

/// Each HIR debug type's node, made on first use.
struct Types<'h> {
    of: HashMap<i64, &'h model::DebugType>,
    made: HashMap<i64, MetadataId>,
}

impl Types<'_> {
    fn node(
        &mut self,
        module: &mut Module,
        id: i64,
    ) -> Emit<MetadataId> {
        if let Some(&made) = self.made.get(&id) {
            return Ok(made);
        }
        let one = *self.of.get(&id).ok_or_else(|| format!("no debug type {id}"))?;
        // An aggregate may be reached from one of its own members: its node is
        // made first, set after.
        let reserved = matches!(one.kind, di::Kind::Struct | di::Kind::Union).then(|| di::reserve_type(module));
        if let Some(reserved) = reserved {
            self.made.insert(id, reserved);
        }
        let target = one.target.map(|target| self.node(module, target)).transpose()?;
        let members = one
            .members
            .iter()
            .map(|member| {
                Ok(di::Member {
                    name: member.name.clone(),
                    r#type: self.node(module, member.r#type)?,
                    offset: member.offset,
                    bits: member.bit_start.zip(member.bit_width),
                })
            })
            .collect::<Emit<Vec<_>>>()?;
        let described = di::Type {
            kind: one.kind,
            name: one.name.clone(),
            size: one.size,
            reach: one.reach,
            target,
            members,
            spelling: one.spelling.clone(),
        };
        let made = match reserved {
            Some(reserved) => {
                di::set_type(module, reserved, &described);
                reserved
            }
            None => di::add_type(module, &described),
        };
        self.made.insert(id, made);
        Ok(made)
    }
}

fn name(
    module: &Module,
    global: GlobalId,
) -> String {
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
    let mut variables = HashMap::default();
    let Some(debug) = &hir.debug else { return Ok(variables) };
    if let Some(language) = debug.language {
        di::set_language(module, language, debug.dialect.unwrap_or(di::Dialect::Bc));
    }
    let mut types = Types { of: debug.types.iter().map(|one| (one.id, one)).collect(), made: HashMap::default() };
    for global in &debug.globals {
        let Some(&object) = data.get(&global.object) else { continue };
        let scope = match global.function {
            Some(function) => match functions.get(&function) {
                Some(&declaring) => Some(name(module, declaring)),
                None => continue,
            },
            None => None,
        };
        let r#type = types.node(module, global.r#type)?;
        let global = di::Global {
            global: name(module, object),
            offset: global.offset,
            name: global.name.clone(),
            r#type,
            scope,
        };
        di::add_global(module, &global);
    }
    for procedure in &debug.functions {
        let (Some(&global), Some(function)) =
            (functions.get(&procedure.function), hir.functions.iter().find(|one| one.id == procedure.function))
        else {
            continue;
        };
        let scope = name(module, global);
        let mut parameters = procedure
            .parameters
            .iter()
            .map(|one| Ok((one.argument, one.name.clone(), types.node(module, one.r#type)?)))
            .collect::<Emit<Vec<_>>>()?;
        let groups = frame_groups(&mut module.context.types, tables, function)?;
        for variable in &procedure.variables {
            let place = function
                .places
                .iter()
                .find(|one| one.id == variable.place)
                .ok_or_else(|| format!("no place {}", variable.place))?;
            let r#type = types.node(module, variable.r#type)?;
            match place.storage {
                Storage::Local => {
                    let group = groups
                        .iter()
                        .find(|group| group.places.iter().any(|one| one.id == place.id))
                        .ok_or("a local outside the frame")?;
                    let node = di::Variable {
                        scope: scope.clone(),
                        name: variable.name.clone(),
                        r#type,
                        offset: place.offset - group.start,
                        parameter: variable.parameter,
                        argument: variable.argument,
                    };
                    variables.insert((function.id, place.id), di::add_variable(module, &node));
                }
                // Where it was passed: its argument's cell.
                Storage::Parameter => {
                    let argument = function
                        .parameters
                        .iter()
                        .position(|&one| one == place.symbol)
                        .ok_or("a parameter's home names no parameter")?;
                    parameters.push((argument as i64, variable.name.clone(), r#type));
                }
                Storage::Static | Storage::Module | Storage::Common | Storage::External => {
                    let Some(&object) = data.get(&place.symbol) else { continue };
                    let global = di::Global {
                        global: name(module, object),
                        offset: place.offset,
                        name: variable.name.clone(),
                        r#type,
                        scope: Some(scope.clone()),
                    };
                    di::add_global(module, &global);
                }
            }
        }
        let r#type = types.node(module, procedure.r#type)?;
        di::add_function(
            module,
            &di::Function {
                function: scope.clone(),
                module: procedure.module,
                name: procedure.name.clone(),
                r#type,
                parameters,
            },
        );
    }
    Ok(variables)
}

/// The number each frame variable is named by while the code is lowered, before
/// its node is made.
pub(super) fn provisional(hir: &model::Module) -> HashMap<(i64, i64), MetadataId> {
    let mut chosen = HashMap::default();
    let Some(debug) = &hir.debug else { return chosen };
    for procedure in &debug.functions {
        let Some(function) = hir.functions.iter().find(|one| one.id == procedure.function) else { continue };
        for variable in &procedure.variables {
            let Some(place) = function.places.iter().find(|one| one.id == variable.place) else { continue };
            if matches!(place.storage, Storage::Local) {
                let number = u32::try_from(chosen.len()).expect("a few variables");
                chosen.insert((function.id, place.id), MetadataId(super::PROVISIONAL_VARIABLES + number));
            }
        }
    }
    chosen
}

impl Body<'_, '_, '_> {
    /// Each frame variable declared where its frame object is.
    pub(super) fn declare_variables(&mut self) {
        for place in &self.function.places {
            let Some(&node) = self.tables.variables.get(&(self.function.id, place.id)) else { continue };
            let Some(&(object, _)) = self.frame.get(&place.id) else { continue };
            self.b.debug_declare(node, self.objects[object]);
        }
    }
}
