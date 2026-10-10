//! The DIE tree as bytes: abbreviations found, offsets laid out, references
//! resolved.

use llrm_object::debug::Info;
use llrm_object::{Object, Unsupported};
use llrm_support::hash::HashMap;

use crate::buffer::{Buf, Done, Strings, uleb};

pub const TAG_COMPILE_UNIT: u16 = 0x11;
pub const AT_NAME: u16 = 0x03;
pub const AT_BYTE_SIZE: u16 = 0x0b;
pub const AT_BIT_SIZE: u16 = 0x0d;
pub const AT_STMT_LIST: u16 = 0x10;
pub const AT_LOW_PC: u16 = 0x11;
pub const AT_HIGH_PC: u16 = 0x12;
pub const AT_LANGUAGE: u16 = 0x13;
pub const AT_PRODUCER: u16 = 0x25;
pub const AT_LOCATION: u16 = 0x02;
pub const AT_SEGMENT: u16 = 0x46;
pub const AT_ADDRESS_CLASS: u16 = 0x33;
pub const AT_CONST_VALUE: u16 = 0x1c;
pub const AT_UPPER_BOUND: u16 = 0x2f;
pub const AT_DATA_MEMBER_LOCATION: u16 = 0x38;
pub const AT_DATA_BIT_OFFSET: u16 = 0x6b;
pub const AT_ENCODING: u16 = 0x3e;
pub const AT_EXTERNAL: u16 = 0x3f;
pub const AT_FRAME_BASE: u16 = 0x40;
pub const AT_PROTOTYPED: u16 = 0x27;
pub const AT_TYPE: u16 = 0x49;

/// An attribute's value, which says its form.
#[derive(Clone, Debug)]
pub enum Value {
    Str(String),
    U8(u8),
    U16(u16),
    Udata(u64),
    Sdata(i64),
    Flag,
    /// Another DIE, by its index in the tree.
    Ref(usize),
    /// An address `delta` bytes from `symbol`.
    Addr {
        symbol: usize,
        delta: i64,
    },
    /// `high_pc` as a length.
    Len(u32),
    /// The start of the line program.
    Line,
    Expr(Vec<u8>),
    /// `DW_OP_addr`: an address `delta` bytes from `symbol`.
    ExprAddr {
        symbol: usize,
        delta: i64,
    },
    /// A location list, at this offset of the section of them.
    LocList(u32),
    /// `DW_OP_const2u` of the selector of a section: a 16-bit program's
    /// `DW_AT_segment`.
    ExprSegment {
        section: usize,
    },
}

#[derive(Clone, Debug)]
pub struct Die {
    pub tag: u16,
    pub attrs: Vec<(u16, Value)>,
    pub children: Vec<usize>,
}

impl Die {
    pub fn new(tag: u16) -> Self {
        Self { tag, attrs: Vec::new(), children: Vec::new() }
    }
}

/// The places of the sections the unit refers to, by index in the expanded
/// object.
pub struct Places {
    pub abbrev: usize,
    pub strings: usize,
    pub line: usize,
    pub info: usize,
    pub line_strings: Option<usize>,
    /// `.debug_loclists` (5) or `.debug_loc` (4), where a variable has a list.
    pub locations: Option<usize>,
}

pub struct Out {
    pub version: u16,
    pub address: u8,
    pub places: Places,
    pub strings: Strings,
    pub line_strings: Strings,
}

impl Out {
    pub fn new(
        version: u16,
        address: u8,
        places: Places,
    ) -> Self {
        Self { version, address, places, strings: Strings::default(), line_strings: Strings::default() }
    }
}

const DW_OP_CONST2U: u8 = 0x0A;

