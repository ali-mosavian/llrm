//! `-g`'s metadata: source types, procedures, parameters and variables. A
//! lowering writes it and a backend reads it, both through here.
//!
//! A type is a node `!{!"kind", !"name", i64 size, !"address", target, !{members}}`
//! and a member `!{!"name", type, i64 offset}`. Named metadata [`TYPES`],
//! [`FUNCTIONS`] and [`GLOBALS`] list the types, each after those it names,
//! the procedures and the variables in data; a variable in a frame is an `llvm.dbg.declare` of its storage,
//! its [`VARIABLE`] attachment naming it.

use crate::module::{MetadataId, MetadataNode, MetadataOperand, Module};
use crate::context::ConstantKind;

/// An enum of fixed spellings, as metadata and HIR's JSON write it.
macro_rules! spelled {
    ($(#[$doc:meta])* $name:ident { $($(#[$each:meta])* $variant:ident = $value:literal,)* }) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $name {
            $($(#[$each])* $variant,)*
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];
            pub const VALUES: &'static [&'static str] = &[$($value,)*];

            pub const fn value(self) -> &'static str {
                match self {
                    $(Self::$variant => $value,)*
                }
            }

            pub fn from_value(value: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|one| one.value() == value)
            }
        }
    };
}

spelled!(
    /// What a source type is.
    Kind {
        /// The [`Scalar`] its name spells.
        Scalar = "scalar",
        /// BASIC's `STRING * size`.
        FixedString = "fixed_string",
        /// BASIC's array of its target, bounded by its descriptor.
        Array = "array",
        /// `size` bytes of its targets in place.
        Sized = "sized",
        /// Named, `size` bytes of its members.
        Struct = "struct",
        Pointer = "pointer",
        /// A parameter passed by reference to its target.
        Reference = "reference",
        /// Returning its target; its members its parameters' types.
        Procedure = "procedure",
    }
);

spelled!(
    Scalar {
        Void = "void",
        Char = "char",
        Int8 = "int8",
        UInt8 = "uint8",
        Int16 = "int16",
        UInt16 = "uint16",
        Int32 = "int32",
        UInt32 = "uint32",
        Float32 = "float32",
        Float64 = "float64",
        Float80 = "float80",
        Currency = "currency",
        /// BASIC's variable-length STRING, by its near descriptor.
        String = "string",
        /// ... and by its far one.
        FarString = "far string",
    }
);

spelled!(
    /// How far a pointer reaches.
    Reach {
        Near = "near",
        Far = "far",
        Huge = "huge",
    }
);

pub const TYPES: &str = "llrm.dbg.types";
pub const FUNCTIONS: &str = "llrm.dbg.functions";
pub const GLOBALS: &str = "llrm.dbg.globals";
/// The attachment naming an `llvm.dbg.declare`'s variable.
pub const VARIABLE: &str = "var";

/// A source type: `name` a scalar's spelling or a structure's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Type {
    pub kind: Kind,
    pub name: String,
    pub size: i64,
    pub reach: Reach,
    pub target: Option<MetadataId>,
    pub members: Vec<Member>,
}

/// A structure's field, or a procedure's parameter by its type alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Member {
    pub name: String,
    pub r#type: MetadataId,
    pub offset: i64,
}

/// A procedure: MIR's `function`, and its parameters by argument index;
/// `module` the module's own code, whose variables are the module's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    pub function: String,
    pub module: bool,
    pub name: String,
    pub r#type: MetadataId,
    pub parameters: Vec<(i64, String, MetadataId)>,
}

/// A variable `offset` bytes into MIR's `global`; `scope` the function
/// declaring it, None for the module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Global {
    pub global: String,
    pub offset: i64,
    pub name: String,
    pub r#type: MetadataId,
    pub scope: Option<String>,
}

/// A variable `offset` bytes into the storage its declare points at, in
/// the function `scope`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variable {
    pub scope: String,
    pub name: String,
    pub r#type: MetadataId,
    pub offset: i64,
}

