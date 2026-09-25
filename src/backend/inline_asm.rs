//! Intel-syntax inline assembly to 16-bit real-mode machine code.
//!
//! `MNEMONICS` says what a block may use; iced's opcode tables say which
//! forms each mnemonic has and which operands each form takes, so a new
//! instruction is one row. Of the forms that take the operands written, the
//! shortest is kept, and iced's block encoder sizes the jumps.

use std::collections::HashMap;
use std::sync::LazyLock;

use iced_x86::{
    BlockEncoder, BlockEncoderOptions, Code, CodeSize, Encoder, EncodingKind, FlowControl, Instruction,
    InstructionBlock, Mnemonic, OpCodeOperandKind as Kind, OpKind, Register,
};

use crate::abi::runtime::Reg;

/// Every mnemonic a block may use.
const MNEMONICS: &[Mnemonic] = &[
    Mnemonic::Mov,
    Mnemonic::Xchg,
    Mnemonic::In,
    Mnemonic::Out,
    Mnemonic::Int,
    Mnemonic::Push,
    Mnemonic::Pop,
    Mnemonic::Pushf,
    Mnemonic::Popf,
    Mnemonic::Cli,
    Mnemonic::Sti,
    Mnemonic::Cld,
    Mnemonic::Std,
    Mnemonic::Add,
    Mnemonic::Sub,
    Mnemonic::And,
    Mnemonic::Or,
    Mnemonic::Xor,
    Mnemonic::Cmp,
    Mnemonic::Test,
    Mnemonic::Inc,
    Mnemonic::Dec,
    Mnemonic::Shl,
    Mnemonic::Shr,
    Mnemonic::Movsb,
    Mnemonic::Movsw,
    Mnemonic::Stosb,
    Mnemonic::Stosw,
    Mnemonic::Lodsb,
    Mnemonic::Lodsw,
    Mnemonic::Insb,
    Mnemonic::Insw,
    Mnemonic::Outsb,
    Mnemonic::Outsw,
    Mnemonic::Cmpsb,
    Mnemonic::Cmpsw,
    Mnemonic::Scasb,
    Mnemonic::Scasw,
    Mnemonic::Jmp,
    Mnemonic::Je,
    Mnemonic::Jne,
    Mnemonic::Jb,
    Mnemonic::Jae,
    Mnemonic::Jbe,
    Mnemonic::Ja,
    Mnemonic::Jl,
    Mnemonic::Jge,
    Mnemonic::Jle,
    Mnemonic::Jg,
    Mnemonic::Js,
    Mnemonic::Jns,
    Mnemonic::Jo,
    Mnemonic::Jno,
    Mnemonic::Jp,
    Mnemonic::Jnp,
    Mnemonic::Loop,
];

/// Other spellings of a mnemonic above.
const ALIASES: &[(&str, &str)] = &[
    ("jz", "je"),
    ("jnz", "jne"),
    ("jc", "jb"),
    ("jnae", "jb"),
    ("jnc", "jae"),
    ("jnb", "jae"),
    ("jna", "jbe"),
    ("jnbe", "ja"),
    ("jnge", "jl"),
    ("jnl", "jge"),
    ("jng", "jle"),
    ("jnle", "jg"),
    ("jpe", "jp"),
    ("jpo", "jnp"),
    ("sal", "shl"),
];

/// The part of its 16-bit register an operand register is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Part {
    Word,
    Low,
    High,
}

/// Registers an input or output may name: the ones a call's argument can be pinned to.
const OPERANDS: &[(&str, Reg, Part)] = &[
    ("ax", Reg::Ax, Part::Word),
    ("al", Reg::Ax, Part::Low),
    ("ah", Reg::Ax, Part::High),
    ("bx", Reg::Bx, Part::Word),
    ("bl", Reg::Bx, Part::Low),
    ("bh", Reg::Bx, Part::High),
    ("cx", Reg::Cx, Part::Word),
    ("cl", Reg::Cx, Part::Low),
    ("ch", Reg::Cx, Part::High),
    ("dx", Reg::Dx, Part::Word),
    ("dl", Reg::Dx, Part::Low),
    ("dh", Reg::Dx, Part::High),
    ("si", Reg::Si, Part::Word),
    ("di", Reg::Di, Part::Word),
];

