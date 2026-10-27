//! `.debug$S`: the string table, the file checksums, the module's symbols and
//! each code range's line table, each a subsection. Where a record names code
//! or data, a `SECREL` and a `SECTION` relocation against the object's symbol
//! fill its offset and its section.

use llrm_object::debug::{ChecksumKind, Function, Info, Kind, Location, Range, Type, Variable};
use llrm_object::{Binding, Definition, Kind as Fixup, Object, Reloc, Target, Unsupported};

use super::types::Types;
use super::{Registers, SIGNATURE, Section, machine, name, put16, put32, record, refused};

const DEBUG_S_SYMBOLS: u32 = 0xF1;
const DEBUG_S_LINES: u32 = 0xF2;
const DEBUG_S_STRINGTABLE: u32 = 0xF3;
const DEBUG_S_FILECHKSMS: u32 = 0xF4;
const S_END: u16 = 0x0006;
const S_OBJNAME: u16 = 0x1101;
const S_BLOCK32: u16 = 0x1103;
const S_UDT: u16 = 0x1108;
const S_LDATA32: u16 = 0x110C;
const S_GDATA32: u16 = 0x110D;
const S_LPROC32: u16 = 0x110F;
const S_GPROC32: u16 = 0x1110;
const S_COMPILE3: u16 = 0x113C;
const S_LOCAL: u16 = 0x113E;
const S_DEFRANGE_REGISTER: u16 = 0x1141;
const S_DEFRANGE_REGISTER_REL: u16 = 0x1145;
const IS_PARAMETER: u16 = 1;
const COLUMNS: u16 = 1;
const IS_STATEMENT: u32 = 0x8000_0000;
/// A range's length is 16 bits: longer ones are written in pieces.
const PIECE: usize = 0xF000;

fn zeros(_: usize) -> u8 {
    0
}

/// A subsection's data, and the relocations in it, by offset in the data.
#[derive(Default)]
struct Data {
    bytes: Vec<u8>,
    relocs: Vec<Reloc>,
}

impl Data {
    fn secrel(
        &mut self,
        symbol: usize,
        addend: i64,
    ) {
        self.relocs.push(Reloc {
            at: self.bytes.len(),
            kind: Fixup::SectionOffset { width: 4 },
            target: Target::Symbol(symbol),
            addend,
        });
        put32(&mut self.bytes, 0);
    }

    fn section(
        &mut self,
        symbol: usize,
    ) {
        self.relocs.push(Reloc {
            at: self.bytes.len(),
            kind: Fixup::SectionIndex,
            target: Target::Symbol(symbol),
            addend: 0,
        });
        put16(&mut self.bytes, 0);
    }

    /// A symbol record, `kind` and what `build` writes into its data.
    fn symbol(
        &mut self,
        kind: u16,
        build: impl FnOnce(&mut Data) -> Result<(), Unsupported>,
    ) -> Result<(), Unsupported> {
        // The record is written whole, then moved: its relocations are rebased
        // to where it lands.
        let mut inner = Data::default();
        build(&mut inner)?;
        let at = self.bytes.len() + 4;
        self.relocs.extend(inner.relocs.into_iter().map(|one| Reloc { at: at + one.at, ..one }));
        record(&mut self.bytes, kind, &inner.bytes, zeros)
    }

    fn end(&mut self) -> Result<(), Unsupported> {
        record(&mut self.bytes, S_END, &[], zeros)
    }
}

struct Strings {
    bytes: Vec<u8>,
}

impl Strings {
    fn add(
        &mut self,
        text: &str,
    ) -> u32 {
        let at = self.bytes.len() as u32;
        name(&mut self.bytes, text);
        at
    }
}

fn pad4(bytes: &mut Vec<u8>) {
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
}

/// The offset of `symbol` from the first byte of `range`, as the addend that
/// makes a relocation against the symbol reach it.
fn anchor(
    object: &Object,
    symbol: usize,
    range: Range,
) -> Result<i64, Unsupported> {
    match object.symbols[symbol].definition {
        Definition::Defined { section, offset } if section == range.section => Ok(range.offset as i64 - offset as i64),
        _ => refused(format!("{} is not defined in the section of the code it names", object.symbols[symbol].name)),
    }
}

