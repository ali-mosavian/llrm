//! Deterministic textual assembly for target-independent Machine IR.
//!
//! The format is deliberately a small, line-oriented interchange format.  It
//! uses decimal identifiers and hexadecimal UTF-8 for names, so it needs no
//! escaping rules and remains stable across platforms.

use std::error::Error;
use std::fmt;

use super::{
    FrameIndex, FrameObject, FrameObjectKind, InstructionFlags, MachineBlock, MachineBlockId,
    MachineFunction, MachineFunctionId, MachineInstruction, MachineInstructionId, MachineModule,
    MachineOperand, MachineOperandKind, MachineRegister, OperandIndex, OperandRole,
    PhysicalRegister, RegisterClass, RegisterConstraint, TargetOpcode, VirtualRegister,
    VirtualRegisterId,
};

/// Version of the `.qmir` textual format.
pub const FORMAT_VERSION: u32 = 1;

/// A syntax or value error in `.qmir` text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextError {
    /// One-based source line.
    pub line: usize,
    /// One-based byte column.
    pub column: usize,
    /// Human-readable description of the malformed construct.
    pub message: String,
}

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "line {}, column {}: {}",
            self.line, self.column, self.message
        )
    }
}

impl Error for TextError {}

/// Writes one canonical `.qmir` document.
pub fn write_text(module: &MachineModule) -> String {
    let mut text = format!("qmir {FORMAT_VERSION}\n");
    for function in &module.functions {
        line(
            &mut text,
            &[
                "function".to_owned(),
                function.id.get().to_string(),
                encode_name(&function.name),
            ],
        );
        for register in &function.virtual_registers {
            line(
                &mut text,
                &[
                    "vreg".to_owned(),
                    register.id.get().to_string(),
                    register.class.get().to_string(),
                ],
            );
        }
        for object in &function.frame_objects {
            line(
                &mut text,
                &[
                    "frame".to_owned(),
                    object.index.get().to_string(),
                    object.size.to_string(),
                    object.alignment.to_string(),
                    frame_kind_name(object.kind).to_owned(),
                ],
            );
        }
        for block in &function.blocks {
            let mut fields = vec![
                "block".to_owned(),
                block.id.get().to_string(),
                block.successors.len().to_string(),
            ];
            fields.extend(block.successors.iter().map(|id| id.get().to_string()));
            line(&mut text, &fields);
            for instruction in &block.instructions {
                line(
                    &mut text,
                    &[
                        "inst".to_owned(),
                        instruction.id.get().to_string(),
                        instruction.opcode.get().to_string(),
                        flags_bits(instruction.flags).to_string(),
                        instruction.operands.len().to_string(),
                    ],
                );
                for operand in &instruction.operands {
                    write_operand(&mut text, operand);
                }
            }
            line(&mut text, &["endblock".to_owned()]);
        }
        line(&mut text, &["endfunction".to_owned()]);
    }
    text
}

/// Parses one `.qmir` document without applying Machine IR verification.
///
/// This keeps text round-tripping useful while constructing invalid modules
/// for verifier tests.  Call [`MachineModule::verify`](super::MachineModule::verify)
/// when a structurally valid module is required.
pub fn parse_text(source: &str) -> Result<MachineModule, TextError> {
    let mut parser = Parser::new(source);
    let header = parser.next()?;
    expect_exact(&header, &["qmir", "1"])?;

    let mut functions = Vec::new();
    while let Some(line) = parser.peek() {
        let tokens = tokenize(line)?;
        match tokens.first().map(|token| token.value) {
            Some("function") => {
                let line = parser.next()?;
                functions.push(parser.parse_function(line)?);
            }
            Some(_) => return Err(unexpected(&tokens[0], "`function` or end of file")),
            None => return Err(error(line.number, 1, "blank lines are not permitted")),
        }
    }
    Ok(MachineModule { functions })
}

fn line(text: &mut String, fields: &[String]) {
    text.push_str(&fields.join(" "));
    text.push('\n');
}

