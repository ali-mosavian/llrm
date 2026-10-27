//! `LibCallSimplifier`: a call of a C library function that a cheaper one
//! does as well becomes that one, as `printf("hi\n")` becomes `puts("hi")`.
//!
//! # How it works
//! A translation of the `printf` and `puts` arms of LLVM's
//! [`SimplifyLibCalls.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/SimplifyLibCalls.cpp)
//! at d1106deb71cc: `optimizePrintFString` (:3466) and `optimizePuts` (:3978).
//! A direct call whose callee is a library function by name and prototype
//! (`TargetLibraryInfo::get_lib_func_of`) is rewritten when its format is a
//! constant string: an empty format is dropped, a one-character one becomes
//! `putchar`, `"%s"` of a constant, `"%c"`, `"%s\n"` and a format ending in
//! `\n` without `%` become `putchar` or `puts` (the new string goes in a
//! private `str.N` table). All but the empty format need the result unused.
//!
//! # Deviations
//! - D1: `puts` and `putchar` are formed only where the compile states them
//!   in `Options::library`, as `strlen` is (loopidiom D11); the pin reads the
//!   triple. Exit: `Target::library` naming them.
//! - D2: the other arms (`fprintf`, `fputs`, `fwrite`, `sprintf`, `str*`,
//!   `mem*`, math) are untranslated, and so are `iprintf`/`__small_printf`.
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved.
//!
//! # Example
//! ```text
//! call i32 (ptr, ...) @printf(ptr @.str)   ; @.str = c"hi\0A\00", unused
//! ```
//! becomes `call i32 @puts(ptr @str.0)` with `@str.0 = c"hi\00"`.

use llrm_mir::constantfolding::{Env, read_byte_array_from_global};
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::edit::Position;
use llrm_mir::libfunc::{Generic, LibFunc, Routine, TargetLibraryInfo};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{Attribute, CallInfo, CastOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::types::Type;

pub struct SimplifyLibCalls {
    routines: Vec<Routine>,
}

impl SimplifyLibCalls {
    /// The pass over a compile that states the library `routines` its program
    /// links.
    pub fn with_routines(routines: Vec<Routine>) -> Self {
        Self { routines }
    }
}

/// `getConstantStringInfo` of a pointer that is a constant global: the bytes
/// up to the first NUL, which must be there.
fn constant_string(
    unit: &Unit,
    env: Env,
    operand: Operand,
) -> Option<Vec<u8>> {
    let Operand::Constant(id) = operand else { return None };
    let ConstantKind::Global(global) = unit.context.get(id).kind else { return None };
    let bytes = read_byte_array_from_global(unit.context, env, global, 0)?;
    let ConstantKind::Bytes(raw) = &unit.context.get(bytes).kind else { return None };
    let end = raw.iter().position(|&byte| byte == 0)?;
    Some(raw[..end].to_vec())
}

/// `emitPutChar` / `emitPutS`: the call of the stated routine `standard` on
/// `argument`, placed before `at`; none where the compile does not state it.
fn emit(
    unit: &mut Unit,
    routines: &[Routine],
    standard: &str,
    argument: Operand,
    returns: llrm_mir::types::TypeId,
    parameter: llrm_mir::types::TypeId,
    at: InstId,
) -> Option<InstId> {
    let routine = routines.iter().find(|one| one.standard == standard)?;
    let ty =
        unit.context.types.intern(Type::Function { returns, parameters: vec![parameter], variadic: false });
    let attrs = vec![llrm_mir::facts::Fact::NoUnwind.carrier()];
    let pointer = unit.context.types.ptr(0);
    let callee_id = unit.declared.routine(&routine.symbol, ty, routine.calling_convention, attrs, Vec::<Attribute>::new());
    let callee = Operand::Constant(unit.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee_id) }));
    let info = CallInfo {
        function_type: ty,
        calling_convention: routine.calling_convention,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new()],
        attrs: Vec::new(),
        tail: Default::default(),
    };
    let call = unit.function.create_instruction(
        Opcode::Call(Box::new(info)),
        returns,
        vec![argument, callee],
        Flags::default(),
        None,
    );
    unit.function.insert(call, Position::Before(at)).expect("a placed call");
    Some(call)
}

enum Rewrite {
    /// The call goes, its result (if read) the constant 0.
    Drop,
    PutChar(Operand),
    PutS(Operand),
}

