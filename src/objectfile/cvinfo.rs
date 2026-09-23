//! Port of `qbopt/objectfile/cvinfo.py`: BC's own /Zi debug symbols, read
//! straight out of the .OBJ, no LINK or CVPACK.
//!
//! $$SYMBOLS holds one variable-length record per name and $$TYPES the type
//! table they index into, in BC's pre-link layout rather than CV4's. The
//! Python module docstring carries the measurements behind every tag.

use std::rc::Rc;
use std::sync::LazyLock;

use crate::frontends::bc::extent;
use crate::objectfile::module;
use crate::objectfile::omf::{self, Record};
use crate::support::hash::IndexMap;
use crate::support::pyrepr::{self, Repr};

/// $$SYMBOLS record kinds -- see docs/codeview.md's own table for each one's data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Block = 0x00,
    Proc = 0x01,
    End = 0x02,
    BpRel = 0x04,
    LData = 0x05,
    Label = 0x0B,
}

impl Kind {
    /// `Kind(value)` where it is a member.
    pub fn of(value: u8) -> Option<Kind> {
        [
            Kind::Block,
            Kind::Proc,
            Kind::End,
            Kind::BpRel,
            Kind::LData,
            Kind::Label,
        ]
        .into_iter()
        .find(|kind| *kind as u8 == value)
    }
}

/// type_index -> BASIC scalar type. STRING has two: which one a compiler
/// picks looks tied to its near/far string memory model.
pub static PRIMITIVES: LazyLock<IndexMap<i64, &'static str>> = LazyLock::new(|| {
    [
        (0x81, "INTEGER"),
        (0x82, "LONG"),
        (0x88, "SINGLE"),
        (0x89, "DOUBLE"),
        (0x97, "STRING"),
        (0x9C, "STRING"),
    ]
    .into_iter()
    .collect()
});

/// FUNCTION's return type, read off its own name -- BASIC's own convention,
/// not something reconstructed from $$TYPES.
pub static SIGILS: LazyLock<IndexMap<char, &'static str>> = LazyLock::new(|| {
    [
        ('%', "INTEGER"),
        ('&', "LONG"),
        ('!', "SINGLE"),
        ('#', "DOUBLE"),
        ('$', "STRING"),
    ]
    .into_iter()
    .collect()
});

/// QB 4.5's own BYREF-parameter codes. Not a $$TYPES index at all; it never
/// leaves the PRIMITIVES-sized number space.
pub static QB45_BYREF_PRIMITIVES: LazyLock<IndexMap<i64, &'static str>> = LazyLock::new(|| {
    [
        (0xA1, "INTEGER"),
        (0xA2, "LONG"),
        (0xB7, "STRING"),
        (0xA8, "SINGLE"),
        (0xA9, "DOUBLE"),
    ]
    .into_iter()
    .collect()
});

pub const BASE_TYPE_INDEX: i64 = 0x0200;

/// $$TYPES data tags: the first byte (or two) of a record's own data, BC's
/// private numbering, unrelated to CVPACK's CV4 leaf ids of the same value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tag {
    /// element type only -- see the module docstring on bounds
    Array = 0x8C,
    /// always followed by a second, constant 0x74 byte
    Pointer = 0x7A,
    /// wraps another record's index, always a POINTER one
    ByRef = 0x76,
    /// a flat list -- of type-refs, or of named offsets
    List = 0x7F,
    /// "a type_index follows", u16
    TypeRef = 0x83,
    /// "a length-prefixed name follows"
    Name = 0x82,
    /// "a u16 numeric follows" -- a member offset, or a count
    Offset = 0x85,
    /// always followed by a second, constant 0x86 byte
    Struct = 0x79,
    /// VBDOS/PDS: always followed by a second, constant 0x00 byte
    FixedString = 0x8D,
    /// QB 4.5's own encoding of the same field: 0x86, a size_bits:u32 8x the
    /// declared length, then a fixed 0x83 0x80 0x00 tail that isn't decoded.
    FixedStringQb45 = 0x78,
    /// a procedure's own return type + arglist -- always followed by 0x80
    Signature = 0x75,
}

impl Tag {
    const ALL: [Tag; 11] = [
        Tag::Array,
        Tag::Pointer,
        Tag::ByRef,
        Tag::List,
        Tag::TypeRef,
        Tag::Name,
        Tag::Offset,
        Tag::Struct,
        Tag::FixedString,
        Tag::FixedStringQb45,
        Tag::Signature,
    ];

    /// `Tag(value)` where it is a member.
    pub fn of(value: u8) -> Option<Tag> {
        Tag::ALL.into_iter().find(|tag| *tag as u8 == value)
    }

    /// The member name.
    pub fn name(self) -> &'static str {
        match self {
            Tag::Array => "ARRAY",
            Tag::Pointer => "POINTER",
            Tag::ByRef => "BYREF",
            Tag::List => "LIST",
            Tag::TypeRef => "TYPEREF",
            Tag::Name => "NAME",
            Tag::Offset => "OFFSET",
            Tag::Struct => "STRUCT",
            Tag::FixedString => "FIXED_STRING",
            Tag::FixedStringQb45 => "FIXED_STRING_QB45",
            Tag::Signature => "SIGNATURE",
        }
    }
}

impl Repr for Tag {
    fn repr(&self) -> String {
        format!("<Tag.{}: {}>", self.name(), *self as u8)
    }
}