/// An input or output register: its 16-bit register and the part named.
pub fn operand_register(name: &str) -> Option<(Reg, Part)> {
    OPERANDS.iter().find(|(spelled, ..)| spelled.eq_ignore_ascii_case(name)).map(|(_, reg, part)| (*reg, *part))
}

/// A register a block may declare it changes. The rest -- sp, bp, ds, ss,
/// cs -- the program depends on, and a block restores any it changes.
pub fn clobbered(name: &str) -> Option<Reg> {
    match name.to_ascii_lowercase().as_str() {
        "es" => Some(Reg::Es),
        "flags" => Some(Reg::Flags),
        _ => operand_register(name).map(|(reg, _)| reg),
    }
}

/// A 16-bit register by the name the HIR spells it with.
pub fn named(name: &str) -> Option<Reg> {
    Reg::ALL.into_iter().find(|reg| reg.name().eq_ignore_ascii_case(name))
}

/// Why line `line` of the block does not assemble.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refusal {
    pub line: usize,
    pub message: String,
}

fn refusal<T>(line: usize, message: impl Into<String>) -> Result<T, Refusal> {
    Err(Refusal { line, message: message.into() })
}

#[derive(Clone, Debug, PartialEq)]
enum Operand {
    Register(Register),
    Immediate(i64),
    Memory(Memory),
    Label(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Memory {
    size: Option<usize>,
    segment: Option<Register>,
    base: Option<Register>,
    index: Option<Register>,
    displacement: i64,
}

/// Every register iced names, by its lower-case name.
static REGISTERS: LazyLock<HashMap<String, Register>> =
    LazyLock::new(|| Register::values().map(|one| (format!("{one:?}").to_lowercase(), one)).collect());

/// The allowed mnemonics by name, aliases included.
static SPELLED: LazyLock<HashMap<String, Mnemonic>> = LazyLock::new(|| {
    let mut spelled: HashMap<String, Mnemonic> =
        MNEMONICS.iter().map(|one| (format!("{one:?}").to_lowercase(), *one)).collect();
    for (alias, name) in ALIASES {
        let mnemonic = spelled[*name];
        spelled.insert((*alias).to_owned(), mnemonic);
    }
    spelled
});

/// Each allowed mnemonic's forms a 16-bit block can encode.
static FORMS: LazyLock<HashMap<Mnemonic, Vec<Code>>> = LazyLock::new(|| {
    let mut forms: HashMap<Mnemonic, Vec<Code>> = HashMap::new();
    for code in Code::values() {
        let info = code.op_code();
        let usable = info.encoding() == EncodingKind::Legacy
            && info.is_instruction()
            && info.mode16()
            && matches!(info.operand_size(), 0 | 16)
            && matches!(info.address_size(), 0 | 16)
            // Control leaves the block only by falling out of its end.
            && matches!(
                code.flow_control(),
                FlowControl::Next | FlowControl::UnconditionalBranch | FlowControl::ConditionalBranch | FlowControl::Interrupt
            );
        if usable && MNEMONICS.contains(&code.mnemonic()) {
            forms.entry(code.mnemonic()).or_default().push(code);
        }
    }
    forms
});

fn register(name: &str, line: usize) -> Result<Option<Register>, Refusal> {
    let Some(register) = REGISTERS.get(&name.to_lowercase()).copied() else {
        return Ok(None);
    };
    if register.is_gpr8() || register.is_gpr16() || matches!(register, Register::ES | Register::CS | Register::SS | Register::DS)
    {
        return Ok(Some(register));
    }
    refusal(line, format!("register {name} is not available: the block is 16-bit code"))
}

/// `123`, `-5`, `0x1A`, `1Ah`, `0b101` or `'A'`.
fn number(text: &str) -> Option<i64> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, text),
    };
    let lower = digits.to_ascii_lowercase();
    let value = if let [b'\'', character, b'\''] = digits.as_bytes() {
        i64::from(*character)
    } else if let Some(hex) = lower.strip_suffix('h').filter(|one| one.starts_with(|first: char| first.is_ascii_digit())) {
        i64::from_str_radix(hex, 16).ok()?
    } else if let Some(hex) = lower.strip_prefix("0x") {
        i64::from_str_radix(hex, 16).ok()?
    } else if let Some(binary) = lower.strip_prefix("0b") {
        i64::from_str_radix(binary, 2).ok()?
    } else {
        lower.parse().ok()?
    };
    Some(if negative { -value } else { value })
}

