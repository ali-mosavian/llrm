//! `SCEV::print`: an expression as LLVM writes it.

use llrm_mir::context::ConstantKind;
use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, Operand, ValueId};

use super::expr::{Kind, Scev, Ty, Use, flags};
use super::{Cx, ScalarEvolution};

/// The names `printAsOperand` gives the function's values and blocks.
pub struct Labels {
    values: IdMap<ValueId, String>,
    blocks: IdMap<BlockId, String>,
}

impl Labels {
    pub fn of(function: &llrm_mir::module::Function) -> Self {
        let mut values = llrm_mir::print::value_labels(function);
        let mut blocks = llrm_mir::print::block_labels(function);
        values.retain(|_, label| {
            *label = printed_name(label);
            true
        });
        blocks.retain(|_, label| {
            *label = printed_name(label);
            true
        });
        Self { values, blocks }
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/AsmWriter.cpp#L400
/// `PrintLLVMName`'s quoting of a name `quoted` left bare: MIR's lexer takes
/// `$` in a bare name, but the printer quotes any name with a character
/// outside `[A-Za-z0-9-._]`.
pub fn printed_name(name: &str) -> String {
    if name.starts_with('"') || !name.contains('$') { name.to_owned() } else { format!("\"{name}\"") }
}

impl Labels {
    /// The label of a block, `%` left off.
    pub fn block_label(
        &self,
        block: BlockId,
    ) -> String {
        self.blocks.get(&block).cloned().unwrap_or_default()
    }
}

/// `Type::print`.
pub fn type_text(ty: Ty) -> String {
    match ty {
        Ty::Int(bits) => format!("i{bits}"),
        Ty::Ptr(0) => "ptr".to_owned(),
        Ty::Ptr(space) => format!("ptr addrspace({space})"),
    }
}

impl ScalarEvolution {
    /// `SCEVUse::print`: the expression and the use-specific flags.
    pub fn print_use(
        &self,
        cx: &Cx,
        labels: &Labels,
        u: Use,
    ) -> String {
        let mut out = self.print(cx, labels, u.scev);
        let f = self.use_flags(u);
        if f & flags::NUW != 0 {
            out.push_str("<u nuw>");
        }
        if f & flags::NSW != 0 {
            out.push_str("<u nsw>");
        }
        out
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L297
    /// `SCEV::print`.
    pub fn print(
        &self,
        cx: &Cx,
        labels: &Labels,
        s: Scev,
    ) -> String {
        match &self.node(s).kind {
            Kind::Constant(value) => value.display(),
            Kind::PtrToAddr(op) => {
                format!(
                    "(ptrtoaddr {} {} to {})",
                    type_text(self.type_of(op.scev)),
                    self.print_use(cx, labels, *op),
                    type_text(self.type_of(s))
                )
            }
            Kind::Truncate(op) => self.print_cast(cx, labels, "trunc", *op, s),
            Kind::ZeroExtend(op) => self.print_cast(cx, labels, "zext", *op, s),
            Kind::SignExtend(op) => self.print_cast(cx, labels, "sext", *op, s),
            Kind::AddRec(ops, header) => {
                let mut out = format!("{{{}", self.print_use(cx, labels, ops[0]));
                for op in &ops[1..] {
                    out.push_str(",+,");
                    out.push_str(&self.print_use(cx, labels, *op));
                }
                out.push_str("}<");
                let f = self.get_no_wrap_flags(s, flags::MASK);
                if f & flags::NUW != 0 {
                    out.push_str("nuw><");
                }
                if f & flags::NSW != 0 {
                    out.push_str("nsw><");
                }
                if f & flags::NW != 0 && f & flags::NUW == 0 && f & flags::NSW == 0 {
                    out.push_str("nw><");
                }
                out.push('%');
                out.push_str(&labels.blocks.get(header).cloned().unwrap_or_default());
                out.push('>');
                out
            }
            Kind::Add(ops)
            | Kind::Mul(ops)
            | Kind::UMax(ops)
            | Kind::SMax(ops)
            | Kind::UMin(ops)
            | Kind::SMin(ops)
            | Kind::SequentialUMin(ops) => {
                let separator = match &self.node(s).kind {
                    Kind::Add(_) => " + ",
                    Kind::Mul(_) => " * ",
                    Kind::UMax(_) => " umax ",
                    Kind::SMax(_) => " smax ",
                    Kind::UMin(_) => " umin ",
                    Kind::SMin(_) => " smin ",
                    _ => " umin_seq ",
                };
                let parts: Vec<String> = ops.iter().map(|op| self.print_use(cx, labels, *op)).collect();
                let mut out = format!("({})", parts.join(separator));
                if matches!(self.node(s).kind, Kind::Add(_) | Kind::Mul(_)) {
                    if self.get_no_wrap_flags(s, flags::NUW) != 0 {
                        out.push_str("<nuw>");
                    }
                    if self.get_no_wrap_flags(s, flags::NSW) != 0 {
                        out.push_str("<nsw>");
                    }
                }
                out
            }
            Kind::UDiv(lhs, rhs) => {
                format!("({} /u {})", self.print_use(cx, labels, *lhs), self.print_use(cx, labels, *rhs))
            }
            Kind::Unknown(value) => self.print_value(cx, labels, *value),
            Kind::CouldNotCompute => "***COULDNOTCOMPUTE***".to_owned(),
        }
    }

    fn print_cast(
        &self,
        cx: &Cx,
        labels: &Labels,
        name: &str,
        op: Use,
        s: Scev,
    ) -> String {
        format!(
            "({name} {} {} to {})",
            type_text(self.type_of(op.scev)),
            self.print_use(cx, labels, op),
            type_text(self.type_of(s))
        )
    }

    /// The `%label` of a value.
    pub(super) fn print_value_name(
        &self,
        labels: &Labels,
        value: Operand,
    ) -> String {
        match value {
            Operand::Value(id) => format!("%{}", labels.values.get(&id).cloned().unwrap_or_default()),
            _ => unreachable!("a value with a label"),
        }
    }

    /// `Value::printAsOperand(OS, false)`.
    pub(crate) fn print_value(
        &self,
        cx: &Cx,
        labels: &Labels,
        value: Operand,
    ) -> String {
        match value {
            Operand::Value(id) => format!("%{}", labels.values.get(&id).cloned().unwrap_or_default()),
            Operand::Constant(id) => match &cx.context.get(id).kind {
                ConstantKind::Null => "null".to_owned(),
                ConstantKind::Poison => "poison".to_owned(),
                ConstantKind::Undef => "undef".to_owned(),
                ConstantKind::Global(global) => {
                    format!(
                        "@{}",
                        printed_name(&llrm_mir::print::quoted(&cx.global(*global).name.owned().unwrap_or_default()))
                    )
                }
                other => format!("{other:?}"),
            },
            Operand::Block(block) => format!("%{}", labels.blocks.get(&block).cloned().unwrap_or_default()),
        }
    }
}
