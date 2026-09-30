//! The inverse of [`cvinfo`](crate::cvinfo): $$SYMBOLS and $$TYPES in BC's
//! pre-link layout, which LINK /CO hands to CVPACK. docs/machine/codeview.md
//! has the measurements behind every byte.

use llrm_support::hash::IndexMap;

use crate::cvinfo::{self, Kind, Tag, BASE_TYPE_INDEX};

/// A type, by its index in [`Module::types`].
pub type TypeId = usize;

/// A scalar: its own primitive code, never a $$TYPES record.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Scalar {
    Void,
    Char,
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Float32,
    Float64,
    Float80,
    Currency,
    /// BASIC's variable-length STRING: a near or a far descriptor.
    String { far: bool },
}

impl Scalar {
    /// The primitive code, each measured against BC's objects or CodeView 4.
    pub fn code(self) -> Result<u16, String> {
        let code = match self {
            Scalar::Void => cvinfo::VOID,
            // CodeView has one char.
            Scalar::Char | Scalar::Int8 => cvinfo::CHAR,
            Scalar::UInt8 => cvinfo::UNSIGNED_CHAR,
            Scalar::UInt16 => cvinfo::UNSIGNED_SHORT,
            Scalar::UInt32 => cvinfo::UNSIGNED_LONG,
            Scalar::Float80 => cvinfo::LONG_DOUBLE,
            Scalar::Int16 => cvinfo::INTEGER,
            Scalar::Int32 => cvinfo::LONG,
            Scalar::Float32 => cvinfo::SINGLE,
            Scalar::Float64 => cvinfo::DOUBLE,
            Scalar::Currency => cvinfo::CURRENCY,
            Scalar::String { far: false } => cvinfo::NEAR_STRING,
            Scalar::String { far: true } => cvinfo::FAR_STRING,
        };
        Ok(code as u16)
    }
}

/// How far a pointer reaches.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Reach {
    Near,
    Far,
    Huge,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Field {
    pub name: String,
    pub r#type: TypeId,
    pub offset: u16,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    Scalar(Scalar),
    /// BASIC's `STRING * n`.
    FixedString(u16),
    /// BASIC's array: its element only, the bounds are its descriptor's.
    Array(TypeId),
    /// An array of `bytes` in place, as C lays one out.
    Sized { element: TypeId, bytes: u32 },
    Struct { name: String, bytes: u32, fields: Vec<Field> },
    Pointer { target: TypeId, reach: Reach },
    /// A parameter passed by reference.
    Reference(TypeId),
    /// `result` None returns nothing: void.
    Procedure { result: Option<TypeId>, parameters: Vec<TypeId> },
}

/// A parameter or local at `bp` from the frame pointer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Local {
    pub name: String,
    pub r#type: TypeId,
    pub bp: i16,
}

/// A variable at `symbol` + `displacement` in a data segment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Data {
    pub name: String,
    pub r#type: TypeId,
    pub symbol: String,
    pub displacement: u16,
}

/// A procedure at `symbol`; the offsets are from its start.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Procedure {
    pub name: String,
    pub symbol: String,
    pub r#type: TypeId,
    pub length: u16,
    pub debug_start: u16,
    pub debug_end: u16,
    pub far: bool,
    pub locals: Vec<Local>,
    pub statics: Vec<Data>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Module {
    /// None: QB 4.5's nameless module record.
    pub name: Option<String>,
    /// Where the module's code starts, and how long it is.
    pub start: String,
    pub length: u16,
    pub types: Vec<Type>,
    pub procedures: Vec<Procedure>,
    pub data: Vec<Data>,
}

/// What differs between the debuggers BC targets.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Flavor {
    /// QB 4.5: a reference to a scalar is its code plus 0x20, and to
    /// anything else a bare pointer; `STRING * n` is an array of chars.
    pub qb45: bool,
}

/// A field `at` in [`Written::symbols`] that LINK fills in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Relocation {
    pub at: usize,
    /// A far pointer (seg:off) rather than an offset.
    pub far: bool,
    pub symbol: String,
    pub displacement: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Written {
    pub symbols: Vec<u8>,
    pub types: Vec<u8>,
    pub relocations: Vec<Relocation>,
}

type Made<T> = Result<T, String>;

fn narrow<T: TryFrom<usize>>(value: usize, what: &str) -> Made<T> {
    T::try_from(value).map_err(|_| format!("{what} {value} does not fit its field"))
}