fn is_label(name: &str) -> bool {
    name.starts_with(|first: char| first.is_ascii_alphabetic() || first == '_' || first == '.')
        && name.chars().all(|one| one.is_ascii_alphanumeric() || matches!(one, '_' | '.'))
}

fn memory(text: &str, line: usize) -> Result<Memory, Refusal> {
    let mut rest = text.trim();
    let mut made = Memory::default();
    let lower = rest.to_ascii_lowercase();
    for (word, size) in [("byte", 1), ("word", 2)] {
        if lower.starts_with(word) && !lower[word.len()..].starts_with(|one: char| one.is_ascii_alphanumeric()) {
            made.size = Some(size);
            rest = rest[word.len()..].trim_start();
            if rest.to_ascii_lowercase().starts_with("ptr") {
                rest = rest[3..].trim_start();
            }
        }
    }
    if let Some((segment, after)) = rest.split_once(':').filter(|(segment, _)| !segment.contains('[')) {
        let segment = register(segment.trim(), line)?.filter(|one| one.is_segment_register()).ok_or_else(|| Refusal {
            line,
            message: format!("{} is not a segment register", segment.trim()),
        })?;
        made.segment = Some(segment);
        rest = after.trim_start();
    }
    let Some(inside) = rest.strip_prefix('[').and_then(|one| one.strip_suffix(']')) else {
        return refusal(line, format!("malformed memory operand '{text}'"));
    };
    let mut terms = Vec::new();
    let mut sign = 1;
    let mut start = 0;
    for (at, character) in inside.char_indices().chain([(inside.len(), '+')]) {
        if matches!(character, '+' | '-') {
            let term = inside[start..at].trim();
            if !term.is_empty() {
                terms.push((sign, term));
            } else if at != 0 {
                return refusal(line, format!("malformed memory operand '{text}'"));
            }
            sign = if character == '-' { -1 } else { 1 };
            start = at + 1;
        }
    }
    for (sign, term) in terms {
        if let Some(value) = number(term) {
            made.displacement += sign * value;
            continue;
        }
        let register = register(term, line)?;
        let slot = match register {
            Some(Register::BX | Register::BP) if sign > 0 && made.base.is_none() => &mut made.base,
            Some(Register::SI | Register::DI) if sign > 0 && made.index.is_none() => &mut made.index,
            _ => return refusal(line, format!("'{text}' is not a 16-bit address: [bx|bp + si|di + number]")),
        };
        *slot = register;
    }
    if made.base.is_none() {
        // iced spells a lone si or di as the base.
        made.base = made.index.take();
    }
    Ok(made)
}

fn operand(text: &str, line: usize) -> Result<Operand, Refusal> {
    let text = text.trim();
    if text.contains('[') {
        return Ok(Operand::Memory(memory(text, line)?));
    }
    if let Some(register) = register(text, line)? {
        return Ok(Operand::Register(register));
    }
    if let Some(value) = number(text) {
        return Ok(Operand::Immediate(value));
    }
    if is_label(text) {
        return Ok(Operand::Label(text.to_owned()));
    }
    refusal(line, format!("malformed operand '{text}'"))
}

/// Operands a string instruction names by its mnemonic alone.
fn implicit(kind: Kind) -> Option<OpKind> {
    match kind {
        Kind::seg_rSI => Some(OpKind::MemorySegSI),
        Kind::es_rDI => Some(OpKind::MemoryESDI),
        Kind::seg_rDI => Some(OpKind::MemorySegDI),
        _ => None,
    }
}

