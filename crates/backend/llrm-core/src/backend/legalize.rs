//! Promotes the integers of an odd width (i2..i63 but 8, 16 and 32) to the
//! next machine width, in MIR before instruction selection.
//!
//! # How it works
//! An operation on `iN` is made on `iM`, M the next of 8, 16, 32 and 64. The
//! bits above N of a promoted value are not defined, so an operation that
//! reads them first extends its operands in the register: `and` with the mask
//! to zero them (`lshr`, `udiv`, `urem`, unsigned compares, `zext`), `shl` and
//! `ashr` to copy the sign (`ashr`, `sdiv`, `srem`, signed compares, `sext`).
//! `add`, `sub`, `mul`, the bit operations, `shl`, `select`, `phi`, `freeze`
//! and `trunc` take them as they are. Poison and overflow flags are dropped,
//! which only makes the result more defined.
//!
//! A function that gives an odd width to anything else (an argument, a load,
//! a store, a call, a return, a switch, a GEP index) is left as it is:
//! selection refuses it as before.
//!
//! # Runs
//! Once per module in `driver::optimized`, after `minmax_expanded` and before
//! `selects`, at every -O level. SCEV's closed forms (`n * (n - 1) / 2` in
//! `i33`) made the first.
//!
//! # See also
//! LLVM's `DAGTypeLegalizer::PromoteIntegerResult` and `PromoteIntegerOperand`.
//!
//! # Example
//! An `i33` operation is made on `i64`; a shift right first zeroes the high
//! bits:
//!
//! ```text
//! %a = add i33 %x, %y     ; add i64
//! %d = lshr i33 %a, 1     ; and i64 %a, 0x1ffffffff, then lshr i64 by 1
//! ```

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, GlobalKind, InstId, Module, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};

/// `module` with its min and max calls expanded, then its odd widths promoted:
/// the compare and select of an `llvm.umin.i33` are odd widths too, and made
/// after the promotion they reached selection ("a i33 value", lcexit.bas).
pub fn legalized(module: &mut Module) -> Result<(), String> {
    crate::backend::selects::minmax_expanded(module)?;
    promoted(module)
}

/// Every integer of an odd width in `module` made the next machine width.
pub fn promoted(module: &mut Module) -> Result<(), String> {
    let context = &mut module.context;
    for global in &mut module.globals {
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        function_promoted(context, function)?;
    }
    Ok(())
}

/// The width an integer of `bits` is held in, where that is not its own.
fn promotion(bits: u32) -> Option<u32> {
    match bits {
        2..=7 => Some(8),
        9..=15 => Some(16),
        17..=31 => Some(32),
        33..=63 => Some(64),
        _ => None,
    }
}

fn bits_of(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> Option<u32> {
    function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty))
}

/// The width an integer of `bits` ends up in.
fn held(bits: u32) -> u32 {
    promotion(bits).unwrap_or(bits)
}

fn is_odd(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> bool {
    bits_of(context, function, operand).is_some_and(|bits| promotion(bits).is_some())
}

/// Whether `inst` can be promoted: the operations the module above lists.
fn promotable(opcode: &Opcode) -> bool {
    match opcode {
        Opcode::Binary(op) => matches!(
            op,
            BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::And
                | BinaryOp::Or
                | BinaryOp::Xor
                | BinaryOp::Shl
                | BinaryOp::LShr
                | BinaryOp::AShr
                | BinaryOp::UDiv
                | BinaryOp::URem
                | BinaryOp::SDiv
                | BinaryOp::SRem
        ),
        Opcode::Cast(op) => matches!(op, CastOp::Trunc | CastOp::ZExt | CastOp::SExt),
        Opcode::ICmp(_) | Opcode::Phi | Opcode::Select | Opcode::Freeze => true,
        _ => false,
    }
}

fn function_promoted(
    context: &mut Context,
    function: &mut Function,
) -> Result<(), String> {
    let found: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| {
            let instruction = function.instruction(inst);
            context.types.int_bits(instruction.ty).is_some_and(|bits| promotion(bits).is_some())
                || instruction.operands.iter().any(|&operand| is_odd(context, function, operand))
        })
        .collect();
    let arguments_odd = found
        .iter()
        .any(
            |&inst| function
                .instruction(inst)
                .operands
                .iter()
                .any(
                    |&operand| matches!(
                        operand,
                        Operand::Value(value) if is_odd(context, function, operand) && matches!(function.value(value).def, ValueDef::Argument(_))
                    ),
                ),
        );
    if found.is_empty() || arguments_odd || found.iter().any(|&inst| !promotable(&function.instruction(inst).opcode)) {
        return Ok(());
    }
    let mut done: Vec<(InstId, Option<ValueId>, Operand)> = Vec::new();
    for inst in found {
        let instruction = function.instruction(inst).clone();
        let new = promote(context, function, inst, &instruction.opcode, &instruction.operands, instruction.ty)?;
        done.push((inst, instruction.result, new));
    }
    for &(_, result, new) in &done {
        if let Some(result) = result {
            function.replace_all_uses_with(result, new);
        }
    }
    for (inst, _, _) in done {
        function.erase(inst)?;
    }
    Ok(())
}