/// `Unresolved.tag`: a plain int, or a `Tag` member. Equal by value, as an
/// `IntEnum` is, but `repr` tells them apart.
#[derive(Clone, Copy, Debug)]
pub enum TagValue {
    Int(i64),
    Tag(Tag),
}

impl TagValue {
    pub fn value(self) -> i64 {
        match self {
            TagValue::Int(value) => value,
            TagValue::Tag(tag) => tag as i64,
        }
    }
}

impl PartialEq for TagValue {
    fn eq(&self, other: &Self) -> bool {
        self.value() == other.value()
    }
}

impl Eq for TagValue {}

impl Repr for TagValue {
    fn repr(&self) -> String {
        match self {
            TagValue::Int(value) => value.repr(),
            TagValue::Tag(tag) => tag.repr(),
        }
    }
}

/// One structure member: BC lists field types and field names/offsets as two
/// separate, parallel records, zipped back together positionally.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    pub name: String,
    pub offset: i64,
    pub type_index: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Struct {
    pub name: String,
    pub size_bits: i64,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Array {
    pub element: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pointer {
    pub target: i64,
}

/// A BYREF parameter's own wrapper. Every one measured points at a Pointer
/// record in turn, and that second hop is resolved away.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByRef {
    pub target: i64,
}

/// A `STRING * n` field inside a TYPE, with its own record naming the length.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixedString {
    pub length: i64,
}

/// A procedure's own return type and argument list, `params` already
/// resolved to the argument types themselves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signature {
    pub return_type: i64,
    pub params: Vec<i64>,
}

/// A flat list of type indices -- a structure's field types, or a
/// procedure's own argument list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeList {
    pub indices: Vec<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedOffset {
    pub name: String,
    pub offset: i64,
}

/// A structure's field names and offsets, positionally parallel to the
/// TypeList naming the same fields' types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedOffsetList {
    pub entries: Vec<NamedOffset>,
}

/// A record whose tag byte, or whose data's own shape, wasn't confirmed
/// against a compiled probe -- reported rather than guessed at.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unresolved {
    pub tag: TagValue,
    pub raw: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeEntry {
    Array(Array),
    Struct(Struct),
    Pointer(Pointer),
    ByRef(ByRef),
    TypeList(TypeList),
    NamedOffsetList(NamedOffsetList),
    FixedString(FixedString),
    Signature(Signature),
    Unresolved(Unresolved),
}

impl TypeEntry {
    /// `Unresolved(tag, data)`.
    fn unresolved(tag: TagValue, data: &[u8]) -> TypeEntry {
        TypeEntry::Unresolved(Unresolved {
            tag,
            raw: data.to_vec(),
        })
    }
}

/// `dict[int, TypeEntry]`, shared by every Local, Variable and Procedure.
pub type Types = IndexMap<i64, TypeEntry>;

impl Repr for Field {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Field",
            &[
                ("name", self.name.repr()),
                ("offset", self.offset.repr()),
                ("type_index", self.type_index.repr()),
            ],
        )
    }
}

impl Repr for NamedOffset {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "NamedOffset",
            &[("name", self.name.repr()), ("offset", self.offset.repr())],
        )
    }
}

impl Repr for Signature {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Signature",
            &[
                ("return_type", self.return_type.repr()),
                ("params", pyrepr::tuple(&self.params)),
            ],
        )
    }
}

impl Repr for TypeEntry {
    fn repr(&self) -> String {
        match self {
            TypeEntry::Array(one) => pyrepr::dataclass("Array", &[("element", one.element.repr())]),
            TypeEntry::Struct(one) => pyrepr::dataclass(
                "Struct",
                &[
                    ("name", one.name.repr()),
                    ("size_bits", one.size_bits.repr()),
                    ("fields", pyrepr::tuple(&one.fields)),
                ],
            ),
            TypeEntry::Pointer(one) => {
                pyrepr::dataclass("Pointer", &[("target", one.target.repr())])
            }
            TypeEntry::ByRef(one) => pyrepr::dataclass("ByRef", &[("target", one.target.repr())]),
            TypeEntry::TypeList(one) => {
                pyrepr::dataclass("TypeList", &[("indices", pyrepr::tuple(&one.indices))])
            }
            TypeEntry::NamedOffsetList(one) => pyrepr::dataclass(
                "NamedOffsetList",
                &[("entries", pyrepr::tuple(&one.entries))],
            ),
            TypeEntry::FixedString(one) => {
                pyrepr::dataclass("FixedString", &[("length", one.length.repr())])
            }
            TypeEntry::Signature(one) => one.repr(),
            TypeEntry::Unresolved(one) => pyrepr::dataclass(
                "Unresolved",
                &[("tag", one.tag.repr()), ("raw", pyrepr::bytes(&one.raw))],
            ),
        }
    }
}

/// `b[lo:hi]`.
fn slice(b: &[u8], lo: usize, hi: usize) -> &[u8] {
    let hi = hi.min(b.len());
    if lo >= hi { &[] } else { &b[lo..hi] }
}

/// `int.from_bytes(b, "little")`.
fn from_le(b: &[u8]) -> i64 {
    b.iter()
        .rev()
        .fold(0i64, |acc, &byte| (acc << 8) | byte as i64)
}

/// `int.from_bytes(b, "little", signed=True)`.
fn from_le_signed(b: &[u8]) -> i64 {
    let value = from_le(b);
    let bits = 8 * b.len() as u32;
    if bits > 0 && value >> (bits - 1) & 1 == 1 {
        value - (1i64 << bits)
    } else {
        value
    }
}