/// `operand` in slot `at` of `instruction`, when `kind` takes it.
fn placed(instruction: &mut Instruction, at: u32, kind: Kind, operand: &Operand, bytes: usize) -> bool {
    match operand {
        Operand::Register(register) => {
            let fits = match kind {
                Kind::r8_reg | Kind::r8_or_mem | Kind::r8_opcode => register.is_gpr8(),
                Kind::r16_reg | Kind::r16_or_mem | Kind::r16_opcode | Kind::r16_rm | Kind::r16_reg_mem => {
                    register.is_gpr16()
                }
                Kind::seg_reg => register.is_segment_register(),
                Kind::al => *register == Register::AL,
                Kind::cl => *register == Register::CL,
                Kind::ax => *register == Register::AX,
                Kind::dx => *register == Register::DX,
                Kind::es => *register == Register::ES,
                Kind::cs => *register == Register::CS,
                Kind::ss => *register == Register::SS,
                Kind::ds => *register == Register::DS,
                _ => false,
            };
            instruction.set_op_kind(at, OpKind::Register);
            instruction.set_op_register(at, *register);
            fits
        }
        Operand::Immediate(value) => {
            let value = *value;
            let (fits, kind) = match kind {
                Kind::imm8 => ((-0x80..=0xFF).contains(&value), OpKind::Immediate8),
                Kind::imm8_const_1 => (value == 1, OpKind::Immediate8),
                Kind::imm16 => ((-0x8000..=0xFFFF).contains(&value), OpKind::Immediate16),
                Kind::imm8sex16 => ((-0x80..=0x7F).contains(&value) || (0xFF80..=0xFFFF).contains(&value), OpKind::Immediate8to16),
                _ => (false, OpKind::Immediate8),
            };
            instruction.set_op_kind(at, kind);
            match kind {
                OpKind::Immediate16 => instruction.set_immediate16(value as u16),
                OpKind::Immediate8to16 => instruction.set_immediate8to16(i16::from(value as i8)),
                _ => instruction.set_immediate8(value as u8),
            }
            fits
        }
        Operand::Memory(memory) => {
            let sized = match kind {
                Kind::r8_or_mem => 1,
                Kind::r16_or_mem => 2,
                Kind::mem_offs if memory.base.is_none() => bytes,
                _ => return false,
            };
            instruction.set_op_kind(at, OpKind::Memory);
            instruction.set_memory_base(memory.base.unwrap_or(Register::None));
            instruction.set_memory_index(memory.index.unwrap_or(Register::None));
            instruction.set_memory_index_scale(1);
            instruction.set_segment_prefix(memory.segment.unwrap_or(Register::None));
            let displacement = memory.displacement as i16;
            instruction.set_memory_displacement32(displacement as u16 as u32);
            instruction.set_memory_displ_size(if memory.base.is_none() {
                2
            } else if displacement == 0 {
                0
            } else if i8::try_from(displacement).is_ok() {
                1
            } else {
                2
            });
            (-0x8000..=0xFFFF).contains(&memory.displacement) && memory.size.is_none_or(|size| size == sized) && sized == bytes
        }
        Operand::Label(_) => {
            instruction.set_op_kind(at, OpKind::NearBranch16);
            matches!(kind, Kind::br16_1 | Kind::br16_2)
        }
    }
}

/// `code` with `operands`, when it takes them.
fn built(code: Code, operands: &[Operand]) -> Option<Instruction> {
    let info = code.op_code();
    let kinds = info.op_kinds();
    let mut instruction = Instruction::default();
    instruction.set_code(code);
    instruction.set_code_size(CodeSize::Code16);
    if kinds.iter().any(|kind| implicit(*kind).is_some()) {
        if !operands.is_empty() {
            return None;
        }
        for (at, kind) in kinds.iter().enumerate() {
            match implicit(*kind) {
                Some(memory) => instruction.set_op_kind(at as u32, memory),
                None => {
                    let register = match kind {
                        Kind::al => Register::AL,
                        Kind::ax => Register::AX,
                        Kind::dx => Register::DX,
                        _ => return None,
                    };
                    instruction.set_op_kind(at as u32, OpKind::Register);
                    instruction.set_op_register(at as u32, register);
                }
            }
        }
        return Some(instruction);
    }
    if kinds.len() != operands.len() {
        return None;
    }
    let bytes = info.memory_size().size();
    kinds
        .iter()
        .zip(operands)
        .enumerate()
        .all(|(at, (kind, operand))| placed(&mut instruction, at as u32, *kind, operand, bytes))
        .then_some(instruction)
}