/// What `inst` becomes: a new instruction, or an operand that already holds
/// the value.
fn promote(
    context: &mut Context,
    function: &mut Function,
    inst: InstId,
    opcode: &Opcode,
    operands: &[Operand],
    ty: llrm_mir::types::TypeId,
) -> Result<Operand, String> {
    let result_bits = context.types.int_bits(ty);
    let wide = result_bits.map(held);
    let wide_ty = wide.map(|bits| context.types.int(bits));
    let emit = |context: &mut Context, function: &mut Function, opcode, ty, operands: Vec<Operand>| {
        let _ = context;
        let made = function.create_instruction(opcode, ty, operands, Flags::default(), None);
        function.insert(made, Position::Before(inst))?;
        Ok::<Operand, String>(Operand::Value(function.instruction(made).result.expect("a value")))
    };
    match opcode {
        Opcode::Binary(op) => {
            let (a_signed, a_zero, b_zero) = match op {
                BinaryOp::LShr | BinaryOp::UDiv | BinaryOp::URem => (false, true, true),
                BinaryOp::AShr => (true, false, true),
                BinaryOp::SDiv | BinaryOp::SRem => (true, false, false),
                BinaryOp::Shl => (false, false, true),
                _ => (false, false, false),
            };
            let b_signed = matches!(op, BinaryOp::SDiv | BinaryOp::SRem);
            let wide = wide.ok_or("an integer result")?;
            let a = if a_signed || a_zero {
                extended(context, function, inst, operands[0], a_signed, wide)?
            } else {
                retyped(context, function, operands[0], wide)
            };
            let b = if b_zero || b_signed {
                extended(context, function, inst, operands[1], b_signed, wide)?
            } else {
                retyped(context, function, operands[1], wide)
            };
            emit(context, function, Opcode::Binary(*op), wide_ty.expect("a type"), vec![a, b])
        }
        Opcode::Select => {
            let wide = wide.ok_or("an integer select")?;
            let (a, b) = (retyped(context, function, operands[1], wide), retyped(context, function, operands[2], wide));
            emit(context, function, Opcode::Select, wide_ty.expect("a type"), vec![operands[0], a, b])
        }
        Opcode::Freeze => {
            let wide = wide.ok_or("an integer freeze")?;
            let a = retyped(context, function, operands[0], wide);
            emit(context, function, Opcode::Freeze, wide_ty.expect("a type"), vec![a])
        }
        Opcode::Phi => {
            let wide = wide.ok_or("an integer phi")?;
            let made = operands
                .iter()
                .enumerate()
                .map(|(at, &operand)| if at % 2 == 0 { retyped(context, function, operand, wide) } else { operand })
                .collect();
            emit(context, function, Opcode::Phi, wide_ty.expect("a type"), made)
        }
        Opcode::ICmp(predicate) => {
            let signed =
                matches!(
                    predicate,
                    IntPredicate::Slt | IntPredicate::Sle | IntPredicate::Sgt | IntPredicate::Sge
                );
            let bits = bits_of(context, function, operands[0]).ok_or("an integer compare")?;
            let wide = held(bits);
            let a = extended(context, function, inst, operands[0], signed, wide)?;
            let b = extended(context, function, inst, operands[1], signed, wide)?;
            emit(context, function, Opcode::ICmp(*predicate), ty, vec![a, b])
        }
        Opcode::Cast(op) => {
            let from = held(bits_of(context, function, operands[0]).ok_or("an integer cast")?);
            let to = held(result_bits.ok_or("an integer cast")?);
            let to_ty = context.types.int(to);
            let source = match op {
                CastOp::Trunc => operands[0],
                _ => extended(context, function, inst, operands[0], *op == CastOp::SExt, from)?,
            };
            if from == to {
                return Ok(source);
            }
            emit(context, function, Opcode::Cast(*op), to_ty, vec![source])
        }
        _ => Err("a promotable operation".into()),
    }
}

