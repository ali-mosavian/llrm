//! `-g`'s metadata: source types, procedures, parameters and variables. A
//! lowering writes it and a backend reads it, both through here.
//!
//! A type is a node `!{!"kind", !"name", i64 size, !"address", target, !{members}}`
//! and a member `!{!"name", type, i64 offset}`, a bit field's with its `i64 start, i64 width` after. Named metadata [`TYPES`],
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
        /// A struct whose members all start at 0.
        Union = "union",
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
        Int64 = "int64",
        UInt64 = "uint64",
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

spelled!(
    /// The language a program is written in, which a debugger reads its values by.
    Language {
        C = "c",
        Basic = "basic",
        Nib = "nib",
    }
);

spelled!(
    /// The form of CodeView 4 a program's records are written in: its frontend says.
    Dialect {
        /// What BASIC's compilers write.
        Bc = "bc",
        /// The standard form.
        Cv4 = "cv4",
    }
);

pub const LANGUAGE: &str = "llrm.dbg.language";
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
    /// A scalar's name in the source, where it has one (`unsigned long`, not `uint32`): what a debugger prints.
    pub spelling: Option<String>,
}

/// A structure's field, or a procedure's parameter by its type alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Member {
    pub name: String,
    pub r#type: MetadataId,
    pub offset: i64,
    /// A bit field's first bit in the unit at `offset`, and its width.
    pub bits: Option<(i64, i64)>,
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
    /// Where a parameter's value is kept, not a variable the body declares.
    pub parameter: bool,
    /// Of a parameter's home: the argument the function was passed it as.
    pub argument: Option<i64>,
}

/// Kind of the metadata that says a store or a `memcpy` is volatile only so the optimiser leaves it for a debugger to read the
/// variable it writes (`-g`): a build that does not run the optimiser, or finds the variable another way, lifts it.
pub const OBSERVED: &str = "llrm.observed";

/// The node [`OBSERVED`] attaches.
pub fn observed_node(module: &mut Module) -> MetadataId {
    node(module, Vec::new())
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
    let id = reserve_type(module);
    set_type(module, id, one);
    id
}

/// A type node to be set, so a member of it can name it: an aggregate that holds a pointer to itself.
pub fn reserve_type(module: &mut Module) -> MetadataId {
    let id = node(module, Vec::new());
    named(module, TYPES, id);
    id
}

