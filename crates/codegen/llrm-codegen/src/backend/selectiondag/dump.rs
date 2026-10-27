//! `SelectionDAG::dump`: the text of `-debug-only=isel`, so a DAG can be read
//! against the pin's.
//!
//! The node ids are creation order (the pin's `PersistentId`); a comparison
//! with the pin renumbers them by first appearance, and drops the memory
//! operand text and block addresses ([`normalized`]).

use std::fmt::Write;

use super::dag::{MemBase, NodeData, SDNode, SDValue, SelectionDAG};
use super::isd::ISD;
use super::mvt::{EVT, MVT};

fn type_text(vt: EVT) -> String {
    match vt {
        EVT::Simple(MVT::Other) => "ch".into(),
        EVT::Simple(MVT::Glue) => "glue".into(),
        EVT::Simple(MVT::F32) => "f32".into(),
        EVT::Simple(MVT::F64) => "f64".into(),
        EVT::Simple(MVT::F80) => "f80".into(),
        other => format!("i{}", other.size_in_bits()),
    }
}

fn types(node: &SDNode) -> String {
    if node.opcode == ISD::RegisterMask {
        return "Untyped".into();
    }
    node.values.iter().map(|vt| type_text(*vt)).collect::<Vec<_>>().join(",")
}

/// `shouldPrintInline`: a leaf other than the entry token.
fn prints_inline(node: &SDNode) -> bool {
    node.opcode != ISD::EntryToken && node.ops.is_empty()
}

/// `%e`: six digits, a signed exponent of at least two.
fn c_exponent(value: f64) -> String {
    let text = format!("{value:.6e}");
    let (mantissa, exponent) = text.split_once('e').expect("an exponent");
    let exponent: i32 = exponent.parse().expect("a number");
    format!("{mantissa}e{}{:02}", if exponent < 0 { '-' } else { '+' }, exponent.abs())
}

impl SelectionDAG {
    fn name_of(
        &self,
        node: &SDNode,
    ) -> String {
        match node.data {
            NodeData::CondCode(cond) => cond.name().to_string(),
            _ => node.opcode.name(),
        }
    }

