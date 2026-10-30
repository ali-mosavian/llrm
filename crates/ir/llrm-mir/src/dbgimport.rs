//! Clang's `-g` types and file-scope variables (`!DIBasicType`,
//! `!DICompositeType`, `!DIGlobalVariable`, ...) as the model [`debuginfo`]
//! writes and a backend reads: `llrm.dbg.types` and `llrm.dbg.globals`.
//!
//! A procedure (`!DISubprogram`) and its variables (`!DILocalVariable`, named
//! by the `#dbg_declare` records of its body) go the same way: the records
//! name the model's variable nodes after, and the procedure lists its
//! parameters.
//!
//! A typedef or qualifier is its target: the model has no node for either.
//! A type the model cannot spell (a size it has no scalar for) is left out,
//! and so is whatever names it.

use crate::context::ConstantKind;
use crate::debuginfo::{self, Dialect, Function, Global, Kind, Language, Member, Reach, Scalar, Type, Variable};
use crate::hash::HashMap;
use crate::module::{DebugWhat, GlobalKind, MetadataId, MetadataOperand, Module};

/// What `module`'s specialized nodes describe, added to the model; the
/// language too. A module with none is left alone.
pub fn types_and_globals(module: &mut Module) {
    if module.special.is_empty() {
        return;
    }
    let mut import = Import { made: HashMap::default(), void: None, near: near_pointers(module) };
    for (at, attached) in module.global_metadata.clone() {
        let Some(name) = module.globals.get(at as usize).and_then(|one| one.name.as_deref()).map(str::to_owned) else {
            continue;
        };
        for (kind, id) in attached {
            if kind == "dbg" {
                import.global(module, &name, id);
            }
        }
    }
    import.functions(module);
    if let Some(language) = language(module) {
        debuginfo::set_language(module, language, Dialect::Cv4);
    }
    if let Some((name, md5)) = unit_file(module) {
        debuginfo::set_file(module, &name, md5.as_deref());
    }
}

/// The unit's source file, from the `!DIFile` of its `!DICompileUnit`: its
/// name, and the checksum where it is an MD5.
fn unit_file(module: &Module) -> Option<(String, Option<String>)> {
    let (id, _) = module.special.iter().find(|(_, one)| one.kind == "DICompileUnit")?;
    let Some(MetadataOperand::Node(file)) = operand(module, *id, "file") else { return None };
    let md5 = ident(module, *file, "checksumkind")
        .filter(|kind| kind == "CSK_MD5")
        .and_then(|_| text(module, *file, "checksum"));
    Some((text(module, *file, "filename")?, md5))
}

