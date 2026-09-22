//! Port of `qbopt/backend/lower_int64.py`: legalize MIR's whole 64-bit
//! integers for the 386 machine boundary.
//!
//! MIR keeps a C `long long` as one eight-byte value; this first
//! target-specific step gives the 16-bit ABI its two dword halves.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use iced_x86::Register;
use indexmap::IndexMap;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::abi::runtime::{self, Reg};
use crate::backend::lower::Unlowered;
use crate::model::ir::Operation;
use crate::model::mir::{
    self, AllocationHints, Arg, Cell, Const, Held, Kind, MemRef, MirBody, Op, OpCode, OrderedMap, Phi, Value,
};
use crate::support::pyrepr::Repr;
use crate::support::pyset::PySet;

type R<T> = Result<T, Unlowered>;

#[derive(Clone, Debug)]
pub struct Legalized {
    pub body: MirBody,
    pub calls: IndexMap<i64, String>,
    pub contracts: IndexMap<i64, runtime::Contract>,
    pub hints: AllocationHints,
    pub inline: IndexMap<i64, Vec<Vec<u8>>>,
}

fn _helper(name: &str, inputs: BTreeSet<Reg>, clobbers: BTreeSet<Reg>) -> runtime::Contract {
    runtime::Contract {
        name: name.to_owned(),
        cleanup: Some(0),
        control: runtime::Control::Returns,
        enters_user_code: false,
        raises_error: false,
        error_handling: false,
        writes: runtime::Memory::None,
        reads: runtime::Memory::None,
        clobbers,
        established: true,
        evidence: "qbopt's inline 386 int64 helper; operands and results follow Open Watcom's register ABI".to_owned(),
        documented: None,
        inputs: Some(inputs),
        direct_inputs: None,
        clobbers_reached: true,
        caller_cleanup: 0,
        // Not a separately called 386 routine: `clobbers` describes the inline bytes exactly.
        i386: false,
        direct_writes: None,
        direct_reads: None,
    }
}

/// `bytes.fromhex(text)`.
fn fromhex(text: &str) -> Vec<u8> {
    text.split_whitespace().map(|byte| u8::from_str_radix(byte, 16).expect("a hex byte")).collect()
}

/// __U8M's register ABI is EDX:EAX * ECX:EBX -> EDX:EAX. No RET: helpers are straight-line.
static _MUL: LazyLock<Vec<u8>> = LazyLock::new(|| {
    fromhex(concat!(
        "66 50 ",          // push eax
        "66 0f af c8 ",    // imul ecx,eax
        "66 0f af d3 ",    // imul edx,ebx
        "66 01 d1 ",       // add ecx,edx
        "66 58 ",          // pop eax
        "66 f7 e3 ",       // mul ebx
        "66 01 ca",        // add edx,ecx
    ))
});

/// EDX:EAX * EBX when the other operand is known to fit one dword.
static _MUL32: LazyLock<Vec<u8>> = LazyLock::new(|| {
    fromhex(concat!(
        "66 0f af d3 ", // imul edx,ebx
        "66 89 d1 ",    // mov ecx,edx
        "66 f7 e3 ",    // mul ebx
        "66 01 ca",     // add edx,ecx
    ))
});

/// EDX:EAX / ECX:EBX, using the Open Watcom runtime's leading-bit division.
static _UDIV: LazyLock<Vec<u8>> = LazyLock::new(|| {
    fromhex(concat!(
        "66 09 c9 75 2a 66 4b 0f 84 c2 00 66 43 66 39 d3 77 0e 66 89 c1 66 89 d0 66 31 d2 ",
        "66 f7 f3 66 91 66 f7 f3 66 89 d3 66 89 ca 66 31 c9 e9 9e 00 66 39 d1 72 28 75 19 ",
        "66 39 c3 77 14 66 29 d8 66 89 c3 66 31 c9 66 31 d2 66 b8 01 00 00 00 eb 7e 66 31 ",
        "c9 66 31 db 66 93 66 87 d1 eb 71 66 55 66 56 66 57 66 31 f6 66 89 f7 66 89 f5 ",
        "66 d1 e3 66 d1 d1 72 19 66 45 66 39 d1 72 f1 77 05 66 39 c3 76 ea f8 66 d1 d6 ",
        "66 d1 d7 66 4d 78 2f 66 d1 d9 66 d1 db 66 29 d8 66 19 ca f5 72 e7 66 d1 e6 66 d1 ",
        "d7 66 4d 78 10 66 d1 e9 66 d1 db 66 01 d8 66 11 ca 73 e8 eb cd 66 01 d8 66 11 ca ",
        "66 89 c3 66 89 d1 66 89 f0 66 89 fa 66 5f 66 5e 66 5d",
    ))
});

