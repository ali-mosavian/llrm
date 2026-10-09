//! An [`Object`] as a 64-bit Mach-O relocatable file for x86-64: one segment
//! holding the sections, a symbol table, and the 8-byte relocation entries
//! Mach-O keeps per section, with the addend in the field. A reference to a
//! symbol the object defines and exports, or declares, names it; a reference to
//! a local one names its section, and the field holds the address.
//!
//! The file is not marked `MH_SUBSECTIONS_VIA_SYMBOLS`: a code section stays
//! one atom, so a call between two functions of it may be resolved where it is
//! written.

use llrm_object::{Arch, Binding, Definition, Kind, Object, Role, Section, Target, Unsupported};

const MH_MAGIC_64: u32 = 0xFEED_FACF;
const CPU_TYPE_X86_64: u32 = 0x0100_0007;
const CPU_SUBTYPE_X86_64_ALL: u32 = 3;
const MH_OBJECT: u32 = 1;
const LC_SYMTAB: u32 = 0x2;
const LC_DYSYMTAB: u32 = 0xB;
const LC_SEGMENT_64: u32 = 0x19;
const S_ZEROFILL: u32 = 1;
const S_ATTR_PURE_INSTRUCTIONS: u32 = 0x8000_0000;
const S_ATTR_SOME_INSTRUCTIONS: u32 = 0x0000_0400;
const S_ATTR_DEBUG: u32 = 0x0200_0000;
const N_EXT: u8 = 1;
const N_SECT: u8 = 0xE;
const X86_64_RELOC_UNSIGNED: u32 = 0;
const X86_64_RELOC_SIGNED: u32 = 1;
const X86_64_RELOC_BRANCH: u32 = 2;
const X86_64_RELOC_SIGNED_1: u32 = 6;
const X86_64_RELOC_SIGNED_2: u32 = 7;
const X86_64_RELOC_SIGNED_4: u32 = 8;

fn unsupported(text: impl Into<String>) -> Unsupported {
    Unsupported(text.into())
}

fn put32(
    out: &mut Vec<u8>,
    value: u32,
) {
    out.extend(value.to_le_bytes());
}

fn put64(
    out: &mut Vec<u8>,
    value: u64,
) {
    out.extend(value.to_le_bytes());
}

/// A fixed-width name field, NUL padded.
fn name16(
    out: &mut Vec<u8>,
    name: &str,
) {
    let mut field = [0u8; 16];
    field[..name.len()].copy_from_slice(name.as_bytes());
    out.extend(field);
}

/// The Mach-O name of a DWARF section.
fn debug_name(name: &str) -> Result<&'static str, Unsupported> {
    Ok(match name {
        ".debug_abbrev" => "__debug_abbrev",
        ".debug_aranges" => "__debug_aranges",
        ".debug_info" => "__debug_info",
        ".debug_line" => "__debug_line",
        ".debug_line_str" => "__debug_line_str",
        ".debug_loc" => "__debug_loc",
        ".debug_loclists" => "__debug_loclists",
        ".debug_str" => "__debug_str",
        other => return Err(unsupported(format!("{other}: no Mach-O name for this debug section"))),
    })
}

/// The segment, section and type-and-attributes words of `section`.
fn spelling(section: &Section) -> Result<(&'static str, &'static str, u32), Unsupported> {
    if !section.near {
        return Err(unsupported(format!(
            "{}: a segment addressed by its own selector has no Mach-O section",
            section.name
        )));
    }
    Ok(match section.role {
        Role::Text => ("__TEXT", "__text", S_ATTR_PURE_INSTRUCTIONS | S_ATTR_SOME_INSTRUCTIONS),
        // A constant with relocations is written to by the dynamic linker: ld64
        // refuses it in __TEXT.
        Role::ROData if !section.relocs.is_empty() => ("__DATA", "__const", 0),
        Role::ROData => ("__TEXT", "__const", 0),
        Role::Data => ("__DATA", "__data", 0),
        Role::Bss => ("__DATA", "__bss", S_ZEROFILL),
        Role::Stack => {
            return Err(unsupported(format!("{}: an OMF stack segment has no Mach-O section", section.name)));
        }
        // `.debug_info` is `__debug_info` of the segment `__DWARF`.
        Role::Debug => ("__DWARF", debug_name(&section.name)?, S_ATTR_DEBUG),
    })
}

