//! Deterministic textual assembly for resolved HIR.
//!
//! The format deliberately mirrors the owned HIR shape. It is a compact
//! replay format, not a general-purpose serialization protocol: every record
//! has a closed grammar, strings are quoted, and the parser rejects unknown
//! fields before it asks the HIR verifier to check cross-record invariants.

use std::fmt::{self, Write};

use super::{
    AddressKind, ArrayOrder, Block, BlockId, CallAbi, CallDistance, Callable, CallableId,
    ConstantValue, DataId, DataObject, DataRelocation, Dialect, FORMAT_VERSION, FloatEvaluation,
    FloatMode, FloatRounding, Function, FunctionId, Instruction, InstructionId, Linkage, Module,
    ModuleId, Opcode, Operand, Parameter, Place, PlaceId, ProcedureAbi, Program, RuntimeProfile,
    StackCleanup, Storage, TargetProfile, Terminator, Type, TypeId, TypeKind, Value, ValueId,
};

/// A structural or semantic `.qhir` decoding failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextError {
    /// One-based input line. A zero denotes a post-parse verifier failure.
    pub line: usize,
    pub message: String,
}

impl TextError {
    fn at(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(formatter, "invalid qhir: {}", self.message)
        } else {
            write!(formatter, "line {}: {}", self.line, self.message)
        }
    }
}

impl std::error::Error for TextError {}

/// Render a program in canonical `.qhir` form.
pub fn write(program: &Program) -> String {
    let mut output = String::new();
    writeln!(output, "qhir {}", program.version).expect("writing a string cannot fail");
    writeln!(
        output,
        "program {} {} {} {} {}",
        dialect_name(program.dialect),
        runtime_name(program.runtime),
        target_name(program.target),
        array_order_name(program.array_order),
        float_mode_name(program.float_mode),
    )
    .expect("writing a string cannot fail");

    for module in &program.modules {
        write_module(&mut output, module);
    }
    output.push_str("endprogram\n");
    output
}

/// Parse a complete `.qhir` replay artifact and verify it before returning.
pub fn parse(input: &str) -> Result<Program, TextError> {
    let mut lines = Lines::new(input);
    let version = parse_version(lines.next_required("qhir version directive")?)?;
    if version != FORMAT_VERSION {
        return Err(TextError::at(
            1,
            format!("unsupported qhir version {version}; expected {FORMAT_VERSION}"),
        ));
    }
    let program_line = lines.next_required("program header")?;
    let mut reader = Reader::new(program_line.number, &program_line.text);
    reader.expect_word("program")?;
    let program = Program {
        version,
        dialect: parse_dialect(reader.word()?, program_line.number)?,
        runtime: parse_runtime(reader.word()?, program_line.number)?,
        target: parse_target(reader.word()?, program_line.number)?,
        array_order: parse_array_order(reader.word()?, program_line.number)?,
        float_mode: parse_float_mode(reader.word()?, program_line.number)?,
        modules: Vec::new(),
    };
    reader.finish()?;

    let mut program = program;
    loop {
        let line = lines.next_required("module or endprogram")?;
        let keyword = line.keyword()?;
        match keyword {
            "module" => program.modules.push(parse_module(&mut lines, line)?),
            "endprogram" => {
                Reader::new(line.number, &line.text).exact_word("endprogram")?;
                break;
            }
            _ => return Err(TextError::at(line.number, "expected module or endprogram")),
        }
    }
    if let Some(line) = lines.next() {
        return Err(TextError::at(line.number, "content after endprogram"));
    }
    program.verify().map_err(|diagnostics| {
        let message = diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect::<Vec<_>>()
            .join("; ");
        TextError::at(0, message)
    })?;
    Ok(program)
}

fn write_module(output: &mut String, module: &Module) {
    write!(output, "module {} ", module.id).expect("writing a string cannot fail");
    write_string(output, &module.name);
    output.push('\n');
    for type_ in &module.types {
        write_type(output, type_);
    }
    for data in &module.data {
        write_data(output, data);
    }
    for callable in &module.callables {
        write_callable(output, callable);
    }
    for function in &module.functions {
        write_function(output, function);
    }
    output.push_str("endmodule\n");
}

fn write_type(output: &mut String, type_: &Type) {
    write!(output, "type {} ", type_.id,).expect("writing a string cannot fail");
    write_string(output, &type_.name);
    write!(
        output,
        " {} {} {} {} {} ",
        type_.kind.as_str(),
        type_.width,
        signed_name(type_.signed),
        type_.evaluation.as_str(),
        optional_id(type_.element),
    )
    .expect("writing a string cannot fail");
    write_bounds(output, &type_.bounds);
    writeln!(output, " {}", type_.address.as_str()).expect("writing a string cannot fail");
}

fn write_data(output: &mut String, data: &DataObject) {
    write!(output, "data {} ", data.id).expect("writing a string cannot fail");
    write_string(output, &data.name);
    write!(
        output,
        " {} {} {} {} ",
        bytes_hex(&data.bytes),
        bool_name(data.readonly),
        data.linkage.as_str(),
        data.address.as_str(),
    )
    .expect("writing a string cannot fail");
    write_relocations(output, &data.relocations);
    output.push('\n');
}

fn write_callable(output: &mut String, callable: &Callable) {
    write!(output, "callable {} ", callable.id).expect("writing a string cannot fail");
    write_string(output, &callable.name);
    write!(output, " {} ", optional_id(callable.result_type))
        .expect("writing a string cannot fail");
    write_parameters(output, &callable.parameters);
    writeln!(output, " {}", bool_name(callable.defined)).expect("writing a string cannot fail");
}

