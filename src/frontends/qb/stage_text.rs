//! Port of `qbopt/frontend/qb/stage_text.py`: diagnostic MASM/Intel
//! projection for virtual and allocated LIR.
//!
//! Outside the backend: it prints pass state and does not participate in
//! selection, allocation, encoding, or object emission.

use iced_x86::Register;

use crate::backend::masm::InlinePart;
use crate::backend::target;
use crate::model::ir::{self, Loc, Operation};
use crate::objectfile::module::{Addr, Space};

fn size_name(width: u32) -> String {
    match width {
        1 => "byte".into(),
        2 => "word".into(),
        4 => "dword".into(),
        8 => "qword".into(),
        10 => "tbyte".into(),
        _ => format!("{width}-byte"),
    }
}

/// Render one LIR operation in Intel operand order and MASM spelling.
pub fn instruction_text(what: &ir::Semantics) -> Vec<String> {
    let name = what.name.clone().unwrap_or_default();
    let dests: Vec<String> = what.dests.iter().map(_operand).collect();
    let sources: Vec<String> = what.sources.iter().map(_operand).collect();
    let one = |text: String| vec![text];
    match what.op {
        Operation::Nothing => {
            if name != "" && name != "nop" {
                one(name)
            } else {
                vec![]
            }
        }
        Operation::Move | Operation::Address => one(format!("{name} {}, {}", dests[0], sources[0])),
        Operation::Binary => one(format!("{name} {}, {}", dests[0], sources[1])),
        Operation::Unary => one(format!("{name} {}", dests[0])),
        Operation::Compare if name.starts_with('f') => {
            let memory: Vec<&String> = sources
                .iter()
                .zip(&what.sources)
                .filter(|(_, source)| matches!(source, Loc::Mem(_)))
                .map(|(text, _)| text)
                .collect();
            one(if let Some(first) = memory.first() { format!("{name} {first}") } else { name })
        }
        Operation::Compare => {
            one(format!("{} {}, {}", if name.is_empty() { "cmp" } else { name.as_str() }, sources[0], sources[1]))
        }
        Operation::Multiply if dests.len() == 1 => {
            if sources.len() == 3 {
                return one(format!("imul {}, {}, {}", dests[0], sources[1], sources[2]));
            }
            if matches!(what.sources[1], Loc::Imm(_)) {
                return one(format!("imul {}, {}, {}", dests[0], sources[0], sources[1]));
            }
            one(format!("imul {}, {}", dests[0], sources[1]))
        }
        Operation::Multiply | Operation::Divide => one(format!("{name} {}", sources[sources.len() - 1])),
        Operation::Extend => one(if name == "movsx" || name == "movzx" {
            format!("{name} {}, {}", dests[0], sources[0])
        } else {
            name
        }),
        Operation::Push => {
            let mut suffix = "";
            if let Loc::Imm(imm) = &what.sources[0] {
                suffix = if imm.width == 4 { "d" } else { "w" };
            }
            one(format!("push{suffix} {}", sources[0]))
        }
        Operation::Pop => one(format!("pop {}", dests[0])),
        Operation::Exchange if name == "fxch" => one(format!("fxch {}", dests[1])),
        Operation::Exchange => one(format!("xchg {}, {}", dests[0], dests[1])),
        Operation::Funnel => one(format!("{name} {}, {}, {}", dests[0], sources[1], sources[2])),
        Operation::Branch | Operation::Jump => {
            one(format!("{name} L0_{}", what.target.map_or_else(|| "None".to_owned(), |one| one.to_string())))
        }
        Operation::Call if what.indirect && !sources.is_empty() => one(format!("call {}", sources[0])),
        Operation::Call => one(format!("call {name}")),
        Operation::Fill => one(format!("rep {name}")),
        Operation::Barrier => {
            let operands = if dests.is_empty() { &sources } else { &dests };
            one(if let Some(first) = operands.first() { format!("{name} {first}") } else { name })
        }
        Operation::FloatLoad => one(if name == "fldz" || name == "fld1" || sources.is_empty() {
            name
        } else {
            format!("{name} {}", sources[0])
        }),
        Operation::FloatStore => {
            let operands = if dests.is_empty() { &sources } else { &dests };
            one(if let Some(first) = operands.first() { format!("{name} {first}") } else { name })
        }
        Operation::FloatArith if matches!(what.sources.last(), Some(Loc::Mem(_))) => {
            one(format!("{name} {}", sources[sources.len() - 1]))
        }
        Operation::FloatArith | Operation::FloatArithPop => {
            one(format!("{name} {}, {}", dests[0], sources[sources.len() - 1]))
        }
        Operation::FloatUnary | Operation::Leave | Operation::Return => {
            if name.is_empty() {
                vec![]
            } else {
                one(name)
            }
        }
        Operation::Escape => one(format!(
            "jmp far ptr {}",
            if let Some(first) = sources.first() { first.clone() } else { name }
        )),
        Operation::Restore => {
            let all: Vec<String> = dests.iter().chain(&sources).cloned().collect();
            one(format!("restore {}", all.join(", ")))
        }
        Operation::Data => one(if name.is_empty() { "db ?".to_owned() } else { name }),
        // Python's `; unprintable {op} {name}` fallback: every operation is matched above.
    }
}