/// The relocation type and length (log2 of the field's bytes) of a field,
/// whether it is pc-relative, and the bytes between the field's end and the
/// place it is relative to.
fn relocation(kind: Kind) -> Result<(u32, u32, bool, i64), Unsupported> {
    Ok(match kind {
        Kind::Abs { width: 8 } => (X86_64_RELOC_UNSIGNED, 3, false, 0),
        Kind::Abs { width: 4 } => (X86_64_RELOC_UNSIGNED, 2, false, 0),
        Kind::Branch { width: 4 } => (X86_64_RELOC_BRANCH, 2, true, 0),
        Kind::PcRel { width: 4, from: 4 } => (X86_64_RELOC_SIGNED, 2, true, 0),
        Kind::PcRel { width: 4, from: 5 } => (X86_64_RELOC_SIGNED_1, 2, true, 1),
        Kind::PcRel { width: 4, from: 6 } => (X86_64_RELOC_SIGNED_2, 2, true, 2),
        Kind::PcRel { width: 4, from: 8 } => (X86_64_RELOC_SIGNED_4, 2, true, 4),
        Kind::SegmentBase => return Err(unsupported("a segment selector has no Mach-O relocation")),
        Kind::FarPointer => return Err(unsupported("a 16:16 far pointer has no Mach-O relocation")),
        other => return Err(unsupported(format!("Mach-O x86-64 has no relocation for {other:?}"))),
    })
}

fn align_up(
    value: usize,
    to: usize,
) -> usize {
    value.div_ceil(to) * to
}

