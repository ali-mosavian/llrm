//! Writes MIR as LLVM's assembly language, following LLVM's `AsmWriter`
//! except that function attributes are written inline, not as `#N` groups.

use std::fmt::Write;

use crate::context::{Constant, ConstantExpr, ConstantId, ConstantKind, Context, signed};
use crate::hash::HashMap;
use crate::lexer::is_name;
use crate::module::{
    BlockId, DebugWhat, Function, GlobalKind, GlobalValue, InstId, LINKAGE, Linkage, MetadataId, MetadataOperand,
    Module, Operand, Special, UnnamedAddr, ValueId,
};
use crate::opcode::{
    AtomicKind, Attribute, CAST, Clause, FLOAT_PREDICATE, INT_PREDICATE, Opcode, RMW_OPS, Tail, spelling,
};
use crate::types::{FloatKind, Type, TypeId, struct_text};

pub fn module(module: &Module) -> String {
    let mut out = String::new();
    let printer = Printer { context: &module.context, globals: global_names(module) };
    if let Some(layout) = &module.datalayout {
        let _ = writeln!(out, "target datalayout = {}", string(layout.as_bytes()));
    }
    let types = &module.context.types;
    section(
        &mut out,
        types
            .named
            .iter()
            .map(
                |(name, body)| {
                    let text = match body {
                        None => "opaque".to_owned(),
                        Some(body) => struct_text(
                            &body.fields.iter().map(|&one| types.display(one)).collect::<Vec<_>>().join(", "),
                            body.packed,
                        ),
                    };
                    format!("%{} = type {text}\n", quoted(name))
                },
            ),
    );
    section(
        &mut out,
        module
            .globals
            .iter()
            .enumerate()
            .filter(|(_, one)| matches!(one.kind, GlobalKind::Variable(_)))
            .map(|(at, one)| printer.variable(at, one, module.global_metadata.get(&(at as u32)))),
    );
    for (at, global) in module.globals.iter().enumerate() {
        if let GlobalKind::Function(function) = &global.kind {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&printer.function(at, global, function));
        }
    }
    section(
        &mut out,
        module
            .named_metadata
            .iter()
            .map(
                |(name, nodes)| {
                    let list = nodes.iter().map(|one| format!("!{}", one.0)).collect::<Vec<_>>().join(", ");
                    format!("!{name} = !{{{list}}}\n")
                },
            ),
    );
    let nodes: Vec<String> = module
        .metadata
        .iter()
        .enumerate()
        .map(|(at, node)| match module.special.get(&MetadataId(at as u32)) {
            Some(special) => format!(
                "!{at} = {}!{}({})\n",
                if node.distinct { "distinct " } else { "" },
                special.kind,
                printer.special_operands(special, &node.operands)
            ),
            None => format!(
                "!{at} = {}!{{{}}}\n",
                if node.distinct { "distinct " } else { "" },
                printer.metadata_operands(&node.operands)
            ),
        })
        .collect();
    section(&mut out, nodes.into_iter());
    out
}

/// `function`'s blocks, naming globals by index: two bodies print alike
/// when they are the same up to value and block numbering.
pub fn body(
    context: &Context,
    function: &Function,
) -> String {
    Printer { context, globals: Vec::new() }.blocks(function, &Slots::new(function))
}