/// The type `id` reserved is `one`.
pub fn set_type(module: &mut Module, id: MetadataId, one: &Type) {
    let members = one
        .members
        .iter()
        .map(|member| {
            let mut made = vec![text(&member.name), MetadataOperand::Node(member.r#type), int(module, member.offset)];
            made.extend(member.bits.into_iter().flat_map(|(start, width)| [int(module, start), int(module, width)]));
            made
        })
        .collect();
    let members = list(module, members);
    let size = int(module, one.size);
    let target = one.target.map_or(MetadataOperand::Null, MetadataOperand::Node);
    let mut operands = vec![text(one.kind.value()), text(&one.name), size, text(one.reach.value()), target, members];
    // After the members, only where there is one: a node without it has no spelling.
    operands.extend(one.spelling.as_deref().map(text));
    module.metadata[id.0 as usize].operands = operands;
}

/// Every type node, in the order they were made: a member may name one made after it (an aggregate that
/// holds a pointer to itself), so a reader reads all before it resolves any.
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
        members: one.list(5, |member| Some(Member { name: member.text(0)?, r#type: member.node(1)?, offset: member.int(2)?, bits: member.int(3).zip(member.int(4)) }))?,
        spelling: one.text(6),
    })
}

pub fn add_function(module: &mut Module, one: &Function) {
    let parameters = one.parameters.iter().map(|(index, name, r#type)| vec![int(module, *index), text(name), MetadataOperand::Node(*r#type)]).collect();
    let parameters = list(module, parameters);
    let flag = int(module, i64::from(one.module));
    let id = node(module, vec![text(&one.function), text(&one.name), MetadataOperand::Node(one.r#type), parameters, flag]);
    named(module, FUNCTIONS, id);
}

/// The module's source language.
pub fn set_language(module: &mut Module, language: Language, dialect: Dialect) {
    let id = node(module, vec![text(language.value()), text(dialect.value())]);
    named(module, LANGUAGE, id);
}

/// The form of CodeView 4 the frontend says: the BASIC compilers' where it says none.
pub fn dialect(module: &Module) -> Dialect {
    listed(module, LANGUAGE).next().and_then(|id| Dialect::from_value(&Reader::of(module, id)?.text(1)?)).unwrap_or(Dialect::Bc)
}

pub fn language(module: &Module) -> Option<Language> {
    let id = listed(module, LANGUAGE).next()?;
    Language::from_value(&Reader::of(module, id)?.text(0)?)
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
    let mut operands = vec![text(&one.scope), text(&one.name), MetadataOperand::Node(one.r#type), offset];
    // After the offset, only where true: a node without it is no parameter's.
    if one.parameter {
        operands.push(int(module, 1));
        // And after that, only for a home whose argument is known.
        if let Some(argument) = one.argument {
            operands.push(int(module, argument));
        }
    }
    node(module, operands)
}

pub fn read_variable(module: &Module, id: MetadataId) -> Option<Variable> {
    let one = Reader::of(module, id)?;
    Some(Variable { scope: one.text(0)?, name: one.text(1)?, r#type: one.node(2)?, offset: one.int(3)?, parameter: one.int(4).is_some_and(|flag| flag != 0), argument: one.int(5) })
}

fn listed<'m>(module: &'m Module, name: &str) -> impl Iterator<Item = MetadataId> + 'm {
    let ids = module.named_metadata.iter().find(|(one, _)| one == name).map_or(&[][..], |(_, ids)| ids.as_slice());
    ids.iter().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bit field's member node has its start and width after its offset;
    /// a member node without them, as every one before them, is no bit
    /// field. Both survive printing and parsing the module.
    #[test]
    fn a_bit_fields_member_round_trips_and_an_older_member_reads_as_none() {
        let mut module = Module::default();
        let scalar = Type { kind: Kind::Scalar, name: "int16".into(), size: 0, reach: Reach::Near, target: None, members: Vec::new(), spelling: None };
        let int16 = add_type(&mut module, &scalar);
        let members = vec![Member { name: "x".into(), r#type: int16, offset: 0, bits: None }, Member { name: "f".into(), r#type: int16, offset: 2, bits: Some((3, 5)) }];
        let structure = Type { kind: Kind::Struct, name: "Pt".into(), size: 4, reach: Reach::Near, target: None, members, spelling: None };
        add_type(&mut module, &structure);
        let reparsed = crate::parse::module(&crate::print::module(&module)).expect("parses");
        let read: Vec<Type> = types(&reparsed).into_iter().filter_map(|id| read_type(&reparsed, id)).collect();
        let pt = read.iter().find(|one| one.kind == Kind::Struct).expect("the struct");
        assert_eq!(pt.members.iter().map(|one| one.bits).collect::<Vec<_>>(), [None, Some((3, 5))]);
    }

    /// A struct reserved before its members, one a pointer to it: it names a node made after it, and it
    /// survives printing and parsing. Made after its members, as every type was, there is no such node.
    #[test]
    fn a_struct_that_points_to_itself_reads_back_whole() {
        let mut module = Module::default();
        let scalar = Type { kind: Kind::Scalar, name: "int16".into(), size: 0, reach: Reach::Near, target: None, members: Vec::new(), spelling: None };
        let int16 = add_type(&mut module, &scalar);
        let node = reserve_type(&mut module);
        let pointer = add_type(&mut module, &Type { kind: Kind::Pointer, name: String::new(), size: 0, reach: Reach::Near, target: Some(node), members: Vec::new(), spelling: None });
        let members = vec![Member { name: "next".into(), r#type: pointer, offset: 0, bits: None }, Member { name: "v".into(), r#type: int16, offset: 2, bits: None }];
        set_type(&mut module, node, &Type { kind: Kind::Struct, name: "node".into(), size: 4, reach: Reach::Near, target: None, members, spelling: None });
        let reparsed = crate::parse::module(&crate::print::module(&module)).expect("parses");
        let read: Vec<Type> = types(&reparsed).into_iter().filter_map(|id| read_type(&reparsed, id)).collect();
        assert_eq!(read.len(), 3);
        let node = read.iter().find(|one| one.kind == Kind::Struct).expect("the struct");
        assert_eq!(node.members.iter().map(|one| one.name.as_str()).collect::<Vec<_>>(), ["next", "v"]);
        let pointer = read.iter().find(|one| one.kind == Kind::Pointer).expect("the pointer");
        assert_eq!(pointer.target, Some(types(&reparsed)[1]));
    }

    /// What is written reads back as it was.
    #[test]
    fn each_record_reads_back() {
        let mut module = Module::default();
        let scalar = Type { kind: Kind::Scalar, name: "int16".into(), size: 0, reach: Reach::Near, target: None, members: Vec::new(), spelling: None };
        let int16 = add_type(&mut module, &scalar);
        let structure = Type { kind: Kind::Struct, name: "Pt".into(), size: 4, reach: Reach::Near, target: None, spelling: None, members: vec![Member { name: "x".into(), r#type: int16, offset: 2, bits: None }, Member { name: "f".into(), r#type: int16, offset: 0, bits: Some((3, 5)) }] };
        let pt = add_type(&mut module, &structure);
        // A spelling is written only where there is one, and reads back.
        let spelled = Type { spelling: Some("short".into()), ..scalar.clone() };
        let short = add_type(&mut module, &spelled);
        assert_eq!(read_type(&module, short), Some(spelled));
        assert_eq!(module.metadata[int16.0 as usize].operands.len(), 6, "a type without one is written as before");
        assert_eq!(read_type(&module, int16), Some(scalar));
        assert_eq!(read_type(&module, pt), Some(structure));
        let function = Function { function: "f".into(), module: false, name: "F".into(), r#type: pt, parameters: vec![(1, "n".into(), int16)] };
        add_function(&mut module, &function);
        assert_eq!(functions(&module), [function]);
        let global = Global { global: "g".into(), offset: 4, name: "G".into(), r#type: int16, scope: Some("f".into()) };
        add_global(&mut module, &global);
        assert_eq!(globals(&module), [global]);
        let variable = Variable { scope: "f".into(), name: "v".into(), r#type: pt, offset: -2, parameter: false, argument: None };
        let id = add_variable(&mut module, &variable);
        assert_eq!(read_variable(&module, id), Some(variable));
    }

    /// A parameter's home read back as an ordinary local: the C frontend keeps a parameter in a
    /// frame slot, and nothing past the frontend could say it was passed in.
    #[test]
    fn a_parameters_home_reads_back_as_one_and_a_plain_node_as_none() {
        let mut module = Module::default();
        let scalar = Type { kind: Kind::Scalar, name: "int16".into(), size: 0, reach: Reach::Near, target: None, members: Vec::new(), spelling: None };
        let int16 = add_type(&mut module, &scalar);
        let home = Variable { scope: "f".into(), name: "a".into(), r#type: int16, offset: 0, parameter: true, argument: None };
        let local = Variable { parameter: false, name: "l".into(), ..home.clone() };
        let (home_id, local_id) = (add_variable(&mut module, &home), add_variable(&mut module, &local));
        assert_eq!(module.metadata[local_id.0 as usize].operands.len(), 4, "a local's node is what it was");
        let reparsed = crate::parse::module(&crate::print::module(&module)).expect("parses");
        assert_eq!((read_variable(&reparsed, home_id), read_variable(&reparsed, local_id)), (Some(home), Some(local)));
    }
}
