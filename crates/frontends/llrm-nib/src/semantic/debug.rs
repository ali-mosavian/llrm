//! `-g`: Nib's side of the debug information. A source variable is a place
//! whose name no compiler temporary's does (`$`), typed by the program's
//! own HIR types; `llrm_core::hir::debug` builds the rest.

use std::collections::{BTreeMap, BTreeSet};

use llrm_core::hir::debug::Builder;
use llrm_core::hir::model::{Debug, DebugReach, DebugScalar};

use super::{hir, TypeRegistry};

struct Described<'t> {
    types: &'t TypeRegistry,
    builder: Builder,
    made: BTreeMap<u32, Option<i64>>,
}

impl Described<'_> {
    /// HIR type `id`'s debug type, where CodeView has one.
    fn r#type(&mut self, id: u32) -> Option<i64> {
        if let Some(&made) = self.made.get(&id) {
            return made;
        }
        // A struct reached again through its own pointer stops here.
        self.made.insert(id, None);
        let one = self.types.types.get(id.checked_sub(1)? as usize)?;
        let scalar = |builder: &mut Builder, scalar| Some(builder.scalar(scalar));
        let made = match (one.kind, one.width, one.signed) {
            ("void", ..) => scalar(&mut self.builder, DebugScalar::Void),
            ("boolean", 1, _) => scalar(&mut self.builder, DebugScalar::UInt8),
            ("integer", 1, _) if one.name == "char" => scalar(&mut self.builder, DebugScalar::Char),
            ("integer", 1, Some(true)) => scalar(&mut self.builder, DebugScalar::Int8),
            ("integer", 1, _) => scalar(&mut self.builder, DebugScalar::UInt8),
            ("integer", 2, Some(true)) => scalar(&mut self.builder, DebugScalar::Int16),
            ("integer", 2, _) => scalar(&mut self.builder, DebugScalar::UInt16),
            ("integer", 4, Some(true)) => scalar(&mut self.builder, DebugScalar::Int32),
            ("integer", 4, _) => scalar(&mut self.builder, DebugScalar::UInt32),
            ("float", 4, _) => scalar(&mut self.builder, DebugScalar::Float32),
            ("float", 8, _) => scalar(&mut self.builder, DebugScalar::Float64),
            ("pointer", ..) => {
                let target = one.element.and_then(|element| self.r#type(element)).unwrap_or_else(|| self.builder.scalar(DebugScalar::Void));
                let reach = match one.address {
                    "far" => DebugReach::Far,
                    "huge" => DebugReach::Huge,
                    _ => DebugReach::Near,
                };
                Some(self.builder.pointer(target, reach))
            }
            ("array", width, _) => {
                let element = self.r#type(one.element?)?;
                Some(self.builder.sized(element, i64::from(width)))
            }
            ("opaque", width, _) => self.structure(id, width),
            // CodeView has no 64-bit integer.
            _ => None,
        };
        self.made.insert(id, made);
        made
    }

    /// Struct `id`, `width` bytes, of its source fields in declaration order.
    fn structure(&mut self, id: u32, width: u32) -> Option<i64> {
        let layout = self.types.structure(id)?;
        let mut fields = Vec::new();
        for name in layout.order.iter().filter(|name| !name.starts_with('$')) {
            let field = layout.fields.get(name)?;
            let element = field.type_.id();
            let Some(mut r#type) = self.r#type(element) else { continue };
            if let Some(shape) = &field.shape {
                r#type = self.builder.sized(r#type, i64::from(self.types.width(element) * shape.len()));
            }
            fields.push((name.clone(), r#type, i64::from(field.offset)));
        }
        let fields: Vec<(&str, i64, i64)> = fields.iter().map(|(name, r#type, offset)| (name.as_str(), *r#type, *offset)).collect();
        Some(self.builder.structure(&layout.name, i64::from(width), &fields))
    }
}

/// `functions`' debug information: each one's source parameters and
/// variables, and the module's variables, once.
pub(super) fn described(functions: &[hir::Function], types: &TypeRegistry) -> Debug {
    let mut described = Described { types, builder: Builder::default(), made: BTreeMap::new() };
    let mut globals = BTreeSet::new();
    for function in functions {
        let value_type = |value: u32| function.values.iter().find(|one| one.id == value).map(|one| one.type_id);
        // A parameter with a home, an owned string's, is that place.
        let homed: BTreeSet<&str> = function.places.iter().map(|one| one.name.as_str()).collect();
        for (value, name) in function.named_parameters.iter().filter(|(_, name)| !homed.contains(name.as_str())) {
            let argument = function.parameters.iter().position(|one| one == value);
            if let (Some(argument), Some(r#type)) = (argument, value_type(*value).and_then(|one| described.r#type(one))) {
                described.builder.parameter(argument as i64, name, r#type);
            }
        }
        for place in function.places.iter().filter(|one| !one.name.starts_with('$')) {
            let Some(r#type) = described.r#type(place.type_id) else { continue };
            match place.storage {
                "local" => described.builder.variable(i64::from(place.id), &place.name, r#type),
                // Every function binds the module's variables; described once.
                "module" if globals.insert((place.symbol, place.offset)) => {
                    described.builder.global(i64::from(place.symbol), i64::from(place.offset), &place.name, r#type);
                }
                _ => {}
            }
        }
        let result = described.r#type(function.result_type).filter(|&one| one != described.builder.scalar(DebugScalar::Void));
        described.builder.function(i64::from(function.id), &function.name, result);
    }
    described.builder.finish()
}