/// A 128-bit digest (not a cryptographic one) of what `body` prints, without
/// printing it: the same for two bodies that print alike (up to value and block
/// numbering), and, but for a collision of 2^-128, different for two that do
/// not. The fixed point keeps one of every state it has been in to know a
/// cycle, and a printed body (formatted a line at a time, then held and
/// compared) was 1.4% of a d_faces compile.
pub fn fingerprint(function: &Function) -> u128 {
    crate::counts::bump(crate::counts::Event::Fingerprints);
    use std::hash::{Hash, Hasher};

    /// Two multiply-and-rotate hashes (rustc's Fx) with different salts and
    /// multipliers, both fed what the other is: not keyed, for input no one
    /// chooses against it.
    struct Wide(u64, u64);
    impl Wide {
        fn add(
            &mut self,
            word: u64,
        ) {
            self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95);
            self.1 = (self.1.rotate_left(23) ^ word).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        }
    }
    impl Hasher for Wide {
        fn write(
            &mut self,
            bytes: &[u8],
        ) {
            let mut chunks = bytes.chunks_exact(8);
            for chunk in &mut chunks {
                self.add(u64::from_le_bytes(chunk.try_into().expect("eight bytes")));
            }
            for &byte in chunks.remainder() {
                self.add(u64::from(byte));
            }
        }

        fn write_u8(
            &mut self,
            word: u8,
        ) {
            self.add(u64::from(word));
        }

        fn write_u32(
            &mut self,
            word: u32,
        ) {
            self.add(u64::from(word));
        }

        fn write_u64(
            &mut self,
            word: u64,
        ) {
            self.add(word);
        }

        fn write_usize(
            &mut self,
            word: usize,
        ) {
            self.add(word as u64);
        }

        fn finish(&self) -> u64 {
            self.0
        }
    }
    let mut digest = Wide(0, 0x2545_f491_4f6c_dd1d);
    // The slots `Slots` gives: parameters, then each block and its results in
    // layout order, a name where there is one, else the next number; what is
    // hashed is the slot where the text shows it, not where it is made.
    let mut values = crate::dense::IdMap::<ValueId, (Option<&str>, u32)>::new();
    let mut blocks = crate::dense::IdMap::<BlockId, (Option<&str>, u32)>::new();
    let mut next = 0u32;
    fn slot<'f>(
        next: &mut u32,
        given: &'f Option<String>,
    ) -> (Option<&'f str>, u32) {
        let number = *next;
        *next += u32::from(given.is_none());
        (given.as_deref(), number)
    }
    for &parameter in &function.parameters {
        values.insert(parameter, slot(&mut next, &function.value(parameter).name));
    }
    for &block in &function.layout {
        blocks.insert(block, slot(&mut next, &function.blocks[block.0 as usize].name));
        for inst in function.iter_block(block) {
            if let Some(result) = function.instruction(inst).result {
                values.insert(result, slot(&mut next, &function.value(result).name));
            }
        }
    }
    fn shown<'a>(
        slot: Option<&(Option<&'a str>, u32)>,
        raw: u32,
    ) -> (u8, &'a str, u32) {
        match slot {
            Some((Some(name), _)) => (1, name, 0),
            Some((None, number)) => (2, "", *number),
            None => (3, "", raw),
        }
    }
    let operand = |digest: &mut Wide, operand: Operand| match operand {
        Operand::Value(id) => (0u8, shown(values.get(&id), id.0)).hash(digest),
        Operand::Constant(id) => (1u8, id).hash(digest),
        Operand::Block(id) => (2u8, shown(blocks.get(&id), id.0)).hash(digest),
    };
    let mut records = crate::dense::IdMap::<InstId, Vec<&crate::module::DebugRecord>>::new();
    for record in function.debug_records() {
        records.get_or_insert_with(record.before, Vec::new).push(record);
    }
    for (index, &block) in function.layout.iter().enumerate() {
        digest.write_u8(0xb);
        if index > 0 || function.block(block).name.is_some() {
            shown(blocks.get(&block), block.0).hash(&mut digest);
        }
        for inst in function.iter_block(block) {
            for record in records.get(&inst).into_iter().flatten() {
                record.variable.hash(&mut digest);
                match record.what {
                    DebugWhat::Declare(at) => {
                        digest.write_u8(1);
                        operand(&mut digest, at);
                    }
                    DebugWhat::Value(at) => {
                        digest.write_u8(2);
                        operand(&mut digest, at);
                    }
                    DebugWhat::Piece { value, offset, bytes } => {
                        digest.write_u8(3);
                        operand(&mut digest, value);
                        (offset, bytes).hash(&mut digest);
                    }
                    DebugWhat::GonePiece { offset, bytes } => (4u8, offset, bytes).hash(&mut digest),
                    DebugWhat::Gone => digest.write_u8(5),
                }
            }
            let instruction = function.instruction(inst);
            instruction.opcode.hash(&mut digest);
            (instruction.ty, instruction.flags, instruction.operands.len()).hash(&mut digest);
            for &one in &instruction.operands {
                operand(&mut digest, one);
            }
            instruction.result.map(|result| shown(values.get(&result), result.0)).hash(&mut digest);
            instruction.metadata.hash(&mut digest);
        }
    }
    u128::from(digest.0) << 64 | u128::from(digest.1)
}