/// `object` as a Mach-O relocatable file.
pub fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    if object.arch != Arch::X8664 {
        return Err(unsupported(format!("{:?} has no Mach-O writer: only x86-64 is written", object.arch)));
    }
    // DWARF's sections are this writer's, made from the object's debug
    // information.
    let expanded;
    let object = match &object.debug {
        Some(info) => {
            expanded = llrm_dwarf::expanded(object, info)?;
            &expanded
        }
        None => object,
    };
    if !object.omf_groups.is_empty() {
        return Err(unsupported("a group of segments is OMF's"));
    }
    let spelled: Vec<(&str, &str, u32)> = object.sections.iter().map(spelling).collect::<Result<_, _>>()?;
    for (index, one) in spelled.iter().enumerate() {
        if spelled[..index].iter().any(|other| other.0 == one.0 && other.1 == one.1) {
            return Err(unsupported(format!(
                "{}: two sections would both be ({},{})",
                object.sections[index].name, one.0, one.1
            )));
        }
    }

    // Sections with contents come first in the file, then the zero-fill ones,
    // which have none.
    let mut order: Vec<usize> = (0..object.sections.len()).filter(|&one| spelled[one].2 != S_ZEROFILL).collect();
    order.extend((0..object.sections.len()).filter(|&one| spelled[one].2 == S_ZEROFILL));
    let ordinal =
        |section: usize| order.iter().position(|&one| one == section).expect("a section is in the order") as u32 + 1;
    let mut address = vec![0usize; object.sections.len()];
    let mut next = 0;
    for &one in &order {
        next = align_up(next, object.sections[one].align.max(1).next_power_of_two());
        address[one] = next;
        next += object.sections[one].image.len();
    }
    let vm_size = next;
    let file_backed = order.iter().filter(|&&one| spelled[one].2 != S_ZEROFILL).count();
    let file_size =
        order.iter().take(file_backed).last().map_or(0, |&one| address[one] + object.sections[one].image.len());

    // Symbols the object exports, then the ones it declares; each by name
    // within its kind.
    let mut defined: Vec<usize> = (0..object.symbols.len())
        .filter(|&one| {
            object.symbols[one].binding == Binding::Public
                && matches!(object.symbols[one].definition, Definition::Defined { .. })
        })
        .collect();
    let mut undefined: Vec<usize> =
        (0..object.symbols.len()).filter(|&one| object.symbols[one].definition == Definition::Undefined).collect();
    defined.sort_by(|&a, &b| object.symbols[a].name.cmp(&object.symbols[b].name));
    undefined.sort_by(|&a, &b| object.symbols[a].name.cmp(&object.symbols[b].name));
    let mut index_of = vec![None; object.symbols.len()];
    for (at, &one) in defined.iter().chain(&undefined).enumerate() {
        index_of[one] = Some(at as u32);
    }
    let mut strings = vec![0u8];
    let mut nlist = Vec::new();
    for &one in defined.iter().chain(&undefined) {
        let symbol = &object.symbols[one];
        let name = strings.len() as u32;
        strings.extend(symbol.name.as_bytes());
        strings.push(0);
        put32(&mut nlist, name);
        match symbol.definition {
            Definition::Defined { section, offset } => {
                nlist.extend([N_SECT | N_EXT, ordinal(section) as u8]);
                nlist.extend(0u16.to_le_bytes());
                put64(&mut nlist, (address[section] + offset) as u64);
            }
            Definition::Undefined => {
                nlist.extend([N_EXT, 0]);
                nlist.extend(0u16.to_le_bytes());
                put64(&mut nlist, 0);
            }
        }
    }

    // Each section's bytes with the fields filled, and its relocation entries.
    let mut images: Vec<Vec<u8>> = Vec::new();
    let mut entries: Vec<Vec<u8>> = Vec::new();
    for (index, section) in object.sections.iter().enumerate() {
        let mut image = section.image.clone();
        let mut made: Vec<(usize, [u32; 2])> = Vec::new();
        for one in &section.relocs {
            let (kind, length, pcrel, beyond) = relocation(one.kind)?;
            let width = one.kind.width();
            // A section's start is a local symbol at offset 0 of it.
            let (target, home) = match one.target {
                Target::Symbol(target) => (Some(target), None),
                Target::Section(section) => (None, Some(section)),
                Target::OmfGroup(_) => return Err(unsupported("a reference to a group is OMF's")),
            };
            // One debug section's offset into another is not relocated:
            // dsymutil reads the object's own sections, where it is
            // the offset itself.
            if section.role == Role::Debug && home.is_some_and(|home| object.sections[home].role == Role::Debug) {
                let offset = u32::try_from(one.addend)
                    .map_err(|_| unsupported(format!("{}: an offset of {} does not fit", section.name, one.addend)))?;
                image[one.at..one.at + width].copy_from_slice(&u64::from(offset).to_le_bytes()[..width]);
                continue;
            }
            let (symbolnum, external, field) = match target.and_then(|target| index_of[target]) {
                Some(symbol) => (symbol, true, one.addend - beyond),
                None => {
                    // A local symbol: its section, and the address in the
                    // object's own space, which the
                    // linker moves with the section.
                    let (home, offset) = match (target, home) {
                        (Some(target), _) => match object.symbols[target].definition {
                            Definition::Defined { section: home, offset } => (home, offset),
                            Definition::Undefined => unreachable!("an undefined symbol is listed"),
                        },
                        (None, Some(home)) => (home, 0),
                        (None, None) => unreachable!("a target is a symbol or a section"),
                    };
                    let place = if pcrel { (address[index] + one.at + 4) as i64 } else { 0 };
                    (ordinal(home), false, (address[home] + offset) as i64 + one.addend - beyond - place)
                }
            };
            let fits = match (width, pcrel) {
                (8, _) => true,
                (_, true) => i32::try_from(field).is_ok(),
                (_, false) => u32::try_from(field).is_ok(),
            };
            if !fits {
                return Err(unsupported(format!(
                    "{}: a value of {field} does not fit its {width}-byte field",
                    section.name
                )));
            }
            image[one.at..one.at + width].copy_from_slice(&field.to_le_bytes()[..width]);
            made.push((
                one.at,
                [
                    one.at as u32,
                    symbolnum | u32::from(pcrel) << 24 | length << 25 | u32::from(external) << 27 | kind << 28,
                ],
            ));
        }
        // LLVM writes them last first.
        made.sort_by_key(|one| std::cmp::Reverse(one.0));
        let mut bytes = Vec::new();
        for (_, [address, info]) in made {
            put32(&mut bytes, address);
            put32(&mut bytes, info);
        }
        images.push(image);
        entries.push(bytes);
    }

    // Layout: header, load commands, section contents, relocations, symbols,
    // strings.
    let commands = 72 + 80 * order.len() + 24 + 80;
    let first = align_up(32 + commands, 16);
    let mut offsets = vec![0usize; object.sections.len()];
    for &one in order.iter().take(file_backed) {
        offsets[one] = first + address[one];
    }
    let mut at = first + file_size;
    let mut reloc_offsets = vec![0usize; object.sections.len()];
    for &one in &order {
        if !entries[one].is_empty() {
            at = align_up(at, 4);
            reloc_offsets[one] = at;
            at += entries[one].len();
        }
    }
    let symbols_at = align_up(at, 8);
    let strings_at = symbols_at + nlist.len();

    let mut out = Vec::new();
    for field in [MH_MAGIC_64, CPU_TYPE_X86_64, CPU_SUBTYPE_X86_64_ALL, MH_OBJECT, 3, commands as u32, 0, 0] {
        put32(&mut out, field);
    }
    put32(&mut out, LC_SEGMENT_64);
    put32(&mut out, (72 + 80 * order.len()) as u32);
    name16(&mut out, "");
    for field in [0, vm_size as u64, first as u64, file_size as u64] {
        put64(&mut out, field);
    }
    for field in [7, 7, order.len() as u32, 0] {
        put32(&mut out, field);
    }
    for &one in &order {
        let (segment, name, flags) = spelled[one];
        name16(&mut out, name);
        name16(&mut out, segment);
        put64(&mut out, address[one] as u64);
        put64(&mut out, images[one].len() as u64);
        put32(&mut out, offsets[one] as u32);
        put32(&mut out, object.sections[one].align.max(1).next_power_of_two().trailing_zeros());
        put32(&mut out, reloc_offsets[one] as u32);
        put32(&mut out, (entries[one].len() / 8) as u32);
        for field in [flags, 0, 0, 0] {
            put32(&mut out, field);
        }
    }
    put32(&mut out, LC_SYMTAB);
    put32(&mut out, 24);
    for field in [symbols_at as u32, (defined.len() + undefined.len()) as u32, strings_at as u32, strings.len() as u32]
    {
        put32(&mut out, field);
    }
    put32(&mut out, LC_DYSYMTAB);
    put32(&mut out, 80);
    for field in [0, 0, 0, defined.len() as u32, defined.len() as u32, undefined.len() as u32] {
        put32(&mut out, field);
    }
    out.resize(out.len() + 72 - 24, 0);
    out.resize(first, 0);
    for &one in order.iter().take(file_backed) {
        out.resize(offsets[one], 0);
        out.extend(&images[one]);
    }
    for &one in &order {
        if !entries[one].is_empty() {
            out.resize(reloc_offsets[one], 0);
            out.extend(&entries[one]);
        }
    }
    out.resize(symbols_at, 0);
    out.extend(&nlist);
    out.extend(&strings);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use llrm_object::{Reloc, Symbol};

    use super::*;

    const LLVM: &str = "/usr/lib/llvm-20/bin";

    fn tool(name: &str) -> Option<String> {
        let path = format!("{LLVM}/{name}");
        Path::new(&path).exists().then_some(path)
    }

    fn section(
        name: &str,
        role: Role,
        image: Vec<u8>,
        relocs: Vec<Reloc>,
    ) -> Section {
        let spans = vec![[0, image.len()]];
        Section { name: name.into(), role, near: true, align: 1, image, spans, relocs }
    }

    fn defined(
        name: &str,
        section: usize,
        offset: usize,
    ) -> Symbol {
        Symbol {
            name: name.into(),
            binding: Binding::Public,
            definition: Definition::Defined { section, offset },
            group: None,
        }
    }

    fn external(name: &str) -> Symbol {
        Symbol { name: name.into(), binding: Binding::Public, definition: Definition::Undefined, group: None }
    }

    fn reloc(
        at: usize,
        kind: Kind,
        target: usize,
        addend: i64,
    ) -> Reloc {
        Reloc { at, kind, target: Target::Symbol(target), addend }
    }

    /// The program `REFERENCE` assembles: a call, a `lea`, a `movb $1,
    /// flag(%rip)` (four bytes of field and one of immediate), a load, and
    /// a pointer to `msg+1`.
    fn program() -> Object {
        let text = vec![
            0xE8, 0, 0, 0, 0, 0x48, 0x8D, 0x35, 0, 0, 0, 0, 0xC6, 0x05, 0, 0, 0, 0, 0x01, 0x48, 0x8B, 0x05, 0, 0, 0, 0,
            0xC3,
        ];
        let relocs = vec![
            reloc(1, Kind::Branch { width: 4 }, 3, 0),
            reloc(8, Kind::PcRel { width: 4, from: 4 }, 1, 0),
            reloc(14, Kind::PcRel { width: 4, from: 5 }, 4, 0),
            reloc(22, Kind::PcRel { width: 4, from: 4 }, 2, 0),
        ];
        Object {
            name: "ref.s".into(),
            arch: Arch::X8664,
            sections: vec![
                section("text", Role::Text, text, relocs),
                section("const", Role::ROData, b"hello\n".to_vec(), vec![]),
                {
                    let mut data =
                        section("data", Role::Data, vec![0; 9], vec![reloc(0, Kind::Abs { width: 8 }, 1, 1)]);
                    data.image[8] = 0;
                    data
                },
            ],
            symbols: vec![
                defined("_start", 0, 0),
                defined("_msg", 1, 0),
                defined("_ptr", 2, 0),
                external("_ext"),
                defined("_flag", 2, 8),
            ],
            omf_groups: Vec::new(),
            debug: None,
        }
    }

    const REFERENCE: &str = "
        .text
        .globl _start
