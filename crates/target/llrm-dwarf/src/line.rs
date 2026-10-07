//! `.debug_line`: the line program, one sequence per section of code with lines.

use llrm_object::debug::{ChecksumKind, Info};
use llrm_object::{Object, Unsupported};

use crate::buffer::{Buf, Done};
use crate::die::Out;
use crate::refused;

const LINE_BASE: i8 = -5;
const LINE_RANGE: u8 = 14;
const OPCODE_BASE: u8 = 13;
const STANDARD_LENGTHS: [u8; 12] = [0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1];

const LNS_COPY: u8 = 1;
const LNS_ADVANCE_PC: u8 = 2;
const LNS_ADVANCE_LINE: u8 = 3;
const LNS_SET_FILE: u8 = 4;
const LNS_SET_COLUMN: u8 = 5;
const LNS_PROLOGUE_END: u8 = 10;
const LNS_EPILOGUE_BEGIN: u8 = 11;
const LNE_END_SEQUENCE: u8 = 1;
const LNE_SET_ADDRESS: u8 = 2;

const LNCT_PATH: u8 = 1;
const LNCT_DIRECTORY_INDEX: u8 = 2;
const LNCT_MD5: u8 = 5;
const FORM_LINE_STRP: u8 = 0x1f;
const FORM_UDATA: u8 = 0x0f;
const FORM_DATA16: u8 = 0x1e;

/// A file's directory and name apart; the directories of every file, the first being the current.
fn directories(info: &Info) -> (Vec<String>, Vec<(usize, String)>) {
    let mut dirs = vec![String::new()];
    let mut files = Vec::new();
    for file in &info.files {
        let (dir, name) = file.name.rsplit_once('/').unwrap_or(("", &file.name));
        let at = match dirs.iter().position(|one| one == dir) {
            Some(at) => at,
            None => {
                dirs.push(dir.to_owned());
                dirs.len() - 1
            }
        };
        files.push((at, name.to_owned()));
    }
    (dirs, files)
}

/// A row of the line table: what `Line` says, and where the body starts and ends.
#[derive(Clone, Copy)]
struct Row {
    offset: usize,
    file: usize,
    line: u32,
    column: u32,
    prologue_end: bool,
    epilogue_begin: bool,
}