/// Lines under a blank line, when there are any.
fn section(
    out: &mut String,
    lines: impl Iterator<Item = String>,
) {
    let lines: Vec<String> = lines.collect();
    if !lines.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        lines.iter().for_each(|line| out.push_str(line));
    }
}

/// `name` as LLVM writes it after a sigil: bare when it can be, else quoted.
pub fn quoted(name: &str) -> String {
    let bare = !name.is_empty() && !name.as_bytes()[0].is_ascii_digit() && name.bytes().all(is_name);
    if bare { name.to_owned() } else { string(name.as_bytes()) }
}

/// A quoted string, escaping as LLVM does.
fn string(bytes: &[u8]) -> String {
    let mut out = String::from("\"");
    for &b in bytes {
        if b == b'\\' {
            out.push_str("\\\\");
        } else if b.is_ascii_graphic() && b != b'"' || b == b' ' {
            out.push(b as char);
        } else {
            let _ = write!(out, "\\{b:02X}");
        }
    }
    out.push('"');
    out
}

/// `@name`, or `@N` for the unnamed, numbered in order.
fn global_names(module: &Module) -> Vec<String> {
    let mut next = 0;
    module
        .globals
        .iter()
        .map(|one| match one.name.as_deref() {
            Some(name) => format!("@{}", quoted(name)),
            None => {
                next += 1;
                format!("@{}", next - 1)
            }
        })
        .collect()
}

/// LLVM's spelling of a floating constant: `%e` when that reads back exactly,
/// else the hex of its bits as a double.
fn float(
    kind: FloatKind,
    bits: u64,
) -> String {
    let value = match kind {
        FloatKind::Double | FloatKind::X86Fp80 => f64::from_bits(bits),
        FloatKind::Float => f64::from(f32::from_bits(bits as u32)),
    };
    if value.is_finite() {
        let text = format!("{value:.6e}");
        let (mantissa, exponent) = text.split_once('e').expect("exponent form");
        let exponent: i32 = exponent.parse().expect("an exponent");
        let text = format!("{mantissa}e{}{:02}", if exponent < 0 { '-' } else { '+' }, exponent.abs());
        if text.parse::<f64>().is_ok_and(|back| back.to_bits() == value.to_bits()) {
            return text;
        }
    }
    format!("0x{:016X}", value.to_bits())
}

