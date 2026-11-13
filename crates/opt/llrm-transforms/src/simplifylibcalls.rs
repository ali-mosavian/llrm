//! `LibCallSimplifier`: a call of a C library function that a cheaper one
//! does as well becomes that one, as `printf("hi\n")` becomes `puts("hi")`.
//!
//! # How it works
//! A translation of the `printf`, `fprintf`, `puts`, `fputs` and `fwrite` arms
//! of LLVM's
//! [`SimplifyLibCalls.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/SimplifyLibCalls.cpp)
//! at d1106deb71cc: `optimizePrintFString` (:3466), `optimizeFPrintFString`
//! (:3887), `optimizeFWrite` (:3923), `optimizeFPuts` (:3950) and
//! `optimizePuts` (:3978).
//! A direct call whose callee is a library function by name and prototype
//! (`TargetLibraryInfo::get_lib_func_of`) is rewritten when its format is a
//! constant string: an empty format is dropped, a one-character one becomes
//! `putchar`, `"%s"` of a constant, `"%c"`, `"%s\n"` and a format ending in
//! `\n` without `%` become `putchar` or `puts` (the new string goes in a
//! private `str.N` table). `fprintf` of a format without `%` is `fwrite`,
//! of `"%c"` `fputc`, of `"%s"` `fputs`; `fputs` of a constant string is
//! `fwrite` (not at `optsize`); `fwrite` of one byte is `fputc`, of none is
//! 0. All but the empty format and the empty `fwrite` need the result unused.
//! `strcmp`, `strncmp`, `memcmp` and `bcmp` of constant arrays fold to their
//! order, `strcmp("", p)` and `strcmp(p, "")` to the first byte,
//! `memcmp(p, q, 1)` to the byte difference, `memcmp(p, q, n) == 0` to one
//! integer compare for a legal `n * 8` and aligned pointers; `strcmp(p, "x")`
//! and `strncmp` of a literal are `memcmp` where the bytes are readable and the
//! result is only compared with zero, and `memcmp(p, q, n) == 0` is `bcmp`
//! (`compare.rs`). `strchr` of a constant string and character, `strpbrk`,
//! `strspn`, `strcspn` and `strstr` over constant strings fold to an address
//! into the first string, null or a length, and `memchr` of a constant array
//! or size folds to compares and a select (`search.rs`). `strcpy`, `stpcpy`,
//! `strcat`, `strncat` and `strncpy` of a string of known length are an
//! `llvm.memcpy` of it with its NUL (`copy.rs`), `strlen` finding the end where
//! the compile states it.
//!
//! # Deviations
//! - D1: `puts`, `putchar`, `fwrite`, `fputc`, `fputs`, `memcmp` and `bcmp` are
//!   formed only where the compile states them in `Options::library`, as
//!   `strlen` is (loopidiom D11); the pin reads the triple. Exit:
//!   `Target::library` naming them.
//! - D2: the other arms (`sprintf`, `snprintf`, the other `str*`, `memchr`,
//!   math) are untranslated, and so are `iprintf`/`__small_printf`,
//!   `optimizeErrorReporting` and the argument annotations. The `memcpy` family
//!   is `memops.rs`.
//! - D4: the arms of `strchr`, `strpbrk`, `strcspn` and `strstr` that call a
//!   routine (`memchr`, `strlen`, `strncmp`, `strchr`) are untranslated, as is
//!   the character compare the pin makes of a `strchr` or `strstr` tested
//!   against its own string: such a call is left as it is.
//! - D5: `memchr` of a constant array and size tested against null (the pin's
//!   bit-field and OR-chain rewrite) is not translated.
//! - D6: `stpcpy` with an unused result (the pin's `strcpy` call) and
//!   `stpcpy(x, x)` (its `strlen` call) are untranslated.
//! - D7: `strncpy` of an empty source (the pin's `memset`) and of a bound past
//!   the source and its NUL (a NUL-padded copy) are untranslated, as is
//!   `stpncpy`.
//! - D3: `strcmp` of two strings whose lengths are known but whose contents are
//!   not (a PHI or select of literals) is not rewritten: `GetStringLength` here
//!   reads a constant global only.
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
use llrm_mir::module::{InstId, Operand, UnnamedAddr};
use llrm_mir::opcode::{Attribute, CallInfo, CastOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::nonzero::strip_and_accumulate_constant_offsets;
use llrm_mir::valuetracking::underlying::get_underlying_object;

mod compare;
mod copy;
mod memops;
mod search;

/// One table per module: the target's library, else the pin's defaults for
/// the pointer's width.
pub(crate) fn target_library_info(
    outer: &llrm_mir::passes::Outer,
    size_bits: u32,
) -> std::rc::Rc<TargetLibraryInfo> {
    outer.memo::<TargetLibraryInfo>(0, || {
        let library =
            outer.program().target.library().unwrap_or_else(|| std::rc::Rc::new(Generic::for_pointer(size_bits)));
        TargetLibraryInfo::new(&*library)
    })
}

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

/// The instruction `opcode` placed before the call, as an operand.
fn build(
    unit: &mut Unit,
    site: &Site,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
) -> Operand {
    let made = unit.function.create_instruction(opcode, ty, operands, Flags::default(), None);
    unit.function.insert(made, Position::Before(site.call)).expect("a placed instruction");
    Operand::Value(unit.function.instruction(made).result.expect("a result"))
}

/// `GetStringLength`: the length with the NUL of a constant string, 0 where
/// the array has no NUL.
fn string_length(
    unit: &Unit,
    site: &Site,
    operand: Operand,
) -> u64 {
    match constant_array(unit, site.env, operand) {
        Some(bytes) => bytes.iter().position(|&byte| byte == 0).map_or(0, |end| end as u64 + 1),
        None => 0,
    }
}

/// The length operand as a constant, `dyn_cast<ConstantInt>(Size)`.
fn constant_length(
    unit: &Unit,
    operand: Operand,
) -> Option<u64> {
    let Operand::Constant(id) = operand else { return None };
    match unit.context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits as u64),
        _ => None,
    }
}