/// Before 4 an expression is a block, a section offset is a 4-byte constant
/// and a flag is a byte.
fn form(
    value: &Value,
    version: u16,
) -> u16 {
    match value {
        Value::Str(_) => 0x0e,
        Value::U8(_) => 0x0b,
        Value::U16(_) => 0x05,
        Value::Len(_) => 0x06,
        Value::Udata(_) => 0x0f,
        Value::Sdata(_) => 0x0d,
        Value::Flag => {
            if version < 4 {
                0x0c
            } else {
                0x19
            }
        }
        Value::Ref(_) => 0x13,
        Value::Addr { .. } => 0x01,
        Value::Line | Value::LocList(_) => {
            if version < 4 {
                0x06
            } else {
                0x17
            }
        }
        Value::Expr(_) | Value::ExprAddr { .. } | Value::ExprSegment { .. } => {
            if version < 4 {
                0x0a
            } else {
                0x18
            }
        }
    }
}

fn size(
    value: &Value,
    address: usize,
    version: u16,
) -> usize {
    match value {
        Value::Str(_) | Value::Len(_) | Value::Ref(_) | Value::Line | Value::LocList(_) => 4,
        Value::U8(_) => 1,
        Value::U16(_) => 2,
        Value::Udata(one) => uleb(*one).len(),
        Value::Sdata(one) => crate::buffer::sleb(*one).len(),
        Value::Flag => usize::from(version < 4),
        Value::Addr { .. } => address,
        Value::Expr(bytes) => block(bytes.len(), version) + bytes.len(),
        Value::ExprAddr { .. } => block(1 + address, version) + 1 + address,
        Value::ExprSegment { .. } => block(3, version) + 3,
    }
}

/// The length that precedes an expression: a ULEB128 from 4, a byte before.
fn block(
    length: usize,
    version: u16,
) -> usize {
    if version < 4 { 1 } else { uleb(length as u64).len() }
}

type Shape = (u16, bool, Vec<(u16, u16)>);

fn shape(
    die: &Die,
    version: u16,
) -> Shape {
    (die.tag, !die.children.is_empty(), die.attrs.iter().map(|(at, value)| (*at, form(value, version))).collect())
}

/// The unit's `.debug_info` and `.debug_abbrev`.
pub fn unit(
    object: &Object,
    info: &Info,
    out: &mut Out,
) -> Result<(Done, Done, Done), Unsupported> {
    let (dies, locations) = crate::types::tree(object, info, out.version, usize::from(out.address))?;
    let address = usize::from(out.address);
    // Abbreviations in the order DIEs are first met, and each DIE's offset.
    let mut layout = Layout {
        codes: HashMap::default(),
        order: Vec::new(),
        code_of: vec![0; dies.len()],
        offsets: vec![0; dies.len()],
        address,
        version: out.version,
    };
    let header = if out.version >= 5 { 12 } else { 11 };
    layout.place(&dies, 0, header);
    let Layout { order, code_of, offsets, .. } = layout;
    let mut buf = Buf::default();
    buf.u32(0);
    if out.version >= 5 {
        buf.u16(5);
        buf.u8(1); // DW_UT_compile
        buf.u8(out.address);
        buf.section_offset(out.places.abbrev, 0);
    } else {
        buf.u16(out.version);
        buf.section_offset(out.places.abbrev, 0);
        buf.u8(out.address);
    }
    debug_assert_eq!(buf.at(), header);
    write(&dies, &code_of, &offsets, 0, out, &mut buf)?;
    let length = buf.at() as u32 - 4;
    buf.patch32(0, length);
    let mut abbrev = Buf::default();
    for (index, (tag, children, attrs)) in order.iter().enumerate() {
        abbrev.uleb(index as u64 + 1);
        abbrev.uleb(u64::from(*tag));
        abbrev.u8(u8::from(*children));
        for (at, form) in attrs {
            abbrev.uleb(u64::from(*at));
            abbrev.uleb(u64::from(*form));
        }
        abbrev.uleb(0);
        abbrev.uleb(0);
    }
    abbrev.uleb(0);
    Ok((buf.done(), abbrev.done(), locations))
}

struct Layout {
    codes: HashMap<Shape, u64>,
    order: Vec<Shape>,
    code_of: Vec<u64>,
    offsets: Vec<usize>,
    address: usize,
    version: u16,
}

