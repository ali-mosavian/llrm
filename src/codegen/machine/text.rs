//! Deterministic textual assembly for target-independent Machine IR.
//!
//! The format is deliberately a small, line-oriented interchange format.  It
//! uses decimal identifiers and hexadecimal UTF-8 for names, so it needs no
//! escaping rules and remains stable across platforms.

use std::error::Error;
use std::fmt;

use super::{
    FrameIndex, FrameObject, FrameObjectKind, InstructionFlags, MachineAddressSpace, MachineBlock,
    MachineBlockId, MachineCallingConvention, MachineDataObject, MachineFunction,
    MachineFunctionId, MachineInstruction, MachineInstructionId, MachineLinkage, MachineModule,
    MachineOperand, MachineOperandKind, MachineRegister, MachineSignature, MachineValueType,
    OperandIndex, OperandRole, PhysicalRegister, RegisterClass, RegisterConstraint, TargetOpcode,
    VirtualRegister, VirtualRegisterId,
};

/// Version of the `.qmir` textual format.
pub const FORMAT_VERSION: u32 = 4;

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
    for object in &module.data_objects {
        line(
            &mut text,
            &[
                "data".to_owned(),
                encode_name(&object.name),
                encode_bytes(&object.bytes),
                object.alignment.to_string(),
                if object.constant {
                    "constant"
                } else {
                    "mutable"
                }
                .to_owned(),
                linkage_name(object.linkage).to_owned(),
            ],
        );
    }
    for function in &module.functions {
        let mut fields = vec![
            "function".to_owned(),
            function.id.get().to_string(),
            function.entry.get().to_string(),
            encode_name(&function.name),
            linkage_name(function.linkage).to_owned(),
            calling_convention_name(function.signature.calling_convention).to_owned(),
            value_type_name(function.signature.result).to_owned(),
            u8::from(function.signature.variadic).to_string(),
            function.signature.parameters.len().to_string(),
        ];
        fields.extend(
            function
                .signature
                .parameters
                .iter()
                .copied()
                .map(|value_type| value_type_name(Some(value_type)).to_owned()),
        );
        line(&mut text, &fields);
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
                    frame_kind_name(object.kind),
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
    let version = FORMAT_VERSION.to_string();
    expect_exact(&header, &["qmir", &version])?;

    let mut data_objects = Vec::new();
    let mut functions = Vec::new();
    while let Some(line) = parser.peek() {
        let tokens = tokenize(line)?;
        match tokens.first().map(|token| token.value) {
            Some("data") => {
                let line = parser.next()?;
                data_objects.push(parse_data(&tokenize(line)?)?);
            }
            Some("function") => {
                let line = parser.next()?;
                functions.push(parser.parse_function(line)?);
            }
            Some(_) => return Err(unexpected(&tokens[0], "`data`, `function`, or end of file")),
            None => return Err(error(line.number, 1, "blank lines are not permitted")),
        }
    }
    Ok(MachineModule {
        data_objects,
        functions,
    })
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
        MachineOperandKind::Function(id) => {
            fields.extend(["function".to_owned(), id.get().to_string()]);
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

fn frame_kind_name(kind: FrameObjectKind) -> String {
    match kind {
        FrameObjectKind::Local => "local".to_owned(),
        FrameObjectKind::Spill => "spill".to_owned(),
        FrameObjectKind::OutgoingArgument => "outgoing-argument".to_owned(),
        FrameObjectKind::IncomingArgument { parameter } => {
            format!("incoming-argument:{parameter}")
        }
    }
}

fn linkage_name(linkage: MachineLinkage) -> &'static str {
    match linkage {
        MachineLinkage::Internal => "internal",
        MachineLinkage::External => "external",
    }
}

fn calling_convention_name(calling_convention: MachineCallingConvention) -> &'static str {
    match calling_convention {
        MachineCallingConvention::C => "c",
        MachineCallingConvention::FarPascal => "far_pascal",
    }
}

fn value_type_name(value_type: Option<MachineValueType>) -> String {
    match value_type {
        None => "-".to_owned(),
        Some(MachineValueType::Integer { bits }) => format!("i{bits}"),
        Some(MachineValueType::Pointer {
            bits,
            address_space,
        }) => format!("p{bits}:{}", address_space_name(address_space)),
    }
}