/// `getConstantDataArrayInfo`
/// ([L7001](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueTracking.cpp#L7001))
/// of a pointer based on a constant global: every byte from the constant
/// offset the pointer adds, through GEPs and casts.
fn constant_array(
    unit: &Unit,
    env: Env,
    operand: Operand,
) -> Option<Vec<u8>> {
    let query = Query { globals: env.globals, ..Query::bare(unit.context, unit.function, env.layout) };
    let Operand::Constant(object) = get_underlying_object(&query, operand, 0) else { return None };
    let ConstantKind::Global(global) = unit.context.get(object).kind else { return None };
    let space = match unit.context.types.get(query.type_of(operand)?) {
        Type::Pointer(space) => *space,
        _ => return None,
    };
    let width = env.layout.pointer(space).index_bits;
    let (base, offset) = strip_and_accumulate_constant_offsets(&query, operand, width, true);
    if base != Operand::Constant(object) {
        return None;
    }
    let bytes = read_byte_array_from_global(unit.context, env, global, u64::try_from(offset).ok()?)?;
    let ConstantKind::Bytes(raw) = &unit.context.get(bytes).kind else { return None };
    Some(raw.to_vec())
}

/// `getConstantStringInfo(V, Str, TrimAtNul)` of a pointer that is a constant
/// global.
fn string_info(
    unit: &Unit,
    site: &Site,
    operand: Operand,
    trim_at_nul: bool,
) -> Option<Vec<u8>> {
    let bytes = constant_array(unit, site.env, operand)?;
    Some(if trim_at_nul { bytes.into_iter().take_while(|&byte| byte != 0).collect() } else { bytes })
}

/// `constant_array` over a context alone.
pub(crate) fn constant_array_in(
    context: &llrm_mir::context::Context,
    env: Env,
    operand: Operand,
) -> Option<Vec<u8>> {
    let Operand::Constant(id) = operand else { return None };
    let ConstantKind::Global(global) = context.get(id).kind else { return None };
    let bytes = read_byte_array_from_global(context, env, global, 0)?;
    let ConstantKind::Bytes(raw) = &context.get(bytes).kind else { return None };
    Some(raw.to_vec())
}

/// `getConstantStringInfo` with `TrimAtNul`: the bytes up to the first NUL,
/// else the whole tail.
fn constant_string(
    unit: &Unit,
    env: Env,
    operand: Operand,
) -> Option<Vec<u8>> {
    let raw = constant_array(unit, env, operand)?;
    let end = raw.iter().position(|&byte| byte == 0).unwrap_or(raw.len());
    Some(raw[..end].to_vec())
}

/// A call of a stated library routine to form: `emitPutChar`, `emitPutS`,
/// `emitFWrite`, `emitFPutC`, `emitFPutS`.
pub(crate) struct NewCall {
    pub(crate) standard: &'static str,
    pub(crate) args: Vec<Operand>,
    pub(crate) returns: llrm_mir::types::TypeId,
    pub(crate) params: Vec<llrm_mir::types::TypeId>,
}