/// `operand`, as an integer of `wide` bits where it is an odd one; its bits
/// above its own width are as they are.
fn retyped(
    context: &Context,
    function: &Function,
    operand: Operand,
    wide: u32,
) -> Operand {
    let Operand::Constant(id) = operand else { return operand };
    let constant = context.get(id).clone();
    if context.types.int_bits(constant.ty).is_none_or(|bits| promotion(bits).is_none()) {
        return operand;
    }
    let _ = function;
    let ty = context.types.int(wide);
    let kind = match constant.kind {
        ConstantKind::Int(bits) => ConstantKind::Int(bits),
        ConstantKind::Poison => ConstantKind::Poison,
        ConstantKind::Undef => ConstantKind::Undef,
        _ => ConstantKind::Int(0),
    };
    Operand::Constant(context.constant(Constant { ty, kind }))
}

/// `operand` as an integer of `wide` bits whose bits above its width are
/// zero, or copies of its sign.
fn extended(
    context: &Context,
    function: &mut Function,
    at: InstId,
    operand: Operand,
    signed: bool,
    wide: u32,
) -> Result<Operand, String> {
    let bits = bits_of(context, function, operand).ok_or("an integer operand")?;
    if promotion(bits).is_none() {
        return Ok(operand);
    }
    let ty = context.types.int(wide);
    if let Operand::Constant(id) = operand {
        let constant = context.get(id).clone();
        let ConstantKind::Int(value) = constant.kind else { return Ok(retyped(context, function, operand, wide)) };
        let value = if signed {
            let shift = 128 - bits;
            ((value << shift) as i128) >> shift
        } else {
            value as i128
        };
        return Ok(Operand::Constant(context.int(ty, value)));
    }
    let before = |function: &mut Function, opcode, operands: Vec<Operand>| -> Result<Operand, String> {
        let made = function.create_instruction(opcode, ty, operands, Flags::default(), None);
        function.insert(made, Position::Before(at))?;
        Ok(Operand::Value(function.instruction(made).result.expect("a value")))
    };
    if signed {
        let shift = Operand::Constant(context.int(ty, i128::from(wide - bits)));
        let high = before(function, Opcode::Binary(BinaryOp::Shl), vec![operand, shift])?;
        before(function, Opcode::Binary(BinaryOp::AShr), vec![high, shift])
    } else {
        let mask = Operand::Constant(context.int(ty, (1_i128 << bits) - 1));
        before(function, Opcode::Binary(BinaryOp::And), vec![operand, mask])
    }
}

#[cfg(test)]
mod tests {
    use llrm_mir::interpret::{Val, run};

    fn promoted(text: &str) -> (llrm_mir::module::Module, llrm_mir::module::Module) {
        let before = llrm_mir::parse::module(text).unwrap();
        let mut after = before.clone();
        super::promoted(&mut after).unwrap();
        assert_eq!(llrm_mir::verify::verify(&after), Vec::<String>::new(), "{}", llrm_mir::print::module(&after));
        (before, after)
    }

    fn odd_widths_left(module: &llrm_mir::module::Module) -> bool {
        let printed = llrm_mir::print::module(module);
        printed.split(|c: char| !c.is_ascii_alphanumeric()).any(|word| {
            word.strip_prefix('i').and_then(|n| n.parse::<u32>().ok()).is_some_and(|n| super::promotion(n).is_some())
        })
    }

    fn same_answers(
        before: &llrm_mir::module::Module,
        after: &llrm_mir::module::Module,
        inputs: &[(u64, u64)],
        context: &str,
    ) {
        for &(a, b) in inputs {
            let arguments =
                || vec![Val::Int { bits: u128::from(a), width: 64 }, Val::Int { bits: u128::from(b), width: 64 }];
            assert_eq!(
                run(after, "f", arguments(), 1000).unwrap(),
                run(before, "f", arguments(), 1000).unwrap(),
                "{context} on {a:#x}, {b:#x}"
            );
        }
    }