fn write_function(output: &mut String, function: &Function) {
    write!(output, "function {} ", function.id).expect("writing a string cannot fail");
    write_string(output, &function.name);
    write!(output, " {} {} ", function.result_type, function.entry,)
        .expect("writing a string cannot fail");
    write_id_list(output, &function.parameters);
    write!(
        output,
        " {} {} {} {} {} ",
        stack_cleanup_name(function.abi.cleanup),
        call_distance_name(function.abi.distance),
        function.abi.parameter_bytes,
        optional_id(function.error_handler),
        bool_name(function.error_handler_local),
    )
    .expect("writing a string cannot fail");
    write_id_list(output, &function.external_entries);
    writeln!(output, " {}", function.linkage.as_str()).expect("writing a string cannot fail");

    for value in &function.values {
        writeln!(output, "value {} {}", value.id, value.type_id)
            .expect("writing a string cannot fail");
    }
    for place in &function.places {
        write!(output, "place {} ", place.id).expect("writing a string cannot fail");
        write_string(output, &place.name);
        writeln!(
            output,
            " {} {} {} {} {} {}",
            place.type_id,
            place.storage,
            place.offset,
            place.symbol,
            place.extent,
            place.address.as_str(),
        )
        .expect("writing a string cannot fail");
    }
    for block in &function.blocks {
        write_block(output, block);
    }
    for call in &function.calls {
        write!(output, "call-abi {} ", call.instruction,).expect("writing a string cannot fail");
        write_usize_list(output, &call.order);
        writeln!(
            output,
            " {} {} {}",
            stack_cleanup_name(call.cleanup),
            call_distance_name(call.distance),
            optional_id(call.callee),
        )
        .expect("writing a string cannot fail");
    }
    output.push_str("endfunction\n");
}

fn write_block(output: &mut String, block: &Block) {
    writeln!(output, "block {}", block.id).expect("writing a string cannot fail");
    for instruction in &block.instructions {
        write!(
            output,
            "instruction {} {} ",
            instruction.id,
            instruction.opcode.as_str()
        )
        .expect("writing a string cannot fail");
        write_id_list(output, &instruction.results);
        output.push(' ');
        write_operands(output, &instruction.operands);
        output.push(' ');
        write_optional_string(output, instruction.callee.as_deref());
        output.push('\n');
    }
    write_terminator(output, &block.terminator);
    output.push_str("endblock\n");
}

fn write_terminator(output: &mut String, terminator: &Terminator) {
    match terminator {
        Terminator::Jump(target) => {
            writeln!(output, "terminator jump {}", target).expect("writing a string cannot fail")
        }
        Terminator::Branch {
            condition,
            then_block,
            else_block,
        } => {
            write!(output, "terminator branch ").expect("writing a string cannot fail");
            write_operand(output, condition);
            writeln!(output, " {} {}", then_block, else_block)
                .expect("writing a string cannot fail");
        }
        Terminator::Switch {
            selector,
            cases,
            default,
        } => {
            write!(output, "terminator switch ").expect("writing a string cannot fail");
            write_operand(output, selector);
            output.push(' ');
            write_cases(output, cases);
            writeln!(output, " {}", default).expect("writing a string cannot fail");
        }
        Terminator::Return(None) => output.push_str("terminator return none\n"),
        Terminator::Return(Some(value)) => {
            write!(output, "terminator return ").expect("writing a string cannot fail");
            write_operand(output, value);
            output.push('\n');
        }
        Terminator::Unreachable => output.push_str("terminator unreachable\n"),
    }
}

fn write_operand(output: &mut String, operand: &Operand) {
    match operand {
        Operand::Value(id) => write!(output, "value({id})").expect("writing a string cannot fail"),
        Operand::Constant { type_id, value } => match value {
            ConstantValue::Integer(value) => {
                write!(output, "integer({type_id},{value})").expect("writing a string cannot fail")
            }
            ConstantValue::Real(value) => {
                write!(output, "real({type_id},").expect("writing a string cannot fail");
                write_string(output, value);
                output.push(')');
            }
        },
        Operand::Place(id) => write!(output, "place({id})").expect("writing a string cannot fail"),
        Operand::Element { place, indices } => {
            write!(output, "element({place},").expect("writing a string cannot fail");
            write_operands(output, indices);
            output.push(')');
        }
        Operand::Projection {
            place,
            indices,
            offset,
            type_id,
        } => {
            write!(output, "projection({place},").expect("writing a string cannot fail");
            write_operands(output, indices);
            write!(output, ",{offset},{type_id})").expect("writing a string cannot fail");
        }
        Operand::Indirect {
            base,
            offset,
            type_id,
            volatile,
        } => write!(
            output,
            "indirect({base},{offset},{type_id},{})",
            bool_name(*volatile)
        )
        .expect("writing a string cannot fail"),
    }
}

fn write_operands(output: &mut String, operands: &[Operand]) {
    output.push('[');
    for (index, operand) in operands.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write_operand(output, operand);
    }
    output.push(']');
}

fn write_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        for escaped in character.escape_default() {
            output.push(escaped);
        }
    }
    output.push('"');
}

fn write_optional_string(output: &mut String, value: Option<&str>) {
    match value {
        Some(value) => write_string(output, value),
        None => output.push_str("none"),
    }
}

fn write_id_list<T: fmt::Display>(output: &mut String, values: &[T]) {
    output.push('[');
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(output, "{value}").expect("writing a string cannot fail");
    }
    output.push(']');
}

fn write_usize_list(output: &mut String, values: &[usize]) {
    write_id_list(output, values);
}

fn write_bounds(output: &mut String, bounds: &[(i64, i64)]) {
    output.push('[');
    for (index, (lower, upper)) in bounds.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(output, "({lower},{upper})").expect("writing a string cannot fail");
    }
    output.push(']');
}

fn write_relocations(output: &mut String, relocations: &[DataRelocation]) {
    output.push('[');
    for (index, relocation) in relocations.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(
            output,
            "({},{},{},{})",
            relocation.at,
            relocation.target,
            relocation.addend,
            relocation.address.as_str(),
        )
        .expect("writing a string cannot fail");
    }
    output.push(']');
}

fn write_parameters(output: &mut String, parameters: &[Parameter]) {
    output.push('[');
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(
            output,
            "({},{},{},{})",
            parameter.type_id,
            bool_name(parameter.by_value),
            bool_name(parameter.segmented),
            bool_name(parameter.array),
        )
        .expect("writing a string cannot fail");
    }
    output.push(']');
}