enum Rewrite {
    /// The call goes, its result (if read) the constant 0.
    Drop,
    /// The call's result becomes this value, built before the call.
    Value(Operand),
    Call(NewCall),
}

/// The call of the stated routine `new`, placed at `at`; none where the
/// compile does not state it.
pub(crate) fn emit(
    unit: &mut Unit,
    routines: &[Routine],
    new: NewCall,
    at: Position,
) -> Option<InstId> {
    let routine = routines.iter().find(|one| one.standard == new.standard)?;
    let ty =
        unit.context.types.intern(Type::Function { returns: new.returns, parameters: new.params, variadic: false });
    let attrs = vec![llrm_mir::facts::Fact::NoUnwind.carrier()];
    let pointer = unit.context.types.ptr(0);
    let callee_id =
        unit.declared.routine(&routine.symbol, ty, routine.calling_convention, attrs, Vec::<Attribute>::new());
    let callee =
        Operand::Constant(unit.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee_id) }));
    let info = CallInfo {
        function_type: ty,
        calling_convention: routine.calling_convention,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new(); new.args.len()],
        attrs: Vec::new(),
        tail: Default::default(),
    };
    let mut operands = new.args;
    operands.push(callee);
    let call =
        unit.function.create_instruction(Opcode::Call(Box::new(info)), new.returns, operands, Flags::default(), None);
    unit.function.insert(call, at).expect("a placed call");
    Some(call)
}

/// What the rewrites need of the call and its target.
struct Site<'a> {
    env: Env<'a>,
    call: InstId,
    /// `int` and `size_t`.
    int: llrm_mir::types::TypeId,
    size: llrm_mir::types::TypeId,
    optimize_size: bool,
}

fn put_char(
    site: &Site,
    value: Operand,
) -> Rewrite {
    Rewrite::Call(NewCall { standard: "putchar", args: vec![value], returns: site.int, params: vec![site.int] })
}

fn put_s(
    site: &Site,
    string: Operand,
    unit: &Unit,
) -> Rewrite {
    let pointer = unit.context.types.ptr(0);
    Rewrite::Call(NewCall { standard: "puts", args: vec![string], returns: site.int, params: vec![pointer] })
}

/// `B.CreateIntCast(value, int, is_signed)` before the call.
fn int_cast(
    unit: &mut Unit,
    site: &Site,
    value: Operand,
    signed: bool,
) -> Option<Operand> {
    let ty = unit.function.operand_type(unit.context, value)?;
    let from = unit.context.types.int_bits(ty)?;
    let to = unit.context.types.int_bits(site.int)?;
    let op = match from.cmp(&to) {
        std::cmp::Ordering::Equal => return Some(value),
        std::cmp::Ordering::Less if signed => CastOp::SExt,
        std::cmp::Ordering::Less => CastOp::ZExt,
        std::cmp::Ordering::Greater => CastOp::Trunc,
    };
    let made = unit.function.create_instruction(Opcode::Cast(op), site.int, vec![value], Flags::default(), None);
    unit.function.insert(made, Position::Before(site.call)).expect("a placed cast");
    Some(Operand::Value(unit.function.instruction(made).result.expect("a cast has a result")))
}

/// `optimizePrintFString`.
fn print_f(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let args = &operands[..operands.len() - 1];
    let format = constant_string(unit, site.env, args[0])?;
    if format.is_empty() {
        return Some(Rewrite::Drop);
    }
    let result = unit.function.instruction(site.call).result.expect("printf returns int");
    if !unit.function.users(result).is_empty() {
        return None;
    }
    let character = |unit: &Unit, byte: u8| Operand::Constant(unit.context.int(site.int, i128::from(byte)));
    if format.len() == 1 || format == b"%%" {
        return Some(put_char(site, character(unit, format[0])));
    }
    if format == b"%s" && args.len() > 1 {
        let text = constant_string(unit, site.env, args[1])?;
        return match text.as_slice() {
            [] => Some(Rewrite::Drop),
            [byte] => Some(put_char(site, character(unit, *byte))),
            [.., b'\n'] => {
                let string = string(unit, &text[..text.len() - 1]);
                Some(put_s(site, string, unit))
            }
            _ => None,
        };
    }
    if format.last() == Some(&b'\n') && !format.contains(&b'%') {
        let string = string(unit, &format[..format.len() - 1]);
        return Some(put_s(site, string, unit));
    }
    if format == b"%c" && args.len() > 1 {
        let ty = unit.function.operand_type(unit.context, args[1])?;
        unit.context.types.int_bits(ty)?;
        let value = int_cast(unit, site, args[1], false)?;
        return Some(put_char(site, value));
    }
    if format == b"%s\n" && args.len() > 1 {
        let ty = unit.function.operand_type(unit.context, args[1])?;
        if matches!(unit.context.types.get(ty), Type::Pointer { .. }) {
            return Some(put_s(site, args[1], unit));
        }
    }
    None
}