/// `bytes.decode("latin1")`.
fn decode_latin1(b: &[u8]) -> String {
    b.iter().map(|&byte| byte as char).collect()
}

/// `a or b` on a str: the empty string is falsy.
fn truthy(s: Option<String>) -> Option<String> {
    s.filter(|s| !s.is_empty())
}

/// The $$TYPES segment's assembled bytes, or b"" if BC wasn't asked for /Zi.
pub fn types(records: &[Rc<Record>]) -> Vec<u8> {
    for (index, segment) in omf::segments(records).into_iter().enumerate() {
        if let Some((name, length)) = segment {
            if name == "$$TYPES" {
                return omf::segment_image(records, index as i64, length);
            }
        }
    }
    Vec::new()
}

/// (type_index, kind, data) for each record, indices from BASE_TYPE_INDEX.
///
/// No trailing pad here the way $$SYMBOLS has one -- measured across every
/// probe, the segment is consumed exactly to its declared length.
fn _type_records(buf: &[u8]) -> Vec<(i64, u8, &[u8])> {
    let mut out = Vec::new();
    let (mut at, mut index) = (0usize, BASE_TYPE_INDEX);
    while at + 3 <= buf.len() {
        let kind = buf[at];
        let length = from_le(slice(buf, at + 1, at + 3)) as usize;
        out.push((index, kind, slice(buf, at + 3, at + 3 + length)));
        at += 3 + length;
        index += 1;
    }
    out
}

fn _type_ref(data: &[u8], at: usize) -> Option<i64> {
    if at + 3 > data.len() || data[at] != Tag::TypeRef as u8 {
        return None;
    }
    Some(from_le(slice(data, at + 1, at + 3)))
}

fn _type_refs(data: &[u8]) -> Option<Vec<i64>> {
    if data.len() % 3 != 0 {
        return None;
    }
    let mut refs = Vec::new();
    for at in (0..data.len()).step_by(3) {
        refs.push(_type_ref(data, at)?);
    }
    Some(refs)
}

fn _named_offsets(data: &[u8]) -> Option<Vec<NamedOffset>> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < data.len() {
        if data[at] != Tag::Name as u8 || at + 2 > data.len() {
            return None;
        }
        let namelen = data[at + 1] as usize;
        at += 2;
        if at + namelen + 3 > data.len() || data[at + namelen] != Tag::Offset as u8 {
            return None;
        }
        let name = decode_latin1(slice(data, at, at + namelen));
        let offset = from_le(slice(data, at + namelen + 1, at + namelen + 3));
        out.push(NamedOffset { name, offset });
        at += namelen + 3;
    }
    Some(out)
}

fn _parse_struct(data: &[u8], table: &Types) -> TypeEntry {
    let refused = || TypeEntry::unresolved(TagValue::Tag(Tag::Struct), data);
    if data.len() < 17
        || data[1] != 0x86
        || data[6] != Tag::Offset as u8
        || data[15] != Tag::Name as u8
    {
        return refused();
    }
    let field_types_index = _type_ref(data, 9);
    let field_names_index = _type_ref(data, 12);
    let (Some(field_types_index), Some(field_names_index)) = (field_types_index, field_names_index)
    else {
        return refused();
    };
    let size_bits = from_le(slice(data, 2, 6));
    let count = from_le(slice(data, 7, 9));
    let field_types = table.get(&field_types_index);
    let field_names = table.get(&field_names_index);
    let namelen = data[16] as usize;
    let valid_lists = matches!(
        (field_types, field_names),
        (
            Some(TypeEntry::TypeList(_)),
            Some(TypeEntry::NamedOffsetList(_))
        )
    );
    if data.len() < 17 + namelen || !valid_lists {
        return refused();
    }
    let (Some(TypeEntry::TypeList(field_types)), Some(TypeEntry::NamedOffsetList(field_names))) =
        (field_types, field_names)
    else {
        unreachable!("valid_lists");
    };
    let name = decode_latin1(slice(data, 17, 17 + namelen));
    if field_types.indices.len() as i64 != count || field_names.entries.len() as i64 != count {
        return refused();
    }
    let fields = field_types
        .indices
        .iter()
        .zip(&field_names.entries)
        .map(|(&ti, no)| Field {
            name: no.name.clone(),
            offset: no.offset,
            type_index: ti,
        })
        .collect();
    TypeEntry::Struct(Struct {
        name,
        size_bits,
        fields,
    })
}

fn _parse_signature(data: &[u8], table: &Types) -> TypeEntry {
    let refused = || TypeEntry::unresolved(TagValue::Tag(Tag::Signature), data);
    if data.len() != 10 || data[1] != 0x80 || data[5] != 0x73 {
        return refused();
    }
    let return_type = _type_ref(data, 2);
    let nparms = data[6] as usize;
    let arglist_index = _type_ref(data, 7);
    let (Some(return_type), Some(arglist_index)) = (return_type, arglist_index) else {
        return refused();
    };
    if nparms == 0 && arglist_index == BASE_TYPE_INDEX {
        // A zero-parameter procedure has no TypeList of its own to point at,
        // so its arglist names the segment's own first (always 1-byte, 0x80)
        // entry instead.
        return TypeEntry::Signature(Signature {
            return_type,
            params: Vec::new(),
        });
    }
    match table.get(&arglist_index) {
        Some(TypeEntry::TypeList(arglist)) if arglist.indices.len() == nparms => {
            TypeEntry::Signature(Signature {
                return_type,
                params: arglist.indices.clone(),
            })
        }
        _ => refused(),
    }
}

