//! Port of `qbopt/hir/dump.py`: stable semantic projection used to diff
//! source and object frontends.

use indexmap::IndexMap;

use crate::hir::lower::Lowered;
use crate::model::ir::Operation;
use crate::model::memory::MemoryObject;
use crate::model::mir::{self, Arg, OpCode};
use crate::support::pyrepr::Repr;

struct _Names {
    blocks: IndexMap<i64, usize>,
    values: IndexMap<mir::Value, usize>,
    objects: IndexMap<MemoryObject, usize>,
}

impl _Names {
    fn new(body: &mir::MirBody) -> Self {
        Self {
            blocks: body.blocks.iter().enumerate().map(|(number, block)| (block.at, number + 1)).collect(),
            values: IndexMap::new(),
            objects: IndexMap::new(),
        }
    }

    fn value(&mut self, value: mir::Value) -> String {
        let next = self.values.len() + 1;
        let number = *self.values.entry(value).or_insert(next);
        if value.flags { format!("f{number}") } else { format!("v{number}") }
    }

    fn block(&self, at: i64) -> String {
        format!("b{}", self.blocks[&at])
    }

    fn object(&mut self, object_: &MemoryObject) -> String {
        let next = self.objects.len() + 1;
        let number = *self.objects.entry(object_.clone()).or_insert(next);
        let extent = object_.extent.map_or_else(|| "?".to_owned(), |extent| extent.to_string());
        format!("{}{number}:{extent}", object_.kind.as_str())
    }
}