static _SDIV: LazyLock<Vec<u8>> = LazyLock::new(|| {
    fromhex(concat!(
        "66 09 d2 78 25 66 09 c9 78 06 e8 60 00 e9 27 01 66 f7 d9 66 f7 db 66 83 d9 00 e8 ",
        "50 00 66 f7 da 66 f7 d8 66 83 da 00 e9 0d 01 66 f7 da 66 f7 d8 66 83 da 00 66 09 c9 ",
        "79 1a 66 f7 d9 66 f7 db 66 83 d9 00 e8 27 00 66 f7 d9 66 f7 db 66 83 d9 00 e9 e4 ",
        "00 e8 17 00 66 f7 d9 66 f7 db 66 83 d9 00 66 f7 da 66 f7 d8 66 83 da 00 e9 ca 00 ",
        "66 09 c9 75 28 66 4b 0f 84 be 00 66 43 66 39 d3 77 0e 66 89 c1 66 89 d0 66 31 ",
        "d2 66 f7 f3 66 91 66 f7 f3 66 89 d3 66 89 ca 66 31 c9 c3 66 39 d1 72 26 75 18 ",
        "66 39 c3 77 13 66 29 d8 66 89 c3 66 31 c9 66 31 d2 66 b8 01 00 00 00 c3 66 31 ",
        "c9 66 31 db 66 93 66 87 d1 c3 66 55 66 56 66 57 66 31 f6 66 89 f7 66 89 f5 ",
        "66 d1 e3 66 d1 d1 72 19 66 45 66 39 d1 72 f1 77 05 66 39 c3 76 ea f8 66 d1 d6 ",
        "66 d1 d7 66 4d 78 2f 66 d1 d9 66 d1 db 66 29 d8 66 19 ca f5 72 e7 66 d1 e6 66 ",
        "d1 d7 66 4d 78 10 66 d1 e9 66 d1 db 66 01 d8 66 11 ca 73 e8 eb cd 66 01 d8 66 ",
        "11 ca 66 89 c3 66 89 d1 66 89 f0 66 89 fa 66 5f 66 5e 66 5d c3",
    ))
});

/// A compile-time dword divisor: at most two hardware divisions.
static _UDIV_CONST32: LazyLock<Vec<u8>> = LazyLock::new(|| {
    fromhex("66 31 c9 66 39 d3 77 0e 66 89 c1 66 89 d0 66 31 d2 66 f7 f3 66 91 66 f7 f3 66 89 d3 66 89 ca 66 31 c9")
});

static _SDIV_CONST32: LazyLock<Vec<u8>> = LazyLock::new(|| {
    fromhex(concat!(
        "66 09 d2 78 24 66 31 c9 66 39 d3 77 0e 66 89 c1 66 89 d0 66 31 d2 66 f7 f3 66 91 ",
        "66 f7 f3 66 89 d3 66 89 ca 66 31 c9 eb 40 66 f7 da 66 f7 d8 66 83 da 00 66 31 c9 ",
        "66 39 d3 77 0e 66 89 c1 66 89 d0 66 31 d2 66 f7 f3 66 91 66 f7 f3 66 89 d3 66 ",
        "89 ca 66 31 c9 66 f7 d9 66 f7 db 66 83 d9 00 66 f7 da 66 f7 d8 66 83 da 00",
    ))
});

fn _four_inputs() -> BTreeSet<Reg> {
    BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx])
}

fn _four_clobbers() -> BTreeSet<Reg> {
    let mut out = _four_inputs();
    out.insert(Reg::Flags);
    out
}

/// `op(..., defines=None, uses=None, loads=(), stores=(), test=None, target=None, ...)`.
struct Made {
    defines: Option<Vec<Value>>,
    uses: Option<Vec<Value>>,
    loads: Vec<MemRef>,
    stores: Vec<MemRef>,
}