fn write_cases(output: &mut String, cases: &[(i64, BlockId)]) {
    output.push('[');
    for (index, (value, target)) in cases.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write!(output, "({value},{target})").expect("writing a string cannot fail");
    }
    output.push(']');
}

fn parse_version(line: Line<'_>) -> Result<u32, TextError> {
    let mut reader = Reader::new(line.number, &line.text);
    reader.expect_word("qhir")?;
    let version = reader.u32()?;
    reader.finish()?;
    Ok(version)
}

fn parse_module(lines: &mut Lines<'_>, header: Line<'_>) -> Result<Module, TextError> {
    let mut reader = Reader::new(header.number, &header.text);
    reader.expect_word("module")?;
    let mut module = Module {
        id: ModuleId::new(reader.u32()?),
        name: reader.string()?,
        types: Vec::new(),
        functions: Vec::new(),
        data: Vec::new(),
        callables: Vec::new(),
    };
    reader.finish()?;

    loop {
        let line = lines.next_required("module record or endmodule")?;
        match line.keyword()? {
            "type" => module.types.push(parse_type(line)?),
            "data" => module.data.push(parse_data(line)?),
            "callable" => module.callables.push(parse_callable(line)?),
            "function" => module.functions.push(parse_function(lines, line)?),
            "endmodule" => {
                Reader::new(line.number, &line.text).exact_word("endmodule")?;
                return Ok(module);
            }
            _ => return Err(TextError::at(line.number, "unknown module record")),
        }
    }
}

fn parse_type(line: Line<'_>) -> Result<Type, TextError> {
    let mut reader = Reader::new(line.number, &line.text);
    reader.expect_word("type")?;
    let type_ = Type {
        id: TypeId::new(reader.u32()?),
        name: reader.string()?,
        kind: parse_type_kind(reader.word()?, line.number)?,
        width: reader.usize()?,
        signed: parse_signed(reader.word()?, line.number)?,
        evaluation: parse_float_evaluation(reader.word()?, line.number)?,
        element: reader.optional_id(TypeId::new)?,
        bounds: reader.bounds()?,
        address: parse_address(reader.word()?, line.number)?,
    };
    reader.finish()?;
    Ok(type_)
}

fn parse_data(line: Line<'_>) -> Result<DataObject, TextError> {
    let mut reader = Reader::new(line.number, &line.text);
    reader.expect_word("data")?;
    let data = DataObject {
        id: DataId::new(reader.u32()?),
        name: reader.string()?,
        bytes: reader.bytes()?,
        readonly: reader.bool()?,
        linkage: parse_linkage(reader.word()?, line.number)?,
        address: parse_address(reader.word()?, line.number)?,
        relocations: reader.relocations()?,
    };
    reader.finish()?;
    Ok(data)
}

fn parse_callable(line: Line<'_>) -> Result<Callable, TextError> {
    let mut reader = Reader::new(line.number, &line.text);
    reader.expect_word("callable")?;
    let callable = Callable {
        id: CallableId::new(reader.u32()?),
        name: reader.string()?,
        result_type: reader.optional_id(TypeId::new)?,
        parameters: reader.parameters()?,
        defined: reader.bool()?,
    };
    reader.finish()?;
    Ok(callable)
}

fn parse_function(lines: &mut Lines<'_>, header: Line<'_>) -> Result<Function, TextError> {
    let mut reader = Reader::new(header.number, &header.text);
    reader.expect_word("function")?;
    let mut function = Function {
        id: FunctionId::new(reader.u32()?),
        name: reader.string()?,
        result_type: TypeId::new(reader.u32()?),
        entry: BlockId::new(reader.u32()?),
        parameters: reader.id_list(ValueId::new)?,
        abi: ProcedureAbi {
            cleanup: parse_stack_cleanup(reader.word()?, header.number)?,
            distance: parse_call_distance(reader.word()?, header.number)?,
            parameter_bytes: reader.usize()?,
        },
        error_handler: reader.optional_id(BlockId::new)?,
        error_handler_local: reader.bool()?,
        external_entries: reader.id_list(BlockId::new)?,
        linkage: parse_linkage(reader.word()?, header.number)?,
        values: Vec::new(),
        places: Vec::new(),
        blocks: Vec::new(),
        calls: Vec::new(),
    };
    reader.finish()?;

    loop {
        let line = lines.next_required("function record or endfunction")?;
        match line.keyword()? {
            "value" => function.values.push(parse_value(line)?),
            "place" => function.places.push(parse_place(line)?),
            "block" => function.blocks.push(parse_block(lines, line)?),
            "call-abi" => function.calls.push(parse_call_abi(line)?),
            "endfunction" => {
                Reader::new(line.number, &line.text).exact_word("endfunction")?;
                return Ok(function);
            }
            _ => return Err(TextError::at(line.number, "unknown function record")),
        }
    }
}

fn parse_value(line: Line<'_>) -> Result<Value, TextError> {
    let mut reader = Reader::new(line.number, &line.text);
    reader.expect_word("value")?;
    let value = Value {
        id: ValueId::new(reader.u32()?),
        type_id: TypeId::new(reader.u32()?),
    };
    reader.finish()?;
    Ok(value)
}

fn parse_place(line: Line<'_>) -> Result<Place, TextError> {
    let mut reader = Reader::new(line.number, &line.text);
    reader.expect_word("place")?;
    let id = PlaceId::new(reader.u32()?);
    let name = reader.string()?;
    let type_id = TypeId::new(reader.u32()?);
    let storage = if reader.peek_word()? == "parameter" {
        reader.expect_word("parameter")?;
        reader.expect_char('(')?;
        let index = reader.u32()?;
        reader.expect_char(')')?;
        Storage::Parameter { index }
    } else {
        parse_storage(reader.word()?, line.number)?
    };
    let place = Place {
        id,
        name,
        type_id,
        storage,
        offset: reader.isize()?,
        symbol: DataId::new(reader.u32()?),
        extent: reader.usize()?,
        address: parse_address(reader.word()?, line.number)?,
    };
    reader.finish()?;
    Ok(place)
}