    /// indvars' closed form `n * (n - 1) / 2` is made in `i33`; selection
    /// refused it ("a i33 value", #2497) and the bench kernel did not compile.
    #[test]
    fn test_an_i33_sum_is_made_in_i64() {
        let text = "define i64 @f(i64 %a, i64 %b) {
entry:
  %n = trunc i64 %a to i32
  %m = sub i32 %n, 1
  %x = zext i32 %n to i33
  %y = zext i32 %m to i33
  %p = mul i33 %x, %y
  %h = lshr i33 %p, 1
  %t = trunc i33 %h to i32
  %r = zext i32 %t to i64
  ret i64 %r
}
";
        let (before, after) = promoted(text);
        assert!(odd_widths_left(&before) && !odd_widths_left(&after), "{}", llrm_mir::print::module(&after));
        same_answers(&before, &after, &[(0, 0), (1, 0), (100, 0), (0xFFFF_FFFF, 0), (0x8000_0000, 0)], "i33 sum");
    }

    /// indvars' `umin.i33` of a widened trip count was expanded to an `i33`
    /// compare and select after the promotion had run, and selection refused
    /// it ("a i33 value", qb/lcexit.bas).
    #[test]
    fn test_an_i33_umin_call_is_expanded_and_made_in_i64() {
        let text = "declare i33 @llvm.umin.i33(i33, i33)
define i64 @f(i64 %a, i64 %b) {
entry:
  %n = trunc i64 %a to i32
  %x = zext i32 %n to i33
  %m = call i33 @llvm.umin.i33(i33 %x, i33 9)
  %t = trunc i33 %m to i32
  %r = zext i32 %t to i64
  ret i64 %r
}
";
        let mut module = llrm_mir::parse::module(text).unwrap();
        super::legalized(&mut module).unwrap();
        assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new(), "{}", llrm_mir::print::module(&module));
        let printed = llrm_mir::print::module(&module);
        let body = &printed[printed.find("define").unwrap()..];
        assert!(!odd_widths_left_in(body), "{body}");
    }

    fn odd_widths_left_in(printed: &str) -> bool {
        printed.split(|c: char| !c.is_ascii_alphanumeric()).any(|word| {
            word.strip_prefix('i').and_then(|n| n.parse::<u32>().ok()).is_some_and(|n| super::promotion(n).is_some())
        })
    }

    /// Every odd width, every operation: the promoted function answers as the
    /// original did, and no odd width is left.
    #[test]
    fn test_every_odd_width_and_operation_is_promoted_to_the_same_answers() {
        let inputs = [
            (0, 1),
            (1, 3),
            (7, 5),
            (0xFFFF_FFFF_FFFF_FFFF, 3),
            (0x1234_5678_9ABC_DEF0, 5),
            (0x8000_0000_0000_0001, 1),
        ];
        let binary = ["add", "sub", "mul", "and", "or", "xor", "udiv", "urem", "sdiv", "srem"];
        for n in (2..=63).filter(|n| ![8, 16, 32].contains(n)) {
            let mut bodies: Vec<(String, &str)> = Vec::new();
            for op in binary {
                bodies.push((format!("%r = {op} i{n} %x, %y"), "zext"));
                bodies.push((format!("%r = {op} i{n} %x, %y"), "sext"));
            }
            for (shift, name) in [("shl", "shl"), ("lshr", "lshr"), ("ashr", "ashr")] {
                bodies.push((
                    format!("%c = and i{n} %y, 1\n  %r = {name} i{n} %x, %c"),
                    if shift == "ashr" { "sext" } else { "zext" },
                ));
            }
            bodies.push((format!("%r = lshr i{n} %x, {}", n - 1), "zext"));
            bodies.push((format!("%r = ashr i{n} %x, {}", n - 1), "sext"));
            bodies.push((format!("%k = freeze i{n} %x\n  %r = add i{n} %k, %y"), "zext"));
            for predicate in ["eq", "ne", "ult", "ule", "ugt", "uge", "slt", "sle", "sgt", "sge"] {
                bodies.push((
                    format!("%k = icmp {predicate} i{n} %x, %y\n  %r = select i1 %k, i{n} %x, i{n} %y"),
                    "zext",
                ));
                bodies
                    .push((format!("%k = icmp {predicate} i{n} %x, 3\n  %r = select i1 %k, i{n} %y, i{n} -1"), "sext"));
            }
            for (body, extension) in bodies {
                let text = format!(
                    "define i64 @f(i64 %a, i64 %b) {{
entry:
  %x = trunc i64 %a to i{n}
  %y = trunc i64 %b to i{n}
  {body}
  %w = {extension} i{n} %r to i64
  ret i64 %w
}}
"
                );
                let (before, after) = promoted(&text);
                assert!(
                    odd_widths_left(&before) && !odd_widths_left(&after),
                    "{text}{}",
                    llrm_mir::print::module(&after)
                );
                same_answers(&before, &after, &inputs, &body);
            }
        }
    }

    /// A loop's phi in `i33` and a compare in `i7`.
    #[test]
    fn test_an_odd_width_phi_is_promoted() {
        let text = "define i64 @f(i64 %a, i64 %b) {
entry:
  br label %loop
loop:
  %i = phi i33 [ 0, %entry ], [ %next, %loop ]
  %next = add i33 %i, 3
  %done = icmp uge i33 %next, 100
  br i1 %done, label %exit, label %loop
exit:
  %r = zext i33 %next to i64
  ret i64 %r
}
";
        let (before, after) = promoted(text);
        assert!(!odd_widths_left(&after), "{}", llrm_mir::print::module(&after));
        same_answers(&before, &after, &[(0, 0)], "phi");
    }
}