impl Made {
    fn plain() -> Self {
        Made { defines: None, uses: None, loads: Vec::new(), stores: Vec::new() }
    }
}

fn held(value: Value, width: u32) -> Arg {
    Arg::Held(Held { value, width })
}

fn constant(n: impl Into<BigInt>, width: u32) -> Arg {
    Arg::Const(Const::new(n, width))
}

fn value_of(arg: &Arg) -> Value {
    match arg {
        Arg::Held(one) => one.value,
        other => panic!("AttributeError: {} has no attribute 'value'", other.repr()),
    }
}

/// `getattr(arg, "width", default)`.
fn width_of(arg: &Arg) -> Option<u32> {
    match arg {
        Arg::Held(one) => Some(one.width),
        Arg::Const(one) => Some(one.width),
        Arg::Symbol(one) => Some(one.width),
        Arg::FrameAddress(one) => Some(one.width),
        Arg::FrameSelector(one) => Some(one.width),
        Arg::Cell(_) | Arg::Opaque(_) => None,
    }
}

fn is_zero_const(arg: &Arg) -> bool {
    matches!(arg, Arg::Const(one) if one.n == BigInt::from(0))
}

struct _Legalizer<'a> {
    body: &'a MirBody,
    calls: IndexMap<i64, String>,
    contracts: IndexMap<i64, runtime::Contract>,
    origins: OrderedMap<u32, Register>,
    pins: OrderedMap<(u32, usize), Register>,
    inline: IndexMap<i64, Vec<Vec<u8>>>,
    next_value: u32,
    pairs: IndexMap<Value, (Value, Value)>,
}

impl<'a> _Legalizer<'a> {
    fn new(
        body: &'a MirBody,
        calls: &IndexMap<i64, String>,
        contracts: &IndexMap<i64, runtime::Contract>,
        hints: &AllocationHints,
    ) -> Self {
        let mut values: BTreeSet<Value> = BTreeSet::new();
        for block in &body.blocks {
            values.extend(block.phis.iter().map(|phi| phi.result));
            values.extend(block.phis.iter().flat_map(|phi| phi.incoming.values().copied()));
            values.extend(block.ops.iter().flat_map(|op| op.defines.iter().chain(&op.uses).copied()));
        }
        let wide: PySet<Value> = body
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .flat_map(|op| op.args.iter().chain(&op.results))
            .filter_map(|arg| match arg {
                Arg::Held(one) if one.width == 8 => Some(one.value),
                _ => None,
            })
            .collect();
        let mut legalizer = _Legalizer {
            body,
            calls: calls.clone(),
            contracts: contracts.clone(),
            origins: hints.origins.clone(),
            pins: hints.pins.clone(),
            inline: IndexMap::new(),
            next_value: values.iter().map(|one| one.id).max().unwrap_or(0) + 1,
            pairs: IndexMap::new(),
        };
        for value in wide.iter() {
            let pair = (legalizer.fresh(value.at, false), legalizer.fresh(value.at, false));
            legalizer.pairs.insert(*value, pair);
        }
        legalizer
    }

    fn fresh(&mut self, at: i64, flags: bool) -> Value {
        let value = Value { id: self.next_value, at, flags, variable: self.next_value, version: 1 };
        self.next_value += 1;
        value
    }

    fn _held(values: (Value, Value)) -> (Arg, Arg) {
        (held(values.0, 4), held(values.1, 4))
    }

    fn pair(&self, arg: &Arg) -> R<(Arg, Arg)> {
        match arg {
            Arg::Held(one) if one.width == 8 => Ok(Self::_held(self.pairs[&one.value])),
            Arg::Const(one) if one.width == 8 => {
                let number = &one.n & BigInt::from(u64::MAX);
                Ok((constant(&number & BigInt::from(0xFFFF_FFFFu32), 4), constant(number >> 32u32, 4)))
            }
            _ => Err(Unlowered(format!("64-bit operand {} has no dword pair", arg.repr()))),
        }
    }