fn parse_block(lines: &mut Lines<'_>, header: Line<'_>) -> Result<Block, TextError> {
    let mut reader = Reader::new(header.number, &header.text);
    reader.expect_word("block")?;
    let id = BlockId::new(reader.u32()?);
    reader.finish()?;
    let mut instructions = Vec::new();
    let terminator = loop {
        let line = lines.next_required("instruction or terminator")?;
        match line.keyword()? {
            "instruction" => instructions.push(parse_instruction(line)?),
            "terminator" => break parse_terminator(line)?,
            _ => {
                return Err(TextError::at(
                    line.number,
                    "expected instruction or terminator",
                ));
            }
        }
    };
    let end = lines.next_required("endblock")?;
    Reader::new(end.number, &end.text).exact_word("endblock")?;
    Ok(Block {
        id,
        instructions,
        terminator,
    })
}

fn parse_instruction(line: Line<'_>) -> Result<Instruction, TextError> {
    let mut reader = Reader::new(line.number, &line.text);
    reader.expect_word("instruction")?;
    let instruction = Instruction {
        id: InstructionId::new(reader.u32()?),
        opcode: parse_opcode(reader.word()?, line.number)?,
        results: reader.id_list(ValueId::new)?,
        operands: reader.operands()?,
        callee: reader.optional_string()?,
    };
    reader.finish()?;
    Ok(instruction)
}

fn parse_terminator(line: Line<'_>) -> Result<Terminator, TextError> {
    let mut reader = Reader::new(line.number, &line.text);
    reader.expect_word("terminator")?;
    let kind = reader.word()?;
    let terminator = match kind {
        "jump" => Terminator::Jump(BlockId::new(reader.u32()?)),
        "branch" => Terminator::Branch {
            condition: reader.operand()?,
            then_block: BlockId::new(reader.u32()?),
            else_block: BlockId::new(reader.u32()?),
        },
        "switch" => Terminator::Switch {
            selector: reader.operand()?,
            cases: reader.cases()?,
            default: BlockId::new(reader.u32()?),
        },
        "return" if reader.peek_word()? == "none" => {
            reader.expect_word("none")?;
            Terminator::Return(None)
        }
        "return" => Terminator::Return(Some(reader.operand()?)),
        "unreachable" => Terminator::Unreachable,
        _ => return Err(TextError::at(line.number, "unknown terminator")),
    };
    reader.finish()?;
    Ok(terminator)
}

fn parse_call_abi(line: Line<'_>) -> Result<CallAbi, TextError> {
    let mut reader = Reader::new(line.number, &line.text);
    reader.expect_word("call-abi")?;
    let call = CallAbi {
        instruction: InstructionId::new(reader.u32()?),
        order: reader.usize_list()?,
        cleanup: parse_stack_cleanup(reader.word()?, line.number)?,
        distance: parse_call_distance(reader.word()?, line.number)?,
        callee: reader.optional_id(CallableId::new)?,
    };
    reader.finish()?;
    Ok(call)
}

struct Lines<'a> {
    lines: std::str::Lines<'a>,
    number: usize,
}

impl<'a> Lines<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            lines: input.lines(),
            number: 0,
        }
    }

    fn next(&mut self) -> Option<Line<'a>> {
        let text = self.lines.next()?;
        self.number += 1;
        Some(Line {
            number: self.number,
            text,
        })
    }

    fn next_required(&mut self, expected: &str) -> Result<Line<'a>, TextError> {
        self.next()
            .ok_or_else(|| TextError::at(self.number + 1, format!("expected {expected}")))
    }
}

#[derive(Clone, Copy)]
struct Line<'a> {
    number: usize,
    text: &'a str,
}

impl<'a> Line<'a> {
    fn keyword(self) -> Result<&'a str, TextError> {
        Reader::new(self.number, self.text).word()
    }
}