impl Import {
    /// Each defined function with a `!DISubprogram`: a procedure of the
    /// model, and its `#dbg_declare` records name variable nodes of the
    /// model.
    fn functions(
        &mut self,
        module: &mut Module,
    ) {
        for at in 0..module.globals.len() {
            let Some(function) = module.globals[at].function() else { continue };
            let Some(&(_, subprogram)) = function.metadata.iter().find(|(kind, _)| kind == "dbg") else { continue };
            let Some(name) = module.globals[at].name.as_deref().map(str::to_owned) else { continue };
            let mut records = function.debug_records.clone();
            let mut parameters: Vec<(i64, String, MetadataId)> = Vec::new();
            let mut made: HashMap<MetadataId, MetadataId> = HashMap::default();
            for record in &mut records {
                if !matches!(
                    record.what,
                    DebugWhat::Declare(_) | DebugWhat::Value(_) | DebugWhat::Args(_)
                ) {
                    continue;
                }
                let old = record.variable;
                if let Some(&new) = made.get(&old) {
                    record.variable = new;
                    continue;
                }
                if kind(module, old) != "DILocalVariable" {
                    continue;
                }
                let Some(variable_name) = text(module, old, "name") else { continue };
                let Some(r#type) = self.r#type(module, node(module, old, "type")) else { continue };
                // `arg: N` counts from 1: the position among the parameters.
                let argument = int(module, old, "arg").filter(|&one| one > 0).map(|one| one - 1);
                let new = debuginfo::add_variable(
                    module,
                    &Variable {
                        scope: name.clone(),
                        name: variable_name.clone(),
                        r#type,
                        offset: 0,
                        parameter: argument.is_some(),
                        argument,
                        // A variable of the function's own scope names none.
                        block: node(module, old, "scope").filter(|&scope| kind(module, scope) == "DILexicalBlock"),
                    },
                );
                if let Some(argument) = argument {
                    parameters.push((argument, variable_name, r#type));
                }
                made.insert(old, new);
                record.variable = new;
            }
            parameters.sort();
            parameters.dedup_by_key(|one| one.0);
            if let (Some(display), Some(procedure)) =
                (text(module, subprogram, "name"), self.r#type(module, node(module, subprogram, "type")))
            {
                debuginfo::add_function(
                    module,
                    &Function { function: name, module: false, name: display, r#type: procedure, parameters },
                );
            }
            if let GlobalKind::Function(function) = &mut module.globals[at].kind {
                function.debug_records = records;
            }
        }
    }
}

/// The unit's language, from its `!DICompileUnit`.
fn language(module: &Module) -> Option<Language> {
    let (id, _) = module.special.iter().find(|(_, one)| one.kind == "DICompileUnit")?;
    match ident(module, *id, "language")?.as_str() {
        word if word.starts_with("DW_LANG_C") => Some(Language::C),
        _ => None,
    }
}

/// A 16-bit target's pointers are near at 16 bits, far at 32.
fn near_pointers(module: &Module) -> u32 {
    let sixteen = module.datalayout.as_deref().is_some_and(|text| text.split('-').any(|one| one.starts_with("p:16")));
    if sixteen { 16 } else { 32 }
}

fn operand<'m>(
    module: &'m Module,
    id: MetadataId,
    field: &str,
) -> Option<&'m MetadataOperand> {
    let at = module.special.get(&id)?.fields.iter().position(|one| one == field)?;
    module.metadata.get(id.0 as usize)?.operands.get(at)
}

fn text(
    module: &Module,
    id: MetadataId,
    field: &str,
) -> Option<String> {
    match operand(module, id, field)? {
        MetadataOperand::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn ident(
    module: &Module,
    id: MetadataId,
    field: &str,
) -> Option<String> {
    match operand(module, id, field)? {
        MetadataOperand::Ident(value) => Some(value.clone()),
        _ => None,
    }
}

fn int(
    module: &Module,
    id: MetadataId,
    field: &str,
) -> Option<i64> {
    match operand(module, id, field)? {
        MetadataOperand::Constant(value) => match module.context.get(*value).kind {
            ConstantKind::Int(value) => Some(value as u64 as i64),
            _ => None,
        },
        _ => None,
    }
}

fn node(
    module: &Module,
    id: MetadataId,
    field: &str,
) -> Option<MetadataId> {
    match operand(module, id, field)? {
        MetadataOperand::Node(value) => Some(*value),
        _ => None,
    }
}

/// The nodes of the tuple `id` names in `field`.
fn elements(
    module: &Module,
    id: MetadataId,
    field: &str,
) -> Vec<MetadataId> {
    let Some(list) = node(module, id, field) else { return Vec::new() };
    module
        .metadata
        .get(list.0 as usize)
        .map_or_else(
            Vec::new,
            |one| one.operands
                .iter()
                .filter_map(|each| match each {
                    MetadataOperand::Node(id) => Some(*id),
                    _ => None,
                })
                .collect(),
        )
}

fn kind(
    module: &Module,
    id: MetadataId,
) -> &str {
    module.special.get(&id).map_or("", |one| one.kind.as_str())
}

struct Import {
    /// The model's node for each of clang's, None where it has none.
    made: HashMap<MetadataId, Option<MetadataId>>,
    void: Option<MetadataId>,
    /// Bits of a near pointer.
    near: u32,
}

impl Import {
    /// The variable `name`, whose clang node is `id`: a
    /// `!DIGlobalVariableExpression` of a `!DIGlobalVariable`.
    fn global(
        &mut self,
        module: &mut Module,
        global: &str,
        id: MetadataId,
    ) {
        let variable =
            if kind(module, id) == "DIGlobalVariableExpression" { node(module, id, "var").unwrap_or(id) } else { id };
        let Some(name) = text(module, variable, "name") else { return };
        let Some(r#type) = self.r#type(module, node(module, variable, "type")) else { return };
        debuginfo::add_global(module, &Global { global: global.to_owned(), offset: 0, name, r#type, scope: None });
    }

    fn scalar(
        module: &mut Module,
        scalar: Scalar,
        name: &str,
        size: i64,
    ) -> MetadataId {
        let spelling = (scalar != Scalar::Void).then(|| name.to_owned());
        debuginfo::add_type(
            module,
            &Type {
                kind: Kind::Scalar,
                name: scalar.value().to_owned(),
                size,
                reach: Reach::Near,
                target: None,
                members: Vec::new(),
                spelling,
            },
        )
    }

    fn void(
        &mut self,
        module: &mut Module,
    ) -> MetadataId {
        *self.void.get_or_insert_with(|| Self::scalar(module, Scalar::Void, "void", 0))
    }

    /// The model's type for the clang type `id`, where it has one; None for a
    /// missing `id` is `void`.
    fn r#type(
        &mut self,
        module: &mut Module,
        id: Option<MetadataId>,
    ) -> Option<MetadataId> {
        let Some(id) = id else { return Some(self.void(module)) };
        if let Some(&made) = self.made.get(&id) {
            return made;
        }
        let made = match kind(module, id) {
            "DIBasicType" => self.basic(module, id),
            "DIDerivedType" => self.derived(module, id),
            "DICompositeType" => self.composite(module, id),
            "DISubroutineType" => self.procedure(module, id),
            _ => None,
        };
        self.made.insert(id, made);
        made
    }

    fn basic(
        &mut self,
        module: &mut Module,
        id: MetadataId,
    ) -> Option<MetadataId> {
        let name = text(module, id, "name")?;
        let size = int(module, id, "size")?;
        let scalar = scalar_of(&ident(module, id, "encoding")?, size)?;
        Some(Self::scalar(module, scalar, &name, size / 8))
    }

    /// A typedef or qualifier is its base; a pointer is a pointer to it.
    fn derived(
        &mut self,
        module: &mut Module,
        id: MetadataId,
    ) -> Option<MetadataId> {
        let base = node(module, id, "baseType");
        match ident(module, id, "tag")?.as_str() {
            "DW_TAG_pointer_type" => {
                let bits = int(module, id, "size")?;
                let reach = if bits <= i64::from(self.near) { Reach::Near } else { Reach::Far };
                let target = self.r#type(module, base)?;
                Some(debuginfo::add_type(
                    module,
                    &Type {
                        kind: Kind::Pointer,
                        name: String::new(),
                        size: bits / 8,
                        reach,
                        target: Some(target),
                        members: Vec::new(),
                        spelling: None,
                    },
                ))
            }
            "DW_TAG_typedef"
            | "DW_TAG_const_type"
            | "DW_TAG_volatile_type"
            | "DW_TAG_restrict_type"
            | "DW_TAG_atomic_type" => self.r#type(module, base),
            _ => None,
        }
    }

    fn composite(
        &mut self,
        module: &mut Module,
        id: MetadataId,
    ) -> Option<MetadataId> {
        let size = int(module, id, "size").unwrap_or(0);
        match ident(module, id, "tag")?.as_str() {
            tag @ ("DW_TAG_structure_type" | "DW_TAG_union_type") => {
                let kind = if tag == "DW_TAG_union_type" { Kind::Union } else { Kind::Struct };
                let name = text(module, id, "name").unwrap_or_default();
                // Reserved first: a member may point back to its struct.
                let reserved = debuginfo::reserve_type(module);
                self.made.insert(id, Some(reserved));
                let mut members = Vec::new();
                for each in elements(module, id, "elements") {
                    if let Some(member) = self.member(module, each) {
                        members.push(member);
                    }
                }
                debuginfo::set_type(
                    module,
                    reserved,
                    &Type { kind, name, size: size / 8, reach: Reach::Near, target: None, members, spelling: None },
                );
                Some(reserved)
            }
            "DW_TAG_array_type" => {
                let element = self.r#type(module, node(module, id, "baseType"))?;
                Some(debuginfo::add_type(
                    module,
                    &Type {
                        kind: Kind::Sized,
                        name: String::new(),
                        size: size / 8,
                        reach: Reach::Near,
                        target: Some(element),
                        members: Vec::new(),
                        spelling: None,
                    },
                ))
            }
            // An enum is the integer it is held in.
            "DW_TAG_enumeration_type" => match node(module, id, "baseType") {
                Some(base) => self.r#type(module, Some(base)),
                None => {
                    let scalar = scalar_of("DW_ATE_unsigned", size)?;
                    Some(Self::scalar(module, scalar, "", size / 8))
                }
            },
            _ => None,
        }
    }

    fn member(
        &mut self,
        module: &mut Module,
        id: MetadataId,
    ) -> Option<Member> {
        if ident(module, id, "tag")? != "DW_TAG_member" {
            return None;
        }
        let name = text(module, id, "name").unwrap_or_default();
        let r#type = self.r#type(module, node(module, id, "baseType"))?;
        let offset = int(module, id, "offset").unwrap_or(0);
        let width = int(module, id, "size").unwrap_or(0);
        let bit_field = ident(module, id, "flags").is_some_and(|flags| flags.contains("DIFlagBitField"));
        Some(if bit_field {
            // `extraData` is where the storage unit starts, `offset` the first
            // bit.
            let unit = int(module, id, "extraData").unwrap_or(offset / 8 * 8);
            Member { name, r#type, offset: unit / 8, bits: Some((offset - unit, width)) }
        } else {
            Member { name, r#type, offset: offset / 8, bits: None }
        })
    }

    fn procedure(
        &mut self,
        module: &mut Module,
        id: MetadataId,
    ) -> Option<MetadataId> {
        let listed = node(module, id, "types").and_then(|list| module.metadata.get(list.0 as usize))?.operands.clone();
        let mut types = listed
            .iter()
            .map(
                |each| match each {
                    MetadataOperand::Node(one) => Some(*one),
                    _ => None,
                },
            );
        // The first is the result, null for `void`.
        let result = match types.next()? {
            Some(result) => Some(self.r#type(module, Some(result))?),
            None => None,
        };
        let mut members = Vec::new();
        for each in types {
            members.push(Member { name: String::new(), r#type: self.r#type(module, each)?, offset: 0, bits: None });
        }
        Some(debuginfo::add_type(
            module,
            &Type {
                kind: Kind::Procedure,
                name: String::new(),
                size: 0,
                reach: Reach::Near,
                target: result,
                members,
                spelling: None,
            },
        ))
    }
}

/// The model's scalar for an encoding at `size` bits.
fn scalar_of(
    encoding: &str,
    size: i64,
) -> Option<Scalar> {
    Some(match (encoding, size) {
        ("DW_ATE_float", 32) => Scalar::Float32,
        ("DW_ATE_float", 64) => Scalar::Float64,
        ("DW_ATE_float", 80 | 96 | 128) => Scalar::Float80,
        ("DW_ATE_signed_char", 8) => Scalar::Char,
        ("DW_ATE_signed", 8) => Scalar::Int8,
        ("DW_ATE_unsigned" | "DW_ATE_unsigned_char" | "DW_ATE_boolean", 8) => Scalar::UInt8,
        ("DW_ATE_signed", 16) => Scalar::Int16,
        ("DW_ATE_unsigned" | "DW_ATE_boolean", 16) => Scalar::UInt16,
        ("DW_ATE_signed", 32) => Scalar::Int32,
        ("DW_ATE_unsigned" | "DW_ATE_boolean", 32) => Scalar::UInt32,
        ("DW_ATE_signed", 64) => Scalar::Int64,
        ("DW_ATE_unsigned" | "DW_ATE_boolean", 64) => Scalar::UInt64,
        _ => return None,
    })
}