fn _parse_type_entry(kind: u8, data: &[u8], table: &Types) -> TypeEntry {
    let tag = data.first().copied();
    let Some(tag) = tag.filter(|_| kind == 0x01) else {
        return TypeEntry::unresolved(TagValue::Int(kind as i64), data);
    };
    let refused = || TypeEntry::unresolved(TagValue::Int(tag as i64), data);
    match Tag::of(tag) {
        Some(Tag::Array) => match _type_ref(data, 1) {
            Some(element) => TypeEntry::Array(Array { element }),
            None => refused(),
        },
        Some(Tag::Pointer) if data.len() >= 2 && data[1] == 0x74 => match _type_ref(data, 2) {
            Some(target) => TypeEntry::Pointer(Pointer { target }),
            None => refused(),
        },
        Some(Tag::ByRef) => match _type_ref(data, 1) {
            Some(target) => TypeEntry::ByRef(ByRef { target }),
            None => refused(),
        },
        Some(Tag::List) => {
            let rest = &data[1..];
            if !rest.is_empty() && rest[0] == Tag::Name as u8 {
                return match _named_offsets(rest) {
                    Some(entries) => TypeEntry::NamedOffsetList(NamedOffsetList { entries }),
                    None => refused(),
                };
            }
            match _type_refs(rest) {
                Some(indices) => TypeEntry::TypeList(TypeList { indices }),
                None => refused(),
            }
        }
        Some(Tag::Struct) => _parse_struct(data, table),
        Some(Tag::FixedString)
            if data.len() >= 5 && data[1] == 0x00 && data[2] == Tag::Offset as u8 =>
        {
            TypeEntry::FixedString(FixedString {
                length: from_le(slice(data, 3, 5)),
            })
        }
        Some(Tag::FixedStringQb45)
            if data.len() == 9 && data[1] == 0x86 && data[6..9] == [0x83, 0x80, 0x00] =>
        {
            let size_bits = from_le(slice(data, 2, 6));
            if size_bits % 8 == 0 {
                TypeEntry::FixedString(FixedString {
                    length: size_bits / 8,
                })
            } else {
                refused()
            }
        }
        Some(Tag::Signature) => _parse_signature(data, table),
        _ => refused(),
    }
}

/// $$TYPES, decoded to one TypeEntry per module-local index.
///
/// A single forward pass suffices: a structure's own record always comes
/// after the two list records it refers to, in every probe measured.
pub fn type_table(records: &[Rc<Record>]) -> Types {
    let mut table = Types::default();
    let buf = types(records);
    for (index, kind, data) in _type_records(&buf) {
        let entry = _parse_type_entry(kind, data, &table);
        table.insert(index, entry);
    }
    table
}

pub fn type_name(type_index: i64, types: Option<&Types>) -> Option<String> {
    if let Some(name) = PRIMITIVES.get(&type_index) {
        return Some((*name).to_owned());
    }
    if let Some(name) = QB45_BYREF_PRIMITIVES.get(&type_index) {
        return Some(format!("BYREF {name}"));
    }
    let types = types.filter(|types| !types.is_empty())?;
    let named = |index: i64| {
        truthy(type_name(index, Some(types))).unwrap_or_else(|| format!("type {index:#06x}"))
    };
    match types.get(&type_index) {
        Some(TypeEntry::Array(Array { element })) => Some(format!("ARRAY OF {}", named(*element))),
        Some(TypeEntry::Struct(Struct { name, .. })) => Some(format!("TYPE {name}")),
        Some(TypeEntry::ByRef(ByRef { target })) => {
            let pointee = match types.get(target) {
                Some(TypeEntry::Pointer(pointer)) => pointer.target,
                _ => *target,
            };
            Some(format!("BYREF {}", named(pointee)))
        }
        // QB 4.5's own array-parameter shape: a bare pointer, no Tag.BYREF
        // hop. Still BYREF in BASIC's own terms.
        Some(TypeEntry::Pointer(Pointer { target })) => Some(format!("BYREF {}", named(*target))),
        Some(TypeEntry::FixedString(FixedString { length })) => Some(format!("STRING * {length}")),
        _ => None,
    }
}

/// A procedure's own parameter or local: BP-relative, sign says which.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Local {
    pub name: String,
    pub bp_offset: i64,
    pub type_index: i64,
    // the module's own $$TYPES, shared by reference with every other Local
    // and Variable parse() builds -- not recomputed per instance.
    pub types: Rc<Types>,
}

impl Local {
    pub fn is_param(&self) -> bool {
        self.bp_offset > 0
    }

    pub fn type_name(&self) -> Option<String> {
        type_name(self.type_index, Some(&self.types))
    }
}

impl Repr for Local {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Local",
            &[
                ("name", self.name.repr()),
                ("bp_offset", self.bp_offset.repr()),
                ("type_index", self.type_index.repr()),
                ("types", self.types.as_ref().repr()),
            ],
        )
    }
}

/// A module-level DIM: an absolute offset into a data segment.
///
/// For an array that offset is the *descriptor*, in BC_CN, while the elements
/// live in BC_DATA. `data` is where the elements are, read out of the
/// descriptor's own relocation, and `stride`/`count` are the element size
/// and how many, which the descriptor writes down at +12 and +14.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variable {
    pub name: String,
    pub offset: i64,
    pub segment: i64,
    pub type_index: i64,
    pub types: Rc<Types>,
    pub data: Option<(i64, i64)>,
    pub stride: i64,
    pub count: i64,
}