fn attributes(
    context: &Context,
    attrs: &[Attribute],
) -> String {
    attrs
        .iter()
        .map(|one| match one {
            Attribute::Flag(name) => name.clone(),
            Attribute::Int(name, value) if name == "align" => format!("align {value}"),
            Attribute::Int(name, value) => format!("{name}({value})"),
            Attribute::Type(name, ty) => format!("{name}({})", context.types.display(*ty)),
            Attribute::Memory(effects) => {
                let list: Vec<String> = effects
                    .iter()
                    .map(|(location, access)| match location {
                        Some(location) => format!("{location}: {access}"),
                        None => access.clone(),
                    })
                    .collect();
                format!("memory({})", list.join(", "))
            }
            Attribute::Range { ty, lower, upper } => {
                let bits = context.types.int_bits(*ty).expect("a range is of an integer type");
                format!("range({} {}, {})", context.types.display(*ty), signed(*lower, bits), signed(*upper, bits))
            }
            Attribute::Initializes(ranges) => {
                let list: Vec<String> = ranges.iter().map(|(lower, upper)| format!("({lower}, {upper})")).collect();
                format!("initializes({})", list.join(", "))
            }
            Attribute::Str(key, None) => string(key.as_bytes()),
            Attribute::Str(key, Some(value)) => format!("{}={}", string(key.as_bytes()), string(value.as_bytes())),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// ` attrs`, or nothing.
fn spaced(text: String) -> String {
    if text.is_empty() { text } else { format!(" {text}") }
}

struct Printer<'a> {
    context: &'a Context,
    globals: Vec<String>,
}

/// A function's local names: named values as they are, the rest numbered.
struct Slots {
    values: HashMap<ValueId, String>,
    blocks: HashMap<BlockId, String>,
}

/// The label each parameter and result is written with, `%` left off: its
/// name, else its slot.
pub fn value_labels(function: &Function) -> crate::dense::IdMap<ValueId, String> {
    let mut labels = crate::dense::IdMap::new();
    for (value, label) in Slots::new(function).values {
        labels.insert(value, label);
    }
    labels
}

/// The label each placed block is written with, `%` left off: its name, else
/// its slot.
pub fn block_labels(function: &Function) -> crate::dense::IdMap<BlockId, String> {
    let mut labels = crate::dense::IdMap::new();
    for (block, label) in Slots::new(function).blocks {
        labels.insert(block, label);
    }
    labels
}

impl Slots {
    fn new(function: &Function) -> Self {
        let mut next = 0;
        let mut name = |given: &Option<String>| match given {
            Some(name) => quoted(name),
            None => {
                next += 1;
                (next - 1).to_string()
            }
        };
        let mut values = HashMap::default();
        let mut blocks = HashMap::default();
        for &parameter in &function.parameters {
            values.insert(parameter, name(&function.value(parameter).name));
        }
        for &block in &function.layout {
            blocks.insert(block, name(&function.block(block).name));
            for inst in function.iter_block(block) {
                if let Some(result) = function.instruction(inst).result {
                    values.insert(result, name(&function.value(result).name));
                }
            }
        }
        Self { values, blocks }
    }
}

impl Printer<'_> {
    fn types(&self) -> &crate::types::Types {
        &self.context.types
    }

    fn ty(
        &self,
        ty: TypeId,
    ) -> String {
        self.types().display(ty)
    }

    fn linkage(linkage: Linkage) -> Option<&'static str> {
        (linkage != Linkage::External).then(|| spelling(&LINKAGE, linkage))
    }

    fn unnamed_addr(unnamed_addr: UnnamedAddr) -> Option<&'static str> {
        match unnamed_addr {
            UnnamedAddr::None => None,
            UnnamedAddr::Local => Some("local_unnamed_addr"),
            UnnamedAddr::Global => Some("unnamed_addr"),
        }
    }

    /// A calling convention as LLVM writes it; none for C's.
    fn convention(number: u32) -> Option<String> {
        // Only the conventions LLVM names are named: llrm's own are `cc N`,
        // which LLVM's parser reads.
        match crate::opcode::CONVENTIONS.iter().find(|(_, one)| *one == number) {
            Some((_, 0)) => None,
            Some((name, one)) if *one < 1000 => Some((*name).to_owned()),
            _ => Some(format!("cc {number}")),
        }
    }

    fn address_space(space: u32) -> Option<String> {
        (space != 0).then(|| format!("addrspace({space})"))
    }

    fn variable(
        &self,
        at: usize,
        global: &GlobalValue,
        attached: Option<&Vec<(String, MetadataId)>>,
    ) -> String {
        let GlobalKind::Variable(variable) = &global.kind else { unreachable!("a variable") };
        let linkage = match (global.linkage, variable.initializer) {
            (Linkage::External, None) => Some("external"),
            (linkage, _) => Self::linkage(linkage),
        };
        let mut words: Vec<String> =
            [linkage, Self::unnamed_addr(global.unnamed_addr)].into_iter().flatten().map(str::to_owned).collect();
        words.extend(Self::address_space(global.address_space));
        if variable.externally_initialized {
            words.push("externally_initialized".to_owned());
        }
        words.push((if variable.constant { "constant" } else { "global" }).to_owned());
        words.push(self.ty(variable.ty));
        words.extend(variable.initializer.map(|one| self.constant(one)));
        let align = variable.align.map(|one| format!(", align {one}")).unwrap_or_default();
        let attached: String =
            attached.into_iter().flatten().map(|(kind, node)| format!(", !{kind} !{}", node.0)).collect();
        format!("{} = {}{align}{attached}\n", self.globals[at], words.join(" "))
    }

    fn function(
        &self,
        at: usize,
        global: &GlobalValue,
        function: &Function,
    ) -> String {
        let (returns, parameter_types, variadic) = self.signature(function.ty);
        let slots = Slots::new(function);
        let declaration = function.is_declaration();
        let mut parameters: Vec<String> = parameter_types
            .iter()
            .enumerate()
            .map(|(index, &ty)| {
                let attrs = spaced(attributes(&self.context, &function.parameter_attrs[index]));
                let name = if declaration {
                    String::new()
                } else {
                    format!(" %{}", slots.values[&function.parameters[index]])
                };
                format!("{}{attrs}{name}", self.ty(ty))
            })
            .collect();
        if variadic {
            parameters.push("...".to_owned());
        }
        let mut words: Vec<String> = vec![(if declaration { "declare" } else { "define" }).to_owned()];
        // A declaration has no linkage but external's, in LLVM's syntax.
        if !declaration {
            words.extend(Self::linkage(global.linkage).map(str::to_owned));
        }
        words.extend(Self::convention(function.calling_convention));
        let return_attrs = attributes(&self.context, &function.return_attrs);
        if !return_attrs.is_empty() {
            words.push(return_attrs);
        }
        words.push(self.ty(returns));
        words.push(format!("{}({})", self.globals[at], parameters.join(", ")));
        words.extend(Self::unnamed_addr(global.unnamed_addr).map(str::to_owned));
        words.extend(Self::address_space(global.address_space));
        let attrs = attributes(&self.context, &function.attrs);
        if !attrs.is_empty() {
            words.push(attrs);
        }
        if let Some(personality) = function.personality {
            words.push(format!("personality {}", self.typed_constant(personality)));
        }
        if declaration {
            // Clang's spelling: the attachments follow `declare`.
            for (at, (kind, node)) in function.metadata.iter().enumerate() {
                words.insert(1 + at, format!("!{kind} !{}", node.0));
            }
        } else {
            for (kind, node) in &function.metadata {
                words.push(format!("!{kind} !{}", node.0));
            }
        }
        let mut out = words.join(" ");
        if declaration {
            out.push('\n');
            return out;
        }
        out.push_str(" {\n");
        out.push_str(&self.blocks(function, &slots));
        out.push_str("}\n");
        out
    }

    fn blocks(
        &self,
        function: &Function,
        slots: &Slots,
    ) -> String {
        let mut out = String::new();
        for (index, &block) in function.layout.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            if index > 0 || function.block(block).name.is_some() {
                let _ = writeln!(out, "{}:", slots.blocks[&block]);
            }
            for inst in function.iter_block(block) {
                for record in function.debug_records().iter().filter(|one| one.before == inst) {
                    let extra: String = record
                        .expression
                        .iter()
                        .chain(record.location.iter())
                        .map(|node| format!(", !{}", node.0))
                        .collect();
                    let _ = match record.what {
                        DebugWhat::Declare(at) => writeln!(
                            out,
                            "  #dbg_declare({}, !{}{extra})",
                            self.typed(function, slots, at),
                            record.variable.0
                        ),
                        DebugWhat::Value(at) => writeln!(
                            out,
                            "  #dbg_value({}, !{}{extra})",
                            self.typed(function, slots, at),
                            record.variable.0
                        ),
                        DebugWhat::Piece { value, offset, bytes } => writeln!(
                            out,
                            "  #dbg_piece({}, !{}, {offset}, {bytes})",
                            self.typed(function, slots, value),
                            record.variable.0
                        ),
                        DebugWhat::GonePiece { offset, bytes } => {
                            writeln!(out, "  #dbg_gonepiece(!{}, {offset}, {bytes})", record.variable.0)
                        }
                        DebugWhat::Gone => writeln!(out, "  #dbg_gone(!{})", record.variable.0),
                    };
                }
                out.push_str("  ");
                out.push_str(&self.instruction(function, slots, inst));
                out.push('\n');
            }
        }
        out
    }

    fn signature(
        &self,
        function_type: TypeId,
    ) -> (TypeId, &[TypeId], bool) {
        match self.context.types.get(function_type) {
            Type::Function { returns, parameters, variadic } => (*returns, parameters, *variadic),
            other => panic!("{other:?} is not a function type"),
        }
    }

    fn operand(
        &self,
        slots: &Slots,
        operand: Operand,
    ) -> String {
        match operand {
            Operand::Value(id) => format!("%{}", slots.values[&id]),
            Operand::Constant(id) => self.constant(id),
            Operand::Block(id) => format!("%{}", slots.blocks[&id]),
        }
    }

    fn typed(
        &self,
        function: &Function,
        slots: &Slots,
        operand: Operand,
    ) -> String {
        match operand {
            Operand::Block(_) => format!("label {}", self.operand(slots, operand)),
            _ => format!(
                "{} {}",
                self.ty(function.operand_type(&self.context, operand).expect("a value")),
                self.operand(slots, operand)
            ),
        }
    }

    fn instruction(
        &self,
        function: &Function,
        slots: &Slots,
        id: InstId,
    ) -> String {
        let inst = function.instruction(id);
        let ops = &inst.operands;
        let typed = |at: usize| self.typed(function, slots, ops[at]);
        let bare = |at: usize| self.operand(slots, ops[at]);
        let flags = inst.flags.words().iter().map(|word| format!(" {word}")).collect::<String>();
        let mut body = match &inst.opcode {
            Opcode::Ret if ops.is_empty() => "ret void".to_owned(),
            Opcode::Ret | Opcode::Resume => format!("{} {}", inst.opcode.mnemonic(), typed(0)),
            Opcode::Br if ops.len() == 1 => format!("br {}", typed(0)),
            Opcode::Br => format!("br {}, {}, {}", typed(0), typed(1), typed(2)),
            Opcode::Switch => {
                let cases: String = ops[2..]
                    .chunks(2)
                    .map(|pair| {
                        format!(
                            "    {}, {}\n",
                            self.typed(function, slots, pair[0]),
                            self.typed(function, slots, pair[1])
                        )
                    })
                    .collect();
                format!("switch{flags} {}, {} [\n{cases}  ]", typed(0), typed(1))
            }
            Opcode::Unreachable => "unreachable".to_owned(),
            Opcode::FNeg | Opcode::Freeze => format!("{}{flags} {}", inst.opcode.mnemonic(), typed(0)),
            Opcode::Binary(_) => format!("{}{flags} {}, {}", inst.opcode.mnemonic(), typed(0), bare(1)),
            Opcode::Cast(_) => format!("{}{flags} {} to {}", inst.opcode.mnemonic(), typed(0), self.ty(inst.ty)),
            Opcode::ICmp(predicate) => {
                format!("icmp{flags} {} {}, {}", spelling(&INT_PREDICATE, *predicate), typed(0), bare(1))
            }
            Opcode::FCmp(predicate) => {
                format!("fcmp{flags} {} {}, {}", spelling(&FLOAT_PREDICATE, *predicate), typed(0), bare(1))
            }
            Opcode::Select => format!("select{flags} {}, {}, {}", typed(0), typed(1), typed(2)),
            Opcode::Phi => {
                let inputs: Vec<String> = ops
                    .chunks(2)
                    .map(|pair| format!("[ {}, {} ]", self.operand(slots, pair[0]), self.operand(slots, pair[1])))
                    .collect();
                format!("phi{flags} {} {}", self.ty(inst.ty), inputs.join(", "))
            }
            Opcode::ExtractValue(indices) | Opcode::InsertValue(indices) => {
                let values: Vec<String> = (0..ops.len()).map(typed).collect();
                let indices: Vec<String> = indices.iter().map(u32::to_string).collect();
                format!("{} {}, {}", inst.opcode.mnemonic(), values.join(", "), indices.join(", "))
            }
            Opcode::Alloca { allocated, align, address_space } => {
                let mut text = format!("alloca {}", self.ty(*allocated));
                if !ops.is_empty() {
                    let _ = write!(text, ", {}", typed(0));
                }
                if let Some(align) = align {
                    let _ = write!(text, ", align {align}");
                }
                if *address_space != 0 {
                    let _ = write!(text, ", addrspace({address_space})");
                }
                text
            }
            Opcode::Load { align, volatile } => {
                let align = align.map(|one| format!(", align {one}")).unwrap_or_default();
                format!("load {}{}, {}{align}", if *volatile { "volatile " } else { "" }, self.ty(inst.ty), typed(0))
            }
            Opcode::Store { align, volatile } => {
                let align = align.map(|one| format!(", align {one}")).unwrap_or_default();
                format!("store {}{}, {}{align}", if *volatile { "volatile " } else { "" }, typed(0), typed(1))
            }
            Opcode::Atomic(info) => {
                let align = info.align.map(|one| format!(", align {one}")).unwrap_or_default();
                let volatile = if info.volatile { "volatile " } else { "" };
                let scope = if info.single_thread { " syncscope(\"singlethread\")" } else { "" };
                let order = info.ordering.spelling();
                match info.kind {
                    AtomicKind::Load => {
                        format!("load atomic {volatile}{}, {}{scope} {order}{align}", self.ty(inst.ty), typed(0))
                    }
                    AtomicKind::Store => {
                        format!("store atomic {volatile}{}, {}{scope} {order}{align}", typed(0), typed(1))
                    }
                    AtomicKind::Rmw(op) => {
                        let name = RMW_OPS.iter().find(|(one, _)| *one == op).map_or("xchg", |(_, name)| name);
                        format!("atomicrmw {volatile}{name} {}, {}{scope} {order}{align}", typed(0), typed(1))
                    }
                    AtomicKind::CmpXchg { failure, weak } => format!(
                        "cmpxchg {}{volatile}{}, {}, {}{scope} {order} {}{align}",
                        if weak { "weak " } else { "" },
                        typed(0),
                        typed(1),
                        typed(2),
                        failure.spelling()
                    ),
                    AtomicKind::Fence => format!("fence{scope} {order}"),
                }
            }
            Opcode::GetElementPtr { source } => {
                let rest: Vec<String> = (0..ops.len()).map(typed).collect();
                format!("getelementptr{flags} {}, {}", self.ty(*source), rest.join(", "))
            }
            Opcode::Call(info) | Opcode::Invoke(info) => {
                let invoke = matches!(inst.opcode, Opcode::Invoke(_));
                let arguments = if invoke { &ops[..ops.len() - 3] } else { &ops[..ops.len() - 1] };
                let (returns, _, variadic) = self.signature(info.function_type);
                let callee_ty = if variadic { self.ty(info.function_type) } else { self.ty(returns) };
                let arguments: Vec<String> = arguments
                    .iter()
                    .enumerate()
                    .map(|(at, &one)| {
                        let attrs = spaced(attributes(&self.context, &info.argument_attrs[at]));
                        format!(
                            "{}{attrs} {}",
                            self.ty(function.operand_type(&self.context, one).expect("a value")),
                            self.operand(slots, one)
                        )
                    })
                    .collect();
                let tail = match info.tail {
                    Tail::None => "",
                    Tail::Tail => "tail ",
                    Tail::MustTail => "musttail ",
                    Tail::NoTail => "notail ",
                };
                let return_attrs = spaced(attributes(&self.context, &info.return_attrs));
                let callee_operand = *ops.last().expect("a callee");
                let callee = self.operand(slots, callee_operand);
                let convention =
                    Self::convention(info.calling_convention).map_or_else(String::new, |one| format!(" {one}"));
                let space =
                    match function.operand_type(&self.context, callee_operand).map(|ty| self.context.types.get(ty)) {
                        Some(Type::Pointer(space)) if *space != 0 => format!(" addrspace({space})"),
                        _ => String::new(),
                    };
                let mut text = format!(
                    "{tail}{}{flags}{convention}{return_attrs}{space} {callee_ty} {callee}({}){}",
                    inst.opcode.mnemonic(),
                    arguments.join(", "),
                    spaced(attributes(&self.context, &info.attrs))
                );
                if invoke {
                    let _ = write!(text, "\n          to {} unwind {}", typed(ops.len() - 3), typed(ops.len() - 2));
                }
                text
            }
            Opcode::LandingPad { cleanup, clauses } => {
                let mut text = format!("landingpad {}", self.ty(inst.ty));
                if *cleanup {
                    text.push_str("\n          cleanup");
                }
                for (at, clause) in clauses.iter().enumerate() {
                    let word = if *clause == Clause::Catch { "catch" } else { "filter" };
                    let _ = write!(text, "\n          {word} {}", typed(at));
                }
                text
            }
        };
        for (kind, node) in &inst.metadata {
            let _ = write!(body, ", !{kind} !{}", node.0);
        }
        match inst.result {
            Some(result) => format!("%{} = {body}", slots.values[&result]),
            None => body,
        }
    }

    fn typed_constant(
        &self,
        id: ConstantId,
    ) -> String {
        format!("{} {}", self.ty(self.context.get(id).ty), self.constant(id))
    }

    fn constant(
        &self,
        id: ConstantId,
    ) -> String {
        let Constant { ty, kind } = self.context.get(id);
        let members =
            |ids: &[ConstantId]| ids.iter().map(|&one| self.typed_constant(one)).collect::<Vec<_>>().join(", ");
        match kind {
            ConstantKind::Int(bits) => match self.types().int_bits(*ty) {
                Some(1) => (if *bits == 0 { "false" } else { "true" }).to_owned(),
                Some(width) => signed(*bits, width).to_string(),
                None => unreachable!("an integer constant has an integer type"),
            },
            ConstantKind::Float(bits) => match self.types().get(*ty) {
                Type::Float(kind) => float(*kind, *bits),
                _ => unreachable!("a floating constant has a floating type"),
            },
            ConstantKind::Float80(bits) => format!("0xK{bits:020X}"),
            ConstantKind::Null => "null".to_owned(),
            ConstantKind::Poison => "poison".to_owned(),
            ConstantKind::Undef => "undef".to_owned(),
            ConstantKind::Zero => "zeroinitializer".to_owned(),
            ConstantKind::Bytes(bytes) => format!("c{}", string(bytes)),
            ConstantKind::Global(global) => {
                self.globals.get(global.0 as usize).cloned().unwrap_or_else(|| format!("@{}", global.0))
            }
            ConstantKind::Aggregate(ids) => match self.types().get(*ty) {
                Type::Array { .. } => format!("[{}]", members(ids)),
                Type::Vector { .. } => format!("<{}>", members(ids)),
                Type::Struct { packed: true, .. } => format!("<{{ {} }}>", members(ids)),
                Type::Named(name) if self.types().body(name).is_some_and(|body| body.packed) => {
                    format!("<{{ {} }}>", members(ids))
                }
                _ if ids.is_empty() => "{}".to_owned(),
                _ => format!("{{ {} }}", members(ids)),
            },
            ConstantKind::Expr(ConstantExpr::GetElementPtr { source, inbounds, nusw, nuw, operands }) => {
                let rest: Vec<String> = operands.iter().map(|&one| self.typed_constant(one)).collect();
                format!(
                    "getelementptr {}({}, {})",
                    gep_flag_words(*inbounds, *nusw, *nuw),
                    self.ty(*source),
                    rest.join(", ")
                )
            }
            ConstantKind::Expr(ConstantExpr::Binary { op, lhs, rhs }) => {
                format!(
                    "{} ({}, {})",
                    spelling(&crate::opcode::BINARY, *op),
                    self.typed_constant(*lhs),
                    self.typed_constant(*rhs)
                )
            }
            ConstantKind::Expr(ConstantExpr::Cast { op, value }) => {
                format!("{} ({} to {})", spelling(&CAST, *op), self.typed_constant(*value), self.ty(*ty))
            }
        }
    }

    fn metadata_operands(
        &self,
        operands: &[MetadataOperand],
    ) -> String {
        operands
            .iter()
            .map(|one| match one {
                MetadataOperand::Null => "null".to_owned(),
                MetadataOperand::Node(node) => format!("!{}", node.0),
                MetadataOperand::String(text) => format!("!{}", string(text.as_bytes())),
                MetadataOperand::Constant(id) => self.typed_constant(*id),
                MetadataOperand::Ident(word) => word.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// A specialized node's values, each after its field's name: strings and
    /// integers bare, as LLVM writes them in `!DILocation(line: 3)`.
    fn special_operands(
        &self,
        special: &Special,
        operands: &[MetadataOperand],
    ) -> String {
        operands
            .iter()
            .zip(&special.fields)
            .map(|(one, field)| {
                let value = match one {
                    MetadataOperand::Null => "null".to_owned(),
                    MetadataOperand::Node(node) => format!("!{}", node.0),
                    MetadataOperand::String(text) => string(text.as_bytes()),
                    MetadataOperand::Constant(id) => match &self.context.get(*id).kind {
                        ConstantKind::Int(value) => (*value as u64 as i64).to_string(),
                        _ => self.typed_constant(*id),
                    },
                    MetadataOperand::Ident(word) => word.clone(),
                };
                if field.is_empty() { value } else { format!("{field}: {value}") }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The flag words of a constant GEP, each followed by a space.
pub(crate) fn gep_flag_words(
    inbounds: bool,
    nusw: bool,
    nuw: bool,
) -> String {
    let mut words = String::new();
    if inbounds {
        words.push_str("inbounds ");
    } else if nusw {
        words.push_str("nusw ");
    }
    if nuw {
        words.push_str("nuw ");
    }
    words
}