fn write_operand(text: &mut String, operand: &MachineOperand) {
    let mut fields = vec![
        "operand".to_owned(),
        role_name(operand.role).to_owned(),
        constraint_name(operand.constraint),
        operand
            .tied_to
            .map(|index| index.get().to_string())
            .unwrap_or_else(|| "-".to_owned()),
    ];
    match &operand.kind {
        MachineOperandKind::Register(MachineRegister::Virtual(id)) => {
            fields.extend(["vreg".to_owned(), id.get().to_string()]);
        }
        MachineOperandKind::Register(MachineRegister::Physical(id)) => {
            fields.extend(["preg".to_owned(), id.get().to_string()]);
        }
        MachineOperandKind::Immediate(value) => {
            fields.extend(["imm".to_owned(), value.to_string()]);
        }
        MachineOperandKind::FrameIndex { index, addend } => {
            fields.extend([
                "frame-index".to_owned(),
                index.get().to_string(),
                addend.to_string(),
            ]);
        }
        MachineOperandKind::Block(id) => {
            fields.extend(["block".to_owned(), id.get().to_string()]);
        }
        MachineOperandKind::Global { name, addend } => {
            fields.extend(["global".to_owned(), encode_name(name), addend.to_string()]);
        }
        MachineOperandKind::ExternalSymbol { name, addend } => {
            fields.extend(["external".to_owned(), encode_name(name), addend.to_string()]);
        }
    }
    line(text, &fields);
}

fn flags_bits(flags: InstructionFlags) -> u8 {
    u8::from(flags.terminator)
        | (u8::from(flags.call) << 1)
        | (u8::from(flags.copy) << 2)
        | (u8::from(flags.side_effects) << 3)
        | (u8::from(flags.may_load) << 4)
        | (u8::from(flags.may_store) << 5)
        | (u8::from(flags.volatile) << 6)
}

fn parse_flags(token: Token<'_>) -> Result<InstructionFlags, TextError> {
    let bits = number::<u8>(token)?;
    if bits & !0x7f != 0 {
        return Err(error(
            token.line,
            token.column,
            "unknown instruction flag bit",
        ));
    }
    Ok(InstructionFlags {
        terminator: bits & 1 != 0,
        call: bits & 2 != 0,
        copy: bits & 4 != 0,
        side_effects: bits & 8 != 0,
        may_load: bits & 16 != 0,
        may_store: bits & 32 != 0,
        volatile: bits & 64 != 0,
    })
}

fn frame_kind_name(kind: FrameObjectKind) -> &'static str {
    match kind {
        FrameObjectKind::Local => "local",
        FrameObjectKind::Spill => "spill",
        FrameObjectKind::OutgoingArgument => "outgoing-argument",
    }
}

fn role_name(role: OperandRole) -> &'static str {
    match role {
        OperandRole::None => "none",
        OperandRole::Use => "use",
        OperandRole::Def => "def",
        OperandRole::UseDef => "use-def",
    }
}

fn constraint_name(constraint: Option<RegisterConstraint>) -> String {
    match constraint {
        None => "-".to_owned(),
        Some(RegisterConstraint::Fixed(register)) => format!("fixed:{}", register.get()),
        Some(RegisterConstraint::Class(class)) => format!("class:{}", class.get()),
    }
}

fn encode_name(name: &str) -> String {
    if name.is_empty() {
        return "-".to_owned();
    }
    let mut result = String::with_capacity(name.len() * 2);
    for byte in name.bytes() {
        use fmt::Write as _;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

fn decode_name(token: Token<'_>) -> Result<String, TextError> {
    if token.value == "-" {
        return Ok(String::new());
    }
    if token.value.len() % 2 != 0 {
        return Err(error(token.line, token.column, "hex name has odd length"));
    }
    if !token.value.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(error(
            token.line,
            token.column,
            "name must contain hexadecimal UTF-8 bytes",
        ));
    }
    let mut bytes = Vec::with_capacity(token.value.len() / 2);
    for offset in (0..token.value.len()).step_by(2) {
        let pair = &token.value[offset..offset + 2];
        let byte = u8::from_str_radix(pair, 16).map_err(|_| {
            error(
                token.line,
                token.column + offset,
                "name must contain hexadecimal UTF-8 bytes",
            )
        })?;
        bytes.push(byte);
    }
    String::from_utf8(bytes).map_err(|_| error(token.line, token.column, "name is not valid UTF-8"))
}

#[derive(Clone, Copy)]
struct SourceLine<'a> {
    number: usize,
    text: &'a str,
}

