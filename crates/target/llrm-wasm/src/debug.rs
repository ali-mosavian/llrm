//! `-g`: the DWARF sections of an object, from the module's `!DI*` nodes and
//! the source line of each encoded instruction.

use llrm_codegen::backend::debuginfo::{self, Machine};
use llrm_codegen::backend::globals;
use llrm_codegen::support::hash::IndexMap;
use llrm_debug::{self as model, DebugOptions, DebugTarget, FrameBase, Function, Info, Producer, Range};
use llrm_object::{Binding, Definition, Object, Reloc, Role, Section, Symbol, Target};

use crate::Width;
use crate::object::{DebugReloc, DebugSection, DebugTarget as At, Encoded};

/// The wasm machine as a debug writer reads it: its register file, whose frame
/// register is `FP32` (wasm has no DWARF register numbers of its own).
fn machine(bits: u32) -> Machine {
    let text = include_str!("registers.regs").to_owned();
    let file = llrm_target::registers::parse(&text).expect("the register file reads");
    let registers = file
        .iter()
        .map(|one| model::Register { name: one.name.clone(), bits: one.bits, dwarf: one.dwarf, codeview: one.codeview })
        .collect();
    let frame =
        llrm_target::registers::of_class(&file, "frame").first().map(|one| (*one).to_owned()).unwrap_or_default();
    Machine {
        near_bits: bits,
        registers_text: text,
        frame: None,
        target: DebugTarget {
            registers: std::sync::Arc::new(model::RegisterFile { registers, frame, return_register: String::new() }),
            ..DebugTarget::of(bits as u8)
        },
    }
}

/// The debug sections of `functions`, in the order of their code; none where
/// the module describes no procedure.
pub fn sections(
    module: &llrm_mir::Module,
    width: Width,
    functions: &[Encoded],
    source: &str,
) -> Result<Vec<DebugSection>, String> {
    let bits = if width == Width::W64 { 64 } else { 32 };
    let symbols: IndexMap<(llrm_codegen::model::ir::Space, i64), String> = (0..module.globals.len())
        .filter_map(|at| {
            let id = llrm_mir::GlobalId(at as u32);
            let name = module.global(id).name.owned()?;
            Some(((globals::space(module, id), i64::from(id.0)), name))
        })
        .collect();
    let machine = machine(bits);
    let Some(debug) = debuginfo::described(module, &symbols, Producer::Native, &machine)? else {
        return Ok(Vec::new());
    };

    let mut object = Object {
        name: source.to_owned(),
        bits: bits as u8,
        sections: Vec::new(),
        symbols: Vec::new(),
        omf_groups: Vec::new(),
        omf_comdat: Vec::new(),
        frames: Vec::new(),
    };
    let mut info = Info {
        options: DebugOptions {
            format: model::Format::Default,
            dialect: model::Dialect::default(),
            producer: Producer::Native,
        },
        language: debug.language,
        target: Some(debug.target.clone()),
        frame_base: FrameBase::Cfa { bias: 0, far_bias: 0 },
        files: vec![match &debug.source {
            Some((name, md5)) => {
                model::File { name: name.clone(), checksum: md5.clone().map(|bytes| (model::ChecksumKind::Md5, bytes)) }
            }
            None => model::File { name: source.to_owned(), checksum: None },
        }],
        types: debug.types.clone(),
        ..Info::default()
    };
    let mut at = 0;
    for (id, function) in functions.iter().enumerate() {
        let (start, length) = (at, function.body.len());
        at += length;
        object
            .symbols
            .push(
                Symbol {
                    name: function.name.clone(),
                    binding: Binding::Public,
                    definition: Definition::Defined { section: 0, offset: start },
                    size: length,
                    group: None,
                },
            );
        let procedure = debug.procedures.get(&function.name);
        // A row at the function's start for the line it is declared at, then
        // one where the line changes.
        let mut previous = None;
        let declared = procedure.and_then(|one| one.scope_line).map(|line| (0, line));
        for &(offset, line) in declared.iter().chain(&function.lines) {
            if previous != Some(line) {
                info.lines.push(model::Line { section: 0, offset: start + offset, file: 0, line, column: 0 });
                previous = Some(line);
            }
        }
        let Some(procedure) = procedure else { continue };
        info.functions.push(Function {
            name: procedure.name.clone(),
            symbol: id,
            r#type: procedure.r#type,
            ranges: vec![Range { section: 0, offset: start, length }],
            body: None,
            far: false,
            module: procedure.module,
            variables: Vec::new(),
            blocks: Vec::new(),
            frame: Vec::new(),
            all_calls: false,
            calls: Vec::new(),
        });
    }
    info.code = vec![Range { section: 0, offset: 0, length: at }];
    object
        .sections
        .push(
            Section {
                name: "CODE".to_owned(),
                role: Role::Text,
                near: true,
                align: 1,
                image: vec![0; at],
                spans: Vec::new(),
                relocs: Vec::new(),
            },
        );
    let expanded = llrm_dwarf::expanded_per_function(&object, &info).map_err(|why| format!("{why:?}"))?;
    // A section a relocation names is one of the debug sections, by its place
    // among them.
    let debug_index: Vec<Option<usize>> = expanded
        .sections
        .iter()
        .scan(0, |next, one| {
            Some(matches!(one.role, Role::Debug(_)).then(|| {
                *next += 1;
                *next - 1
            }))
        })
        .collect();
    Ok(expanded
        .sections
        .iter()
        .filter(|one| matches!(one.role, Role::Debug(_)))
        .map(|one| DebugSection {
            name: one.name.clone(),
            bytes: one.image.clone(),
            relocs: one.relocs.iter().map(|reloc| debug_reloc(reloc, &expanded.symbols, &debug_index)).collect(),
        })
        .collect())
}

/// A relocation of a debug section as the wasm object writer states it.
fn debug_reloc(
    reloc: &Reloc,
    symbols: &[Symbol],
    debug_index: &[Option<usize>],
) -> DebugReloc {
    let target = match reloc.target {
        Target::Symbol(at) => At::Function(symbols[at].name.clone()),
        Target::Section(at) => At::Section(debug_index[at].expect("a debug section")),
        Target::OmfGroup(_) => unreachable!("no OMF group in a wasm object"),
    };
    DebugReloc { offset: reloc.at, target, addend: reloc.addend }
}
