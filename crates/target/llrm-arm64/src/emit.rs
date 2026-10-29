//! The object: the functions' machine code in one text section, the globals in
//! data sections, the symbols and the relocations between them.

use std::collections::HashMap;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::{GlobalKind, Linkage, Module};
use llrm_object::assembler::{Fragment, layout as place};
use llrm_object::{Binding, Definition, Object, Reloc, Role, Section, Symbol, Target as RelocTarget};

use crate::data::{self, DataReloc};
use crate::frame::Lowered;
use crate::machine::Platform;
use crate::mc::encode::{FixupKind, encode, resolve};
use crate::mc::inst::{Inst, Target};

/// One lowered function.
pub struct Function {
    pub symbol: String,
    pub binding: Binding,
    pub blocks: Vec<Lowered>,
}

/// A function's pieces for the layout: a block's label, then its words.
enum Laid<'a> {
    Label(&'a str),
    Words(usize),
}

impl Fragment for Laid<'_> {
    type Context = ();

    fn label(&self) -> Option<&str> {
        match self {
            Laid::Label(name) => Some(name),
            Laid::Words(_) => None,
        }
    }

    fn size(
        &self,
        _: i64,
        _: &(),
    ) -> usize {
        match self {
            Laid::Label(_) => 0,
            Laid::Words(count) => count * 4,
        }
    }

    fn relax(
        &mut self,
        _: i64,
        _: &HashMap<String, i64>,
        _: &(),
    ) -> Result<bool, llrm_object::Unsupported> {
        Ok(false)
    }
}

struct Symbols {
    list: Vec<Symbol>,
    index: HashMap<String, usize>,
}

impl Symbols {
    fn defined(
        &mut self,
        name: &str,
        binding: Binding,
        section: usize,
        offset: usize,
        size: usize,
    ) {
        self.index.insert(name.to_owned(), self.list.len());
        self.list.push(Symbol {
            name: name.to_owned(),
            binding,
            definition: Definition::Defined { section, offset },
            size,
            group: None,
        });
    }

    /// The symbol `name`, undefined if nothing defined it.
    fn find(
        &mut self,
        name: &str,
    ) -> usize {
        if let Some(found) = self.index.get(name) {
            return *found;
        }
        self.index.insert(name.to_owned(), self.list.len());
        self.list.push(Symbol {
            name: name.to_owned(),
            binding: Binding::Public,
            definition: Definition::Undefined,
            size: 0,
            group: None,
        });
        self.list.len() - 1
    }
}

fn section(
    name: &str,
    role: Role,
    align: usize,
    image: Vec<u8>,
    relocs: Vec<Reloc>,
) -> Section {
    let spans = if role == Role::Bss || image.is_empty() { Vec::new() } else { vec![[0, image.len()]] };
    Section { name: name.to_owned(), role, near: true, align, image, spans, relocs }
}