struct Reader<'a> {
    line: usize,
    input: &'a str,
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(line: usize, input: &'a str) -> Self {
        Self {
            line,
            input,
            position: 0,
        }
    }

    fn exact_word(mut self, word: &str) -> Result<(), TextError> {
        self.expect_word(word)?;
        self.finish()
    }

    fn finish(&mut self) -> Result<(), TextError> {
        self.skip_space();
        if self.position == self.input.len() {
            Ok(())
        } else {
            Err(self.error("unexpected trailing content"))
        }
    }

    fn expect_word(&mut self, expected: &str) -> Result<(), TextError> {
        let actual = self.word()?;
        if actual == expected {
            Ok(())
        } else {
            Err(self.error(format!("expected {expected}, found {actual}")))
        }
    }

    fn peek_word(&mut self) -> Result<&'a str, TextError> {
        let saved = self.position;
        let word = self.word()?;
        self.position = saved;
        Ok(word)
    }

    fn word(&mut self) -> Result<&'a str, TextError> {
        self.skip_space();
        let start = self.position;
        while let Some(character) = self.current() {
            if character.is_ascii_whitespace() || matches!(character, '[' | ']' | '(' | ')' | ',') {
                break;
            }
            self.position += character.len_utf8();
        }
        if start == self.position {
            Err(self.error("expected token"))
        } else {
            Ok(&self.input[start..self.position])
        }
    }

    fn string(&mut self) -> Result<String, TextError> {
        self.skip_space();
        if self.take() != Some('"') {
            return Err(self.error("expected quoted string"));
        }
        let mut value = String::new();
        loop {
            let character = self
                .take()
                .ok_or_else(|| self.error("unterminated string"))?;
            match character {
                '"' => return Ok(value),
                '\\' => value.push(self.escape()?),
                character if character.is_control() => {
                    return Err(self.error("control character in string"));
                }
                character => value.push(character),
            }
        }
    }

    fn optional_string(&mut self) -> Result<Option<String>, TextError> {
        self.skip_space();
        if self.current() == Some('"') {
            self.string().map(Some)
        } else {
            self.expect_word("none")?;
            Ok(None)
        }
    }

    fn escape(&mut self) -> Result<char, TextError> {
        match self.take().ok_or_else(|| self.error("unfinished escape"))? {
            'n' => Ok('\n'),
            'r' => Ok('\r'),
            't' => Ok('\t'),
            '\\' => Ok('\\'),
            '"' => Ok('"'),
            'u' => {
                self.expect_char('{')?;
                let start = self.position;
                while matches!(self.current(), Some(character) if character.is_ascii_hexdigit()) {
                    self.take();
                }
                if start == self.position {
                    return Err(self.error("empty unicode escape"));
                }
                let digits = &self.input[start..self.position];
                self.expect_char('}')?;
                let code = u32::from_str_radix(digits, 16)
                    .map_err(|_| self.error("invalid unicode escape"))?;
                char::from_u32(code).ok_or_else(|| self.error("invalid unicode scalar"))
            }
            _ => Err(self.error("unknown string escape")),
        }
    }

    fn bool(&mut self) -> Result<bool, TextError> {
        match self.word()? {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(self.error("expected true or false")),
        }
    }

    fn u32(&mut self) -> Result<u32, TextError> {
        self.word()?
            .parse()
            .map_err(|_| self.error("expected unsigned 32-bit integer"))
    }

    fn usize(&mut self) -> Result<usize, TextError> {
        self.word()?
            .parse()
            .map_err(|_| self.error("expected unsigned integer"))
    }

    fn i64(&mut self) -> Result<i64, TextError> {
        self.word()?
            .parse()
            .map_err(|_| self.error("expected signed 64-bit integer"))
    }

    fn isize(&mut self) -> Result<isize, TextError> {
        self.word()?
            .parse()
            .map_err(|_| self.error("expected signed integer"))
    }

    fn optional_id<T>(&mut self, make: impl FnOnce(u32) -> T) -> Result<Option<T>, TextError> {
        if self.peek_word()? == "none" {
            self.expect_word("none")?;
            Ok(None)
        } else {
            self.u32().map(|id| Some(make(id)))
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), TextError> {
        self.skip_space();
        if self.take() == Some(expected) {
            Ok(())
        } else {
            Err(self.error(format!("expected {expected}")))
        }
    }

    fn id_list<T>(&mut self, make: impl Fn(u32) -> T) -> Result<Vec<T>, TextError> {
        self.expect_char('[')?;
        let mut values = Vec::new();
        if self.current_after_space() == Some(']') {
            self.take();
            return Ok(values);
        }
        loop {
            values.push(make(self.u32()?));
            if self.current_after_space() == Some(']') {
                self.take();
                return Ok(values);
            }
            self.expect_char(',')?;
        }
    }

    fn usize_list(&mut self) -> Result<Vec<usize>, TextError> {
        self.expect_char('[')?;
        let mut values = Vec::new();
        if self.current_after_space() == Some(']') {
            self.take();
            return Ok(values);
        }
        loop {
            values.push(self.usize()?);
            if self.current_after_space() == Some(']') {
                self.take();
                return Ok(values);
            }
            self.expect_char(',')?;
        }
    }

    fn bounds(&mut self) -> Result<Vec<(i64, i64)>, TextError> {
        self.expect_char('[')?;
        let mut bounds = Vec::new();
        if self.current_after_space() == Some(']') {
            self.take();
            return Ok(bounds);
        }
        loop {
            self.expect_char('(')?;
            let lower = self.i64()?;
            self.expect_char(',')?;
            let upper = self.i64()?;
            self.expect_char(')')?;
            bounds.push((lower, upper));
            if self.current_after_space() == Some(']') {
                self.take();
                return Ok(bounds);
            }
            self.expect_char(',')?;
        }
    }

    fn bytes(&mut self) -> Result<Vec<u8>, TextError> {
        let word = self.word()?;
        if word == "-" {
            return Ok(Vec::new());
        }
        if word.len() % 2 != 0 || !word.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(self.error("expected even-length hexadecimal bytes"));
        }
        (0..word.len())
            .step_by(2)
            .map(|index| {
                u8::from_str_radix(&word[index..index + 2], 16)
                    .map_err(|_| self.error("invalid byte"))
            })
            .collect()
    }

    fn relocations(&mut self) -> Result<Vec<DataRelocation>, TextError> {
        self.expect_char('[')?;
        let mut relocations = Vec::new();
        if self.current_after_space() == Some(']') {
            self.take();
            return Ok(relocations);
        }
        loop {
            self.expect_char('(')?;
            let at = self.usize()?;
            self.expect_char(',')?;
            let target = DataId::new(self.u32()?);
            self.expect_char(',')?;
            let addend = self.isize()?;
            self.expect_char(',')?;
            let address = parse_address(self.word()?, self.line)?;
            self.expect_char(')')?;
            relocations.push(DataRelocation {
                at,
                target,
                addend,
                address,
            });
            if self.current_after_space() == Some(']') {
                self.take();
                return Ok(relocations);
            }
            self.expect_char(',')?;
        }
    }

    fn parameters(&mut self) -> Result<Vec<Parameter>, TextError> {
        self.expect_char('[')?;
        let mut parameters = Vec::new();
        if self.current_after_space() == Some(']') {
            self.take();
            return Ok(parameters);
        }
        loop {
            self.expect_char('(')?;
            let type_id = TypeId::new(self.u32()?);
            self.expect_char(',')?;
            let by_value = self.bool()?;
            self.expect_char(',')?;
            let segmented = self.bool()?;
            self.expect_char(',')?;
            let array = self.bool()?;
            self.expect_char(')')?;
            parameters.push(Parameter {
                type_id,
                by_value,
                segmented,
                array,
            });
            if self.current_after_space() == Some(']') {
                self.take();
                return Ok(parameters);
            }
            self.expect_char(',')?;
        }
    }

    fn operands(&mut self) -> Result<Vec<Operand>, TextError> {
        self.expect_char('[')?;
        let mut operands = Vec::new();
        if self.current_after_space() == Some(']') {
            self.take();
            return Ok(operands);
        }
        loop {
            operands.push(self.operand()?);
            if self.current_after_space() == Some(']') {
                self.take();
                return Ok(operands);
            }
            self.expect_char(',')?;
        }
    }

    fn operand(&mut self) -> Result<Operand, TextError> {
        let kind = self.word()?;
        self.expect_char('(')?;
        let operand = match kind {
            "value" => Operand::Value(ValueId::new(self.u32()?)),
            "integer" => {
                let type_id = TypeId::new(self.u32()?);
                self.expect_char(',')?;
                Operand::Constant {
                    type_id,
                    value: ConstantValue::Integer(self.i64()?),
                }
            }
            "real" => {
                let type_id = TypeId::new(self.u32()?);
                self.expect_char(',')?;
                Operand::Constant {
                    type_id,
                    value: ConstantValue::Real(self.string()?),
                }
            }
            "place" => Operand::Place(PlaceId::new(self.u32()?)),
            "element" => {
                let place = PlaceId::new(self.u32()?);
                self.expect_char(',')?;
                Operand::Element {
                    place,
                    indices: self.operands()?,
                }
            }
            "projection" => {
                let place = PlaceId::new(self.u32()?);
                self.expect_char(',')?;
                let indices = self.operands()?;
                self.expect_char(',')?;
                let offset = self.usize()?;
                self.expect_char(',')?;
                let type_id = TypeId::new(self.u32()?);
                Operand::Projection {
                    place,
                    indices,
                    offset,
                    type_id,
                }
            }
            "indirect" => {
                let base = ValueId::new(self.u32()?);
                self.expect_char(',')?;
                let offset = self.usize()?;
                self.expect_char(',')?;
                let type_id = TypeId::new(self.u32()?);
                self.expect_char(',')?;
                let volatile = self.bool()?;
                Operand::Indirect {
                    base,
                    offset,
                    type_id,
                    volatile,
                }
            }
            _ => return Err(self.error("unknown operand")),
        };
        self.expect_char(')')?;
        Ok(operand)
    }

    fn cases(&mut self) -> Result<Vec<(i64, BlockId)>, TextError> {
        self.expect_char('[')?;
        let mut cases = Vec::new();
        if self.current_after_space() == Some(']') {
            self.take();
            return Ok(cases);
        }
        loop {
            self.expect_char('(')?;
            let value = self.i64()?;
            self.expect_char(',')?;
            let target = BlockId::new(self.u32()?);
            self.expect_char(')')?;
            cases.push((value, target));
            if self.current_after_space() == Some(']') {
                self.take();
                return Ok(cases);
            }
            self.expect_char(',')?;
        }
    }

    fn skip_space(&mut self) {
        while matches!(self.current(), Some(character) if character.is_ascii_whitespace()) {
            self.take();
        }
    }

    fn current_after_space(&mut self) -> Option<char> {
        self.skip_space();
        self.current()
    }

    fn current(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn take(&mut self) -> Option<char> {
        let character = self.current()?;
        self.position += character.len_utf8();
        Some(character)
    }

    fn error(&self, message: impl Into<String>) -> TextError {
        TextError::at(self.line, message)
    }
}