fn node(module: &mut Module, operands: Vec<MetadataOperand>) -> MetadataId {
    module.metadata.push(MetadataNode { distinct: false, operands });
    MetadataId(module.metadata.len() as u32 - 1)
}

fn text(value: &str) -> MetadataOperand {
    MetadataOperand::String(value.to_owned())
}

fn int(module: &mut Module, value: i64) -> MetadataOperand {
    let i64 = module.context.types.int(64);
    MetadataOperand::Constant(module.context.int(i64, i128::from(value)))
}

fn named(module: &mut Module, name: &str, id: MetadataId) {
    match module.named_metadata.iter_mut().find(|(one, _)| one == name) {
        Some((_, ids)) => ids.push(id),
        None => module.named_metadata.push((name.to_owned(), vec![id])),
    }
}

/// Reads a node's operands.
struct Reader<'m> {
    module: &'m Module,
    operands: &'m [MetadataOperand],
}

impl<'m> Reader<'m> {
    fn of(module: &'m Module, id: MetadataId) -> Option<Self> {
        Some(Self { module, operands: &module.metadata.get(id.0 as usize)?.operands })
    }

    fn text(&self, at: usize) -> Option<String> {
        match self.operands.get(at)? {
            MetadataOperand::String(value) => Some(value.clone()),
            _ => None,
        }
    }

    fn int(&self, at: usize) -> Option<i64> {
        match self.operands.get(at)? {
            MetadataOperand::Constant(value) => match self.module.context.get(*value).kind {
                // Written as i64.
                ConstantKind::Int(value) => Some(value as u64 as i64),
                _ => None,
            },
            _ => None,
        }
    }

    fn node(&self, at: usize) -> Option<MetadataId> {
        match self.operands.get(at)? {
            MetadataOperand::Node(id) => Some(*id),
            _ => None,
        }
    }