/// The object for `functions` and the globals of `module`.
pub fn object(
    module: &Module,
    layout: &DataLayout,
    platform: Platform,
    name: &str,
    functions: &[Function],
) -> Result<Object, String> {
    let placed = data::globals(module, layout, platform)?;
    let mut symbols = Symbols { list: Vec::new(), index: HashMap::new() };
    // Section indices: text, then each data role that has contents.
    let text_index = 0;
    let mut text_offsets = Vec::new();
    let mut at = 0usize;
    for function in functions {
        text_offsets.push(at);
        at += function.blocks.iter().map(|block| block.insts.len() * 4).sum::<usize>();
    }
    let text_size = at;
    for (function, offset) in functions.iter().zip(&text_offsets) {
        let size = function.blocks.iter().map(|block| block.insts.len() * 4).sum();
        symbols.defined(&function.symbol, function.binding, text_index, *offset, size);
    }
    // Data sections: constants, writable data, zero-filled.
    let roles =
        [(Role::ROData, ".rodata"), (Role::Data, ".data"), (Role::Bss, ".bss"), (Role::InitFunctions, ".init_array")];
    let role_of = |one: &data::Placed| {
        if one.init_functions {
            Role::InitFunctions
        } else if one.zero && !one.constant {
            Role::Bss
        } else if one.constant {
            Role::ROData
        } else {
            Role::Data
        }
    };
    let mut data_sections: Vec<(Role, &str, Vec<u8>, Vec<(usize, &data::Placed)>, usize)> = Vec::new();
    for (role, section_name) in roles {
        let mut image = Vec::new();
        let mut members = Vec::new();
        let mut align = 1;
        for one in placed.iter().filter(|one| role_of(one) == role) {
            image.resize(image.len().next_multiple_of(one.align), 0);
            members.push((image.len(), one));
            align = align.max(one.align);
            image.extend(&one.bytes);
        }
        if !members.is_empty() {
            data_sections.push((role, section_name, image, members, align));
        }
    }
    for (offset_of_section, (_, _, _, members, _)) in data_sections.iter().enumerate() {
        for (offset, one) in members.iter().filter(|(_, one)| !one.init_functions) {
            symbols.defined(&one.name, one.binding, 1 + offset_of_section, *offset, one.bytes.len());
        }
    }
    // The code.
    let mut text = Vec::with_capacity(text_size);
    let mut text_relocs = Vec::new();
    let mut frames = Vec::new();
    for (function, function_offset) in functions.iter().zip(&text_offsets) {
        // Every instruction is a word, so nothing relaxes: the layout only
        // places the blocks' labels.
        let mut fragments = Vec::new();
        for block in &function.blocks {
            fragments.push(Laid::Label(block.label.as_str()));
            fragments.push(Laid::Words(block.insts.len()));
        }
        let placed = place(&mut fragments, &()).map_err(|error| format!("{}: {}", function.symbol, error.0))?;
        let labels: HashMap<String, usize> =
            placed.into_iter().map(|(name, at)| (name, function_offset + at as usize)).collect();
        let insts: Vec<&Inst> = function.blocks.iter().flat_map(|block| block.insts.iter()).collect();
        frames.extend(crate::mc::unwind::frame(0, *function_offset, &insts));
        let mut cursor = *function_offset;
        for block in &function.blocks {
            for inst in &block.insts {
                let encoded = encode(inst).map_err(|error| format!("{}: {inst}: {error}", function.symbol))?;
                let mut word = encoded.word;
                if let Some(fixup) = encoded.fixup {
                    match fixup.target {
                        Target::Label(label) => {
                            let Some(&target) = labels.get(label.as_str()) else {
                                return Err(format!("{}: no block {label}", function.symbol));
                            };
                            word = resolve(word, fixup.kind, target as i64 - cursor as i64)
                                .map_err(|error| format!("{}: {inst}: {error}", function.symbol))?;
                        }
                        Target::Symbol { name, addend } => {
                            let Some(fix) = fixup.kind.relocation() else {
                                return Err(format!("{}: {inst}: no relocation for {:?}", function.symbol, fixup.kind));
                            };
                            let target = RelocTarget::Symbol(symbols.find(&name));
                            text_relocs.push(Reloc::new(cursor, fix, target, addend));
                        }
                        Target::Offset(_) => unreachable!("an offset is encoded"),
                    }
                }
                text.extend(word.to_le_bytes());
                cursor += 4;
            }
        }
    }
    let _ = FixupKind::Branch26;
    let mut sections = vec![section(".text", Role::Text, 4, text, text_relocs)];
    for (role, section_name, image, members, align) in data_sections {
        let mut relocs = Vec::new();
        for (offset, one) in &members {
            for DataReloc { at, symbol, addend } in &one.relocs {
                relocs.push(Reloc::new(
                    offset + at,
                    llrm_object::Fix::abs(8),
                    RelocTarget::Symbol(symbols.find(symbol)),
                    *addend,
                ));
            }
        }
        sections.push(section(section_name, role, align, image, relocs));
    }
    let _ = (GlobalKind::Variable, Linkage::External);
    Ok(Object {
        name: name.to_owned(),
        bits: 64,
        sections,
        symbols: symbols.list,
        omf_groups: Vec::new(),
        omf_comdat: Vec::new(),
        frames,
        debug: None,
    })
}