fn optional_id<T: fmt::Display>(id: Option<T>) -> String {
    id.map_or_else(|| "none".to_owned(), |id| id.to_string())
}

fn bytes_hex(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "-".to_owned();
    }
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing a string cannot fail");
    }
    output
}

fn bool_name(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn signed_name(value: Option<bool>) -> &'static str {
    match value {
        None => "none",
        Some(true) => "signed",
        Some(false) => "unsigned",
    }
}

fn dialect_name(value: Dialect) -> &'static str {
    match value {
        Dialect::Qbasic11 => "qbasic11",
        Dialect::Qb45 => "qb45",
        Dialect::Pds71 => "pds71",
        Dialect::Vbdos => "vbdos",
    }
}

fn runtime_name(value: RuntimeProfile) -> &'static str {
    match value {
        RuntimeProfile::Qb45 => "qb45",
        RuntimeProfile::Pds71 => "pds71",
        RuntimeProfile::Vbdos => "vbdos",
    }
}

fn target_name(value: TargetProfile) -> &'static str {
    match value {
        TargetProfile::I386RealMode => "i386-real-mode",
    }
}

fn array_order_name(value: ArrayOrder) -> &'static str {
    match value {
        ArrayOrder::ColumnMajor => "column-major",
        ArrayOrder::RowMajor => "row-major",
    }
}

fn float_mode_name(value: FloatMode) -> &'static str {
    match value {
        FloatMode::Inline => "inline",
        FloatMode::Alternate => "alternate",
    }
}

fn stack_cleanup_name(value: StackCleanup) -> &'static str {
    match value {
        StackCleanup::Caller => "caller",
        StackCleanup::Callee => "callee",
    }
}

fn call_distance_name(value: CallDistance) -> &'static str {
    match value {
        CallDistance::Near => "near",
        CallDistance::Far => "far",
    }
}

fn parse_dialect(value: &str, line: usize) -> Result<Dialect, TextError> {
    match value {
        "qbasic11" => Ok(Dialect::Qbasic11),
        "qb45" => Ok(Dialect::Qb45),
        "pds71" => Ok(Dialect::Pds71),
        "vbdos" => Ok(Dialect::Vbdos),
        _ => Err(TextError::at(line, "unknown dialect")),
    }
}

fn parse_runtime(value: &str, line: usize) -> Result<RuntimeProfile, TextError> {
    match value {
        "qb45" => Ok(RuntimeProfile::Qb45),
        "pds71" => Ok(RuntimeProfile::Pds71),
        "vbdos" => Ok(RuntimeProfile::Vbdos),
        _ => Err(TextError::at(line, "unknown runtime profile")),
    }
}

fn parse_target(value: &str, line: usize) -> Result<TargetProfile, TextError> {
    match value {
        "i386-real-mode" => Ok(TargetProfile::I386RealMode),
        _ => Err(TextError::at(line, "unknown target profile")),
    }
}

fn parse_array_order(value: &str, line: usize) -> Result<ArrayOrder, TextError> {
    match value {
        "column-major" => Ok(ArrayOrder::ColumnMajor),
        "row-major" => Ok(ArrayOrder::RowMajor),
        _ => Err(TextError::at(line, "unknown array order")),
    }
}

