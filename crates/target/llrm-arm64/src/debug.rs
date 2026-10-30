//! `-g` for arm64: the debug information of an object whose functions are
//! lowered, from the module's `!DI*` nodes (types, procedures, variables) and
//! where each function's frame puts a variable.

use llrm_codegen::backend::debuginfo::{self, Machine};
use llrm_codegen::backend::globals;
use llrm_codegen::support::hash::IndexMap;
use llrm_debug::{
    self as model, DebugOptions, DebugTarget, FrameBase, Function, Info, Kind, Language, Location, Producer, Range,
    RegisterFile, Variable,
};
use llrm_mir::debuginfo as di;
use llrm_object::Object;

use crate::emit::{self, words};
use crate::machine::Platform;

/// The arm64 machine as a debug writer reads it: 64-bit addresses and the
/// register file's DWARF numbers, the frame register being `x29`.
pub fn machine() -> Machine {
    let text = include_str!("registers.regs").to_owned();
    let file = llrm_target::registers::parse(&text).expect("the register file reads");
    let registers = file
        .iter()
        .map(|one| model::Register { name: one.name.clone(), bits: one.bits, dwarf: one.dwarf, codeview: one.codeview })
        .collect();
    let frame =
        llrm_target::registers::of_class(&file, "frame").first().map(|one| (*one).to_owned()).unwrap_or_default();
    Machine {
        near_bits: 64,
        registers_text: text,
        frame: None,
        target: DebugTarget {
            registers: std::sync::Arc::new(RegisterFile { registers, frame, return_register: String::new() }),
            ..DebugTarget::of(64)
        },
    }
}