    fn reference(one: &MemRef, high: bool) -> R<MemRef> {
        if one.width != 8 {
            return Ok(one.clone());
        }
        if high && one.addr.is_none() {
            return Err(Unlowered("an unlocated 64-bit cell cannot name its high dword".to_owned()));
        }
        let mut split = one.clone();
        split.width = 4;
        split.addr = if high { one.addr.map(|addr| addr.plus(4)) } else { one.addr };
        Ok(split)
    }

    fn op(&self, source: &Op, kind: Kind, args: Vec<Arg>, results: Vec<Arg>, made: Made) -> Op {
        self.op_with(source, kind, args, results, made, None, None)
    }

    #[allow(clippy::too_many_arguments)]
    fn op_with(
        &self,
        source: &Op,
        kind: Kind,
        args: Vec<Arg>,
        results: Vec<Arg>,
        made: Made,
        test: Option<Kind>,
        target: Option<i64>,
    ) -> Op {
        let defines = made.defines.unwrap_or_else(|| {
            results
                .iter()
                .filter_map(|arg| match arg {
                    Arg::Held(one) => Some(one.value),
                    _ => None,
                })
                .collect()
        });
        let uses = made.uses.unwrap_or_else(|| {
            let mut unique = Vec::new();
            for arg in &args {
                if let Arg::Held(one) = arg {
                    if !unique.contains(&one.value) {
                        unique.push(one.value);
                    }
                }
            }
            unique
        });
        let mut op = Op::new(source.at, OpCode::Operation(Operation::Nothing), "", defines, uses);
        op.loads = made.loads;
        op.stores = made.stores;
        op.kind = kind;
        op.test = test;
        op.args = args;
        op.results = results;
        op.target = target;
        op.id = Some(mir::next_id());
        op.args_known = true;
        op.reads_complete = true;
        op.memory_complete = true;
        op
    }

    fn binary(&mut self, source: &Op) -> R<Vec<Op>> {
        let left = self.pair(&source.args[0])?;
        let right = self.pair(&source.args[1])?;
        let out = self.pair(&source.results[0])?;
        if matches!(source.kind, Kind::And | Kind::Or | Kind::Xor) {
            return Ok(vec![
                self.op(source, source.kind, vec![left.0, right.0], vec![out.0], Made::plain()),
                self.op(source, source.kind, vec![left.1, right.1], vec![out.1], Made::plain()),
            ]);
        }
        let carry = self.fresh(source.at, true);
        let (low, high) = match source.kind {
            Kind::Add => (Kind::Add, Kind::AddCarry),
            Kind::Sub => (Kind::Sub, Kind::SubBorrow),
            kind => return Err(Unlowered(format!("64-bit {kind} at {:#x}", source.at))),
        };
        let first = self.op(
            source,
            low,
            vec![left.0, right.0],
            vec![out.0.clone()],
            Made { defines: Some(vec![value_of(&out.0), carry]), ..Made::plain() },
        );
        let uses = vec![value_of(&left.1), value_of(&right.1), carry];
        let second = self.op(
            source,
            high,
            vec![left.1, right.1],
            vec![out.1.clone()],
            Made { defines: Some(vec![value_of(&out.1)]), uses: Some(uses), ..Made::plain() },
        );
        Ok(vec![first, second])
    }