impl Variable {
    /// How many bytes the variable occupies where that is known.
    pub fn size(&self) -> i64 {
        self.stride * self.count
    }

    pub fn type_name(&self) -> Option<String> {
        type_name(self.type_index, Some(&self.types))
    }
}

impl Repr for Variable {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Variable",
            &[
                ("name", self.name.repr()),
                ("offset", self.offset.repr()),
                ("segment", self.segment.repr()),
                ("type_index", self.type_index.repr()),
                ("types", self.types.as_ref().repr()),
                ("data", self.data.repr()),
                ("stride", self.stride.repr()),
                ("count", self.count.repr()),
            ],
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Label {
    pub name: String,
    pub offset: i64,
}

impl Repr for Label {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Label",
            &[("name", self.name.repr()), ("offset", self.offset.repr())],
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Procedure {
    pub name: String,
    pub offset: i64,
    pub proc_length: i64,
    pub debug_start: i64,
    pub debug_end: i64,
    pub flags: i64,
    // a $$TYPES index naming this procedure's own Tag.SIGNATURE record --
    // see Procedure.signature and the module docstring.
    pub proc_type_index: i64,
    pub locals: Vec<Local>,
    pub types: Rc<Types>,
}

impl Procedure {
    pub fn params(&self) -> Vec<&Local> {
        self.locals.iter().filter(|loc| loc.is_param()).collect()
    }

    pub fn own_locals(&self) -> Vec<&Local> {
        self.locals.iter().filter(|loc| !loc.is_param()).collect()
    }

    /// A FUNCTION's return type from its own name's sigil; None for a SUB.
    pub fn return_type(&self) -> Option<&'static str> {
        self.name
            .chars()
            .last()
            .and_then(|sigil| SIGILS.get(&sigil).copied())
    }

    /// The procedure's own Tag.SIGNATURE record, if proc_type_index resolves
    /// to one. A SUB's signature carries the same placeholder an INTEGER
    /// FUNCTION's would, so this is not folded into return_type.
    pub fn signature(&self) -> Option<&Signature> {
        match self.types.get(&self.proc_type_index) {
            Some(TypeEntry::Signature(entry)) => Some(entry),
            _ => None,
        }
    }
}

impl Repr for Procedure {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Procedure",
            &[
                ("name", self.name.repr()),
                ("offset", self.offset.repr()),
                ("proc_length", self.proc_length.repr()),
                ("debug_start", self.debug_start.repr()),
                ("debug_end", self.debug_end.repr()),
                ("flags", self.flags.repr()),
                ("proc_type_index", self.proc_type_index.repr()),
                ("locals", self.locals.repr()),
                ("types", self.types.as_ref().repr()),
            ],
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugInfo {
    pub module: Option<String>,
    pub procedures: Vec<Procedure>,
    pub variables: Vec<Variable>,
    pub labels: Vec<Label>,
    pub types: Rc<Types>,
    pub code_length: i64,
}

impl Repr for DebugInfo {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "DebugInfo",
            &[
                ("module", self.module.repr()),
                ("procedures", self.procedures.repr()),
                ("variables", self.variables.repr()),
                ("labels", self.labels.repr()),
                ("types", self.types.as_ref().repr()),
                ("code_length", self.code_length.repr()),
            ],
        )
    }
}

fn _pstr(buf: &[u8], at: usize) -> (String, usize) {
    let n = buf[at] as usize;
    (decode_latin1(slice(buf, at + 1, at + 1 + n)), at + 1 + n)
}

/// (kind, data, where) for each record.
///
/// `data` excludes the length and kind bytes; `where` is that data's own
/// offset in the segment, which is what a fixup against $$SYMBOLS names.
fn _records(buf: &[u8]) -> Vec<(u8, &[u8], usize)> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < buf.len() {
        let length = buf[at] as usize;
        if length == 0 {
            // trailing pad
            at += 1;
            continue;
        }
        out.push((buf[at + 1], slice(buf, at + 2, at + 1 + length), at + 2));
        at += 1 + length;
    }
    out
}

/// Where each relocated field in $$SYMBOLS points, by its own offset.
///
/// BC writes a module variable's offset as zero and leaves a fixup to fill
/// it in; read without them every DIM comes back at address zero.
fn _relocated(records: &[Rc<Record>]) -> IndexMap<i64, (i64, i64)> {
    let index = omf::segments(records)
        .into_iter()
        .position(|segment| segment.is_some_and(|(name, _)| name == "$$SYMBOLS"));
    let Some(index) = index else {
        return IndexMap::default();
    };
    // A module variable's offset and segment are one far pointer, so the
    // fixup is ptr16:16 and its own offset is the record's data offset.
    let mut out = IndexMap::default();
    for one in omf::fixups(records) {
        if one.seg == Some(index as i64) {
            out.insert(one.offset, (one.index, one.disp));
        }
    }
    out
}

/// The $$SYMBOLS segment's assembled bytes, or b"" if BC wasn't asked for /Zi.
pub fn symbols(records: &[Rc<Record>]) -> Vec<u8> {
    for (index, segment) in omf::segments(records).into_iter().enumerate() {
        if let Some((name, length)) = segment {
            if name == "$$SYMBOLS" {
                return omf::segment_image(records, index as i64, length);
            }
        }
    }
    Vec::new()
}