    fn details(
        &self,
        node: &SDNode,
    ) -> String {
        let mut text = String::new();
        let flags = node.flags;
        for (flag, name) in [
            (super::SDNodeFlags::NO_UNSIGNED_WRAP, " nuw"),
            (super::SDNodeFlags::NO_SIGNED_WRAP, " nsw"),
            (super::SDNodeFlags::EXACT, " exact"),
            (super::SDNodeFlags::DISJOINT, " disjoint"),
            (super::SDNodeFlags::SAME_SIGN, " samesign"),
            (super::SDNodeFlags::IN_BOUNDS, " inbounds"),
            (super::SDNodeFlags::NON_NEG, " nneg"),
            (super::SDNodeFlags::NO_NANS, " nnan"),
            (super::SDNodeFlags::NO_INFS, " ninf"),
            (super::SDNodeFlags::NO_SIGNED_ZEROS, " nsz"),
            (super::SDNodeFlags::ALLOW_RECIPROCAL, " arcp"),
            (super::SDNodeFlags::ALLOW_CONTRACT, " contract"),
            (super::SDNodeFlags::ALLOW_REASSOC, " reassoc"),
        ] {
            if flags.has(flag) {
                text.push_str(name);
            }
        }
        match node.data {
            NodeData::Const { bits, .. } => {
                let width = node.values[0].size_in_bits();
                let signed = ((bits << (128 - width)) as i128) >> (128 - width);
                write!(text, "<{signed}>").ok();
            }
            NodeData::ConstFp { bits } => match node.values[0] {
                EVT::Simple(MVT::F32) => {
                    write!(text, "<{}>", c_exponent(f64::from(f32::from_bits(bits as u32)))).ok().unwrap_or(())
                }
                EVT::Simple(MVT::F64) => {
                    write!(text, "<{}>", c_exponent(f64::from_bits(bits as u64))).ok().unwrap_or(())
                }
                _ => write!(text, "<APFloat({bits})>").ok().unwrap_or(()),
            },
            NodeData::Global { symbol, offset } => {
                write!(text, "<ptr @{}>", self.global_names[symbol as usize]).ok();
                if offset > 0 {
                    write!(text, " + {offset}").ok();
                } else {
                    write!(text, " {offset}").ok();
                }
            }
            NodeData::ValueType(vt) => {
                write!(text, ":{}", type_text(vt)).ok();
            }
            NodeData::PoolIndex(index, offset) => {
                write!(text, "<{index}>").ok();
                if offset != 0 {
                    write!(text, "{offset:+}").ok();
                }
            }
            NodeData::Symbol(id) => {
                write!(text, "'{}'", self.symbol_names[id as usize]).ok();
            }
            NodeData::Src(key) => {
                write!(text, "<0x{key:x}>").ok();
            }
            NodeData::JumpTable(index) => {
                write!(text, "<{index}>").ok();
            }
            NodeData::FrameIndex(index) => {
                write!(text, "<{index}>").ok();
            }
            NodeData::Block(block) => {
                write!(text, "<{} 0x0>", self.block_name(block)).ok();
            }
            NodeData::PhysReg(reg) => {
                // the allocator's x87 registers are iced's MM0..MM6, the pin's
                // FP0..FP6
                let name = format!("{reg:?}").to_lowercase();
                write!(text, " ${}", name.strip_prefix("mm").map_or(name.clone(), |n| format!("fp{n}"))).ok();
            }
            NodeData::Register(reg) => {
                write!(text, " %{reg}").ok();
            }
            NodeData::Mem(info) => {
                let (verb, to) = if node.opcode == ISD::Load { ("load", "from") } else { ("store", "into") };
                let base = match info.base {
                    MemBase::Unknown => "unknown-address".to_string(),
                    MemBase::Global(id) => format!("@{}", self.global_names[id as usize]),
                    MemBase::Stack(index) => format!("%stack.{index}"),
                    MemBase::CallArgument(offset) => format!("call-argument+{offset}"),
                    MemBase::ConstantPool(_) => "constant-pool".to_string(),
                    MemBase::JumpTable => "jump-table".to_string(),
                };
                let volatile = if info.volatile { "volatile " } else { "" };
                write!(text, "<({volatile}{verb} (s{}) {to} {base}", info.mem_vt.size_in_bits()).ok();
                if info.space != 0 {
                    write!(text, ", addrspace {}", info.space).ok();
                }
                text.push(')');
                if let Some(ext) = info.ext {
                    let name = match ext {
                        super::actions::LoadExt::ExtLoad => "anyext",
                        super::actions::LoadExt::SExtLoad => "sext",
                        super::actions::LoadExt::ZExtLoad => "zext",
                    };
                    write!(text, ", {name} from {}", type_text(info.mem_vt)).ok();
                }
                text.push('>');
            }
            _ => {}
        }
        text
    }

    fn operand_text(
        &self,
        value: SDValue,
    ) -> String {
        let node = self.node(value);
        if prints_inline(node) {
            return format!("{}:{}{}", self.name_of(node), types(node), self.details(node));
        }
        match value.res {
            0 => format!("t{}", value.node),
            res => format!("t{}:{res}", value.node),
        }
    }

    fn node_line(
        &self,
        index: u32,
    ) -> String {
        let node = &self.nodes()[index as usize];
        let mut line = format!("t{index}: {} = {}{}", types(node), self.name_of(node), self.details(node));
        for (at, op) in node.ops.iter().enumerate() {
            line.push_str(if at == 0 { " " } else { ", " });
            line.push_str(&self.operand_text(*op));
        }
        line
    }

    fn dump_nodes(
        &self,
        index: u32,
        indent: usize,
        out: &mut String,
    ) {
        for op in &self.nodes()[index as usize].ops {
            if prints_inline(self.node(*op)) {
                continue;
            }
            if self.use_count(op.node) == 1 {
                self.dump_nodes(op.node, indent + 2, out);
            }
        }
        writeln!(out, "{:indent$}{}", "", self.node_line(index)).ok();
    }