/// Render a finalized in-place intrinsic body as MASM data directives.
pub fn inline_text(parts: &[InlinePart]) -> Vec<String> {
    let mut lines = Vec::new();
    for part in parts {
        match part {
            InlinePart::Bytes(bytes) => {
                for chunk in bytes.chunks(16) {
                    lines.push(
                        "db ".to_owned()
                            + &chunk.iter().map(|byte| format!("0{byte:02x}h")).collect::<Vec<_>>().join(","),
                    );
                }
            }
            InlinePart::Fixup(kind, name, offset) if kind == "offset" => {
                lines.push(format!("dw offset {name}{}", _signed(*offset)));
            }
            InlinePart::Fixup(kind, name, _) if kind == "segment" => lines.push(format!("dw seg {name}")),
            InlinePart::Fixup(..) => {}
        }
    }
    lines
}

fn _operand(r#where: &Loc) -> String {
    match r#where {
        Loc::Reg(reg) => target::name_of(reg.register),
        Loc::Held(held) => format!("v{}", held.value),
        Loc::St(st) => format!("st({})", st.index),
        Loc::Imm(ir::Imm { value, address: None, .. }) => value.to_string(),
        Loc::Imm(ir::Imm { value, address: Some(address), .. }) => {
            if address.space == Space::Group {
                return _symbol(address);
            }
            format!("offset {}{}", _symbol(address), _signed(address.disp + value))
        }
        Loc::Mem(cell) => _memory(cell),
        Loc::Address(address) if address.index == Register::None && address.addr.is_some() => {
            let text = _memory(&ir::Mem::new(address.addr, 2));
            text.strip_prefix("word ptr ").map_or(text.clone(), str::to_owned)
        }
        Loc::Address(address) => {
            format!("[{}{}]", _registers(address.through, address.index, address.scale), _signed(address.offset))
        }
    }
}

fn _memory(cell: &ir::Mem) -> String {
    let size = format!("{} ptr ", size_name(cell.width));
    let address = cell.addr;
    if cell.base.is_some() || cell.index.is_some() || cell.selector.is_some() {
        let mut registers = Vec::new();
        if let Some(base) = cell.base {
            registers.push(format!("v{}", base.value));
        }
        if let Some(index) = cell.index {
            registers.push(format!("v{}", index.value) + &if cell.scale != 1 { format!("*{}", cell.scale) } else { String::new() });
        }
        let mut inside = registers.join("+");
        let displacement = address.map_or(cell.offset, |address| address.disp);
        if let Some(address) = address {
            if matches!(address.space, Space::Segment | Space::External | Space::Group) {
                inside = _symbol(&address) + &if inside.is_empty() { String::new() } else { format!("+{inside}") };
            }
        }
        let selector = cell.selector.map_or_else(String::new, |selector| format!("v{}:", selector.value));
        return format!("{size}{selector}[{inside}{}]", _signed(displacement));
    }
    let Some(address) = address else {
        let base = if cell.through != Register::None { target::name_of(cell.through) } else { "?".to_owned() };
        return format!("{size}[{base}{}]", _signed(cell.offset));
    };
    let registers = _registers(cell.through, cell.index_through, cell.scale);
    let disp = _signed(address.disp);
    match address.space {
        Space::Frame => format!("{size}[bp{disp}]"),
        Space::Segment | Space::External | Space::Group => format!(
            "{size}{}{disp}{}",
            _symbol(&address),
            if registers.is_empty() { String::new() } else { format!("[{registers}]") }
        ),
        Space::Literal => {
            let segment = if address.segment == Register::None {
                String::new()
            } else {
                format!("{}:", target::name_of(address.segment))
            };
            format!("{size}{segment}[{registers}{disp}]")
        }
        Space::Far => format!("{size}{}:[{registers}{disp}]", target::name_of(address.segment)),
        Space::Stack => format!("{size}[sp{disp}]"),
    }
}

fn _registers(base: Register, index: Register, scale: i64) -> String {
    let mut parts = if base != Register::None { vec![target::name_of(base)] } else { vec![] };
    if index != Register::None {
        parts.push(target::name_of(index) + &if scale != 1 { format!("*{scale}") } else { String::new() });
    }
    parts.join("+")
}

fn _symbol(address: &Addr) -> String {
    format!("{}_{}", address.space.value(), address.index)
}

fn _signed(number: i64) -> String {
    if number > 0 {
        format!("+{number}")
    } else if number < 0 {
        number.to_string()
    } else {
        String::new()
    }
}