/// `optimizeFPrintFString`.
fn fprint_f(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let args = &operands[..operands.len() - 1];
    let format = constant_string(unit, site.env, *args.get(1)?)?;
    let result = unit.function.instruction(site.call).result.expect("fprintf returns int");
    if !unit.function.users(result).is_empty() {
        return None;
    }
    let stream = args[0];
    let pointer = unit.context.types.ptr(0);
    // fprintf(F, "foo") --> fwrite("foo", 3, 1, F)
    if args.len() == 2 {
        if format.contains(&b'%') {
            return None;
        }
        return Some(fwrite(unit, site, args[1], format.len() as u64, stream));
    }
    if format.len() != 2 || format[0] != b'%' || args.len() < 3 {
        return None;
    }
    match format[1] {
        b'c' => {
            let ty = unit.function.operand_type(unit.context, args[2])?;
            unit.context.types.int_bits(ty)?;
            let value = int_cast(unit, site, args[2], true)?;
            Some(Rewrite::Call(NewCall {
                standard: "fputc",
                args: vec![value, stream],
                returns: site.int,
                params: vec![site.int, pointer],
            }))
        }
        b's' => {
            let ty = unit.function.operand_type(unit.context, args[2])?;
            if !matches!(unit.context.types.get(ty), Type::Pointer { .. }) {
                return None;
            }
            Some(Rewrite::Call(NewCall {
                standard: "fputs",
                args: vec![args[2], stream],
                returns: site.int,
                params: vec![pointer, pointer],
            }))
        }
        _ => None,
    }
}

/// `emitFWrite(string, length, F)`: `fwrite(string, length, 1, F)`.
fn fwrite(
    unit: &Unit,
    site: &Site,
    string: Operand,
    length: u64,
    stream: Operand,
) -> Rewrite {
    let pointer = unit.context.types.ptr(0);
    Rewrite::Call(NewCall {
        standard: "fwrite",
        args: vec![
            string,
            Operand::Constant(unit.context.int(site.size, i128::from(length))),
            Operand::Constant(unit.context.int(site.size, 1)),
            stream,
        ],
        returns: site.size,
        params: vec![pointer, site.size, site.size, pointer],
    })
}

/// `optimizeFPuts`: `fputs(s, F)` of a constant string is `fwrite(s, len, 1,
/// F)` (not when optimizing for size: `fwrite` takes more arguments).
fn fputs(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    if site.optimize_size {
        return None;
    }
    let result = unit.function.instruction(site.call).result?;
    if !unit.function.users(result).is_empty() {
        return None;
    }
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let text = constant_string(unit, site.env, operands[0])?;
    // `GetStringLength` is zero for a string that is not one (and counts the
    // NUL).
    Some(fwrite(unit, site, operands[0], text.len() as u64, operands[1]))
}

/// `optimizeFWrite`: nothing written is 0, one byte is `fputc`.
fn fwrite_call(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let operands = unit.function.instruction(site.call).operands.to_vec();
    let constant = |unit: &Unit, operand: Operand| match operand {
        Operand::Constant(id) => match unit.context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            ConstantKind::Zero | ConstantKind::Null => Some(0),
            _ => None,
        },
        _ => None,
    };
    let (size, count) = (constant(unit, operands[1])?, constant(unit, operands[2])?);
    let bytes = size.checked_mul(count)?;
    if bytes == 0 {
        return Some(Rewrite::Drop);
    }
    let result = unit.function.instruction(site.call).result?;
    if bytes != 1 || !unit.function.users(result).is_empty() {
        return None;
    }
    // fwrite(S, 1, 1, F) --> fputc(S[0], F)
    let i8 = unit.context.types.int(8);
    let load = unit
        .function
        .create_instruction(
            Opcode::Load { align: None, volatile: false },
            i8,
            vec![operands[0]],
            Flags::default(),
            None,
        );
    unit.function.insert(load, Position::Before(site.call)).expect("a placed load");
    let character = Operand::Value(unit.function.instruction(load).result.expect("a load has a result"));
    let value = int_cast(unit, site, character, true)?;
    let pointer = unit.context.types.ptr(0);
    Some(Rewrite::Call(NewCall {
        standard: "fputc",
        args: vec![value, operands[3]],
        returns: site.int,
        params: vec![site.int, pointer],
    }))
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
    let global = unit.declared.table("str", ty, initializer, UnnamedAddr::Global, None);
    let pointer = unit.context.types.ptr(0);
    Operand::Constant(unit.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(global) }))
}