fn _arg(one: &Arg, names: &mut _Names) -> String {
    match one {
        Arg::Held(held) => format!("{}:{}", names.value(held.value), held.width),
        Arg::Const(constant) => format!("{}:{}", constant.n, constant.width),
        Arg::Cell(cell) => {
            let reference = &cell.r#ref;
            let where_ = match &reference.provenance {
                None => reference.where_().map_or_else(|| "None".to_owned(), |space| space.to_string()),
                Some(provenance) => {
                    let mut slices: Vec<_> = provenance.slices.iter().collect();
                    slices.sort_by(|one, other| {
                        (one.object.kind.as_str(), one.object.identity.repr(), one.low, one.high).cmp(&(
                            other.object.kind.as_str(),
                            other.object.identity.repr(),
                            other.low,
                            other.high,
                        ))
                    });
                    slices
                        .into_iter()
                        .map(|part| {
                            format!(
                                "{}[{}:{}:{}/{}]",
                                names.object(&part.object),
                                part.low,
                                part.high,
                                part.stride,
                                part.width
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("|")
                }
            };
            let base = reference.base.map_or_else(String::new, |base| format!("+{}", names.value(base)));
            let segment = reference.segment.map_or_else(String::new, |segment| format!("@{}", names.value(segment)));
            format!("cell({where_}{base}{segment}):{}", reference.width)
        }
        Arg::Symbol(mir::Symbol { space, index, offset, width, addend }) => {
            let displacement = offset + addend;
            let suffix = if displacement > 0 {
                format!("+{displacement}")
            } else if displacement != 0 {
                displacement.to_string()
            } else {
                String::new()
            };
            format!("{}{index}{suffix}:{width}", space.value())
        }
        Arg::FrameAddress(mir::FrameAddress { offset, width, .. }) => {
            let sign = if *offset > 0 { format!("+{offset}") } else { offset.to_string() };
            format!("frame[{sign}]:{width}")
        }
        Arg::Opaque(opaque) => {
            if opaque.name.is_empty() {
                "opaque".to_owned()
            } else {
                opaque.name.clone()
            }
        }
        Arg::FrameSelector(_) => "frameselector".to_owned(),
    }
}

const _INFIX: [mir::Kind; 31] = [
    mir::Kind::Add,
    mir::Kind::Sub,
    mir::Kind::AddCarry,
    mir::Kind::SubBorrow,
    mir::Kind::Mul,
    mir::Kind::Smulhi,
    mir::Kind::Div,
    mir::Kind::Rem,
    mir::Kind::Divmod,
    mir::Kind::Udivmod,
    mir::Kind::And,
    mir::Kind::Or,
    mir::Kind::Xor,
    mir::Kind::Shl,
    mir::Kind::Shr,
    mir::Kind::Sar,
    mir::Kind::Lt,
    mir::Kind::Le,
    mir::Kind::Gt,
    mir::Kind::Ge,
    mir::Kind::Eq,
    mir::Kind::Ne,
    mir::Kind::Below,
    mir::Kind::BelowEq,
    mir::Kind::Above,
    mir::Kind::AboveEq,
    mir::Kind::Fadd,
    mir::Kind::Fsub,
    mir::Kind::Fmul,
    mir::Kind::Fdiv,
    mir::Kind::Fcompare,
];

fn _operation(op: &mir::Op, names: &mut _Names) -> String {
    let args: Vec<String> = op.args.iter().map(|one| _arg(one, names)).collect();
    let results: Vec<String> = op
        .results
        .iter()
        .map(|one| match one {
            Arg::Held(held) => names.value(held.value),
            one => _arg(one, names),
        })
        .collect();
    let defined: Vec<String> = op.defines.iter().map(|one| names.value(*one)).collect();
    let left = if results.is_empty() { defined } else { results.clone() };
    let spelling = if op.op == Some(OpCode::Operation(Operation::FloatUnary)) && !op.name.is_empty() {
        op.name.as_str()
    } else {
        op.kind.as_str()
    };

    if op.kind == mir::Kind::Store && results.len() == 1 && args.len() == 1 {
        return format!("{} <- {}", results[0], args[0]);
    }
    let expression = if _INFIX.contains(&op.kind) && args.len() == 2 {
        format!("{} {spelling} {}", args[0], args[1])
    } else if op.kind == mir::Kind::Call {
        format!("call {}({})", op.name, args.join(", "))
    } else if op.kind == mir::Kind::Branch {
        format!("branch{}", args.first().map_or_else(String::new, |one| format!(" {one}")))
    } else if op.kind == mir::Kind::Return {
        format!("return{}", if args.is_empty() { String::new() } else { format!(" {}", args.join(", ")) })
    } else if args.len() == 1 {
        format!("{spelling} {}", args[0])
    } else if args.is_empty() {
        spelling.to_owned()
    } else {
        format!("{spelling}({})", args.join(", "))
    };
    if left.is_empty() { expression } else { format!("{} <- {expression}", left.join(", ")) }
}

/// Print meaning with incidental block, value, and object identities removed.
pub fn mir_text(lowered: &Lowered) -> String {
    let mut names = _Names::new(&lowered.body);
    let mut lines = vec![format!("function {} entry {}", lowered.name, names.block(lowered.body.entry))];
    for block in &lowered.body.blocks {
        let successors = block.succ.iter().map(|one| names.block(*one)).collect::<Vec<_>>().join(", ");
        let successors = if successors.is_empty() { "-".to_owned() } else { successors };
        lines.push(format!("{} -> {successors}", names.block(block.at)));
        for op in &block.ops {
            let operation = _operation(op, &mut names);
            let target = op.target.map_or_else(String::new, |target| format!(" -> {}", names.block(target)));
            let cases = if op.cases.is_empty() {
                String::new()
            } else {
                let items: Vec<String> =
                    op.cases.iter().map(|(value, at)| format!("({value}, '{}')", names.block(*at))).collect();
                if items.len() == 1 { format!(" ({},)", items[0]) } else { format!(" ({})", items.join(", ")) }
            };
            let floating = match &op.floating {
                None => String::new(),
                Some(floating) => {
                    let inputs = floating.inputs.iter().map(|one| one.as_str()).collect::<Vec<_>>().join(",");
                    format!(
                        " [{inputs}->{};{}/{}]",
                        floating.result.as_str(),
                        floating.precision.as_str(),
                        floating.rounding.as_str()
                    )
                }
            };
            lines.push(format!("  {operation}{target}{cases}{floating}"));
        }
    }
    lines.join("\n") + "\n"
}