    /// Each operand of the list node at `at`, read by `each`.
    fn list<T>(&self, at: usize, each: impl Fn(&Reader<'m>) -> Option<T>) -> Option<Vec<T>> {
        let list = Reader::of(self.module, self.node(at)?)?;
        list.operands
            .iter()
            .map(|one| match one {
                MetadataOperand::Node(id) => each(&Reader::of(self.module, *id)?),
                _ => None,
            })
            .collect()
    }
}

fn list(module: &mut Module, items: Vec<Vec<MetadataOperand>>) -> MetadataOperand {
    let ids: Vec<MetadataOperand> = items.into_iter().map(|one| MetadataOperand::Node(node(module, one))).collect();
    MetadataOperand::Node(node(module, ids))
}

pub fn add_type(module: &mut Module, one: &Type) -> MetadataId {
    let members = one.members.iter().map(|member| vec![text(&member.name), MetadataOperand::Node(member.r#type), int(module, member.offset)]).collect();
    let members = list(module, members);
    let size = int(module, one.size);
    let target = one.target.map_or(MetadataOperand::Null, MetadataOperand::Node);
    let id = node(module, vec![text(one.kind.value()), text(&one.name), size, text(one.reach.value()), target, members]);
    named(module, TYPES, id);
    id
}

/// Every type node, each after those it names.
pub fn types(module: &Module) -> Vec<MetadataId> {
    listed(module, TYPES).collect()
}

pub fn read_type(module: &Module, id: MetadataId) -> Option<Type> {
    let one = Reader::of(module, id)?;
    Some(Type {
        kind: Kind::from_value(&one.text(0)?)?,
        name: one.text(1)?,
        size: one.int(2)?,
        reach: Reach::from_value(&one.text(3)?)?,
        target: one.node(4),
        members: one.list(5, |member| Some(Member { name: member.text(0)?, r#type: member.node(1)?, offset: member.int(2)? }))?,
    })
}

pub fn add_function(module: &mut Module, one: &Function) {
    let parameters = one.parameters.iter().map(|(index, name, r#type)| vec![int(module, *index), text(name), MetadataOperand::Node(*r#type)]).collect();
    let parameters = list(module, parameters);
    let flag = int(module, i64::from(one.module));
    let id = node(module, vec![text(&one.function), text(&one.name), MetadataOperand::Node(one.r#type), parameters, flag]);
    named(module, FUNCTIONS, id);
}

pub fn functions(module: &Module) -> Vec<Function> {
    let read = |id: MetadataId| {
        let one = Reader::of(module, id)?;
        Some(Function {
            function: one.text(0)?,
            name: one.text(1)?,
            r#type: one.node(2)?,
            parameters: one.list(3, |parameter| Some((parameter.int(0)?, parameter.text(1)?, parameter.node(2)?)))?,
            module: one.int(4)? != 0,
        })
    };
    listed(module, FUNCTIONS).filter_map(read).collect()
}

pub fn add_global(module: &mut Module, one: &Global) {
    let offset = int(module, one.offset);
    let scope = one.scope.as_deref().map_or(MetadataOperand::Null, text);
    let id = node(module, vec![text(&one.global), offset, text(&one.name), MetadataOperand::Node(one.r#type), scope]);
    named(module, GLOBALS, id);
}

pub fn globals(module: &Module) -> Vec<Global> {
    let read = |id: MetadataId| {
        let one = Reader::of(module, id)?;
        Some(Global { global: one.text(0)?, offset: one.int(1)?, name: one.text(2)?, r#type: one.node(3)?, scope: one.text(4) })
    };
    listed(module, GLOBALS).filter_map(read).collect()
}

/// The node an `llvm.dbg.declare` attaches as [`VARIABLE`].
pub fn add_variable(module: &mut Module, one: &Variable) -> MetadataId {
    let offset = int(module, one.offset);
    node(module, vec![text(&one.scope), text(&one.name), MetadataOperand::Node(one.r#type), offset])
}

pub fn read_variable(module: &Module, id: MetadataId) -> Option<Variable> {
    let one = Reader::of(module, id)?;
    Some(Variable { scope: one.text(0)?, name: one.text(1)?, r#type: one.node(2)?, offset: one.int(3)? })
}

fn listed<'m>(module: &'m Module, name: &str) -> impl Iterator<Item = MetadataId> + 'm {
    let ids = module.named_metadata.iter().find(|(one, _)| one == name).map_or(&[][..], |(_, ids)| ids.as_slice());
    ids.iter().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What is written reads back as it was.
    #[test]
    fn each_record_reads_back() {
        let mut module = Module::default();
        let scalar = Type { kind: Kind::Scalar, name: "int16".into(), size: 0, reach: Reach::Near, target: None, members: Vec::new() };
        let int16 = add_type(&mut module, &scalar);
        let structure = Type { kind: Kind::Struct, name: "Pt".into(), size: 4, reach: Reach::Near, target: None, members: vec![Member { name: "x".into(), r#type: int16, offset: 2 }] };
        let pt = add_type(&mut module, &structure);
        assert_eq!(read_type(&module, int16), Some(scalar));
        assert_eq!(read_type(&module, pt), Some(structure));
        let function = Function { function: "f".into(), module: false, name: "F".into(), r#type: pt, parameters: vec![(1, "n".into(), int16)] };
        add_function(&mut module, &function);
        assert_eq!(functions(&module), [function]);
        let global = Global { global: "g".into(), offset: 4, name: "G".into(), r#type: int16, scope: Some("f".into()) };
        add_global(&mut module, &global);
        assert_eq!(globals(&module), [global]);
        let variable = Variable { scope: "f".into(), name: "v".into(), r#type: pt, offset: -2 };
        let id = add_variable(&mut module, &variable);
        assert_eq!(read_variable(&module, id), Some(variable));
    }
}