/// `optimizePuts`: `puts("")` is `putchar('\n')`.
fn puts(
    unit: &mut Unit,
    site: &Site,
) -> Option<Rewrite> {
    let result = unit.function.instruction(site.call).result?;
    if !unit.function.users(result).is_empty() {
        return None;
    }
    let operands = unit.function.instruction(site.call).operands.to_vec();
    if !constant_string(unit, site.env, operands[0])?.is_empty() {
        return None;
    }
    Some(put_char(site, Operand::Constant(unit.context.int(site.int, i128::from(b'\n')))))
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
        let size_bits = unit.layout.pointer(0).index_bits;
        // One table per module: the target's library, else the pin's defaults
        // for the pointer's width.
        let info = target_library_info(&outer, size_bits);
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
        let optimize_size = unit
            .function
            .attrs
            .iter()
            .any(
                |attr| matches!(
                    attr,
                    Attribute::Flag(name) if name == "optsize" || name == "minsize"
                ),
            );
        let int = unit.context.types.int(info.int_bits());
        let size = unit.context.types.int(size_bits);
        for call in calls {
            let Some(global) = llrm_mir::memory::callee(unit.context, unit.function, call) else { continue };
            let Some(func) =
                outer.globals.get(global.0 as usize).and_then(|one| info.get_lib_func_of(&unit.context.types, one))
            else {
                continue;
            };
            let site = Site { env, call, int, size, optimize_size };
            let rewrite = match func {
                LibFunc::Printf => print_f(unit, &site),
                LibFunc::Fprintf => fprint_f(unit, &site),
                LibFunc::Puts => puts(unit, &site),
                LibFunc::Fputs => fputs(unit, &site),
                LibFunc::Fwrite => fwrite_call(unit, &site),
                LibFunc::Strcmp => compare::str_cmp(unit, &site, &outer),
                LibFunc::Strncmp => compare::str_n_cmp(unit, &site, &outer),
                LibFunc::Memcmp => compare::mem_cmp(unit, &site, true),
                LibFunc::Bcmp => compare::mem_cmp(unit, &site, false),
                LibFunc::Strchr => search::str_chr(unit, &site),
                LibFunc::Memchr => search::mem_chr(unit, &site),
                LibFunc::Strcpy => copy::str_cpy(unit, &site),
                LibFunc::Stpcpy => copy::stp_cpy(unit, &site),
                LibFunc::Strcat => copy::str_cat(unit, &site, &self.routines),
                LibFunc::Strncat => copy::str_n_cat(unit, &site, &self.routines),
                LibFunc::Strncpy => copy::str_n_cpy(unit, &site),
                LibFunc::Strpbrk => search::str_pbrk(unit, &site),
                LibFunc::Strspn => search::str_spn(unit, &site, true),
                LibFunc::Strcspn => search::str_spn(unit, &site, false),
                LibFunc::Strstr => search::str_str(unit, &site),
                LibFunc::Memcpy => memops::mem_copy(unit, &site, crate::loopidioms::How::Apart),
                LibFunc::Memmove => memops::mem_copy(unit, &site, crate::loopidioms::How::Overlapping),
                LibFunc::Mempcpy => memops::mem_p_cpy(unit, &site),
                LibFunc::Memset => memops::mem_set(unit, &site),
                LibFunc::Memccpy => memops::mem_c_cpy(unit, &site),
                _ => None,
            };
            let Some(rewrite) = rewrite else { continue };
            let result = unit.function.instruction(call).result;
            let replacement = match rewrite {
                Rewrite::Drop => {
                    let ty = unit.function.instruction(call).ty;
                    Operand::Constant(unit.context.int(ty, 0))
                }
                Rewrite::Value(value) => value,
                Rewrite::Call(new) => match emit(unit, &self.routines, new, Position::Before(call)) {
                    Some(new) => Operand::Value(unit.function.instruction(new).result.expect("a result")),
                    None => continue,
                },
            };
            if let Some(result) = result {
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

#[cfg(test)]
#[path = "simplifylibcalls_tests.rs"]
mod tests;