/// THEADR's own name, rather than the $$SYMBOLS module-open record, which
/// QB 4.5 emits without a name.
pub fn module_name(records: &[Rc<Record>]) -> Option<String> {
    for r in records {
        if r.r#type == omf::THEADR {
            let n = r.body[0] as usize;
            return Some(decode_latin1(slice(&r.body, 1, 1 + n)));
        }
    }
    None
}

/// Each array told where its own elements are.
///
/// A descriptor's first four bytes are a far pointer to the data and BC
/// leaves them zero with a ptr16:16 fixup, as it does for the symbol
/// record's own address.
fn _elements(records: &[Rc<Record>], variables: Vec<Variable>) -> Vec<Variable> {
    let mut images: IndexMap<i64, Vec<u8>> = IndexMap::default();
    for (index, segment) in omf::segments(records).into_iter().enumerate() {
        if let Some((_, length)) = segment {
            images.insert(
                index as i64,
                omf::segment_image(records, index as i64, length),
            );
        }
    }
    let mut r#where: IndexMap<(Option<i64>, i64), (i64, i64)> = IndexMap::default();
    for one in omf::fixups(records) {
        r#where.insert((one.seg, one.offset), (one.index, one.disp));
    }

    let mut out = Vec::new();
    for one in variables {
        let found = r#where.get(&(Some(one.segment), one.offset)).copied();
        let image = images.get(&one.segment).map_or(&[][..], Vec::as_slice);
        let Some(found) = found.filter(|_| image.len() as i64 >= one.offset + 16) else {
            out.push(one);
            continue;
        };
        let at = one.offset as usize;
        let stride = from_le(slice(image, at + 12, at + 14));
        let count = from_le(slice(image, at + 14, at + 16));
        out.push(Variable {
            data: Some(found),
            stride,
            count,
            ..one
        });
    }
    out
}

pub fn parse(records: &[Rc<Record>]) -> DebugInfo {
    let buf = symbols(records);
    let module = if buf.is_empty() {
        None
    } else {
        module_name(records)
    };
    let types = Rc::new(type_table(records));
    let mut procedures: Vec<Procedure> = Vec::new();
    let mut variables: Vec<Variable> = Vec::new();
    let mut labels: Vec<Label> = Vec::new();
    // the index in `procedures` of the one BPREL records belong to
    let mut current: Option<usize> = None;

    let r#where = _relocated(records);
    for (kind, data, at) in _records(&buf) {
        match Kind::of(kind) {
            Some(Kind::Block) => {} // module-open record: shape (and presence of a name) varies by compiler
            Some(Kind::Proc) => {
                // data[10:12] is still unaccounted for -- always 0x0000 across
                // every procedure measured.
                let off = from_le(slice(data, 0, 2));
                let proc_type_index = from_le(slice(data, 2, 4));
                let proc_length = from_le(slice(data, 4, 6));
                let debug_start = from_le(slice(data, 6, 8));
                let debug_end = from_le(slice(data, 8, 10));
                let flags = data[12] as i64;
                let (name, _) = _pstr(data, 13);
                procedures.push(Procedure {
                    name,
                    offset: off,
                    proc_length,
                    debug_start,
                    debug_end,
                    flags,
                    proc_type_index,
                    locals: Vec::new(),
                    types: Rc::clone(&types),
                });
                current = Some(procedures.len() - 1);
            }
            Some(Kind::End) => current = None,
            Some(Kind::BpRel) if current.is_some() => {
                let bp_offset = from_le_signed(slice(data, 0, 2));
                let type_index = from_le(slice(data, 2, 4));
                let (name, _) = _pstr(data, 4);
                procedures[current.unwrap()].locals.push(Local {
                    name,
                    bp_offset,
                    type_index,
                    types: Rc::clone(&types),
                });
            }
            Some(Kind::LData) => {
                let (off, seg, type_index) = (
                    from_le(slice(data, 0, 2)),
                    from_le(slice(data, 2, 4)),
                    from_le(slice(data, 4, 6)),
                );
                let (name, _) = _pstr(data, 6);
                // The record's own two bytes are zero; the fixup on them is
                // the address. Keep whatever is written where there is none.
                let (seg, off) = r#where.get(&(at as i64)).copied().unwrap_or((seg, off));
                variables.push(Variable {
                    name,
                    offset: off,
                    segment: seg,
                    type_index,
                    types: Rc::clone(&types),
                    data: None,
                    stride: 0,
                    count: 0,
                });
            }
            Some(Kind::Label) => {
                let off = from_le(slice(data, 0, 2));
                let (name, _) = _pstr(data, 3);
                labels.push(Label { name, offset: off });
            }
            _ => {}
        }
    }

    let variables = _elements(records, variables);
    let code_segment = omf::code_segment(records);
    let code_length = code_segment.map_or(0, |(_, _, length)| length);
    DebugInfo {
        module,
        procedures,
        variables,
        labels,
        types,
        code_length,
    }
}

fn _fmt_type(type_index: i64, resolved: Option<String>) -> String {
    truthy(resolved).unwrap_or_else(|| format!("custom (type {type_index:#06x}, unresolved)"))
}

/// A struct's own field list, name/type/offset, one line per field.
fn _fmt_fields(type_index: i64, types: &Types, indent: &str) -> Vec<String> {
    let Some(TypeEntry::Struct(entry)) = types.get(&type_index) else {
        return Vec::new();
    };
    entry
        .fields
        .iter()
        .map(|f| {
            let resolved = _fmt_type(f.type_index, type_name(f.type_index, Some(types)));
            format!("{indent}.{:<12} +{:<3} {resolved}", f.name, f.offset)
        })
        .collect()
}