#[derive(Clone, Copy)]
struct Token<'a> {
    line: usize,
    column: usize,
    value: &'a str,
}

struct Parser<'a> {
    lines: Vec<SourceLine<'a>>,
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        let source_lines = source.split('\n').collect::<Vec<_>>();
        let source_line_count = source_lines.len();
        Self {
            lines: source_lines
                .into_iter()
                .enumerate()
                .filter_map(|(index, text)| {
                    if index + 1 == source_line_count && text.is_empty() {
                        None
                    } else {
                        Some(SourceLine {
                            number: index + 1,
                            text: text.strip_suffix('\r').unwrap_or(text),
                        })
                    }
                })
                .collect(),
            position: 0,
        }
    }

    fn next(&mut self) -> Result<SourceLine<'a>, TextError> {
        let Some(line) = self.lines.get(self.position).copied() else {
            return Err(error(self.lines.len() + 1, 1, "unexpected end of file"));
        };
        self.position += 1;
        Ok(line)
    }

    fn peek(&self) -> Option<SourceLine<'a>> {
        self.lines.get(self.position).copied()
    }

    fn parse_function(&mut self, source: SourceLine<'a>) -> Result<MachineFunction, TextError> {
        let tokens = tokenize(source)?;
        require_len(&tokens, 3)?;
        expect_value(tokens[0], "function")?;
        let id = MachineFunctionId::new(number(tokens[1])?);
        let name = decode_name(tokens[2])?;
        let mut virtual_registers = Vec::new();
        let mut blocks = Vec::new();
        let mut frame_objects = Vec::new();

        loop {
            let line = self.next()?;
            let tokens = tokenize(line)?;
            let Some(first) = tokens.first() else {
                return Err(error(line.number, 1, "blank lines are not permitted"));
            };
            match first.value {
                "vreg" => virtual_registers.push(parse_vreg(&tokens)?),
                "frame" => frame_objects.push(parse_frame(&tokens)?),
                "block" => blocks.push(self.parse_block(line)?),
                "endfunction" => {
                    require_len(&tokens, 1)?;
                    break;
                }
                _ => {
                    return Err(unexpected(
                        first,
                        "`vreg`, `frame`, `block`, or `endfunction`",
                    ));
                }
            }
        }

        Ok(MachineFunction {
            id,
            name,
            virtual_registers,
            blocks,
            frame_objects,
        })
    }

    fn parse_block(&mut self, source: SourceLine<'a>) -> Result<MachineBlock, TextError> {
        let tokens = tokenize(source)?;
        require_at_least(&tokens, 3)?;
        expect_value(tokens[0], "block")?;
        let id = MachineBlockId::new(number(tokens[1])?);
        let count = number::<usize>(tokens[2])?;
        if count.checked_add(3) != Some(tokens.len()) {
            return Err(error(
                tokens[2].line,
                tokens[2].column,
                "successor count does not match the number of successor ids",
            ));
        }
        let successors = tokens[3..]
            .iter()
            .copied()
            .map(number::<u32>)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(MachineBlockId::new)
            .collect();
        let mut instructions = Vec::new();

        loop {
            let line = self.next()?;
            let tokens = tokenize(line)?;
            let Some(first) = tokens.first() else {
                return Err(error(line.number, 1, "blank lines are not permitted"));
            };
            match first.value {
                "inst" => instructions.push(self.parse_instruction(line)?),
                "endblock" => {
                    require_len(&tokens, 1)?;
                    break;
                }
                _ => return Err(unexpected(first, "`inst` or `endblock`")),
            }
        }

        Ok(MachineBlock {
            id,
            instructions,
            successors,
        })
    }

    fn parse_instruction(
        &mut self,
        source: SourceLine<'a>,
    ) -> Result<MachineInstruction, TextError> {
        let tokens = tokenize(source)?;
        require_len(&tokens, 5)?;
        expect_value(tokens[0], "inst")?;
        let id = MachineInstructionId::new(number(tokens[1])?);
        let opcode = TargetOpcode::new(number(tokens[2])?);
        let flags = parse_flags(tokens[3])?;
        let count = number::<usize>(tokens[4])?;
        let mut operands = Vec::new();
        for _ in 0..count {
            let line = self.next()?;
            operands.push(parse_operand(&tokenize(line)?)?);
        }
        Ok(MachineInstruction {
            id,
            opcode,
            operands,
            flags,
        })
    }
}