    fn shift(&mut self, source: &Op) -> R<Vec<Op>> {
        let value = self.pair(&source.args[0])?;
        let out = self.pair(&source.results[0])?;
        let Arg::Const(count) = &source.args[1] else {
            return Err(Unlowered(format!("variable 64-bit shift at {:#x}", source.at)));
        };
        let amount = (&count.n & BigInt::from(63)).to_u32().expect("a shift count below 64");
        if amount == 0 {
            return Ok(vec![
                self.op(source, Kind::Copy, vec![value.0], vec![out.0], Made::plain()),
                self.op(source, Kind::Copy, vec![value.1], vec![out.1], Made::plain()),
            ]);
        }
        if source.kind == Kind::Shl {
            if amount >= 32 {
                return Ok(vec![
                    self.op(source, Kind::Copy, vec![constant(0, 4)], vec![out.0], Made::plain()),
                    self.op(source, Kind::Shl, vec![value.0, constant(amount - 32, 1)], vec![out.1], Made::plain()),
                ]);
            }
            let a = self.fresh(source.at, false);
            let b = self.fresh(source.at, false);
            return Ok(vec![
                self.op(source, Kind::Shl, vec![value.1, constant(amount, 1)], vec![held(a, 4)], Made::plain()),
                self.op(source, Kind::Shr, vec![value.0.clone(), constant(32 - amount, 1)], vec![held(b, 4)], Made::plain()),
                self.op(source, Kind::Or, vec![held(a, 4), held(b, 4)], vec![out.1], Made::plain()),
                self.op(source, Kind::Shl, vec![value.0, constant(amount, 1)], vec![out.0], Made::plain()),
            ]);
        }
        let high_kind = if source.kind == Kind::Sar { Kind::Sar } else { Kind::Shr };
        if amount >= 32 {
            let top = if source.kind == Kind::Sar {
                self.op(source, high_kind, vec![value.1.clone(), constant(31, 1)], vec![out.1], Made::plain())
            } else {
                self.op(source, Kind::Copy, vec![constant(0, 4)], vec![out.1], Made::plain())
            };
            return Ok(vec![
                self.op(source, high_kind, vec![value.1, constant(amount - 32, 1)], vec![out.0], Made::plain()),
                top,
            ]);
        }
        let a = self.fresh(source.at, false);
        let b = self.fresh(source.at, false);
        Ok(vec![
            self.op(source, Kind::Shr, vec![value.0, constant(amount, 1)], vec![held(a, 4)], Made::plain()),
            self.op(source, Kind::Shl, vec![value.1.clone(), constant(32 - amount, 1)], vec![held(b, 4)], Made::plain()),
            self.op(source, Kind::Or, vec![held(a, 4), held(b, 4)], vec![out.0], Made::plain()),
            self.op(source, high_kind, vec![value.1, constant(amount, 1)], vec![out.1], Made::plain()),
        ])
    }

    fn materialize(&mut self, source: &Op, incoming: Vec<Arg>) -> (Vec<Op>, Vec<Arg>) {
        let mut prefix = Vec::new();
        let mut arguments = Vec::new();
        for arg in incoming {
            if let Arg::Held(_) = arg {
                arguments.push(arg);
                continue;
            }
            let value = self.fresh(source.at, false);
            let made = held(value, width_of(&arg).expect("an operand with a width"));
            prefix.push(self.op(source, Kind::Copy, vec![arg], vec![made.clone()], Made::plain()));
            arguments.push(made);
        }
        (prefix, arguments)
    }

    #[allow(clippy::too_many_arguments)]
    fn inline_helper(
        &mut self,
        source: &Op,
        name: &str,
        code: &[u8],
        args: Vec<Arg>,
        results: Vec<Arg>,
        inputs: BTreeSet<Reg>,
        clobbers: BTreeSet<Reg>,
    ) -> Op {
        let made = self.op(source, Kind::Call, args, results, Made::plain());
        self.calls.insert(made.at, name.to_owned());
        self.contracts.insert(made.at, _helper(name, inputs, clobbers));
        self.inline.insert(made.at, vec![code.to_vec()]);
        made
    }

    fn call_helper(&mut self, source: &Op, name: &str, code: &[u8]) -> R<Vec<Op>> {
        let left = self.pair(&source.args[0])?;
        let right = self.pair(&source.args[1])?;
        let quotient = self.pair(&source.results[0])?;
        let mut results = vec![quotient.0, quotient.1];
        if source.results.len() == 2 {
            let remainder = self.pair(&source.results[1])?;
            results.extend([remainder.0, remainder.1]);
        }
        let (prefix, args) = self.materialize(source, vec![left.0, right.0, right.1, left.1]);
        let made = self.inline_helper(source, name, code, args, results.clone(), _four_inputs(), _four_clobbers());
        if results.len() == 4 {
            self.origins.insert(value_of(&results[2]).variable, Register::EBX);
            self.origins.insert(value_of(&results[3]).variable, Register::ECX);
        }
        Ok(prefix.into_iter().chain([made]).collect())
    }