fn _fmt_body(body: &extent::Body) -> String {
    let ranges: Vec<String> = body.ranges.iter().map(|(lo, hi)| format!("{lo:#06x}-{hi:#06x}")).collect();
    let label = body.name.clone().filter(|name| !name.is_empty()).unwrap_or_else(|| body.kind.value().to_owned());
    format!("  {:9} {label:<12} {}  ({} bytes)", body.kind.value(), ranges.join(", "), body.length())
}

pub fn main(path: impl AsRef<std::path::Path>) -> Result<(), omf::ReadError> {
    let path = path.as_ref();
    let records = omf::read(path)?;
    println!("{}", path.display());

    // No CodeView needed for this part -- extent.py's own reachability answers
    // it, and does so for every real object in the corpus, /Zi or not.
    match module::of(&records) {
        None => println!("  no code segment"),
        Some(found) => match extent::partition(&found) {
            Err(refused) => println!("  body partition refused: {refused}"),
            Ok(found_partition) => {
                for body in &found_partition.bodies {
                    println!("{}", _fmt_body(body));
                }
            }
        },
    }

    let info = parse(&records);
    let Some(name) = &info.module else {
        println!("  no /Zi debug info ($$SYMBOLS is empty)");
        return Ok(());
    };
    println!("  module: {name}");
    for proc in &info.procedures {
        let params: Vec<String> = proc
            .params()
            .iter()
            .map(|p| format!("{}:{}", p.name, _fmt_type(p.type_index, p.type_name())))
            .collect();
        let ret = proc.return_type().map_or(String::new(), |one| format!(" returns {one}"));
        println!(
            "  sub/function {}{ret}  off={:#06x} len={} flags={:#x}",
            proc.name, proc.offset, proc.proc_length, proc.flags
        );
        let params = params.join(", ");
        println!("    params: {}", if params.is_empty() { "(none)" } else { &params });
        for p in proc.params() {
            for line in _fmt_fields(p.type_index, &info.types, "      ") {
                println!("{line}");
            }
        }
        for loc in proc.own_locals() {
            println!(
                "    local  {:<12} bp={:+5}  {}",
                loc.name,
                loc.bp_offset,
                _fmt_type(loc.type_index, loc.type_name())
            );
            for line in _fmt_fields(loc.type_index, &info.types, "      ") {
                println!("{line}");
            }
        }
    }
    for v in &info.variables {
        println!(
            "  var   {:<12} seg={:3} off={:#06x}  {}",
            v.name,
            v.segment,
            v.offset,
            _fmt_type(v.type_index, v.type_name())
        );
        for line in _fmt_fields(v.type_index, &info.types, "        ") {
            println!("{line}");
        }
    }
    for lbl in &info.labels {
        println!("  label {:<12} off={:#06x}", lbl.name, lbl.offset);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use super::*;

    fn fixture(name: &str) -> DebugInfo {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/omf")
            .join(name);
        parse(&omf::read(path).unwrap())
    }

    /// Every one of them came back at address zero: BC leaves a module
    /// variable's address to a ptr16:16 fixup, and without it every DIM is at
    /// zero and no name says which cell it is.
    #[test]
    fn test_a_module_variable_is_where_its_fixup_says() {
        let got = fixture("procs-p-g2-zi.obj");
        assert!(
            !got.variables.is_empty(),
            "the object carries no symbols; it was not built with /Zi"
        );
        let r#where: BTreeSet<(i64, i64)> = got
            .variables
            .iter()
            .map(|one| (one.segment, one.offset))
            .collect();
        assert!(
            !r#where.contains(&(0, 0)),
            "a variable at address zero is an unrelocated field"
        );
        assert_eq!(
            r#where.len(),
            got.variables.len(),
            "two variables cannot share one address"
        );
        let names: BTreeSet<&str> = got.variables.iter().map(|one| one.name.as_str()).collect();
        assert_eq!(names, BTreeSet::from(["A&", "B&", "R&"]));
    }

    /// `Twice&(n AS LONG)` has one parameter at bp+6 and one local at bp-22:
    /// the sign of the offset says which, with no prologue to infer from.
    #[test]
    fn test_a_parameter_is_above_the_frame_pointer_and_a_local_below() {
        let got = fixture("procs-p-g2-zi.obj");
        let named: IndexMap<&str, &Procedure> = got
            .procedures
            .iter()
            .map(|one| (one.name.as_str(), one))
            .collect();
        assert!(
            named.contains_key("TWICE&") && named.contains_key("REPORT"),
            "only {:?}",
            named.keys()
        );

        let twice = named["TWICE&"];
        let params: Vec<(&str, i64)> = twice
            .locals
            .iter()
            .filter(|one| one.bp_offset > 0)
            .map(|one| (one.name.as_str(), one.bp_offset))
            .collect();
        let locals: Vec<(&str, i64)> = twice
            .locals
            .iter()
            .filter(|one| one.bp_offset < 0)
            .map(|one| (one.name.as_str(), one.bp_offset))
            .collect();
        assert_eq!(params, [("N&", 6)]);
        assert_eq!(locals, [("T&", -22)]);
        // Report takes two and declares no local of its own
        let report: BTreeSet<&str> = named["REPORT"]
            .locals
            .iter()
            .filter(|one| one.bp_offset > 0)
            .map(|one| one.name.as_str())
            .collect();
        assert_eq!(report, BTreeSet::from(["N&", "TAG$"]));
    }

    /// BC names an array in BC_CN and the code only ever uses BC_DATA: the
    /// descriptor's first four bytes are a far pointer to the elements, left
    /// zero with a ptr16:16 fixup, and without following it no array access
    /// can be named.
    #[test]
    fn test_an_array_is_where_its_descriptor_points() {
        let got = fixture("arridx-p-g2-zi.obj");
        let is_array = |one: &Variable| one.type_name().unwrap_or_default().contains("ARRAY");
        let arrays: Vec<&Variable> = got.variables.iter().filter(|one| is_array(one)).collect();
        assert!(
            !arrays.is_empty(),
            "the object declares no array, so this proves nothing"
        );
        for one in &arrays {
            assert!(
                one.data.is_some(),
                "{}: the descriptor was not followed",
                one.name
            );
            assert_ne!(
                one.data,
                Some((one.segment, one.offset)),
                "{}: named where its descriptor is",
                one.name
            );
            assert!(
                one.stride != 0 && one.count != 0,
                "{}: {} x {}",
                one.name,
                one.count,
                one.stride
            );
        }
        // a% is 21 INTEGERs -- `DIM a%(20)`, which BASIC bases at zero
        let a = arrays.iter().find(|one| one.name == "A%").unwrap();
        assert_eq!((a.stride, a.count), (2, 21));

        // A scalar has no descriptor and is not given one.
        let scalars: Vec<&Variable> = got.variables.iter().filter(|one| !is_array(one)).collect();
        assert!(!scalars.is_empty() && scalars.iter().all(|one| one.data.is_none()));
    }

    /// No fixture refuses a STRUCT or SIGNATURE record, so the `Tag` member's
    /// repr is checked here. Expected text printed by Python.
    #[test]
    fn a_refused_record_keeps_python_repr_and_equality() {
        let cases: [(u8, &[u8], &str); 4] = [
            (
                1,
                b"\x79\x00",
                "Unresolved(tag=<Tag.STRUCT: 121>, raw=b'y\\x00')",
            ),
            (
                1,
                b"\x75\x80",
                "Unresolved(tag=<Tag.SIGNATURE: 117>, raw=b'u\\x80')",
            ),
            (2, b"\x8c", "Unresolved(tag=2, raw=b'\\x8c')"),
            (1, b"\x7a\x00", "Unresolved(tag=122, raw=b'z\\x00')"),
        ];
        for (kind, data, want) in cases {
            let got = _parse_type_entry(kind, data, &Types::default());
            assert_eq!(got.repr(), want);
            let plain = if kind == 1 { data[0] } else { kind };
            assert_eq!(
                got,
                TypeEntry::unresolved(TagValue::Int(plain as i64), data)
            );
        }
    }

    fn objects(dir: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path: PathBuf = entry.unwrap().path();
            if path.is_dir() {
                objects(&path, out);
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("obj"))
            {
                out.push(path.to_str().unwrap().to_owned());
            }
        }
    }

    fn block(path: &str) -> Vec<String> {
        let mut out = vec![format!("== {path}")];
        let records = match omf::read(path) {
            Ok(records) => records,
            Err(e) => return [out, vec![format!("read ValueError: {e}")]].concat(),
        };
        out.push(format!("module_name {}", module_name(&records).repr()));
        out.push(format!("symbols {}", pyrepr::bytes(&symbols(&records))));
        out.push(format!("types {}", pyrepr::bytes(&types(&records))));
        out.push(format!("type_table {}", type_table(&records).repr()));
        let info = parse(&records);
        out.push(format!("parse {}", info.repr()));
        for &index in info.types.keys() {
            let (with, without) = (type_name(index, Some(&info.types)), type_name(index, None));
            out.push(format!(
                "type_name {index} {} {}",
                with.repr(),
                without.repr()
            ));
            out.extend(_fmt_fields(index, &info.types, "  "));
        }
        let names = |locals: Vec<&Local>| {
            locals
                .iter()
                .map(|one| one.name.clone())
                .collect::<Vec<_>>()
                .repr()
        };
        for proc in &info.procedures {
            out.push(format!(
                "proc {} params={} own={} ret={} sig={}",
                proc.name.repr(),
                names(proc.params()),
                names(proc.own_locals()),
                proc.return_type().repr(),
                proc.signature().repr()
            ));
            for loc in &proc.locals {
                let fmt = _fmt_type(loc.type_index, loc.type_name());
                let (name, param, resolved) = (
                    loc.name.repr(),
                    loc.is_param().repr(),
                    loc.type_name().repr(),
                );
                out.push(format!("  local {name} {param} {resolved} {fmt}"));
            }
        }
        for v in &info.variables {
            let fmt = _fmt_type(v.type_index, v.type_name());
            out.push(format!(
                "var {} {} size={} {fmt}",
                v.name.repr(),
                v.type_name().repr(),
                v.size()
            ));
        }
        out
    }

    /// The Rust half of the port's comparison against Python: every OMF
    /// fixture, one block each, written to `$CVINFO_DUMP`.
    #[test]
    #[ignore]
    fn dump() {
        std::env::set_current_dir(env!("CARGO_MANIFEST_DIR")).unwrap();
        let mut paths = Vec::new();
        objects(Path::new("fixtures"), &mut paths);
        paths.sort();
        let text: String = paths
            .iter()
            .map(|path| block(path).join("\n") + "\n")
            .collect();
        std::fs::write(std::env::var("CVINFO_DUMP").unwrap(), text).unwrap();
    }
}
