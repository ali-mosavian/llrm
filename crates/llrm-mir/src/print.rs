//! Writes MIR as LLVM's assembly language, following LLVM's `AsmWriter`
//! except that function attributes are written inline, not as `#N` groups.

use std::collections::HashMap;
use std::fmt::Write;

use crate::context::{Constant, ConstantExpr, ConstantId, ConstantKind, Context, signed};
use crate::lexer::is_name;
use crate::module::{
    BlockId, Function, GlobalKind, GlobalValue, InstId, LINKAGE, Linkage, MetadataOperand, Module, Operand, UnnamedAddr, ValueId,
};
use crate::opcode::{Attribute, CAST, Clause, FLOAT_PREDICATE, INT_PREDICATE, Opcode, Tail, spelling};
use crate::types::{FloatKind, Type, TypeId, struct_text};

pub fn module(module: &Module) -> String {
    let mut out = String::new();
    let printer = Printer { module, globals: global_names(module) };
    if let Some(layout) = &module.datalayout {
        let _ = writeln!(out, "target datalayout = {}", string(layout.as_bytes()));
    }
    let types = &module.context.types;
    section(&mut out, types.named.iter().map(|(name, body)| {
        let text = match body {
            None => "opaque".to_owned(),
            Some(body) => struct_text(&body.fields.iter().map(|&one| types.display(one)).collect::<Vec<_>>().join(", "), body.packed),
        };
        format!("%{} = type {text}\n", quoted(name))
    }));
    section(
        &mut out,
        module.globals.iter().enumerate().filter(|(_, one)| matches!(one.kind, GlobalKind::Variable(_))).map(|(at, one)| printer.variable(at, one)),
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
        module.named_metadata.iter().map(|(name, nodes)| {
            let list = nodes.iter().map(|one| format!("!{}", one.0)).collect::<Vec<_>>().join(", ");
            format!("!{name} = !{{{list}}}\n")
        }),
    );
    let nodes: Vec<String> = module
        .metadata
        .iter()
        .enumerate()
        .map(|(at, node)| format!("!{at} = {}!{{{}}}\n", if node.distinct { "distinct " } else { "" }, printer.metadata_operands(&node.operands)))
        .collect();
    section(&mut out, nodes.into_iter());
    out
}