/// The debug information of `object`, whose text section holds `functions` in
/// order; none where the module has no procedure or global described.
pub fn info(
    module: &llrm_mir::Module,
    platform: Platform,
    object: &Object,
    functions: &[emit::Function],
    source: &str,
) -> Result<Option<Info>, String> {
    let symbols: IndexMap<(llrm_codegen::model::ir::Space, i64), String> = (0..module.globals.len())
        .filter_map(|at| {
            let id = llrm_mir::GlobalId(at as u32);
            let name = module.global(id).name.owned()?;
            Some(((globals::space(module, id), i64::from(id.0)), crate::data::symbol_name(platform, &name)))
        })
        .collect();
    let machine = machine();
    let Some(debug) = debuginfo::described(module, &symbols, Producer::Native, &machine)? else { return Ok(None) };
    let id_of = |name: &str| object.symbols.iter().position(|one| one.name == name);
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
    let declared = |symbol: &str| {
        module.functions().find_map(|(_, global, body)| {
            let name = global.name.owned()?;
            (crate::data::symbol_name(platform, &name) == symbol).then(|| subprogram_line(module, body)).flatten()
        })
    };
    for function in functions {
        let length: usize = function.blocks.iter().map(|block| words(block) * 4).sum();
        let (start, symbol) = (at, id_of(&function.symbol));
        at += length;
        let (rows, body, not_stmt) = lines(function, start, declared(&function.symbol));
        info.lines.extend(rows);
        info.not_stmt.extend(not_stmt.into_iter().map(|offset| (0, offset)));
        let (Some(procedure), Some(symbol)) = (debug.procedures.get(&function.symbol), symbol) else { continue };
        let mut variables = Vec::new();
        for &(node, disp) in &function.variables {
            let Some(one) = di::read_variable(module, node) else { continue };
            let Some(&r#type) = debug.nodes.get(&one.r#type) else { continue };
            variables.push(Variable {
                name: one.name,
                r#type,
                kind: if one.parameter { Kind::Parameter } else { Kind::Local },
                location: Location::Frame { disp },
            });
        }
        // Parameters first, in the order the source declares them.
        let order = |name: &str| procedure.parameters.iter().position(|one| one == name).unwrap_or(usize::MAX);
        variables.sort_by_key(|one| (one.kind != Kind::Parameter, order(&one.name)));
        info.functions.push(Function {
            name: procedure.name.clone(),
            symbol,
            r#type: procedure.r#type,
            ranges: vec![Range { section: 0, offset: start, length }],
            body,
            far: false,
            module: procedure.module,
            variables,
            blocks: Vec::new(),
            frame: Vec::new(),
            all_calls: false,
            calls: Vec::new(),
        });
    }
    info.code = vec![Range { section: 0, offset: 0, length: at }];
    let _ = Language::default();
    Ok(Some(info))
}

/// The line a function's `!DISubprogram` says it starts at.
fn subprogram_line(
    module: &llrm_mir::Module,
    function: &llrm_mir::Function,
) -> Option<u32> {
    let (_, node) = function.metadata.iter().find(|(kind, _)| kind == "dbg")?;
    let at = module.special.get(node)?.fields.iter().position(|one| one == "line")?;
    match module.metadata.get(node.0 as usize)?.operands.get(at)? {
        llrm_mir::MetadataOperand::Constant(value) => match module.context.get(*value).kind {
            llrm_mir::ConstantKind::Int(number) => u32::try_from(number).ok(),
            _ => None,
        },
        _ => None,
    }
}

/// The line table rows of `function`, whose code starts at `start`: one where
/// the source place changes, and one at its start for the line it is declared
/// at (the prologue is from no instruction); and where its body begins and its
/// first epilogue starts, as offsets from `start`.
///
/// The pin flags the first row after the frame is built `prologue_end`, and the
/// row of the first instruction that takes the frame down `epilogue_begin`,
/// which has the place of the return that follows
/// (`DwarfDebug::beginInstruction`).
fn lines(
    function: &emit::Function,
    start: usize,
    declared: Option<u32>,
) -> (Vec<model::Line>, Option<(usize, usize)>, Vec<usize>) {
    use crate::mc::inst::{FrameMark, Inst};
    let mut out = Vec::new();
    let mut at = start;
    let mut last: Option<(u32, u32)> = None;
    let row = |offset: usize, (line, column): (u32, u32)| model::Line { section: 0, offset, file: 0, line, column };
    let (mut body_start, mut body_end) = (None, None);
    let (mut after_frame, mut taking_down) = (false, None);
    // A row starts a statement when its line is not the one before it
    // (`DwarfDebug::beginInstruction`, for a function without key
    // instructions).
    let mut previous = declared;
    let mut not_stmt = Vec::new();
    let mut push = |out: &mut Vec<model::Line>, offset: usize, place: (u32, u32)| {
        if previous == Some(place.0) {
            not_stmt.push(offset);
        }
        previous = Some(place.0);
        out.push(row(offset, place));
    };
    for block in &function.blocks {
        debug_assert_eq!(block.insts.len(), block.locs.len(), "a place for each instruction");
        for (inst, place) in block.insts.iter().zip(&block.locs) {
            match inst {
                Inst::Mark(FrameMark::PrologueEnd) => after_frame = true,
                Inst::Mark(FrameMark::EpilogueBegin) => taking_down = taking_down.or(Some(at)),
                _ => {}
            }
            if inst.is_directive() {
                continue;
            }
            if let Some(place) = *place {
                if after_frame {
                    body_start.get_or_insert(at - start);
                    after_frame = false;
                }
                if let Some(mark) = taking_down.take() {
                    // The epilogue's row has the return's place.
                    body_end.get_or_insert(mark - start);
                    if out.last().is_none_or(|one: &model::Line| one.offset != mark) {
                        push(&mut out, mark, place);
                    }
                    last = Some(place);
                }
                if last != Some(place) {
                    push(&mut out, at, place);
                    last = Some(place);
                }
            }
            at += 4;
        }
    }
    if let Some(line) = declared
        && out.first().is_none_or(|first| first.offset > start)
    {
        out.insert(0, row(start, (line, 0)));
    }
    let length = at - start;
    (out, body_start.map(|first| (first, body_end.unwrap_or(length))), not_stmt)
}