pub fn program(object: &Object, info: &Info, out: &mut Out) -> Result<Done, Unsupported> {
    let (dirs, files) = directories(info);
    let mut buf = Buf::default();
    buf.u32(0);
    buf.u16(out.version);
    if out.version >= 5 {
        buf.u8(out.address);
        buf.u8(0);
    }
    let header_length = buf.at();
    buf.u32(0);
    buf.u8(1); // minimum instruction length
    if out.version >= 4 {
        buf.u8(1); // maximum operations per instruction
    }
    buf.u8(1); // default is_stmt
    buf.u8(LINE_BASE as u8);
    buf.u8(LINE_RANGE);
    buf.u8(OPCODE_BASE);
    buf.bytes.extend(STANDARD_LENGTHS);
    let md5 = !info.files.is_empty() && info.files.iter().all(|one| matches!(&one.checksum, Some((ChecksumKind::Md5, sum)) if sum.len() == 16));
    if info.files.iter().any(|one| one.checksum.is_some()) && !(md5 && out.version >= 5) {
        return refused("a file checksum needs DWARF 5 and an MD5 of every file");
    }
    if out.version >= 5 {
        let strings = out.places.line_strings.expect("version 5 has .debug_line_str");
        buf.u8(1);
        buf.uleb(u64::from(LNCT_PATH));
        buf.uleb(u64::from(FORM_LINE_STRP));
        buf.uleb(dirs.len() as u64);
        for dir in &dirs {
            // Directory 0 is the compilation directory: the current one, spelled ".".
            let at = out.line_strings.add(if dir.is_empty() { "." } else { dir });
            buf.section_offset(strings, at);
        }
        buf.u8(if md5 { 3 } else { 2 });
        buf.uleb(u64::from(LNCT_PATH));
        buf.uleb(u64::from(FORM_LINE_STRP));
        buf.uleb(u64::from(LNCT_DIRECTORY_INDEX));
        buf.uleb(u64::from(FORM_UDATA));
        if md5 {
            buf.uleb(u64::from(LNCT_MD5));
            buf.uleb(u64::from(FORM_DATA16));
        }
        buf.uleb(files.len() as u64);
        for index in 0..files.len() {
            let (dir, name) = &files[index];
            let at = out.line_strings.add(name);
            buf.section_offset(strings, at);
            buf.uleb(*dir as u64);
            if md5 {
                if let Some((_, sum)) = &info.files[index].checksum {
                    buf.bytes.extend(sum);
                }
            }
        }
    } else {
        for dir in dirs.iter().skip(1) {
            buf.string(dir);
        }
        buf.u8(0);
        for (dir, name) in &files {
            buf.string(name);
            buf.uleb(*dir as u64);
            buf.uleb(0);
            buf.uleb(0);
        }
        buf.u8(0);
    }
    let length = (buf.at() - header_length - 4) as u32;
    buf.patch32(header_length, length);

    // The file number of file `index` in the table above: 0-based in 5, 1-based before. A sequence
    // starts at file 1, so one in file 0 sets it.
    let number = |index: usize| -> u64 { index as u64 + u64::from(out.version < 5) };
    let mut sections: Vec<usize> = Vec::new();
    for one in &info.lines {
        if one.file >= info.files.len() {
            return refused(format!("line {} is in file {}, which is not listed", one.line, one.file));
        }
        if !sections.contains(&one.section) {
            sections.push(one.section);
        }
    }
    let address = usize::from(out.address);
    for section in sections {
        let mut rows: Vec<Row> = info.lines.iter().filter(|one| one.section == section).map(|one| Row { offset: one.offset, file: one.file, line: one.line, column: one.column, prologue_end: false, epilogue_begin: false }).collect();
        rows.sort_by_key(|one| one.offset);
        // Where each body starts and ends is a row, however the source's lines fall.
        for function in &info.functions {
            let Some(range) = function.ranges.first().filter(|range| range.section == section) else { continue };
            let Some((start, end)) = function.body else { continue };
            for (at, prologue) in [(range.offset + start, true), (range.offset + end, false)] {
                if at >= range.offset + range.length {
                    continue;
                }
                let at_row = match rows.iter().position(|one| one.offset == at) {
                    Some(index) => index,
                    None => {
                        let Some(before) = rows.iter().rposition(|one| one.offset < at) else { continue };
                        let copy = Row { offset: at, prologue_end: false, epilogue_begin: false, ..rows[before] };
                        rows.insert(before + 1, copy);
                        before + 1
                    }
                };
                if prologue {
                    rows[at_row].prologue_end = true;
                } else {
                    rows[at_row].epilogue_begin = true;
                }
            }
        }
        let (symbol, base) = crate::anchor(object, section)?;
        let start = rows[0].offset;
        buf.u8(0);
        buf.uleb(1 + address as u64);
        buf.u8(LNE_SET_ADDRESS);
        buf.address(address, symbol, start as i64 - base as i64);
        let (mut at, mut line, mut file, mut column) = (start, 1i64, 1u64, 0u64);
        for row in rows {
            if number(row.file) != file {
                file = number(row.file);
                buf.u8(LNS_SET_FILE);
                buf.uleb(file);
            }
            if u64::from(row.column) != column {
                column = u64::from(row.column);
                buf.u8(LNS_SET_COLUMN);
                buf.uleb(column);
            }
            if i64::from(row.line) != line {
                buf.u8(LNS_ADVANCE_LINE);
                buf.sleb(i64::from(row.line) - line);
                line = i64::from(row.line);
            }
            if row.offset != at {
                buf.u8(LNS_ADVANCE_PC);
                buf.uleb((row.offset - at) as u64);
                at = row.offset;
            }
            if row.prologue_end {
                buf.u8(LNS_PROLOGUE_END);
            }
            if row.epilogue_begin {
                buf.u8(LNS_EPILOGUE_BEGIN);
            }
            buf.u8(LNS_COPY);
        }
        // The sequence ends where the code does.
        let end = info.code.iter().filter(|range| range.section == section).map(|range| range.offset + range.length).max().unwrap_or(object.sections[section].image.len());
        buf.u8(LNS_ADVANCE_PC);
        buf.uleb(end.saturating_sub(at) as u64);
        buf.u8(0);
        buf.uleb(1);
        buf.u8(LNE_END_SEQUENCE);
    }
    let total = buf.at() as u32 - 4;
    buf.patch32(0, total);
    Ok(buf.done())
}