impl Layout {
    /// DIE `index` at `at`; where the next one starts.
    fn place(
        &mut self,
        dies: &[Die],
        index: usize,
        mut at: usize,
    ) -> usize {
        let die = &dies[index];
        let key = shape(die, self.version);
        let next = self.order.len() as u64 + 1;
        let code = *self.codes.entry(key.clone()).or_insert(next);
        if code == next {
            self.order.push(key);
        }
        self.code_of[index] = code;
        self.offsets[index] = at;
        at += uleb(code).len()
            + die.attrs.iter().map(|(_, value)| size(value, self.address, self.version)).sum::<usize>();
        for &child in &die.children {
            at = self.place(dies, child, at);
        }
        if die.children.is_empty() { at } else { at + 1 }
    }
}

fn write(
    dies: &[Die],
    codes: &[u64],
    offsets: &[usize],
    index: usize,
    out: &mut Out,
    buf: &mut Buf,
) -> Result<(), Unsupported> {
    let die = &dies[index];
    debug_assert_eq!(buf.at(), offsets[index], "the layout and the writing agree");
    buf.uleb(codes[index]);
    let address = usize::from(out.address);
    for (_, value) in &die.attrs {
        match value {
            Value::Str(text) => {
                let at = out.strings.add(text);
                buf.section_offset(out.places.strings, at);
            }
            Value::U8(one) => buf.u8(*one),
            Value::U16(one) => buf.u16(*one),
            Value::Len(one) => buf.u32(*one),
            Value::Udata(one) => buf.uleb(*one),
            Value::Sdata(one) => buf.sleb(*one),
            Value::Flag => {
                if out.version < 4 {
                    buf.u8(1);
                }
            }
            Value::Ref(target) => buf.u32(offsets[*target] as u32),
            Value::Addr { symbol, delta } => buf.address(address, *symbol, *delta),
            Value::Line => buf.section_offset(out.places.line, 0),
            Value::LocList(at) => buf.section_offset(out.places.locations.expect("a section of lists"), *at),
            Value::Expr(bytes) => {
                length(buf, bytes.len(), out.version)?;
                buf.bytes.extend(bytes);
            }
            Value::ExprAddr { symbol, delta } => {
                length(buf, 1 + address, out.version)?;
                buf.u8(0x03);
                buf.address(address, *symbol, *delta);
            }
            Value::ExprSegment { section } => {
                length(buf, 3, out.version)?;
                buf.u8(DW_OP_CONST2U);
                buf.segment(*section);
            }
        }
    }
    if !die.children.is_empty() {
        for &child in &die.children {
            write(dies, codes, offsets, child, out, buf)?;
        }
        buf.u8(0);
    }
    Ok(())
}

fn length(
    buf: &mut Buf,
    bytes: usize,
    version: u16,
) -> Result<(), Unsupported> {
    if version >= 4 {
        buf.uleb(bytes as u64);
    } else {
        buf.u8(u8::try_from(bytes).or_else(|_| crate::refused("an expression of 256 bytes before DWARF 4"))?);
    }
    Ok(())
}

/// `.debug_aranges`: the module's code.
pub fn aranges(
    object: &Object,
    info: &Info,
    out: &Out,
) -> Result<Done, Unsupported> {
    let mut buf = Buf::default();
    buf.u32(0);
    buf.u16(2);
    buf.section_offset(out.places.info, 0);
    buf.u8(out.address);
    // A 16-bit program's address is an offset and a segment: a tuple is the
    // offset, the segment and the length, 2 bytes each, as Open Watcom writes
    // it.
    let segmented = out.address == 2;
    buf.u8(if segmented { 2 } else { 0 });
    let tuple = if segmented { 6 } else { 2 * usize::from(out.address) };
    while !segmented && buf.at() % tuple != 0 {
        buf.u8(0);
    }
    let address = usize::from(out.address);
    for range in &info.code {
        let (symbol, base) = crate::anchor(object, range.section)?;
        buf.address(address, symbol, range.offset as i64 - base as i64);
        if segmented {
            buf.segment(range.section);
        }
        buf.bytes.extend((range.length as u64).to_le_bytes().iter().take(address));
    }
    buf.bytes.extend(std::iter::repeat_n(0, tuple));
    let length = buf.at() as u32 - 4;
    buf.patch32(0, length);
    Ok(buf.done())
}