    fn multiply(&mut self, source: &Op) -> R<Vec<Op>> {
        let left = self.pair(&source.args[0])?;
        let right = self.pair(&source.args[1])?;
        let out = self.pair(&source.results[0])?;
        let narrow_inputs = || BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Dx]);
        let narrow_clobbers = || BTreeSet::from([Reg::Ax, Reg::Cx, Reg::Dx, Reg::Flags]);
        if is_zero_const(&right.1) {
            let (prefix, args) = self.materialize(source, vec![left.0, right.0, left.1]);
            let made =
                self.inline_helper(source, "__U8M32", &_MUL32, args, vec![out.0, out.1], narrow_inputs(), narrow_clobbers());
            return Ok(prefix.into_iter().chain([made]).collect());
        }
        if is_zero_const(&left.1) {
            let (prefix, args) = self.materialize(source, vec![right.0, left.0, right.1]);
            let made =
                self.inline_helper(source, "__U8M32", &_MUL32, args, vec![out.0, out.1], narrow_inputs(), narrow_clobbers());
            return Ok(prefix.into_iter().chain([made]).collect());
        }
        self.call_helper(source, "__U8M", &_MUL)
    }

    fn divide(&mut self, source: &Op) -> R<Vec<Op>> {
        let left = self.pair(&source.args[0])?;
        let right = self.pair(&source.args[1])?;
        if !is_zero_const(&right.1) {
            let signed = source.kind == Kind::Divmod;
            let (name, code) = if signed { ("__I8D", &*_SDIV) } else { ("__U8D", &*_UDIV) };
            return self.call_helper(source, name, code);
        }
        let quotient = self.pair(&source.results[0])?;
        let remainder = self.pair(&source.results[1])?;
        let results = vec![quotient.0, quotient.1, remainder.0, remainder.1];
        let (prefix, args) = self.materialize(source, vec![left.0, right.0, left.1]);
        let signed = source.kind == Kind::Divmod;
        let name = if signed { "__I8D32" } else { "__U8D32" };
        let code = if signed { &*_SDIV_CONST32 } else { &*_UDIV_CONST32 };
        let inputs = BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Dx]);
        let made = self.inline_helper(source, name, code, args, results.clone(), inputs, _four_clobbers());
        self.origins.insert(value_of(&results[2]).variable, Register::EBX);
        self.origins.insert(value_of(&results[3]).variable, Register::ECX);
        Ok(prefix.into_iter().chain([made]).collect())
    }

    fn compare(&mut self, source: &Op) -> R<Vec<Op>> {
        let Some(flag) = source.defines.first().copied().filter(|one| one.flags) else {
            return Err(Unlowered(format!("64-bit comparison at {:#x} defines no condition", source.at)));
        };
        let tests: BTreeSet<Option<Kind>> = self
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .filter(|op| op.uses.contains(&flag) && op.kind == Kind::Branch)
            .map(|op| op.test)
            .collect();
        if tests.is_empty() || !tests.iter().all(|one| matches!(one, Some(Kind::Eq | Kind::Ne))) {
            // Python prints the set in its string-hash order; this prints it sorted.
            let shown: Vec<String> = tests
                .iter()
                .map(|one| one.map_or_else(|| "None".to_owned(), |kind| kind.repr()))
                .collect();
            let shown = if shown.is_empty() { "set()".to_owned() } else { format!("{{{}}}", shown.join(", ")) };
            return Err(Unlowered(format!("64-bit comparison at {:#x} needs {shown}, not equality", source.at)));
        }
        let left = self.pair(&source.args[0])?;
        let right = self.pair(&source.args[1])?;
        let low = self.fresh(source.at, false);
        let high = self.fresh(source.at, false);
        let joined = self.fresh(source.at, false);
        Ok(vec![
            self.op(source, Kind::Xor, vec![left.0, right.0], vec![held(low, 4)], Made::plain()),
            self.op(source, Kind::Xor, vec![left.1, right.1], vec![held(high, 4)], Made::plain()),
            self.op(
                source,
                Kind::Or,
                vec![held(low, 4), held(high, 4)],
                vec![held(joined, 4)],
                Made { defines: Some(vec![joined, flag]), ..Made::plain() },
            ),
        ])
    }

    fn operation(&mut self, source: &Op) -> R<Vec<Op>> {
        let wide = source
            .args
            .iter()
            .chain(&source.results)
            .any(|arg| matches!(arg, Arg::Held(one) if one.width == 8) || matches!(arg, Arg::Const(one) if one.width == 8));
        if !wide {
            // A whole value observed through a narrower Held view reads its low dword once split.
            let mut narrowed: IndexMap<Value, Value> = IndexMap::new();
            let mut view = |arg: &Arg| match arg {
                Arg::Held(one) if self.pairs.contains_key(&one.value) => {
                    let (low, _high) = self.pairs[&one.value];
                    narrowed.insert(one.value, low);
                    held(low, one.width)
                }
                _ => arg.clone(),
            };
            let args: Vec<Arg> = source.args.iter().map(&mut view).collect();
            let results: Vec<Arg> = source.results.iter().map(&mut view).collect();
            if narrowed.is_empty() {
                return Ok(vec![source.clone()]);
            }
            let mut replaced = source.clone();
            replaced.args = args;
            replaced.results = results;
            replaced.defines = source.defines.iter().map(|value| *narrowed.get(value).unwrap_or(value)).collect();
            replaced.uses = source.uses.iter().map(|value| *narrowed.get(value).unwrap_or(value)).collect();
            return Ok(vec![replaced]);
        }
        let cell_ref = |arg: &Arg| match arg {
            Arg::Cell(cell) => cell.r#ref.clone(),
            other => panic!("AttributeError: {} has no attribute 'ref'", other.repr()),
        };
        match source.kind {
            Kind::Load => {
                let (low, high) = self.pair(&source.results[0])?;
                let reference = cell_ref(&source.args[0]);
                let (first, second) = (Self::reference(&reference, false)?, Self::reference(&reference, true)?);
                return Ok(vec![
                    self.op(
                        source,
                        Kind::Load,
                        vec![Arg::Cell(Cell { r#ref: first.clone() })],
                        vec![low],
                        Made { loads: vec![first], ..Made::plain() },
                    ),
                    self.op(
                        source,
                        Kind::Load,
                        vec![Arg::Cell(Cell { r#ref: second.clone() })],
                        vec![high],
                        Made { loads: vec![second], ..Made::plain() },
                    ),
                ]);
            }
            Kind::Store => {
                let (low, high) = self.pair(&source.args[0])?;
                let reference = cell_ref(&source.results[0]);
                let (first, second) = (Self::reference(&reference, false)?, Self::reference(&reference, true)?);
                return Ok(vec![
                    self.op(
                        source,
                        Kind::Store,
                        vec![low],
                        vec![Arg::Cell(Cell { r#ref: first.clone() })],
                        Made { stores: vec![first], ..Made::plain() },
                    ),
                    self.op(
                        source,
                        Kind::Store,
                        vec![high],
                        vec![Arg::Cell(Cell { r#ref: second.clone() })],
                        Made { stores: vec![second], ..Made::plain() },
                    ),
                ]);
            }
            _ => {}
        }
        if matches!(source.kind, Kind::Add | Kind::Sub | Kind::And | Kind::Or | Kind::Xor) && !source.results.is_empty()
        {
            return self.binary(source);
        }
        if source.kind == Kind::Sub && source.results.is_empty() {
            return self.compare(source);
        }
        if matches!(source.kind, Kind::Shl | Kind::Shr | Kind::Sar) {
            return self.shift(source);
        }
        if source.kind == Kind::Mul {
            return self.multiply(source);
        }
        if matches!(source.kind, Kind::Divmod | Kind::Udivmod) {
            return self.divide(source);
        }
        if matches!(source.kind, Kind::ZeroExtend | Kind::SignExtend) {
            let (low, high) = self.pair(&source.results[0])?;
            let arg = source.args[0].clone();
            let low_kind = if width_of(&arg).unwrap_or(4) < 4 { source.kind } else { Kind::Copy };
            let first = self.op(source, low_kind, vec![arg], vec![low.clone()], Made::plain());
            let second = if source.kind == Kind::SignExtend {
                self.op(source, Kind::Sar, vec![low, constant(31, 1)], vec![high], Made::plain())
            } else {
                self.op(source, Kind::Copy, vec![constant(0, 4)], vec![high], Made::plain())
            };
            return Ok(vec![first, second]);
        }
        if source.kind == Kind::Copy {
            let incoming = self.pair(&source.args[0])?;
            let outgoing = self.pair(&source.results[0])?;
            return Ok(vec![
                self.op(source, Kind::Copy, vec![incoming.0], vec![outgoing.0], Made::plain()),
                self.op(source, Kind::Copy, vec![incoming.1], vec![outgoing.1], Made::plain()),
            ]);
        }
        if source.kind == Kind::Arg {
            let (low, high) = self.pair(&source.args[0])?;
            let high_stores = source.stores.iter().map(|one| Self::reference(one, true)).collect::<R<Vec<_>>>()?;
            let low_stores = source.stores.iter().map(|one| Self::reference(one, false)).collect::<R<Vec<_>>>()?;
            return Ok(vec![
                self.op(source, Kind::Arg, vec![high], vec![], Made { stores: high_stores, ..Made::plain() }),
                self.op(source, Kind::Arg, vec![low], vec![], Made { stores: low_stores, ..Made::plain() }),
            ]);
        }
        if source.kind == Kind::Call {
            let results = self.pair(&source.results[0])?;
            let result = value_of(&source.results[0]);
            let mut replaced = source.clone();
            replaced.defines = source.defines.iter().copied().filter(|one| *one != result).collect();
            replaced.defines.extend([value_of(&results.0), value_of(&results.1)]);
            replaced.results = vec![results.0, results.1];
            return Ok(vec![replaced]);
        }
        if source.kind == Kind::Return {
            let args = self.pair(&source.args[0])?;
            let mut replaced = source.clone();
            replaced.uses = vec![value_of(&args.0), value_of(&args.1)];
            replaced.args = vec![args.0, args.1];
            return Ok(vec![replaced]);
        }
        Err(Unlowered(format!("64-bit {} at {:#x} has no target legalization", source.kind, source.at)))
    }

    fn run(mut self) -> R<Legalized> {
        let mut blocks = Vec::new();
        for block in &self.body.blocks {
            let mut phis = Vec::new();
            for phi in &block.phis {
                let Some((low, high)) = self.pairs.get(&phi.result).copied() else {
                    phis.push(phi.clone());
                    continue;
                };
                phis.push(Phi {
                    result: low,
                    incoming: phi.incoming.iter().map(|(at, value)| (*at, self.pairs[value].0)).collect(),
                });
                phis.push(Phi {
                    result: high,
                    incoming: phi.incoming.iter().map(|(at, value)| (*at, self.pairs[value].1)).collect(),
                });
            }
            let mut ops = Vec::new();
            for source in &block.ops {
                ops.extend(self.operation(source)?);
            }
            let mut block = block.clone();
            block.phis = phis;
            block.ops = ops;
            blocks.push(block);
        }
        let mut body = self.body.clone();
        body.blocks = blocks;
        let problems = mir::verify(&body);
        if !problems.is_empty() {
            return Err(Unlowered(format!("invalid int64 legalization: {}", problems.join("; "))));
        }
        Ok(Legalized {
            body,
            calls: self.calls,
            contracts: self.contracts,
            hints: AllocationHints { origins: self.origins, pins: self.pins },
            inline: self.inline,
        })
    }
}

/// Split every eight-byte integer into ABI dwords, if the body has one.
pub fn expanded(
    body: &MirBody,
    calls: Option<&IndexMap<i64, String>>,
    contracts: Option<&IndexMap<i64, runtime::Contract>>,
    hints: Option<&AllocationHints>,
) -> R<Legalized> {
    let empty_calls = IndexMap::new();
    let empty_contracts = IndexMap::new();
    let calls = calls.unwrap_or(&empty_calls);
    let contracts = contracts.unwrap_or(&empty_contracts);
    let hints = hints.cloned().unwrap_or_default();
    let has_wide = body.blocks.iter().flat_map(|block| &block.ops).flat_map(|op| op.args.iter().chain(&op.results)).any(
        |arg| matches!(arg, Arg::Held(one) if one.width == 8) || matches!(arg, Arg::Const(one) if one.width == 8),
    );
    if !has_wide {
        return Ok(Legalized {
            body: body.clone(),
            calls: calls.clone(),
            contracts: contracts.clone(),
            hints,
            inline: IndexMap::new(),
        });
    }
    _Legalizer::new(body, calls, contracts, &hints).run()
}