/// Lines under a blank line, when there are any.
fn section(out: &mut String, lines: impl Iterator<Item = String>) {
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
        .map(|one| match &one.name {
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
fn float(kind: FloatKind, bits: u64) -> String {
    let value = match kind {
        FloatKind::Double => f64::from_bits(bits),
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

fn attributes(context: &Context, attrs: &[Attribute]) -> String {
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
    module: &'a Module,
    globals: Vec<String>,
}

/// A function's local names: named values as they are, the rest numbered.
struct Slots {
    values: HashMap<ValueId, String>,
    blocks: HashMap<BlockId, String>,
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
        let mut values = HashMap::new();
        let mut blocks = HashMap::new();
        for &parameter in &function.parameters {
            values.insert(parameter, name(&function.value(parameter).name));
        }
        for &block in &function.layout {
            blocks.insert(block, name(&function.block(block).name));
            for &inst in &function.block(block).instructions {
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
        &self.module.context.types
    }

    fn ty(&self, ty: TypeId) -> String {
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
        match crate::opcode::CONVENTIONS.iter().find(|(_, one)| *one == number) {
            Some((_, 0)) => None,
            Some((name, _)) => Some((*name).to_owned()),
            None => Some(format!("cc{number}")),
        }
    }

    fn address_space(space: u32) -> Option<String> {
        (space != 0).then(|| format!("addrspace({space})"))
    }

    fn variable(&self, at: usize, global: &GlobalValue) -> String {
        let GlobalKind::Variable(variable) = &global.kind else { unreachable!("a variable") };
        let linkage = match (global.linkage, variable.initializer) {
            (Linkage::External, None) => Some("external"),
            (linkage, _) => Self::linkage(linkage),
        };
        let mut words: Vec<String> = [linkage, Self::unnamed_addr(global.unnamed_addr)].into_iter().flatten().map(str::to_owned).collect();
        words.extend(Self::address_space(global.address_space));
        words.push((if variable.constant { "constant" } else { "global" }).to_owned());
        words.push(self.ty(variable.ty));
        words.extend(variable.initializer.map(|one| self.constant(one)));
        let align = variable.align.map(|one| format!(", align {one}")).unwrap_or_default();
        format!("{} = {}{align}\n", self.globals[at], words.join(" "))
    }

    fn function(&self, at: usize, global: &GlobalValue, function: &Function) -> String {
        let (returns, parameter_types, variadic) = self.module.signature(function.ty);
        let slots = Slots::new(function);
        let declaration = function.is_declaration();
        let mut parameters: Vec<String> = parameter_types
            .iter()
            .enumerate()
            .map(|(index, &ty)| {
                let attrs = spaced(attributes(&self.module.context, &function.parameter_attrs[index]));
                let name = if declaration { String::new() } else { format!(" %{}", slots.values[&function.parameters[index]]) };
                format!("{}{attrs}{name}", self.ty(ty))
            })
            .collect();
        if variadic {
            parameters.push("...".to_owned());
        }
        let mut words: Vec<String> = vec![(if declaration { "declare" } else { "define" }).to_owned()];
        words.extend(Self::linkage(global.linkage).map(str::to_owned));
        words.extend(Self::convention(function.calling_convention));
        let return_attrs = attributes(&self.module.context, &function.return_attrs);
        if !return_attrs.is_empty() {
            words.push(return_attrs);
        }
        words.push(self.ty(returns));
        words.push(format!("{}({})", self.globals[at], parameters.join(", ")));
        words.extend(Self::unnamed_addr(global.unnamed_addr).map(str::to_owned));
        words.extend(Self::address_space(global.address_space));
        let attrs = attributes(&self.module.context, &function.attrs);
        if !attrs.is_empty() {
            words.push(attrs);
        }
        if let Some(personality) = function.personality {
            words.push(format!("personality {}", self.typed_constant(personality)));
        }
        let mut out = words.join(" ");
        if declaration {
            out.push('\n');
            return out;
        }
        out.push_str(" {\n");
        for (index, &block) in function.layout.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            if index > 0 || function.block(block).name.is_some() {
                let _ = writeln!(out, "{}:", slots.blocks[&block]);
            }
            for &inst in &function.block(block).instructions {
                out.push_str("  ");
                out.push_str(&self.instruction(function, &slots, inst));
                out.push('\n');
            }
        }
        out.push_str("}\n");
        out
    }

    fn operand(&self, slots: &Slots, operand: Operand) -> String {
        match operand {
            Operand::Value(id) => format!("%{}", slots.values[&id]),
            Operand::Constant(id) => self.constant(id),
            Operand::Block(id) => format!("%{}", slots.blocks[&id]),
        }
    }

    fn typed(&self, function: &Function, slots: &Slots, operand: Operand) -> String {
        match operand {
            Operand::Block(_) => format!("label {}", self.operand(slots, operand)),
            _ => format!("{} {}", self.ty(function.operand_type(&self.module.context, operand).expect("a value")), self.operand(slots, operand)),
        }
    }

    fn instruction(&self, function: &Function, slots: &Slots, id: InstId) -> String {
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
                let cases: String = ops[2..].chunks(2).map(|pair| format!("    {}, {}\n", self.typed(function, slots, pair[0]), self.typed(function, slots, pair[1]))).collect();
                format!("switch {}, {} [\n{cases}  ]", typed(0), typed(1))
            }
            Opcode::Unreachable => "unreachable".to_owned(),
            Opcode::FNeg | Opcode::Freeze => format!("{}{flags} {}", inst.opcode.mnemonic(), typed(0)),
            Opcode::Binary(_) => format!("{}{flags} {}, {}", inst.opcode.mnemonic(), typed(0), bare(1)),
            Opcode::Cast(_) => format!("{}{flags} {} to {}", inst.opcode.mnemonic(), typed(0), self.ty(inst.ty)),
            Opcode::ICmp(predicate) => format!("icmp{flags} {} {}, {}", spelling(&INT_PREDICATE, *predicate), typed(0), bare(1)),
            Opcode::FCmp(predicate) => format!("fcmp{flags} {} {}, {}", spelling(&FLOAT_PREDICATE, *predicate), typed(0), bare(1)),
            Opcode::Select => format!("select{flags} {}, {}, {}", typed(0), typed(1), typed(2)),
            Opcode::Phi => {
                let inputs: Vec<String> = ops.chunks(2).map(|pair| format!("[ {}, {} ]", self.operand(slots, pair[0]), self.operand(slots, pair[1]))).collect();
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
            Opcode::GetElementPtr { source } => {
                let rest: Vec<String> = (0..ops.len()).map(typed).collect();
                format!("getelementptr{flags} {}, {}", self.ty(*source), rest.join(", "))
            }
            Opcode::Call(info) | Opcode::Invoke(info) => {
                let invoke = matches!(inst.opcode, Opcode::Invoke(_));
                let arguments = if invoke { &ops[..ops.len() - 3] } else { &ops[..ops.len() - 1] };
                let (returns, _, variadic) = self.module.signature(info.function_type);
                let callee_ty = if variadic { self.ty(info.function_type) } else { self.ty(returns) };
                let arguments: Vec<String> = arguments
                    .iter()
                    .enumerate()
                    .map(|(at, &one)| {
                        let attrs = spaced(attributes(&self.module.context, &info.argument_attrs[at]));
                        format!("{}{attrs} {}", self.ty(function.operand_type(&self.module.context, one).expect("a value")), self.operand(slots, one))
                    })
                    .collect();
                let tail = match info.tail {
                    Tail::None => "",
                    Tail::Tail => "tail ",
                    Tail::MustTail => "musttail ",
                    Tail::NoTail => "notail ",
                };
                let return_attrs = spaced(attributes(&self.module.context, &info.return_attrs));
                let callee_operand = *ops.last().expect("a callee");
                let callee = self.operand(slots, callee_operand);
                let convention = Self::convention(info.calling_convention).map_or_else(String::new, |one| format!(" {one}"));
                let space = match function.operand_type(&self.module.context, callee_operand).map(|ty| self.module.context.types.get(ty)) {
                    Some(Type::Pointer(space)) if *space != 0 => format!(" addrspace({space})"),
                    _ => String::new(),
                };
                let mut text = format!(
                    "{tail}{}{flags}{convention}{return_attrs}{space} {callee_ty} {callee}({}){}",
                    inst.opcode.mnemonic(),
                    arguments.join(", "),
                    spaced(attributes(&self.module.context, &info.attrs))
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

    fn typed_constant(&self, id: ConstantId) -> String {
        format!("{} {}", self.ty(self.module.context.get(id).ty), self.constant(id))
    }

    fn constant(&self, id: ConstantId) -> String {
        let Constant { ty, kind } = self.module.context.get(id);
        let members = |ids: &[ConstantId]| ids.iter().map(|&one| self.typed_constant(one)).collect::<Vec<_>>().join(", ");
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
            ConstantKind::Null => "null".to_owned(),
            ConstantKind::Poison => "poison".to_owned(),
            ConstantKind::Zero => "zeroinitializer".to_owned(),
            ConstantKind::Bytes(bytes) => format!("c{}", string(bytes)),
            ConstantKind::Global(global) => self.globals[global.0 as usize].clone(),
            ConstantKind::Aggregate(ids) => match self.types().get(*ty) {
                Type::Array { .. } => format!("[{}]", members(ids)),
                Type::Vector { .. } => format!("<{}>", members(ids)),
                Type::Struct { packed: true, .. } => format!("<{{ {} }}>", members(ids)),
                _ if ids.is_empty() => "{}".to_owned(),
                _ => format!("{{ {} }}", members(ids)),
            },
            ConstantKind::Expr(ConstantExpr::GetElementPtr { source, inbounds, operands }) => {
                let rest: Vec<String> = operands.iter().map(|&one| self.typed_constant(one)).collect();
                format!("getelementptr {}({}, {})", if *inbounds { "inbounds " } else { "" }, self.ty(*source), rest.join(", "))
            }
            ConstantKind::Expr(ConstantExpr::Cast { op, value }) => {
                format!("{} ({} to {})", spelling(&CAST, *op), self.typed_constant(*value), self.ty(*ty))
            }
        }
    }

    fn metadata_operands(&self, operands: &[MetadataOperand]) -> String {
        operands
            .iter()
            .map(|one| match one {
                MetadataOperand::Null => "null".to_owned(),
                MetadataOperand::Node(node) => format!("!{}", node.0),
                MetadataOperand::String(text) => format!("!{}", string(text.as_bytes())),
                MetadataOperand::Constant(id) => self.typed_constant(*id),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}
