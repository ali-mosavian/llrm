//! m68k as an ELF32 big-endian relocatable object: the container is
//! `llrm-elf`'s, this is the machine number, the relocation types and the
//! assembler backend (`M68kELFObjectWriter`, `M68kAsmBackend`).
//!
//! # Example
//! `jsr g` on an undefined `g` is two bytes of opcode and a 32-bit field the
//! linker fills: `R_68K_32` at offset 2, addend 0. A `bra` to a label of the
//! same function is resolved here and leaves no relocation.
//!
//! # Complexity
//! O(items): each is encoded once per layout, and a layout repeats only for
//! the branches that grow, each at most once.
//!
//! # See also
//! `llvm/lib/Target/M68k/MCTargetDesc/M68kELFObjectWriter.cpp`, pin
//! d1106deb71cc.

use llrm_elf::{Machine, Relocation};
use llrm_mir::image::{Export, Variable};
use llrm_object::assembler::AsmBackend;
use llrm_object::{
    Binding, Definition, Fix, FixupKind, FixupKindInfo, Object, Reloc, Role, Section, Symbol, Target, Unsupported,
};
use llrm_support::hash::HashMap;

use crate::compile::Item;
use crate::encode::encode;
use crate::object::relaxed;

const R_68K_32: u32 = 1;
const R_68K_16: u32 = 2;
const R_68K_8: u32 = 3;
const R_68K_PC32: u32 = 4;
const R_68K_PC16: u32 = 5;
const R_68K_PC8: u32 = 6;

struct M68kElf;

impl Machine for M68kElf {
    const BITS: u8 = 32;
    const WIDE: bool = false;
    const NUMBER: u16 = 4;
    const ADDEND: bool = true;
    const BIG_ENDIAN: bool = true;
    const MIN_SECTION_ALIGN: usize = 1;

    fn backend() -> &'static dyn AsmBackend {
        &Backend
    }

    fn relocation(reloc: &Reloc) -> Result<Relocation, Unsupported> {
        let (kind, width) = match (reloc.kind, reloc.pc_rel) {
            (FixupKind::DATA_4, false) => (R_68K_32, 4),
            (FixupKind::DATA_2, false) => (R_68K_16, 2),
            (FixupKind::DATA_1, false) => (R_68K_8, 1),
            (FixupKind::DATA_4, true) => (R_68K_PC32, 4),
            (FixupKind::DATA_2, true) => (R_68K_PC16, 2),
            (FixupKind::DATA_1, true) => (R_68K_PC8, 1),
            other => return Err(Unsupported(format!("ELF m68k has no relocation for {other:?}"))),
        };
        Ok(Relocation { kind, width })
    }
}

/// `M68kAsmBackend`: data fields, big-endian.
struct Backend;

impl AsmBackend for Backend {
    fn kind_info(
        &self,
        kind: FixupKind,
    ) -> FixupKindInfo {
        kind.info()
    }

    /// `writeNopData`: whole words of `nop`.
    fn write_nop_data(
        &self,
        out: &mut Vec<u8>,
        count: usize,
    ) -> bool {
        if count % 2 != 0 {
            return false;
        }
        for _ in 0..count / 2 {
            out.extend([0x4E, 0x71]);
        }
        true
    }

    fn apply_fixup(
        &self,
        image: &mut [u8],
        at: usize,
        kind: FixupKind,
        pc_rel: bool,
        value: i64,
        section: &str,
    ) -> Result<(), Unsupported> {
        llrm_object::apply_fixup_be(image, at, kind.width(), pc_rel, value, section)
    }
}

/// `object` as an ELF32 big-endian relocatable file.
pub fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    llrm_elf::write::<M68kElf>(object)
}

fn data_kind(size: u8) -> FixupKind {
    match size {
        1 => FixupKind::DATA_1,
        2 => FixupKind::DATA_2,
        _ => FixupKind::DATA_4,
    }
}

/// `items` as one object: a `.text` section whose labels are symbols (those
/// with a leading `.` are local). A pc-relative fixup to a label of `items` is
/// resolved here; every other fixup is a relocation. A branch whose 8-bit
/// displacement does not fit, or is 0, takes its 16-bit form and the code is
/// laid out again.
pub fn object(
    name: &str,
    items: &[Item],
) -> Result<Object, String> {
    object_with(name, items, &HashMap::default(), &[])
}