fn address_space_name(address_space: MachineAddressSpace) -> &'static str {
    match address_space {
        MachineAddressSpace::Generic => "generic",
        MachineAddressSpace::NearData => "near-data",
        MachineAddressSpace::FarData => "far-data",
        MachineAddressSpace::HugeData => "huge-data",
        MachineAddressSpace::Code => "code",
        MachineAddressSpace::Segment => "segment",
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

fn encode_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "-".to_owned();
    }
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
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

fn decode_bytes(token: Token<'_>) -> Result<Vec<u8>, TextError> {
    if token.value == "-" {
        return Ok(Vec::new());
    }
    if token.value.len() % 2 != 0 {
        return Err(error(token.line, token.column, "hex bytes have odd length"));
    }
    if !token.value.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(error(
            token.line,
            token.column,
            "bytes must contain hexadecimal values",
        ));
    }
    (0..token.value.len())
        .step_by(2)
        .map(|offset| {
            u8::from_str_radix(&token.value[offset..offset + 2], 16).map_err(|_| {
                error(
                    token.line,
                    token.column + offset,
                    "bytes must contain hexadecimal values",
                )
            })
        })
        .collect()
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
        require_at_least(&tokens, 9)?;
        expect_value(tokens[0], "function")?;
        let id = MachineFunctionId::new(number(tokens[1])?);
        let entry = MachineBlockId::new(number(tokens[2])?);
        let name = decode_name(tokens[3])?;
        let linkage = parse_linkage(tokens[4])?;
        let calling_convention = parse_calling_convention(tokens[5])?;
        let result = parse_value_type(tokens[6], true)?;
        let variadic = parse_bool(tokens[7])?;
        let parameter_count = number::<usize>(tokens[8])?;
        if parameter_count.checked_add(9) != Some(tokens.len()) {
            return Err(error(
                tokens[8].line,
                tokens[8].column,
                "parameter count does not match the number of parameter types",
            ));
        }
        let parameters = tokens[9..]
            .iter()
            .copied()
            .map(|token| {
                parse_value_type(token, false)?
                    .ok_or_else(|| error(token.line, token.column, "parameter type cannot be `-`"))
            })
            .collect::<Result<Vec<_>, _>>()?;
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
            linkage,
            signature: MachineSignature {
                result,
                parameters,
                variadic,
                calling_convention,
            },
            entry,
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
        value => {
            let Some(parameter) = value.strip_prefix("incoming-argument:") else {
                return Err(unexpected(&tokens[4], "a frame object kind"));
            };
            let parameter = parameter.parse::<u32>().map_err(|_| {
                error(
                    tokens[4].line,
                    tokens[4].column + "incoming-argument:".len(),
                    "expected an unsigned 32-bit parameter index",
                )
            })?;
            FrameObjectKind::IncomingArgument { parameter }
        }
    };
    Ok(FrameObject {
        index: FrameIndex::new(number(tokens[1])?),
        size: number(tokens[2])?,
        alignment: number(tokens[3])?,
        kind,
    })
}