    /// `SelectionDAG::dump()`.
    pub fn dump(&self) -> String {
        let mut out = format!("SelectionDAG has {} nodes:\n", self.nodes().len());
        let root = self.get_root().node;
        for (at, node) in self.nodes().iter().enumerate() {
            let at = at as u32;
            let uses = self.use_count(at);
            if uses != 1 && at != root && (!prints_inline(node) || uses == 0) {
                self.dump_nodes(at, 2, &mut out);
            }
        }
        self.dump_nodes(root, 2, &mut out);
        out
    }
}

/// A dump with node ids renumbered by first appearance, memory operand text
/// and block addresses removed, and the node count line dropped: what the
/// pin's dump and ours must agree on.
pub fn normalized(dump: &str) -> String {
    let mut ids: Vec<String> = Vec::new();
    let mut regs: Vec<String> = Vec::new();
    let mut out = String::new();
    for line in dump.lines().filter(|line| !line.starts_with("SelectionDAG has") && !line.trim().is_empty()) {
        let mut text = String::new();
        let mut rest = line;
        while let Some(at) = rest.find('t') {
            let (head, tail) = rest.split_at(at);
            text.push_str(head);
            let digits: String = tail[1..].chars().take_while(char::is_ascii_digit).collect();
            let before_ok = head.chars().last().is_none_or(|c| !c.is_alphanumeric());
            if before_ok && !digits.is_empty() {
                let id = match ids.iter().position(|seen| *seen == digits) {
                    Some(at) => at,
                    None => {
                        ids.push(digits.clone());
                        ids.len() - 1
                    }
                };
                write!(text, "t{id}").ok();
                rest = &tail[1 + digits.len()..];
            } else {
                text.push('t');
                rest = &tail[1..];
            }
        }
        text.push_str(rest);
        out.push_str(&renumber_registers(&strip_decorations(&text), &mut regs));
        out.push('\n');
    }
    out
}

/// Virtual register numbers by first appearance: selection of earlier blocks
/// allocates registers between the exported ones (SD17).
fn renumber_registers(
    line: &str,
    regs: &mut Vec<String>,
) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(at) = rest.find(" %") {
        out.push_str(&rest[..at + 2]);
        let tail = &rest[at + 2..];
        let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            rest = tail;
            continue;
        }
        let id = regs
            .iter()
            .position(|seen| *seen == digits)
            .unwrap_or_else(
                || {
                    regs.push(digits.clone());
                    regs.len() - 1
                },
            );
        write!(out, "{id}").ok();
        rest = &tail[digits.len()..];
    }
    out.push_str(rest);
    out
}

/// `<(...)>` memory operands, `[TF=n]` relocation flags (SD100) and `0x...`
/// addresses are not compared.
fn strip_decorations(line: &str) -> String {
    let mut text = line.to_string();
    while let Some(start) = text.find("<(") {
        let mut depth = 0;
        let close = text[start + 1..]
            .char_indices()
            .find_map(|(at, c)| {
                depth += i32::from(c == '(') - i32::from(c == ')');
                (depth == 0).then_some(start + 1 + at + 1)
            })
            .unwrap_or(text.len());
        text.replace_range(start + 1..close, "");
        if text[start..].starts_with("<>") {
            text.replace_range(start..start + 2, "");
        } else if text[start..].starts_with("<, ") {
            text.replace_range(start + 1..start + 3, "");
        }
    }
    while let Some(start) = text.find(" [TF=") {
        let end = text[start..].find(']').map_or(text.len(), |end| start + end + 1);
        text.replace_range(start..end, "");
    }
    while let Some(start) = text.find("<0x") {
        let end = text[start..].find('>').map_or(text.len(), |end| start + end + 1);
        text.replace_range(start..end, "<>");
    }
    while let Some(start) = text.find(" 0x") {
        let end = text[start + 3..].find(|c: char| !c.is_ascii_hexdigit()).map_or(text.len(), |end| start + 3 + end);
        text.replace_range(start..end, "");
    }
    text
}