/// [`object`] with the module's globals: `.rodata`, `.data` and `.bss` hold
/// `variables`, defined as symbols the code's fixups and the variables' own
/// pointers relocate against. `exports` is how each function leaves the
/// object (public unless it says otherwise).
pub fn object_with(
    name: &str,
    items: &[Item],
    exports: &HashMap<String, Export>,
    variables: &[Variable],
) -> Result<Object, String> {
    let mut items = items.to_vec();
    loop {
        match place(name, &items, exports, variables)? {
            Placed::Wide(wide) => {
                for at in wide {
                    let Item::Inst(inst) = &mut items[at] else { continue };
                    inst.opcode = relaxed(inst.opcode).ok_or_else(|| format!("{} cannot be relaxed", inst.opcode))?;
                }
            }
            Placed::Done(object) => return Ok(object),
        }
    }
}

enum Placed {
    Wide(Vec<usize>),
    Done(Object),
}

fn place(
    name: &str,
    items: &[Item],
    exports: &HashMap<String, Export>,
    variables: &[Variable],
) -> Result<Placed, String> {
    let mut code = Vec::new();
    let mut labels: Vec<(&str, usize)> = Vec::new();
    let mut defined: HashMap<&str, usize> = HashMap::default();
    let mut pending = Vec::new();
    for (index, item) in items.iter().enumerate() {
        match item {
            Item::Label(label) => {
                if defined.insert(label, code.len()).is_some() {
                    return Err(format!("{label} is defined twice"));
                }
                labels.push((label, code.len()));
            }
            Item::Bytes(bytes) => code.extend_from_slice(bytes),
            Item::Table(labels) => {
                let encoded = crate::encode::table(labels);
                pending.push((code.len(), index, encoded.fixups));
                code.extend_from_slice(&encoded.bytes);
            }
            Item::WordTable(labels) => {
                let encoded = crate::encode::word_table(labels);
                pending.push((code.len(), index, encoded.fixups));
                code.extend_from_slice(&encoded.bytes);
            }
            Item::Loc(_) => {}
            Item::Inst(one) => {
                let encoded = encode(one)?;
                pending.push((code.len(), index, encoded.fixups.clone()));
                code.extend_from_slice(&encoded.bytes);
            }
        }
    }
    let resolved = |start: usize, fixup: &crate::encode::Fixup| -> Option<i64> {
        let target = *defined.get(fixup.symbol.as_str())?;
        fixup.pc_rel.then(|| target as i64 + fixup.addend - (start + fixup.offset) as i64)
    };
    let mut wide = Vec::new();
    for (start, index, fixups) in &pending {
        for fixup in fixups.iter().filter(|one| one.size == 1) {
            if let Some(value) = resolved(*start, fixup)
                && (!(-128..128).contains(&value) || value == 0)
            {
                wide.push(*index);
            }
        }
    }
    if !wide.is_empty() {
        return Ok(Placed::Wide(wide));
    }

    // Symbols: the labels, then the undefined names the fixups use.
    let mut symbols: Vec<Symbol> = Vec::new();
    let mut index_of: HashMap<String, usize> = HashMap::default();
    let functions: Vec<&(&str, usize)> = labels.iter().filter(|(label, _)| !label.starts_with('.')).collect();
    for (label, offset) in &labels {
        let local = label.starts_with('.');
        let size = if local {
            0
        } else {
            let at = functions.iter().position(|one| one.0 == *label).expect("a function label");
            functions.get(at + 1).map_or(code.len(), |next| next.1) - offset
        };
        index_of.insert((*label).to_owned(), symbols.len());
        symbols.push(Symbol {
            name: (*label).to_owned(),
            binding: if local { Binding::Local } else { binding_of(exports.get(*label).copied()) },
            definition: Definition::Defined { section: 0, offset: *offset },
            size,
            group: None,
            visibility: Default::default(),
        });
    }
    // The globals: one section per role, each variable a symbol in it.
    let mut data_sections: Vec<Section> = Vec::new();
    let mut data_relocs: Vec<(usize, usize, usize, i64, String)> = Vec::new();
    for (role, section_name) in [(Role::ROData, ".rodata"), (Role::Data, ".data"), (Role::Bss, ".bss")] {
        let members: Vec<&Variable> = variables.iter().filter(|one| role_of(one) == role).collect();
        if members.is_empty() {
            continue;
        }
        let (mut image, mut align) = (Vec::new(), 1);
        let section = 1 + data_sections.len();
        for one in members {
            image.resize(image.len().next_multiple_of(one.align), 0);
            let name = crate::data::symbol(&one.name);
            index_of.insert(name.clone(), symbols.len());
            symbols.push(Symbol {
                name,
                binding: binding_of(Some(one.export)),
                definition: Definition::Defined { section, offset: image.len() },
                size: one.bytes.len(),
                group: None,
                visibility: Default::default(),
            });
            for reloc in &one.relocs {
                data_relocs.push((data_sections.len(), image.len() + reloc.at, 0, reloc.addend, reloc.symbol.clone()));
            }
            align = align.max(one.align);
            image.extend(&one.bytes);
        }
        let spans = if role == Role::Bss { Vec::new() } else { vec![[0, image.len()]] };
        data_sections.push(Section {
            name: section_name.to_owned(),
            role,
            near: true,
            align,
            image,
            spans,
            relocs: Vec::new(),
        });
    }
    let undefined = |symbols: &mut Vec<Symbol>, index_of: &mut HashMap<String, usize>, name: &str| -> usize {
        *index_of
            .entry(name.to_owned())
            .or_insert_with(
                || {
                    symbols.push(Symbol {
                        name: name.to_owned(),
                        binding: Binding::Public,
                        definition: Definition::Undefined,
                        size: 0,
                        group: None,
                        visibility: Default::default(),
                    });
                    symbols.len() - 1
                },
            )
    };
    for (section, at, _, addend, symbol) in &data_relocs {
        let target = undefined(&mut symbols, &mut index_of, symbol);
        data_sections[*section]
            .relocs
            .push(
                Reloc::new(
                    *at,
                    Fix::plain(FixupKind::DATA_4),
                    Target::Symbol(target),
                    *addend,
                ),
            );
    }
    let mut relocs = Vec::new();
    for (start, _, fixups) in &pending {
        for fixup in fixups {
            let at = start + fixup.offset;
            if let Some(value) = resolved(*start, fixup) {
                llrm_object::apply_fixup_be(&mut code, at, usize::from(fixup.size), true, value, ".text")
                    .map_err(|error| error.0)?;
                continue;
            }
            let target = undefined(&mut symbols, &mut index_of, &fixup.symbol);
            let fix =
                if fixup.pc_rel { Fix::relative(data_kind(fixup.size), 0) } else { Fix::plain(data_kind(fixup.size)) };
            relocs.push(Reloc::new(at, fix, Target::Symbol(target), fixup.addend));
        }
    }
    Ok(Placed::Done(Object {
        name: name.to_owned(),
        bits: 32,
        sections: std::iter::once(Section {
            name: ".text".to_owned(),
            role: Role::Text,
            near: true,
            align: 2,
            spans: vec![[0, code.len()]],
            image: code,
            relocs,
        })
        .chain(data_sections)
        .collect(),
        symbols,
        omf_groups: Vec::new(),
        omf_comdat: Vec::new(),
        comdats: Vec::new(),
        frames: Vec::new(),
    }))
}

/// `items` as an ELF32 big-endian relocatable file.
pub fn relocatable(
    name: &str,
    items: &[Item],
) -> Result<Vec<u8>, String> {
    write(&object(name, items)?).map_err(|error| error.0)
}

/// [`relocatable`] with the module's globals; see [`object_with`].
pub fn relocatable_with(
    name: &str,
    items: &[Item],
    exports: &HashMap<String, Export>,
    variables: &[Variable],
) -> Result<Vec<u8>, String> {
    write(&object_with(name, items, exports, variables)?).map_err(|error| error.0)
}

fn role_of(variable: &Variable) -> Role {
    if variable.zero && !variable.constant {
        Role::Bss
    } else if variable.constant {
        Role::ROData
    } else {
        Role::Data
    }
}

fn binding_of(export: Option<Export>) -> Binding {
    match export {
        Some(Export::Local) => Binding::Local,
        Some(Export::Weak) => Binding::Weak,
        _ => Binding::Public,
    }
}