fn parse_data(tokens: &[Token<'_>]) -> Result<MachineDataObject, TextError> {
    require_len(tokens, 6)?;
    expect_value(tokens[0], "data")?;
    let constant = match tokens[4].value {
        "constant" => true,
        "mutable" => false,
        _ => return Err(unexpected(&tokens[4], "`constant` or `mutable`")),
    };
    Ok(MachineDataObject {
        name: decode_name(tokens[1])?,
        bytes: decode_bytes(tokens[2])?,
        alignment: number(tokens[3])?,
        constant,
        linkage: parse_linkage(tokens[5])?,
    })
}

fn parse_linkage(token: Token<'_>) -> Result<MachineLinkage, TextError> {
    match token.value {
        "internal" => Ok(MachineLinkage::Internal),
        "external" => Ok(MachineLinkage::External),
        _ => Err(unexpected(&token, "`internal` or `external`")),
    }
}

fn parse_calling_convention(token: Token<'_>) -> Result<MachineCallingConvention, TextError> {
    match token.value {
        "c" => Ok(MachineCallingConvention::C),
        "far_pascal" => Ok(MachineCallingConvention::FarPascal),
        _ => Err(unexpected(&token, "a calling convention")),
    }
}

fn parse_value_type(
    token: Token<'_>,
    allow_absent: bool,
) -> Result<Option<MachineValueType>, TextError> {
    if token.value == "-" {
        return if allow_absent {
            Ok(None)
        } else {
            Err(error(
                token.line,
                token.column,
                "parameter type cannot be `-`",
            ))
        };
    }
    if let Some(bits) = token.value.strip_prefix('i') {
        return bits
            .parse::<u16>()
            .map(|bits| Some(MachineValueType::Integer { bits }))
            .map_err(|_| {
                error(
                    token.line,
                    token.column + 1,
                    "expected an unsigned 16-bit width",
                )
            });
    }
    let Some(pointer) = token.value.strip_prefix('p') else {
        return Err(unexpected(&token, "a machine value type"));
    };
    let Some((bits, address_space)) = pointer.split_once(':') else {
        return Err(error(
            token.line,
            token.column,
            "pointer type must be `p<bits>:<address-space>`",
        ));
    };
    let bits = bits.parse::<u16>().map_err(|_| {
        error(
            token.line,
            token.column + 1,
            "expected an unsigned 16-bit width",
        )
    })?;
    let address_space = match address_space {
        "generic" => MachineAddressSpace::Generic,
        "near-data" => MachineAddressSpace::NearData,
        "far-data" => MachineAddressSpace::FarData,
        "huge-data" => MachineAddressSpace::HugeData,
        "code" => MachineAddressSpace::Code,
        "segment" => MachineAddressSpace::Segment,
        _ => {
            return Err(error(
                token.line,
                token.column,
                "unknown pointer address space",
            ));
        }
    };
    Ok(Some(MachineValueType::Pointer {
        bits,
        address_space,
    }))
}

fn parse_bool(token: Token<'_>) -> Result<bool, TextError> {
    match token.value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(error(
            token.line,
            token.column,
            "boolean value must be `0` or `1`",
        )),
    }
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
        "function" => {
            require_len(tokens, 6)?;
            MachineOperandKind::Function(MachineFunctionId::new(number(tokens[5])?))
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
            data_objects: vec![MachineDataObject {
                name: "data.å".to_owned(),
                bytes: vec![0, 17, 255],
                alignment: 8,
                constant: true,
                linkage: MachineLinkage::Internal,
            }],
            functions: vec![MachineFunction {
                id: MachineFunctionId::new(7),
                name: "main.å".to_owned(),
                linkage: MachineLinkage::External,
                signature: MachineSignature {
                    result: Some(MachineValueType::Pointer {
                        bits: 16,
                        address_space: MachineAddressSpace::Code,
                    }),
                    parameters: vec![
                        MachineValueType::Integer { bits: 16 },
                        MachineValueType::Pointer {
                            bits: 16,
                            address_space: MachineAddressSpace::FarData,
                        },
                    ],
                    variadic: true,
                    calling_convention: MachineCallingConvention::FarPascal,
                },
                entry: MachineBlockId::new(5),
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
                        kind: FrameObjectKind::IncomingArgument { parameter: 1 },
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
                                    kind: MachineOperandKind::Function(MachineFunctionId::new(7)),
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
        assert!(text.contains(" far_pascal "));
        assert!(!text.contains(" basic "));
        assert!(!text.contains(" runtime "));
        let reparsed = parse_text(&text).expect("printer output must parse");
        assert_eq!(reparsed, module);
        assert_eq!(write_text(&reparsed), text);
    }

    #[test]
    fn reports_the_malformed_operand_location() {
        let error =
            parse_text("qmir 4\nfunction 0 0 66 internal c - 0 0\nblock 0 0\ninst 0 0 0 1\noperand use - - wat\n")
                .expect_err("unknown operand kind must be rejected");
        assert_eq!(error.line, 5);
        assert_eq!(error.column, 17);
        assert!(error.message.contains("machine operand kind"));
    }

    #[test]
    fn accepts_only_the_version_four_schema() {
        let error = parse_text("qmir 3\n").expect_err("qmir version three is not accepted");
        assert_eq!(error.line, 1);
        assert!(error.message.contains("invalid qmir format header"));

        for obsolete in ["basic", "runtime"] {
            let source = format!("qmir 4\nfunction 0 0 66 internal {obsolete} - 0 0\n");
            let error = parse_text(&source)
                .expect_err("source-language ABI labels must not enter Machine IR");
            assert!(error.message.contains("calling convention"));
        }
    }
}