fn parse_float_mode(value: &str, line: usize) -> Result<FloatMode, TextError> {
    match value {
        "inline" => Ok(FloatMode::Inline),
        "alternate" => Ok(FloatMode::Alternate),
        _ => Err(TextError::at(line, "unknown float mode")),
    }
}

fn parse_type_kind(value: &str, line: usize) -> Result<TypeKind, TextError> {
    match value {
        "void" => Ok(TypeKind::Void),
        "boolean" => Ok(TypeKind::Boolean),
        "integer" => Ok(TypeKind::Integer),
        "float" => Ok(TypeKind::Float),
        "array" => Ok(TypeKind::Array),
        "pointer" => Ok(TypeKind::Pointer),
        "opaque" => Ok(TypeKind::Opaque),
        _ => Err(TextError::at(line, "unknown type kind")),
    }
}

fn parse_signed(value: &str, line: usize) -> Result<Option<bool>, TextError> {
    match value {
        "none" => Ok(None),
        "signed" => Ok(Some(true)),
        "unsigned" => Ok(Some(false)),
        _ => Err(TextError::at(line, "expected none, signed, or unsigned")),
    }
}

fn parse_float_evaluation(value: &str, line: usize) -> Result<FloatEvaluation, TextError> {
    match value {
        "none" => Ok(FloatEvaluation::None),
        "binary32" => Ok(FloatEvaluation::Binary32),
        "binary64" => Ok(FloatEvaluation::Binary64),
        "extended80" => Ok(FloatEvaluation::Extended80),
        _ => Err(TextError::at(line, "unknown float evaluation")),
    }
}

fn parse_address(value: &str, line: usize) -> Result<AddressKind, TextError> {
    match value {
        "none" => Ok(AddressKind::None),
        "near" => Ok(AddressKind::Near),
        "far" => Ok(AddressKind::Far),
        "huge" => Ok(AddressKind::Huge),
        "code" => Ok(AddressKind::Code),
        "segment" => Ok(AddressKind::Segment),
        _ => Err(TextError::at(line, "unknown address kind")),
    }
}

fn parse_storage(value: &str, line: usize) -> Result<Storage, TextError> {
    match value {
        "local" => Ok(Storage::Local),
        "static" => Ok(Storage::Static),
        "module" => Ok(Storage::Module),
        "common" => Ok(Storage::Common),
        "external" => Ok(Storage::External),
        _ => Err(TextError::at(line, "unknown storage")),
    }
}

fn parse_linkage(value: &str, line: usize) -> Result<Linkage, TextError> {
    match value {
        "internal" => Ok(Linkage::Internal),
        "external" => Ok(Linkage::External),
        _ => Err(TextError::at(line, "unknown linkage")),
    }
}

fn parse_opcode(value: &str, line: usize) -> Result<Opcode, TextError> {
    match value {
        "copy" => Ok(Opcode::Copy),
        "load" => Ok(Opcode::Load),
        "store" => Ok(Opcode::Store),
        "address" => Ok(Opcode::Address),
        "ptr_offset" => Ok(Opcode::OffsetPointer),
        "pointer_offset" => Ok(Opcode::PointerOffset),
        "pointer_segment" => Ok(Opcode::PointerSegment),
        "concat" => Ok(Opcode::Concat),
        "convert" => Ok(Opcode::Convert),
        "float_to_int_dynamic" => Ok(Opcode::FloatToInteger {
            rounding: FloatRounding::Dynamic,
        }),
        "float_to_int_toward_zero" => Ok(Opcode::FloatToInteger {
            rounding: FloatRounding::TowardZero,
        }),
        "float_to_int_nearest_even" => Ok(Opcode::FloatToInteger {
            rounding: FloatRounding::NearestEven,
        }),
        "sign_extend" => Ok(Opcode::SignExtend),
        "zero_extend" => Ok(Opcode::ZeroExtend),
        "add" => Ok(Opcode::Add),
        "sub" => Ok(Opcode::Subtract),
        "mul" => Ok(Opcode::Multiply),
        "div" => Ok(Opcode::Divide),
        "rem" => Ok(Opcode::Remainder),
        "divmod" => Ok(Opcode::DivideRemainder),
        "and" => Ok(Opcode::And),
        "or" => Ok(Opcode::Or),
        "xor" => Ok(Opcode::Xor),
        "shl" => Ok(Opcode::ShiftLeft),
        "shr" => Ok(Opcode::ShiftRight),
        "sar" => Ok(Opcode::ShiftRightArithmetic),
        "neg" => Ok(Opcode::Negate),
        "not" => Ok(Opcode::Not),
        "eq" => Ok(Opcode::Equal),
        "ne" => Ok(Opcode::NotEqual),
        "lt" => Ok(Opcode::LessThan),
        "le" => Ok(Opcode::LessEqual),
        "gt" => Ok(Opcode::GreaterThan),
        "ge" => Ok(Opcode::GreaterEqual),
        "string_eq" => Ok(Opcode::StringEqual),
        "string_ne" => Ok(Opcode::StringNotEqual),
        "string_lt" => Ok(Opcode::StringLessThan),
        "string_le" => Ok(Opcode::StringLessEqual),
        "string_gt" => Ok(Opcode::StringGreaterThan),
        "string_ge" => Ok(Opcode::StringGreaterEqual),
        "fadd" => Ok(Opcode::FloatAdd),
        "fsub" => Ok(Opcode::FloatSubtract),
        "fmul" => Ok(Opcode::FloatMultiply),
        "fdiv" => Ok(Opcode::FloatDivide),
        "fneg" => Ok(Opcode::FloatNegate),
        "fabs" => Ok(Opcode::FloatAbsolute),
        "fsqrt" => Ok(Opcode::FloatSquareRoot),
        "fsin" => Ok(Opcode::FloatSine),
        "fcos" => Ok(Opcode::FloatCosine),
        "fatan" => Ok(Opcode::FloatArctangent),
        "flog2" => Ok(Opcode::FloatLog2),
        "fexp2" => Ok(Opcode::FloatExp2),
        "call" => Ok(Opcode::Call),
        _ => Err(TextError::at(line, "unknown opcode")),
    }
}