fn length(instruction: &Instruction) -> usize {
    let mut probe = *instruction;
    if probe.op0_kind() == OpKind::NearBranch16 {
        probe.set_near_branch16(0x100);
    }
    Encoder::new(16).encode(&probe, 0x100).unwrap_or(usize::MAX)
}

/// One line's instruction and the label it jumps to.
fn instruction(text: &str, line: usize) -> Result<(Instruction, Option<String>), Refusal> {
    let (mut head, mut rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let mut prefix = None;
    if matches!(head.to_ascii_lowercase().as_str(), "rep" | "repe" | "repz" | "repne" | "repnz") {
        prefix = Some(head.to_ascii_lowercase());
        (head, rest) = rest.trim().split_once(char::is_whitespace).unwrap_or((rest.trim(), ""));
    }
    let Some(mnemonic) = SPELLED.get(&head.to_ascii_lowercase()).copied() else {
        return refusal(line, format!("'{head}' is not an instruction inline assembly supports"));
    };
    let operands = if rest.trim().is_empty() {
        Vec::new()
    } else {
        rest.split(',').map(|one| operand(one, line)).collect::<Result<Vec<_>, _>>()?
    };
    let fitting: Vec<Instruction> =
        FORMS.get(&mnemonic).into_iter().flatten().filter_map(|code| built(*code, &operands)).collect();
    let sizeless = operands.iter().any(|one| matches!(one, Operand::Memory(Memory { size: None, .. })));
    let sizes: std::collections::BTreeSet<usize> = fitting.iter().map(|one| one.op_code().memory_size().size()).collect();
    if sizeless && sizes.len() > 1 {
        return refusal(line, format!("say 'byte' or 'word': the size of {head}'s memory operand is ambiguous"));
    }
    let Some(mut chosen) = fitting.into_iter().min_by_key(length) else {
        return refusal(line, format!("no form of '{head}' takes the operands '{}'", rest.trim()));
    };
    match prefix.as_deref() {
        None => {}
        Some("rep" | "repe" | "repz") if chosen.op_code().can_use_rep_prefix() => chosen.set_has_rep_prefix(true),
        Some("repne" | "repnz") if chosen.op_code().can_use_repne_prefix() => chosen.set_has_repne_prefix(true),
        Some(prefix) => return refusal(line, format!("'{prefix}' does not apply to '{head}'")),
    }
    let target = operands.iter().find_map(|one| match one {
        Operand::Label(name) => Some(name.clone()),
        _ => None,
    });
    Ok((chosen, target))
}

/// The machine code of `lines`, one statement each; `;` starts a comment.
pub fn assembled(lines: &[&str]) -> Result<Vec<u8>, Refusal> {
    let mut instructions: Vec<(Instruction, Option<String>, usize)> = Vec::new();
    // Each label's instruction, by index; the end of the block is one past the last.
    let mut labels: HashMap<String, usize> = HashMap::new();
    for (line, text) in lines.iter().enumerate() {
        let mut text = text.split(';').next().unwrap_or("").trim();
        while let Some((label, after)) = text.split_once(':').filter(|(label, _)| is_label(label.trim())) {
            let label = label.trim();
            if REGISTERS.contains_key(&label.to_lowercase()) {
                break;
            }
            if labels.insert(label.to_lowercase(), instructions.len()).is_some() {
                return refusal(line, format!("label '{label}' is defined twice"));
            }
            text = after.trim();
        }
        if text.is_empty() {
            continue;
        }
        let (made, target) = instruction(text, line)?;
        instructions.push((made, target, line));
    }
    // An instruction's IP names it: a branch targets the IP of the one after its label.
    let ip = |index: usize| 0x10 * (index as u64 + 1);
    let end = instructions.len();
    let mut block = Vec::with_capacity(end + 1);
    for (index, (mut made, target, line)) in instructions.into_iter().enumerate() {
        made.set_ip(ip(index));
        if let Some(target) = target {
            let Some(at) = labels.get(&target.to_lowercase()) else {
                return refusal(line, format!("unknown label '{target}'"));
            };
            made.set_near_branch16(ip(*at) as u16);
        }
        block.push(made);
    }
    // A label at the very end targets a placeholder, dropped once encoded.
    let mut placeholder = Instruction::with(Code::Nopw);
    placeholder.set_ip(ip(end));
    block.push(placeholder);
    let encoded = BlockEncoder::encode(16, InstructionBlock::new(&block, 0), BlockEncoderOptions::RETURN_NEW_INSTRUCTION_OFFSETS)
        .map_err(|error| Refusal { line: lines.len().saturating_sub(1), message: error.to_string() })?;
    let mut code = encoded.code_buffer;
    code.truncate(encoded.new_instruction_offsets[end] as usize);
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(lines: &[&str]) -> Vec<u8> {
        assembled(lines).unwrap_or_else(|refusal| panic!("line {}: {}", refusal.line, refusal.message))
    }

    fn refused(lines: &[&str]) -> String {
        assembled(lines).expect_err("refused").message
    }

    #[test]
    fn each_form_is_the_shortest_that_takes_its_operands() {
        assert_eq!(bytes(&["mov al, 0B6h"]), [0xB0, 0xB6]);
        assert_eq!(bytes(&["out 43h, al"]), [0xE6, 0x43]);
        assert_eq!(bytes(&["in al, dx", "out dx, al"]), [0xEC, 0xEE]);
        assert_eq!(bytes(&["int 1Ah"]), [0xCD, 0x1A]);
        assert_eq!(bytes(&["add bx, 5"]), [0x83, 0xC3, 0x05]);
        assert_eq!(bytes(&["add ax, 1234h"]), [0x05, 0x34, 0x12]);
        assert_eq!(bytes(&["shl ax, 1", "shr dx, cl"]), [0xD1, 0xE0, 0xD3, 0xEA]);
        assert_eq!(bytes(&["xchg bx, ax"]), [0x93]);
        assert_eq!(bytes(&["push es", "pop ds", "pushf", "popf", "cli", "sti"]), [0x06, 0x1F, 0x9C, 0x9D, 0xFA, 0xFB]);
        assert_eq!(bytes(&["mov es, ax"]), [0x8E, 0xC0]);
    }

    #[test]
    fn memory_operands_take_16_bit_addresses_and_a_size_where_nothing_else_says_one() {
        assert_eq!(bytes(&["mov ax, [bx+si+2]"]), [0x8B, 0x40, 0x02]);
        assert_eq!(bytes(&["mov ax, [bx-2]"]), [0x8B, 0x47, 0xFE]);
        assert_eq!(bytes(&["mov al, es:[46Ch]"]), [0x26, 0xA0, 0x6C, 0x04]);
        assert_eq!(bytes(&["inc word ptr [di]"]), [0xFF, 0x05]);
        assert!(refused(&["inc [di]"]).contains("ambiguous"));
        assert!(refused(&["mov ax, [ax]"]).contains("16-bit address"));
    }

    #[test]
    fn string_instructions_take_a_rep_prefix() {
        assert_eq!(bytes(&["cld", "rep stosb", "rep outsw", "repne scasb"]), [0xFC, 0xF3, 0xAA, 0xF3, 0x6F, 0xF2, 0xAE]);
        assert!(refused(&["rep mov ax, bx"]).contains("does not apply"));
    }

    #[test]
    fn a_jump_is_short_unless_its_label_is_out_of_reach() {
        assert_eq!(bytes(&["again: dec cx", "jnz again"]), [0x49, 0x75, 0xFD]);
        assert_eq!(bytes(&["jmp done", "nop_free: inc ax", "done:"]), [0xEB, 0x01, 0x40]);
        let far: Vec<&str> = std::iter::once("je done").chain(std::iter::repeat_n("mov ax, 1234h", 50)).chain(["done:"]).collect();
        assert_eq!(bytes(&far)[..4], [0x0F, 0x84, 0x96, 0x00]);
    }

    #[test]
    fn anything_else_is_refused_by_name() {
        assert!(refused(&["hlt"]).contains("'hlt' is not an instruction"));
        assert!(refused(&["mov eax, 1"]).contains("register eax is not available"));
        assert!(refused(&["mov al, 300"]).contains("no form of 'mov'"));
        assert!(refused(&["jmp nowhere"]).contains("unknown label 'nowhere'"));
        assert!(refused(&["jmp ax"]).contains("no form of 'jmp'"));
        assert_eq!(assembled(&["", "x: dec cx", "x: inc cx"]).unwrap_err().line, 2);
    }
}