/// A length-prefixed name, in the latin-1 BC writes.
fn pascal(name: &str) -> Made<Vec<u8>> {
    let bytes = name.chars().map(|one| u8::try_from(u32::from(one)).map_err(|_| format!("{name:?} is not latin-1"))).collect::<Made<Vec<u8>>>()?;
    Ok([&[narrow::<u8>(bytes.len(), "a name's length")?][..], &bytes].concat())
}

fn reference(index: u16) -> [u8; 3] {
    let [lo, hi] = index.to_le_bytes();
    [Tag::TypeRef as u8, lo, hi]
}

/// The $$TYPES table being built: each record's index by its bytes.
struct Table<'m> {
    module: &'m Module,
    flavor: Flavor,
    bytes: Vec<u8>,
    indices: IndexMap<Vec<u8>, u16>,
    of: IndexMap<TypeId, u16>,
}

impl Table<'_> {
    /// The index of the record `leaf` is, made if new.
    fn record(&mut self, leaf: Vec<u8>) -> Made<u16> {
        if let Some(&index) = self.indices.get(&leaf) {
            return Ok(index);
        }
        let index = narrow::<u16>(BASE_TYPE_INDEX as usize + self.indices.len(), "a type index")?;
        self.bytes.push(0x01);
        self.bytes.extend(narrow::<u16>(leaf.len(), "a type record's length")?.to_le_bytes());
        self.bytes.extend(&leaf);
        self.indices.insert(leaf, index);
        Ok(index)
    }

    fn list(&mut self, indices: &[u16]) -> Made<u16> {
        if indices.is_empty() {
            return Ok(BASE_TYPE_INDEX as u16);
        }
        self.record(std::iter::once(Tag::List as u8).chain(indices.iter().flat_map(|&one| reference(one))).collect())
    }

    /// An array in place: `bits` of `element`s.
    fn sized(&mut self, element: u16, bits: u32) -> Made<u16> {
        let mut leaf = vec![Tag::FixedStringQb45 as u8, cvinfo::U32];
        leaf.extend(bits.to_le_bytes());
        leaf.extend(reference(element));
        self.record(leaf)
    }

    fn pointer(&mut self, target: u16, reach: Reach) -> Made<u16> {
        let reach = match reach {
            Reach::Near => cvinfo::NEAR,
            Reach::Far => cvinfo::FAR,
            Reach::Huge => cvinfo::HUGE,
        };
        self.record([&[Tag::Pointer as u8, reach][..], &reference(target)].concat())
    }

    fn bits(bytes: u32) -> Made<u32> {
        bytes.checked_mul(8).ok_or_else(|| format!("{bytes} bytes do not fit a size in bits"))
    }

    /// `id`'s index: a scalar's code, or its record's, dependencies first.
    fn index(&mut self, id: TypeId) -> Made<u16> {
        if let Some(&index) = self.of.get(&id) {
            return Ok(index);
        }
        let index = match &self.module.types[id] {
            Type::Scalar(scalar) => scalar.code()?,
            &Type::FixedString(length) if self.flavor.qb45 => self.sized(cvinfo::CHAR as u16, u32::from(length) * 8)?,
            Type::FixedString(length) => {
                let mut leaf = vec![Tag::FixedString as u8, 0x00, Tag::Offset as u8];
                leaf.extend(length.to_le_bytes());
                self.record(leaf)?
            }
            &Type::Array(element) => {
                let element = self.index(element)?;
                self.record([&[Tag::Array as u8][..], &reference(element)].concat())?
            }
            &Type::Sized { element, bytes } => {
                let element = self.index(element)?;
                self.sized(element, Self::bits(bytes)?)?
            }
            Type::Struct { name, bytes, fields } => {
                let types = fields.iter().map(|field| self.index(field.r#type)).collect::<Made<Vec<u16>>>()?;
                let types = self.list(&types)?;
                let mut names = vec![Tag::List as u8];
                for field in fields {
                    names.push(Tag::Name as u8);
                    names.extend(pascal(&field.name)?);
                    names.push(Tag::Offset as u8);
                    names.extend(field.offset.to_le_bytes());
                }
                let names = self.record(names)?;
                let mut leaf = vec![Tag::Struct as u8, cvinfo::U32];
                leaf.extend(Self::bits(*bytes)?.to_le_bytes());
                leaf.push(Tag::Offset as u8);
                leaf.extend(narrow::<u16>(fields.len(), "a field count")?.to_le_bytes());
                leaf.extend(reference(types));
                leaf.extend(reference(names));
                leaf.push(Tag::Name as u8);
                leaf.extend(pascal(name)?);
                leaf.push(cvinfo::UNPACKED);
                self.record(leaf)?
            }
            &Type::Pointer { target, reach } => {
                let target = self.index(target)?;
                self.pointer(target, reach)?
            }
            &Type::Reference(target) => match self.module.types[target] {
                Type::Scalar(scalar) if self.flavor.qb45 => narrow::<u16>(usize::from(scalar.code()?) + cvinfo::QB45_BYREF as usize, "a reference code")?,
                _ if self.flavor.qb45 => {
                    let target = self.index(target)?;
                    self.pointer(target, Reach::Near)?
                }
                _ => {
                    let target = self.index(target)?;
                    let pointer = self.pointer(target, Reach::Near)?;
                    self.record([&[Tag::ByRef as u8][..], &reference(pointer)].concat())?
                }
            },
            Type::Procedure { result, parameters } => {
                let result = match result {
                    Some(result) => self.index(*result)?,
                    None => Scalar::Void.code()?,
                };
                let parameters = parameters.iter().map(|&one| self.index(one)).collect::<Made<Vec<u16>>>()?;
                let list = self.list(&parameters)?;
                // The count is a numeric leaf's value itself: under 0x80.
                let count = u8::try_from(parameters.len()).ok().filter(|&count| count < 0x80).ok_or("a procedure of 128 parameters or more")?;
                let mut leaf = vec![Tag::Signature as u8, cvinfo::NIL];
                leaf.extend(reference(result));
                leaf.extend([cvinfo::BASIC_CALL, count]);
                leaf.extend(reference(list));
                self.record(leaf)?
            }
        };
        self.of.insert(id, index);
        Ok(index)
    }
}

/// $$SYMBOLS being built.
#[derive(Default)]
struct Symbols {
    bytes: Vec<u8>,
    relocations: Vec<Relocation>,
}

impl Symbols {
    fn record(&mut self, kind: Kind, data: &[u8], relocation: Option<(bool, &str, u16)>) -> Made<()> {
        self.bytes.push(narrow::<u8>(1 + data.len(), "a symbol record's length")?);
        self.bytes.push(kind as u8);
        if let Some((far, symbol, displacement)) = relocation {
            self.relocations.push(Relocation { at: self.bytes.len(), far, symbol: symbol.to_owned(), displacement });
        }
        self.bytes.extend(data);
        Ok(())
    }

    fn data(&mut self, table: &mut Table, one: &Data) -> Made<()> {
        let mut data = vec![0; 4];
        data.extend(table.index(one.r#type)?.to_le_bytes());
        data.extend(pascal(&one.name)?);
        self.record(Kind::LData, &data, Some((true, &one.symbol, one.displacement)))
    }
}

/// `module`'s $$SYMBOLS and $$TYPES.
pub fn written(module: &Module, flavor: Flavor) -> Made<Written> {
    let mut table = Table { module, flavor, bytes: Vec::new(), indices: IndexMap::default(), of: IndexMap::default() };
    table.record(vec![cvinfo::NIL])?;
    let mut symbols = Symbols::default();
    let mut head = vec![0, 0];
    head.extend(module.length.to_le_bytes());
    if let Some(name) = &module.name {
        head.extend(pascal(name)?);
    }
    symbols.record(Kind::Block, &head, Some((false, &module.start, 0)))?;
    for procedure in &module.procedures {
        let mut data = vec![0, 0];
        data.extend(table.index(procedure.r#type)?.to_le_bytes());
        for field in [procedure.length, procedure.debug_start, procedure.debug_end, 0] {
            data.extend(field.to_le_bytes());
        }
        data.push(if procedure.far { 0x04 } else { 0x00 });
        data.extend(pascal(&procedure.name)?);
        symbols.record(Kind::Proc, &data, Some((false, &procedure.symbol, 0)))?;
        for local in &procedure.locals {
            let mut data = local.bp.to_le_bytes().to_vec();
            data.extend(table.index(local.r#type)?.to_le_bytes());
            data.extend(pascal(&local.name)?);
            symbols.record(Kind::BpRel, &data, None)?;
        }
        for one in &procedure.statics {
            symbols.data(&mut table, one)?;
        }
        symbols.record(Kind::End, &[], None)?;
    }
    for one in &module.data {
        symbols.data(&mut table, one)?;
    }
    symbols.record(Kind::End, &[], None)?;
    Ok(Written { symbols: symbols.bytes, types: table.bytes, relocations: symbols.relocations })
}

#[cfg(test)]
#[path = "cvwrite_tests.rs"]
mod cvwrite_tests;