fn parse_vreg(tokens: &[Token<'_>]) -> Result<VirtualRegister, TextError> {
    require_len(tokens, 3)?;
    expect_value(tokens[0], "vreg")?;
    Ok(VirtualRegister {
        id: VirtualRegisterId::new(number(tokens[1])?),
        class: RegisterClass::new(number(tokens[2])?),
    })
}

fn parse_frame(tokens: &[Token<'_>]) -> Result<FrameObject, TextError> {
    require_len(tokens, 5)?;
    expect_value(tokens[0], "frame")?;
    let kind = match tokens[4].value {
        "local" => FrameObjectKind::Local,
        "spill" => FrameObjectKind::Spill,
        "outgoing-argument" => FrameObjectKind::OutgoingArgument,
        _ => return Err(unexpected(&tokens[4], "a frame object kind")),
    };
    Ok(FrameObject {
        index: FrameIndex::new(number(tokens[1])?),
        size: number(tokens[2])?,
        alignment: number(tokens[3])?,
        kind,
    })
}

fn parse_operand(tokens: &[Token<'_>]) -> Result<MachineOperand, TextError> {
    require_at_least(tokens, 5)?;
    expect_value(tokens[0], "operand")?;
    let role = match tokens[1].value {
        "none" => OperandRole::None,
        "use" => OperandRole::Use,
        "def" => OperandRole::Def,
        "use-def" => OperandRole::UseDef,
        _ => return Err(unexpected(&tokens[1], "an operand role")),
    };
    let constraint = parse_constraint(tokens[2])?;
    let tied_to = if tokens[3].value == "-" {
        None
    } else {
        Some(OperandIndex::new(number(tokens[3])?))
    };
    let kind = match tokens[4].value {
        "vreg" => {
            require_len(tokens, 6)?;
            MachineOperandKind::Register(MachineRegister::Virtual(VirtualRegisterId::new(number(
                tokens[5],
            )?)))
        }
        "preg" => {
            require_len(tokens, 6)?;
            MachineOperandKind::Register(MachineRegister::Physical(PhysicalRegister::new(number(
                tokens[5],
            )?)))
        }
        "imm" => {
            require_len(tokens, 6)?;
            MachineOperandKind::Immediate(number(tokens[5])?)
        }
        "frame-index" => {
            require_len(tokens, 7)?;
            MachineOperandKind::FrameIndex {
                index: FrameIndex::new(number(tokens[5])?),
                addend: number(tokens[6])?,
            }
        }
        "block" => {
            require_len(tokens, 6)?;
            MachineOperandKind::Block(MachineBlockId::new(number(tokens[5])?))
        }
        "global" => {
            require_len(tokens, 7)?;
            MachineOperandKind::Global {
                name: decode_name(tokens[5])?,
                addend: number(tokens[6])?,
            }
        }
        "external" => {
            require_len(tokens, 7)?;
            MachineOperandKind::ExternalSymbol {
                name: decode_name(tokens[5])?,
                addend: number(tokens[6])?,
            }
        }
        _ => return Err(unexpected(&tokens[4], "a machine operand kind")),
    };
    Ok(MachineOperand {
        kind,
        role,
        constraint,
        tied_to,
    })
}

fn parse_constraint(token: Token<'_>) -> Result<Option<RegisterConstraint>, TextError> {
    if token.value == "-" {
        return Ok(None);
    }
    let Some((kind, number)) = token.value.split_once(':') else {
        return Err(error(
            token.line,
            token.column,
            "constraint must be `fixed:<id>`, `class:<id>`, or `-`",
        ));
    };
    let value = number.parse::<u32>().map_err(|_| {
        error(
            token.line,
            token.column + kind.len() + 1,
            "expected an unsigned 32-bit integer",
        )
    })?;
    match kind {
        "fixed" => Ok(Some(RegisterConstraint::Fixed(PhysicalRegister::new(
            value,
        )))),
        "class" => Ok(Some(RegisterConstraint::Class(RegisterClass::new(value)))),
        _ => Err(error(
            token.line,
            token.column,
            "unknown register constraint",
        )),
    }
}

fn tokenize(line: SourceLine<'_>) -> Result<Vec<Token<'_>>, TextError> {
    let mut tokens = Vec::new();
    let mut start = None;
    for (index, character) in line.text.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = start.take() {
                tokens.push(Token {
                    line: line.number,
                    column: start + 1,
                    value: &line.text[start..index],
                });
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(start) = start {
        tokens.push(Token {
            line: line.number,
            column: start + 1,
            value: &line.text[start..],
        });
    }
    if tokens.is_empty() && !line.text.is_empty() {
        return Err(error(
            line.number,
            1,
            "whitespace-only lines are not permitted",
        ));
    }
    Ok(tokens)
}

fn expect_exact(line: &SourceLine<'_>, expected: &[&str]) -> Result<(), TextError> {
    let tokens = tokenize(*line)?;
    if tokens.len() != expected.len() {
        return Err(error(line.number, 1, "invalid qmir format header"));
    }
    for (token, expected) in tokens.iter().zip(expected) {
        if token.value != *expected {
            return Err(error(
                token.line,
                token.column,
                "invalid qmir format header",
            ));
        }
    }
    Ok(())
}

fn expect_value(token: Token<'_>, expected: &str) -> Result<(), TextError> {
    if token.value == expected {
        Ok(())
    } else {
        Err(unexpected(&token, &format!("`{expected}`")))
    }
}

fn require_len(tokens: &[Token<'_>], expected: usize) -> Result<(), TextError> {
    if tokens.len() == expected {
        Ok(())
    } else {
        let token = tokens.last().copied();
        Err(error(
            token.map_or(1, |item| item.line),
            token.map_or(1, |item| item.column + item.value.len()),
            format!("expected {expected} fields, found {}", tokens.len()),
        ))
    }
}

fn require_at_least(tokens: &[Token<'_>], minimum: usize) -> Result<(), TextError> {
    if tokens.len() >= minimum {
        Ok(())
    } else {
        let token = tokens.last().copied();
        Err(error(
            token.map_or(1, |item| item.line),
            token.map_or(1, |item| item.column + item.value.len()),
            format!("expected at least {minimum} fields, found {}", tokens.len()),
        ))
    }
}

fn number<T>(token: Token<'_>) -> Result<T, TextError>
where
    T: std::str::FromStr,
{
    token
        .value
        .parse()
        .map_err(|_| error(token.line, token.column, "invalid numeric value"))
}

fn unexpected(token: &Token<'_>, expected: &str) -> TextError {
    error(
        token.line,
        token.column,
        format!("expected {expected}, found `{}`", token.value),
    )
}

fn error(line: usize, column: usize, message: impl Into<String>) -> TextError {
    TextError {
        line,
        column,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_machine_ir_field_canonically() {
        let module = MachineModule {
            functions: vec![MachineFunction {
                id: MachineFunctionId::new(7),
                name: "main.å".to_owned(),
                virtual_registers: vec![
                    VirtualRegister {
                        id: VirtualRegisterId::new(4),
                        class: RegisterClass::new(2),
                    },
                    VirtualRegister {
                        id: VirtualRegisterId::new(9),
                        class: RegisterClass::new(3),
                    },
                ],
                frame_objects: vec![
                    FrameObject {
                        index: FrameIndex::new(1),
                        size: 8,
                        alignment: 8,
                        kind: FrameObjectKind::Local,
                    },
                    FrameObject {
                        index: FrameIndex::new(3),
                        size: 4,
                        alignment: 4,
                        kind: FrameObjectKind::Spill,
                    },
                    FrameObject {
                        index: FrameIndex::new(8),
                        size: 2,
                        alignment: 2,
                        kind: FrameObjectKind::OutgoingArgument,
                    },
                ],
                blocks: vec![
                    MachineBlock {
                        id: MachineBlockId::new(2),
                        successors: vec![MachineBlockId::new(5)],
                        instructions: vec![MachineInstruction {
                            id: MachineInstructionId::new(10),
                            opcode: TargetOpcode::new(99),
                            flags: InstructionFlags {
                                terminator: true,
                                call: true,
                                copy: true,
                                side_effects: true,
                                may_load: true,
                                may_store: true,
                                volatile: true,
                            },
                            operands: vec![
                                MachineOperand {
                                    kind: MachineOperandKind::Register(MachineRegister::Virtual(
                                        VirtualRegisterId::new(4),
                                    )),
                                    role: OperandRole::Def,
                                    constraint: Some(RegisterConstraint::Class(
                                        RegisterClass::new(2),
                                    )),
                                    tied_to: Some(OperandIndex::new(1)),
                                },
                                MachineOperand {
                                    kind: MachineOperandKind::Register(MachineRegister::Physical(
                                        PhysicalRegister::new(6),
                                    )),
                                    role: OperandRole::Use,
                                    constraint: None,
                                    tied_to: None,
                                },
                                MachineOperand {
                                    kind: MachineOperandKind::Register(MachineRegister::Virtual(
                                        VirtualRegisterId::new(9),
                                    )),
                                    role: OperandRole::UseDef,
                                    constraint: Some(RegisterConstraint::Fixed(
                                        PhysicalRegister::new(7),
                                    )),
                                    tied_to: None,
                                },
                                MachineOperand {
                                    kind: MachineOperandKind::Immediate(-42),
                                    role: OperandRole::None,
                                    constraint: None,
                                    tied_to: None,
                                },
                                MachineOperand {
                                    kind: MachineOperandKind::FrameIndex {
                                        index: FrameIndex::new(3),
                                        addend: -8,
                                    },
                                    role: OperandRole::None,
                                    constraint: None,
                                    tied_to: None,
                                },
                                MachineOperand {
                                    kind: MachineOperandKind::Block(MachineBlockId::new(5)),
                                    role: OperandRole::None,
                                    constraint: None,
                                    tied_to: None,
                                },
                                MachineOperand {
                                    kind: MachineOperandKind::Global {
                                        name: "data.å".to_owned(),
                                        addend: 12,
                                    },
                                    role: OperandRole::None,
                                    constraint: None,
                                    tied_to: None,
                                },
                                MachineOperand {
                                    kind: MachineOperandKind::ExternalSymbol {
                                        name: String::new(),
                                        addend: -1,
                                    },
                                    role: OperandRole::None,
                                    constraint: None,
                                    tied_to: None,
                                },
                            ],
                        }],
                    },
                    MachineBlock {
                        id: MachineBlockId::new(5),
                        successors: Vec::new(),
                        instructions: Vec::new(),
                    },
                ],
            }],
        };

        let text = write_text(&module);
        let reparsed = parse_text(&text).expect("printer output must parse");
        assert_eq!(reparsed, module);
        assert_eq!(write_text(&reparsed), text);
    }

    #[test]
    fn reports_the_malformed_operand_location() {
        let error =
            parse_text("qmir 1\nfunction 0 66\nblock 0 0\ninst 0 0 0 1\noperand use - - wat\n")
                .expect_err("unknown operand kind must be rejected");
        assert_eq!(error.line, 5);
        assert_eq!(error.column, 17);
        assert!(error.message.contains("machine operand kind"));
    }
}