fn parse_stack_cleanup(value: &str, line: usize) -> Result<StackCleanup, TextError> {
    match value {
        "caller" => Ok(StackCleanup::Caller),
        "callee" => Ok(StackCleanup::Callee),
        _ => Err(TextError::at(line, "unknown stack cleanup")),
    }
}

fn parse_call_distance(value: &str, line: usize) -> Result<CallDistance, TextError> {
    match value {
        "near" => Ok(CallDistance::Near),
        "far" => Ok(CallDistance::Far),
        _ => Err(TextError::at(line, "unknown call distance")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_indexed_parameter_place() {
        let place = parse_place(Line {
            number: 1,
            text: "place 3 \"value\" 2 parameter(4) 0 0 4 near",
        })
        .expect("indexed parameter storage parses");

        assert_eq!(place.storage, Storage::Parameter { index: 4 });
        assert_eq!(place.offset, 0);
        assert_eq!(place.extent, 4);
        assert_eq!(place.storage.to_string(), "parameter(4)");
    }

    #[test]
    fn round_trips_a_complete_minimal_program() {
        let program = Program {
            version: FORMAT_VERSION,
            dialect: Dialect::Qb45,
            runtime: RuntimeProfile::Qb45,
            target: TargetProfile::I386RealMode,
            array_order: ArrayOrder::ColumnMajor,
            float_mode: FloatMode::Inline,
            modules: vec![Module {
                id: ModuleId::new(0),
                name: "main\\nmodule".into(),
                types: vec![Type {
                    id: TypeId::new(0),
                    name: "integer".into(),
                    kind: TypeKind::Integer,
                    width: 16,
                    signed: Some(true),
                    evaluation: FloatEvaluation::None,
                    element: None,
                    bounds: Vec::new(),
                    address: AddressKind::None,
                }],
                data: vec![DataObject {
                    id: DataId::new(0),
                    name: "data".into(),
                    bytes: vec![0x12],
                    readonly: true,
                    relocations: vec![DataRelocation {
                        at: 0,
                        target: DataId::new(0),
                        addend: -1,
                        address: AddressKind::Far,
                    }],
                    linkage: Linkage::Internal,
                    address: AddressKind::Near,
                }],
                callables: vec![Callable {
                    id: CallableId::new(0),
                    name: "callee".into(),
                    result_type: Some(TypeId::new(0)),
                    parameters: vec![Parameter {
                        type_id: TypeId::new(0),
                        by_value: true,
                        segmented: false,
                        array: false,
                    }],
                    defined: false,
                }],
                functions: vec![Function {
                    id: FunctionId::new(0),
                    name: "main".into(),
                    result_type: TypeId::new(0),
                    values: vec![Value {
                        id: ValueId::new(0),
                        type_id: TypeId::new(0),
                    }],
                    places: vec![Place {
                        id: PlaceId::new(0),
                        name: "local".into(),
                        type_id: TypeId::new(0),
                        storage: Storage::Local,
                        offset: -2,
                        symbol: DataId::new(0),
                        extent: 2,
                        address: AddressKind::Near,
                    }],
                    blocks: vec![Block {
                        id: BlockId::new(0),
                        instructions: vec![Instruction {
                            id: InstructionId::new(0),
                            opcode: Opcode::Call,
                            results: vec![ValueId::new(0)],
                            operands: vec![Operand::Projection {
                                place: PlaceId::new(0),
                                indices: vec![Operand::Constant {
                                    type_id: TypeId::new(0),
                                    value: ConstantValue::Real("1.0".into()),
                                }],
                                offset: 0,
                                type_id: TypeId::new(0),
                            }],
                            callee: Some("callee".into()),
                        }],
                        terminator: Terminator::Return(Some(Operand::Indirect {
                            base: ValueId::new(0),
                            offset: 0,
                            type_id: TypeId::new(0),
                            volatile: false,
                        })),
                    }],
                    entry: BlockId::new(0),
                    parameters: vec![ValueId::new(0)],
                    abi: ProcedureAbi {
                        cleanup: StackCleanup::Caller,
                        distance: CallDistance::Far,
                        parameter_bytes: 2,
                    },
                    calls: vec![CallAbi {
                        instruction: InstructionId::new(0),
                        order: vec![0],
                        cleanup: StackCleanup::Callee,
                        distance: CallDistance::Near,
                        callee: Some(CallableId::new(0)),
                    }],
                    error_handler: Some(BlockId::new(0)),
                    error_handler_local: true,
                    external_entries: vec![BlockId::new(0)],
                    linkage: Linkage::External,
                }],
            }],
        };
        let text = write(&program);
        assert_eq!(parse(&text), Ok(program));
        assert_eq!(write(&parse(&text).expect("valid qhir")), text);
    }

    #[test]
    fn rejects_an_unknown_version() {
        let error = parse("qhir 3\n").expect_err("unknown version is invalid");
        assert_eq!(error.line, 1);
        assert!(error.message.contains("unsupported qhir version"));
        let error = parse("qhir 1\n").expect_err("the previous qhir schema is invalid");
        assert_eq!(error.line, 1);
        assert!(error.message.contains("unsupported qhir version"));
    }

    #[test]
    fn float_to_integer_rounding_opcodes_have_distinct_canonical_spellings() {
        for (opcode, text) in [
            (
                Opcode::FloatToInteger {
                    rounding: FloatRounding::Dynamic,
                },
                "float_to_int_dynamic",
            ),
            (
                Opcode::FloatToInteger {
                    rounding: FloatRounding::TowardZero,
                },
                "float_to_int_toward_zero",
            ),
            (
                Opcode::FloatToInteger {
                    rounding: FloatRounding::NearestEven,
                },
                "float_to_int_nearest_even",
            ),
        ] {
            assert_eq!(opcode.as_str(), text);
            assert_eq!(parse_opcode(text, 1), Ok(opcode));
        }
    }
}