fn pieces(range: Range) -> Vec<Range> {
    (0..range.length.div_ceil(PIECE).max(1))
        .map(|at| Range { offset: range.offset + at * PIECE, length: (range.length - at * PIECE).min(PIECE), ..range })
        .collect()
}

struct Writer<'a> {
    object: &'a Object,
    registers: &'a Registers<'a>,
    types: &'a Types,
    /// The symbol the function being written is, which its code is reached
    /// through.
    function: usize,
}

impl Writer<'_> {
    fn register(
        &self,
        register: &str,
    ) -> Result<u16, Unsupported> {
        self.registers.number(register)
    }

    /// `S_DEFRANGE_*` for `location` over each piece of `ranges`.
    fn defined(
        &self,
        out: &mut Data,
        location: &Location,
        ranges: &[Range],
    ) -> Result<(), Unsupported> {
        for &range in ranges {
            for piece in pieces(range) {
                let anchored = anchor(self.object, self.function, piece)?;
                let at = |out: &mut Data| {
                    out.secrel(self.function, anchored);
                    out.section(self.function);
                    put16(&mut out.bytes, piece.length as u16);
                };
                match location {
                    Location::Frame { disp } => out.symbol(S_DEFRANGE_REGISTER_REL, |out| {
                        put16(&mut out.bytes, self.registers.frame()?);
                        put16(&mut out.bytes, 0);
                        put32(
                            &mut out.bytes,
                            i32::try_from(*disp)
                                .or_else(|_| refused(format!("a frame offset {disp} does not fit its field")))?
                                as u32,
                        );
                        at(out);
                        Ok(())
                    })?,
                    Location::Relative { register, disp } => out.symbol(S_DEFRANGE_REGISTER_REL, |out| {
                        put16(&mut out.bytes, self.register(register)?);
                        put16(&mut out.bytes, 0);
                        put32(
                            &mut out.bytes,
                            i32::try_from(*disp)
                                .or_else(|_| refused(format!("a stack offset {disp} does not fit its field")))?
                                as u32,
                        );
                        at(out);
                        Ok(())
                    })?,
                    Location::Register(register) => out.symbol(S_DEFRANGE_REGISTER, |out| {
                        put16(&mut out.bytes, self.register(register)?);
                        put16(&mut out.bytes, 0);
                        at(out);
                        Ok(())
                    })?,
                    Location::List(_) | Location::Static { .. } | Location::Constant(_) | Location::Pieces(_) => {
                        return refused("a location list holds only registers and frame cells");
                    }
                }
            }
        }
        Ok(())
    }

    fn data(
        &self,
        out: &mut Data,
        variable: &Variable,
    ) -> Result<(), Unsupported> {
        let Location::Static { symbol, disp } = &variable.location else {
            return refused(format!("{} is not in data", variable.name));
        };
        let public = self.object.symbols[*symbol].binding == Binding::Public;
        out.symbol(if public { S_GDATA32 } else { S_LDATA32 }, |out| {
            put32(&mut out.bytes, self.types.index[variable.r#type]);
            out.secrel(*symbol, *disp);
            out.section(*symbol);
            name(&mut out.bytes, &variable.name);
            Ok(())
        })
    }

    /// A variable of a scope covering `ranges`.
    fn variable(
        &self,
        out: &mut Data,
        variable: &Variable,
        ranges: &[Range],
    ) -> Result<(), Unsupported> {
        if matches!(variable.location, Location::Static { .. }) {
            return self.data(out, variable);
        }
        out.symbol(S_LOCAL, |out| {
            put32(&mut out.bytes, self.types.index[variable.r#type]);
            put16(&mut out.bytes, if variable.kind == Kind::Parameter { IS_PARAMETER } else { 0 });
            name(&mut out.bytes, &variable.name);
            Ok(())
        })?;
        match &variable.location {
            Location::List(places) => {
                for (range, place) in places {
                    self.defined(out, place, &[*range])?;
                }
                Ok(())
            }
            place => self.defined(out, place, ranges),
        }
    }

    fn block(
        &self,
        out: &mut Data,
        block: &llrm_object::debug::Block,
    ) -> Result<(), Unsupported> {
        let [range] = block.ranges[..] else { return refused("a block scope in several ranges") };
        let anchored = anchor(self.object, self.function, range)?;
        out.symbol(S_BLOCK32, |out| {
            put32(&mut out.bytes, 0);
            put32(&mut out.bytes, 0);
            put32(&mut out.bytes, u32::try_from(range.length).or_else(|_| refused("a block's length"))?);
            out.secrel(self.function, anchored);
            out.section(self.function);
            name(&mut out.bytes, "");
            Ok(())
        })?;
        for variable in &block.variables {
            self.variable(out, variable, &block.ranges)?;
        }
        for inner in &block.blocks {
            self.block(out, inner)?;
        }
        out.end()?;
        Ok(())
    }

    fn procedure(
        &mut self,
        out: &mut Data,
        function: &Function,
    ) -> Result<(), Unsupported> {
        let [range] = function.ranges[..] else {
            return refused(format!("{} is in {} ranges", function.name, function.ranges.len()));
        };
        if function.far {
            return refused(format!("{} is a far function", function.name));
        }
        self.function = function.symbol;
        let anchored = anchor(self.object, function.symbol, range)?;
        let (start, end) = function.body.unwrap_or((0, range.length));
        let public = self.object.symbols[function.symbol].binding == Binding::Public;
        out.symbol(if public { S_GPROC32 } else { S_LPROC32 }, |out| {
            for _ in 0..3 {
                put32(&mut out.bytes, 0);
            }
            put32(
                &mut out.bytes,
                u32::try_from(range.length).or_else(|_| refused(format!("{}'s length", function.name)))?,
            );
            put32(&mut out.bytes, start as u32);
            put32(&mut out.bytes, end as u32);
            put32(&mut out.bytes, self.types.index[function.r#type]);
            out.secrel(function.symbol, anchored);
            out.section(function.symbol);
            out.bytes.push(0);
            name(&mut out.bytes, &function.name);
            Ok(())
        })?;
        for variable in &function.variables {
            self.variable(out, variable, &function.ranges)?;
        }
        for block in &function.blocks {
            self.block(out, block)?;
        }
        out.end()?;
        Ok(())
    }
}

/// One subsection, its data padded to four bytes, appended to `section` with
/// its relocations.
fn subsection(
    section: &mut Section,
    kind: u32,
    mut data: Data,
) {
    pad4(&mut data.bytes);
    put32(&mut section.image, kind);
    put32(&mut section.image, data.bytes.len() as u32);
    let at = section.image.len();
    section.relocs.extend(data.relocs.into_iter().map(|one| Reloc { at: at + one.at, ..one }));
    section.image.extend(data.bytes);
}

/// The line table of `range`: its header, then a block for each run of lines in
/// one file.
fn lines(
    object: &Object,
    info: &Info,
    function: usize,
    range: Range,
    offsets: &[u32],
) -> Result<Option<Data>, Unsupported> {
    let mut inside: Vec<_> = info
        .lines
        .iter()
        .filter(|one| one.section == range.section && (range.offset..range.offset + range.length).contains(&one.offset))
        .collect();
    if inside.is_empty() {
        return Ok(None);
    }
    inside.sort_by_key(|one| one.offset);
    if let Some(one) = inside.iter().find(|one| one.line >= IS_STATEMENT || one.column > u32::from(u16::MAX)) {
        return refused(format!("line {} column {} does not fit a line entry's 31 and 16 bits", one.line, one.column));
    }
    let columns = inside.iter().any(|one| one.column != 0);
    let mut out = Data::default();
    out.secrel(function, anchor(object, function, range)?);
    out.section(function);
    put16(&mut out.bytes, if columns { COLUMNS } else { 0 });
    put32(&mut out.bytes, range.length as u32);
    let mut at = 0;
    while at < inside.len() {
        let file = inside[at].file;
        let run = inside[at..].iter().take_while(|one| one.file == file).count();
        let Some(&checksum) = offsets.get(file) else {
            return refused(format!("a line is in file {file}, which the module does not list"));
        };
        put32(&mut out.bytes, checksum);
        put32(&mut out.bytes, run as u32);
        put32(&mut out.bytes, 12 + run as u32 * if columns { 12 } else { 8 });
        for one in &inside[at..at + run] {
            put32(&mut out.bytes, (one.offset - range.offset) as u32);
            put32(&mut out.bytes, one.line | IS_STATEMENT);
        }
        if columns {
            for one in &inside[at..at + run] {
                put16(&mut out.bytes, one.column as u16);
                put16(&mut out.bytes, 0);
            }
        }
        at += run;
    }
    Ok(Some(out))
}

pub(super) fn encode(
    object: &Object,
    info: &Info,
    registers: &Registers<'_>,
    types: &Types,
) -> Result<Section, Unsupported> {
    let mut strings = Strings { bytes: vec![0] };
    let mut checksums = Data::default();
    let mut offsets = Vec::new();
    for file in &info.files {
        offsets.push(checksums.bytes.len() as u32);
        put32(&mut checksums.bytes, strings.add(&file.name));
        match &file.checksum {
            None => checksums.bytes.extend([0, 0]),
            Some((kind, sum)) => {
                checksums.bytes.push(sum.len() as u8);
                checksums
                    .bytes
                    .push(
                        match kind {
                            ChecksumKind::Md5 => 1,
                            ChecksumKind::Sha1 => 2,
                            ChecksumKind::Sha256 => 3,
                        },
                    );
                checksums.bytes.extend(sum);
            }
        }
        pad4(&mut checksums.bytes);
    }

    let mut writer = Writer { object, registers, types, function: 0 };
    let mut symbols = Data::default();
    symbols.symbol(S_OBJNAME, |out| {
        put32(&mut out.bytes, 0);
        name(&mut out.bytes, &object.name);
        Ok(())
    })?;
    symbols.symbol(S_COMPILE3, |out| {
        // Language 0 is C.
        put32(&mut out.bytes, 0);
        put16(&mut out.bytes, machine(object.arch)?);
        [0u16, 1, 0, 0, 0, 1, 0, 0].iter().for_each(|&one| put16(&mut out.bytes, one));
        name(&mut out.bytes, "llrm");
        Ok(())
    })?;
    for global in &info.globals {
        writer.data(&mut symbols, global)?;
    }
    for function in info.functions.iter().filter(|one| one.module) {
        for variable in &function.variables {
            writer.data(&mut symbols, variable)?;
        }
    }
    for (id, one) in info.types.iter().enumerate() {
        let label = match one {
            Type::Struct { name, .. } | Type::Enum { name, .. } | Type::Typedef { name, .. } if !name.is_empty() => {
                name
            }
            _ => continue,
        };
        symbols.symbol(S_UDT, |out| {
            put32(&mut out.bytes, types.index[id]);
            name(&mut out.bytes, label);
            Ok(())
        })?;
    }
    for function in info.functions.iter().filter(|one| !one.module) {
        writer.procedure(&mut symbols, function)?;
    }

    let mut section = Section::default();
    put32(&mut section.image, SIGNATURE);
    subsection(&mut section, DEBUG_S_SYMBOLS, symbols);
    let mut covered = 0;
    for function in info.functions.iter().filter(|one| !one.module) {
        for &range in &function.ranges {
            if let Some(table) = lines(object, info, function.symbol, range, &offsets)? {
                covered += table.bytes.len();
                subsection(&mut section, DEBUG_S_LINES, table);
            }
        }
    }
    if covered == 0 && !info.lines.is_empty() {
        return refused("the line table's lines are in no function");
    }
    subsection(&mut section, DEBUG_S_FILECHKSMS, checksums);
    subsection(&mut section, DEBUG_S_STRINGTABLE, Data { bytes: strings.bytes, relocs: Vec::new() });
    Ok(section)
}