/// `optimizePrintFString`.
fn print_f(
    unit: &mut Unit,
    env: Env,
    call: InstId,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(call).operands.to_vec();
    let args = &operands[..operands.len() - 1];
    let format = constant_string(unit, env, args[0])?;
    if format.is_empty() {
        return Some(Rewrite::Drop);
    }
    let result = unit.function.instruction(call).result.expect("printf returns int");
    if !unit.function.users(result).is_empty() {
        return None;
    }
    let int = unit.function.value(result).ty;
    let character = |unit: &Unit, byte: u8| Operand::Constant(unit.context.int(int, i128::from(byte)));
    if format.len() == 1 || format == b"%%" {
        return Some(Rewrite::PutChar(character(unit, format[0])));
    }
    if format == b"%s" && args.len() > 1 {
        let text = constant_string(unit, env, args[1])?;
        return match text.as_slice() {
            [] => Some(Rewrite::Drop),
            [byte] => Some(Rewrite::PutChar(character(unit, *byte))),
            [.., b'\n'] => Some(Rewrite::PutS(string(unit, &text[..text.len() - 1]))),
            _ => None,
        };
    }
    if format.last() == Some(&b'\n') && !format.contains(&b'%') {
        return Some(Rewrite::PutS(string(unit, &format[..format.len() - 1])));
    }
    if format == b"%c" && args.len() > 1 {
        let ty = unit.function.operand_type(unit.context, args[1])?;
        let from = unit.context.types.int_bits(ty)?;
        let to = unit.context.types.int_bits(int)?;
        let cast = |unit: &mut Unit, op| {
            let made = unit.function.create_instruction(Opcode::Cast(op), int, vec![args[1]], Flags::default(), None);
            unit.function.insert(made, Position::Before(call)).expect("a placed cast");
            Operand::Value(unit.function.instruction(made).result.expect("a cast has a result"))
        };
        let value = match from.cmp(&to) {
            std::cmp::Ordering::Equal => args[1],
            std::cmp::Ordering::Less => cast(unit, CastOp::ZExt),
            std::cmp::Ordering::Greater => cast(unit, CastOp::Trunc),
        };
        return Some(Rewrite::PutChar(value));
    }
    if format == b"%s\n" && args.len() > 1 {
        let ty = unit.function.operand_type(unit.context, args[1])?;
        if matches!(unit.context.types.get(ty), Type::Pointer { .. }) {
            return Some(Rewrite::PutS(args[1]));
        }
    }
    None
}

/// `CreateGlobalString(text, "str")`: a private constant `text` and a NUL.
fn string(
    unit: &mut Unit,
    text: &[u8],
) -> Operand {
    let mut raw = text.to_vec();
    raw.push(0);
    let i8 = unit.context.types.int(8);
    let ty = unit.context.types.intern(Type::Array { element: i8, count: raw.len() as u64 });
    let initializer = unit.context.constant(Constant { ty, kind: ConstantKind::Bytes(raw) });
    let global = unit.declared.table("str", ty, initializer);
    let pointer = unit.context.types.ptr(0);
    Operand::Constant(unit.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(global) }))
}

/// `optimizePuts`: `puts("")` is `putchar('\n')`.
fn put_s(
    unit: &mut Unit,
    env: Env,
    call: InstId,
) -> Option<Rewrite> {
    let result = unit.function.instruction(call).result?;
    if !unit.function.users(result).is_empty() {
        return None;
    }
    let operands = unit.function.instruction(call).operands.to_vec();
    if !constant_string(unit, env, operands[0])?.is_empty() {
        return None;
    }
    let int = unit.function.value(result).ty;
    Some(Rewrite::PutChar(Operand::Constant(unit.context.int(int, i128::from(b'\n')))))
}

impl FunctionPass for SimplifyLibCalls {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "simplifylibcalls"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("simplifylibcalls")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        let int_bits = unit.layout.pointer(0).index_bits;
        let info = TargetLibraryInfo::new(&Generic { int_bits: 32, size_t_bits: int_bits });
        let env = Env { globals: &outer.globals, layout: unit.layout };
        let mut changed = false;
        let calls: Vec<InstId> = unit
            .function
            .layout()
            .to_vec()
            .into_iter()
            .flat_map(|block| unit.function.iter_block(block).collect::<Vec<_>>())
            .filter(|&inst| matches!(unit.function.instruction(inst).opcode, Opcode::Call(_)))
            .collect();
        for call in calls {
            let Some(global) = llrm_mir::memory::callee(unit.context, unit.function, call) else { continue };
            let Some(func) = outer.globals.get(global.0 as usize).and_then(|one| info.get_lib_func_of(&unit.context.types, one))
            else {
                continue;
            };
            let rewrite = match func {
                LibFunc::Printf => print_f(unit, env, call),
                LibFunc::Puts => put_s(unit, env, call),
                _ => None,
            };
            let Some(rewrite) = rewrite else { continue };
            let result = unit.function.instruction(call).result;
            let int = result.map(|value| unit.function.value(value).ty);
            let pointer = unit.context.types.ptr(0);
            let new = match rewrite {
                Rewrite::Drop => None,
                Rewrite::PutChar(argument) => {
                    let int = int.expect("an int result");
                    match emit(unit, &self.routines, "putchar", argument, int, int, call) {
                        Some(new) => Some(new),
                        None => continue,
                    }
                }
                Rewrite::PutS(argument) => {
                    let int = int.expect("an int result");
                    match emit(unit, &self.routines, "puts", argument, int, pointer, call) {
                        Some(new) => Some(new),
                        None => continue,
                    }
                }
            };
            if let Some(result) = result {
                let replacement = match new {
                    Some(new) => Operand::Value(unit.function.instruction(new).result.expect("an int result")),
                    None => Operand::Constant(unit.context.int(int.expect("an int result"), 0)),
                };
                unit.function.replace_value(result, replacement);
            }
            unit.function.erase(call).expect("nothing reads it");
            changed = true;
        }
        if changed {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}