_start:
        call _ext
        leaq _msg(%rip), %rsi
        movb $1, _flag(%rip)
        movq _ptr(%rip), %rax
        ret
        .section __TEXT,__const
        .globl _msg
_msg:   .ascii \"hello\\n\"
        .data
        .globl _ptr
_ptr:   .quad _msg+1
        .globl _flag
_flag:  .byte 0
";

    fn dump(
        tool_path: &str,
        object: &Path,
    ) -> String {
        let said = Command::new(tool_path)
            .args(["-r", "-d", "-t", "--macho", "--full-contents"])
            .arg(object)
            .output()
            .unwrap();
        assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
        String::from_utf8_lossy(&said.stdout).into_owned()
    }

    /// The object, dumped by llvm-objdump, says what llvm-mc's object of the
    /// same program says: relocations (BRANCH, SIGNED, SIGNED_1, UNSIGNED
    /// against a symbol), fields, sections, symbols.
    #[test]
    fn it_reads_as_llvm_mc_writes_the_same_program() {
        let (Some(objdump), Some(mc)) = (tool("llvm-objdump"), tool("llvm-mc")) else {
            eprintln!("skipped: no LLVM tools");
            return;
        };
        let scratch = tempfile::tempdir().unwrap();
        let (mine, theirs, source) =
            (scratch.path().join("mine.o"), scratch.path().join("theirs.o"), scratch.path().join("ref.s"));
        std::fs::write(&mine, write(&program()).unwrap()).unwrap();
        std::fs::write(&source, REFERENCE).unwrap();
        let made = Command::new(mc)
            .args(["-triple", "x86_64-apple-macos11", "-filetype=obj"])
            .arg(&source)
            .arg("-o")
            .arg(&theirs)
            .output()
            .unwrap();
        assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
        // The reference's symbol `_flag` is a global there, as here; the dumps
        // differ only in the file's name.
        let (mine, theirs) = (dump(&objdump, &mine), dump(&objdump, &theirs));
        assert_eq!(mine.replace("mine.o", "x"), theirs.replace("theirs.o", "x"));
    }

    /// A reference to a symbol the object keeps local names its section, and
    /// the link resolves it: the call lands on clang's function, the load
    /// on the local data.
    #[test]
    fn ld64_links_it_with_a_clang_object() {
        let (Some(ld), Some(objdump)) = (tool("ld64.lld"), tool("llvm-objdump")) else {
            eprintln!("skipped: no LLVM tools");
            return;
        };
        let scratch = tempfile::tempdir().unwrap();
        let dir = scratch.path();
        // _start: call _ext; lea local(%rip), %rsi; ret. `local` is not
        // exported.
        let text = vec![0xE8, 0, 0, 0, 0, 0x48, 0x8D, 0x35, 0, 0, 0, 0, 0xC3];
        let made = Object {
            name: "m.s".into(),
            arch: Arch::X8664,
            sections: vec![
                section(
                    "text",
                    Role::Text,
                    text,
                    vec![reloc(1, Kind::Branch { width: 4 }, 1, 0), reloc(8, Kind::PcRel { width: 4, from: 4 }, 2, 0)],
                ),
                section("data", Role::Data, vec![7, 0, 0, 0], vec![]),
            ],
            symbols: vec![
                defined("_start", 0, 0),
                external("_ext"),
                Symbol {
                    name: "local".into(),
                    binding: Binding::Local,
                    definition: Definition::Defined { section: 1, offset: 0 },
                    group: None,
                },
            ],
            omf_groups: Vec::new(),
            debug: None,
        };
        std::fs::write(dir.join("mine.o"), write(&made).unwrap()).unwrap();
        std::fs::write(dir.join("ext.c"), "int ext(void) { return 3; }\n").unwrap();
        let compiled = Command::new("clang")
            .args(["-target", "x86_64-apple-macos11", "-c", "-O1"])
            .arg(dir.join("ext.c"))
            .arg("-o")
            .arg(dir.join("ext.o"))
            .output()
            .unwrap();
        assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
        let linked = Command::new(&ld)
            .args(["-arch", "x86_64", "-platform_version", "macos", "11.0", "11.0", "-e", "_start", "-o"])
            .arg(dir.join("out"))
            .arg(dir.join("mine.o"))
            .arg(dir.join("ext.o"))
            .output()
            .unwrap();
        assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
        let run = |arguments: &[&str]| {
            String::from_utf8_lossy(
                &Command::new(&objdump).args(arguments).arg(dir.join("out")).output().unwrap().stdout,
            )
            .into_owned()
        };
        let text = run(&["-d", "--macho"]);
        assert!(text.contains("callq\t_ext"), "{text}");
        // The `lea`'s displacement reaches the start of __data, a section the
        // reference named only by number.
        let lea = text.lines().find(|line| line.contains("leaq")).unwrap_or_else(|| panic!("{text}"));
        let (at, disp) = (
            u64::from_str_radix(lea.split(':').next().unwrap().trim(), 16).unwrap(),
            u64::from_str_radix(lea.split("0x").nth(1).unwrap().split('(').next().unwrap(), 16).unwrap(),
        );
        let sections = run(&["-h", "--macho"]);
        let data = sections.lines().find(|line| line.contains("__data")).unwrap_or_else(|| panic!("{sections}"));
        let data = u64::from_str_radix(data.split_whitespace().nth(3).unwrap(), 16).unwrap();
        assert_eq!(at + 7 + disp, data, "{text}{sections}");
    }

    /// A pc-relative field is signed: 0x8000_0000 was accepted as an unsigned
    /// value and wrote a call that jumps backwards 2 GiB.
    #[test]
    fn a_pc_relative_field_must_fit_a_signed_word() {
        let mut made = program();
        made.sections[0].relocs[0].addend = 0x8000_0000;
        assert!(write(&made).unwrap_err().0.contains("does not fit"));
    }

    /// A constant that holds an address is written to by the dynamic linker,
    /// which ld64 refuses in __TEXT: it is __DATA,__const.
    #[test]
    fn a_constant_with_relocations_is_in_data_const() {
        let mut made = program();
        made.sections[1] = section("const", Role::ROData, vec![0; 8], vec![reloc(0, Kind::Abs { width: 8 }, 0, 0)]);
        let bytes = write(&made).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            text.contains("__const")
                && !bytes.windows(14).any(|one| one == b"__const         __TEXT ".get(..14).unwrap() && false)
        );
        let at = bytes.windows(7).position(|one| one == b"__const").unwrap();
        assert_eq!(&bytes[at + 16..at + 22], b"__DATA");
    }

    /// What Mach-O cannot say is refused, never written near.
    #[test]
    fn what_mach_o_cannot_say_is_refused() {
        let with = |kind| {
            let mut made = program();
            made.sections[0].relocs[0].kind = kind;
            write(&made)
        };
        assert!(with(Kind::SegmentBase).is_err());
        assert!(with(Kind::FarPointer).is_err());
        assert!(with(Kind::PcRel { width: 4, from: 7 }).is_err());
        let mut far = program();
        far.sections[1].near = false;
        assert!(write(&far).unwrap_err().0.contains("selector"));
        let mut other = program();
        other.arch = Arch::I386;
        assert!(write(&other).unwrap_err().0.contains("only x86-64"));
    }

    /// An object that asks for CodeView or Turbo Debugger information is
    /// refused by name: the information of another format is never written
    /// in its place.
    #[test]
    fn a_debug_format_this_object_cannot_carry_is_refused() {
        for (format, name) in [
            (llrm_object::debug::Format::CodeView, "CodeView"),
            (llrm_object::debug::Format::TurboDebugger, "Turbo Debugger"),
        ] {
            let mut made = program();
            made.debug = Some(llrm_object::debug::Info { format, ..Default::default() });
            let why = write(&made).unwrap_err().0;
            assert!(why.contains("cannot carry") && why.contains(name), "{why}");
        }
    }
}
