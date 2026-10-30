//! The library functions the optimizer recognizes by name and prototype, and
//! which of them a target's runtime has: LLVM's `TargetLibraryInfo`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `TargetLibraryInfo.h`, `.td` and
//! `.cpp`. The functions and their prototypes are `TargetLibraryInfo.td`'s,
//! turned into `table.rs` by `tools/gen/gen_libfunc.py` (the pin's TableGen
//! emitter makes the same enum and tables); `get_lib_func` finds one by its
//! name (`getLibFunc(StringRef)`) and checks a declaration's type against its
//! prototype (`isValidProtoForLibFunc`).
//!
//! Which functions a runtime has is not a property of this analysis. The
//! pin's `initialize` asks the target triple; here a target states it in a
//! [`Library`] (the sizes of `int` and `size_t`, what is missing, what a
//! function is called instead), and [`TargetLibraryInfo::new`] reads that. The
//! `Machine` trait of the target carries it (`Machine::library`); a target
//! that states none has the pin's defaults for an unknown OS ([`Generic`]).
//!
//! DEVIATE (anything MIR or Rust lacks):
//! 1. No `Triple`: the `initialize` branches on the OS and architecture are the
//!    target's [`Library`], one description per target, not one function that
//!    tests them all. [`BASE_UNAVAILABLE`] is the part of `initializeLibCalls`
//!    that tests no triple (the unlocked IO functions, `memset_pattern*`, the
//!    `sinpi` family, `bcmp`, `iprintf`, `__small_printf` and their kin) for an
//!    OS that is none of the ones that have them.
//! 2. No vector library (`addVectorizableFunctions`, `VecLib`): nothing in MIR
//!    vectorizes a call.
//! 3. A function that returns a struct or vector (`sincospi_stret`, the
//!    `size_returning_new` family) and `cabs` take the array or struct forms
//!    the pin tests: MIR has the types, no LibFunc of those is recognized yet
//!    (`SpecialPrototype`), so `get_lib_func` gives none for them, as the pin
//!    does for a prototype it does not take.
//!
//! # Example
//! `declare ptr @memchr(ptr, i32, i32)` on a 32-bit `int` is
//! [`LibFunc::Memchr`]; the same name with a `double` argument is none.
//!
//! # Used by
//! Nothing yet: the passes that read it (aggressive-instcombine's folds,
//! InstCombine's `LibCallSimplifier`, the alias analyses) take it from here.

pub mod attrs;
pub mod infer;
pub mod table;

pub use table::{ALL, LibFunc, NAMES, NUM_LIB_FUNCS, SIGNATURES};

use crate::module::GlobalValue;
use crate::types::{FloatKind, Type, TypeId, Types};

/// `TargetLibraryInfoImpl::FuncArgTypeID`: what the table says of a return or
/// parameter type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FuncArgType {
    Void,
    /// 8 bits on all targets.
    Bool,
    Int16,
    Int32,
    Int,
    /// `int` or bigger.
    IntPlus,
    /// Either 32 or 64 bits.
    Long,
    /// Any integer type.
    IntX,
    Int64,
    /// 64 bits on all targets.
    LLong,
    SizeT,
    SSizeT,
    /// IEEE float.
    Flt,
    /// IEEE double.
    Dbl,
    /// Any floating type.
    LDbl,
    /// Any floating type.
    Floating,
    /// Any pointer type.
    Ptr,
    /// Any struct type.
    Struct,
    /// The ellipsis.
    Ellip,
    /// Same argument type as the previous one.
    Same,
}

impl LibFunc {
    /// The standard name of the function (`getName` of a LibFunc with no
    /// override).
    pub fn standard_name(self) -> &'static str {
        NAMES[self as usize]
    }

    /// The prototype the table states: the return type, then the arguments.
    pub fn signature(self) -> &'static [FuncArgType] {
        SIGNATURES[self as usize]
    }
}

/// What a target's runtime does with a library function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    /// Called by its standard name.
    Standard,
    /// Not in the runtime (`setUnavailable`).
    Unavailable,
    /// Called by another name (`setAvailableWithName`).
    Named(&'static str),
}

/// The part of `initializeLibCalls` that tests no triple, for an OS that has
/// none of the functions it removes (DEVIATE 1).
pub const BASE_UNAVAILABLE: &[LibFunc] = &[
    LibFunc::GetcUnlocked,
    LibFunc::GetcharUnlocked,
    LibFunc::PutcUnlocked,
    LibFunc::PutcharUnlocked,
    LibFunc::FputcUnlocked,
    LibFunc::FgetcUnlocked,
    LibFunc::FreadUnlocked,
    LibFunc::FwriteUnlocked,
    LibFunc::FputsUnlocked,
    LibFunc::FgetsUnlocked,
    LibFunc::MemsetPattern4,
    LibFunc::MemsetPattern8,
    LibFunc::MemsetPattern16,
    LibFunc::Sinpi,
    LibFunc::Sinpif,
    LibFunc::Cospi,
    LibFunc::Cospif,
    LibFunc::SincospiStret,
    LibFunc::SincospifStret,
    LibFunc::Bcmp,
    LibFunc::Iprintf,
    LibFunc::Siprintf,
    LibFunc::Fiprintf,
    LibFunc::SmallPrintf,
    LibFunc::SmallSprintf,
    LibFunc::SmallFprintf,
];

/// A target's library, as its description states it: what `initialize` reads
/// of the triple, in the target's own terms.
pub trait Library {
    /// `setIntSize`: the width of `int`.
    fn int_bits(&self) -> u32;

    /// `getSizeTSize`: the width of `size_t`.
    fn size_t_bits(&self) -> u32;

    /// What the runtime does with `func`.
    fn availability(
        &self,
        func: LibFunc,
    ) -> Availability;

    /// The standard name of the function a link `symbol` is: the pin sees the
    /// IR's name, here a front end has decorated it (`_sin@3`) before the
    /// optimizer runs. The symbol itself where the target does not.
    fn standard<'a>(
        &self,
        symbol: &'a str,
    ) -> &'a str {
        symbol
    }
}

/// The pin's defaults for a target that is none of the OSes it knows
/// (`initializeBase`, and `initializeLibCalls` up to the first OS test): a
/// 32-bit `int` unless the pointer is 16 bits wide, and [`BASE_UNAVAILABLE`]
/// missing.
#[derive(Clone, Copy, Debug)]
pub struct Generic {
    pub int_bits: u32,
    pub size_t_bits: u32,
}

impl Library for Generic {
    fn int_bits(&self) -> u32 {
        self.int_bits
    }

    fn size_t_bits(&self) -> u32 {
        self.size_t_bits
    }

    fn availability(
        &self,
        func: LibFunc,
    ) -> Availability {
        if BASE_UNAVAILABLE.contains(&func) { Availability::Unavailable } else { Availability::Standard }
    }
}

/// The C library of a target whose linker symbols are C's decorated: a leading
/// underscore, and `@N` after the name for a callee-pops convention. What
/// `Generic` states of the functions, under those names.
#[derive(Clone, Copy, Debug)]
pub struct Cdecl {
    pub int_bits: u32,
    pub size_t_bits: u32,
}

impl Library for Cdecl {
    fn int_bits(&self) -> u32 {
        self.int_bits
    }

    fn size_t_bits(&self) -> u32 {
        self.size_t_bits
    }

    fn availability(
        &self,
        func: LibFunc,
    ) -> Availability {
        Generic { int_bits: self.int_bits, size_t_bits: self.size_t_bits }.availability(func)
    }

    fn standard<'a>(
        &self,
        symbol: &'a str,
    ) -> &'a str {
        let name = symbol.strip_prefix('_').unwrap_or(symbol);
        match name.rsplit_once('@') {
            Some((base, bytes)) if !bytes.is_empty() && bytes.bytes().all(|one| one.is_ascii_digit()) => base,
            _ => name,
        }
    }
}

/// `TargetLibraryInfo` (`TargetLibraryInfoImpl`): the library functions of one
/// target.
#[derive(Clone, Debug)]
pub struct TargetLibraryInfo {
    int_bits: u32,
    size_t_bits: u32,
    availability: Vec<Availability>,
}

impl TargetLibraryInfo {
    /// `TargetLibraryInfoImpl(Triple)`, the triple's part stated by `library`.
    pub fn new(library: &dyn Library) -> Self {
        Self {
            int_bits: library.int_bits(),
            size_t_bits: library.size_t_bits(),
            availability: ALL.iter().map(|&func| library.availability(func)).collect(),
        }
    }

    /// `setIntSize`
    /// ([TargetLibraryInfo.h](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/TargetLibraryInfo.h)).
    pub fn int_bits(&self) -> u32 {
        self.int_bits
    }

    /// `has`: whether the runtime has `func`.
    pub fn has(
        &self,
        func: LibFunc,
    ) -> bool {
        self.availability[func as usize] != Availability::Unavailable
    }

    /// `getName`: the name `func` is called by, where the runtime has it.
    pub fn name(
        &self,
        func: LibFunc,
    ) -> Option<&'static str> {
        match self.availability[func as usize] {
            Availability::Standard => Some(func.standard_name()),
            Availability::Named(name) => Some(name),
            Availability::Unavailable => None,
        }
    }

    /// `disableAllFunctions`.
    pub fn disable_all_functions(&mut self) {
        self.availability.fill(Availability::Unavailable);
    }

    /// `getLibFunc(StringRef)`: the function called `name` in the standard
    /// table
    /// ([TargetLibraryInfo.cpp:988](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/TargetLibraryInfo.cpp#L988)).
    pub fn get_lib_func_by_name(name: &str) -> Option<LibFunc> {
        // Filter out empty names and names containing null bytes, those can't
        // be in our table; the `\01` prefix that mangles an `__asm`
        // declaration is dropped (`dropLLVMManglingEscape`).
        let name = name.strip_prefix('\u{1}').unwrap_or(name);
        if name.is_empty() || name.contains('\0') {
            return None;
        }
        INDEX.with(|index| index.get(name).copied())
    }

    /// `getLibFunc(const Function &)`: the function a declaration `name` of
    /// type `function_type` is, if its name and prototype are a library
    /// function's
    /// ([TargetLibraryInfo.cpp:1174](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/TargetLibraryInfo.cpp#L1174)).
    pub fn get_lib_func(
        &self,
        types: &Types,
        name: &str,
        function_type: TypeId,
    ) -> Option<LibFunc> {
        let func = Self::get_lib_func_by_name(name)?;
        self.is_valid_proto_for_lib_func(types, function_type, func).then_some(func)
    }

    /// `getLibFunc` of a global: none for an intrinsic (`llvm.`) and for a
    /// global that is not a function.
    pub fn get_lib_func_of(
        &self,
        types: &Types,
        global: &GlobalValue,
    ) -> Option<LibFunc> {
        let name = global.name.as_deref()?;
        if name.starts_with("llvm.") {
            return None;
        }
        self.get_lib_func(types, name, global.function()?.ty)
    }

    /// `isValidProtoForLibFunc`: whether `function_type` is `func`'s prototype
    /// ([TargetLibraryInfo.cpp:1093](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/TargetLibraryInfo.cpp#L1093)).
    pub fn is_valid_proto_for_lib_func(
        &self,
        types: &Types,
        function_type: TypeId,
        func: LibFunc,
    ) -> bool {
        let Type::Function { returns, parameters, variadic } = types.get(function_type) else { return false };
        // DEVIATE 3: the special prototypes are none.
        if special_prototype(func) {
            return false;
        }
        let num_params = parameters.len();
        let mut at = 0;
        // Iterate over the type ids in the function prototype, matching each
        // against the function's type, starting with its return type. Return
        // true if both match in number and kind, including the ellipsis.
        let mut ty: Option<TypeId> = Some(*returns);
        let mut last_ty = ty;
        let proto = func.signature();
        while at < proto.len() {
            let id = proto[at];
            if id == FuncArgType::Ellip {
                // The ellipsis ends the prototype list but is not a part of the
                // function type's argument list.
                return *variadic;
            }
            if id == FuncArgType::Same {
                assert!(at != 0, "Type ID 'Same' must not be first!");
                if ty != last_ty {
                    return false;
                }
            } else {
                let Some(one) = ty else { return false };
                if !self.match_type(types, id, one) {
                    return false;
                }
                last_ty = ty;
            }
            if at == num_params {
                // There's at least one and at most two more type ids than
                // there are arguments in the function type's argument list.
                ty = None;
                at += 1;
                continue;
            }
            ty = Some(parameters[at]);
            at += 1;
        }
        // Return success only if all entries on both lists have been processed
        // and the function is not a variadic one.
        at == num_params + 1 && !*variadic
    }

    /// `matchType`: whether `ty` is what `id` says.
    fn match_type(
        &self,
        types: &Types,
        id: FuncArgType,
        ty: TypeId,
    ) -> bool {
        use FuncArgType::*;
        let ty = types.get(ty);
        let int = |bits: u32| matches!(ty, Type::Int(width) if *width == bits);
        let int_at_least = |bits: u32| matches!(ty, Type::Int(width) if *width >= bits);
        match id {
            Void => matches!(ty, Type::Void),
            Bool => int(8),
            Int16 => int(16),
            Int32 => int(32),
            Int => int(self.int_bits),
            IntPlus => int_at_least(self.int_bits),
            IntX => matches!(ty, Type::Int(_)),
            // TODO in the pin: figure out and use long size.
            Long => int_at_least(self.int_bits),
            Int64 | LLong => int(64),
            SizeT | SSizeT => int(self.size_t_bits),
            Flt => matches!(ty, Type::Float(FloatKind::Float)),
            Dbl => matches!(ty, Type::Float(FloatKind::Double)),
            // TODO in the pin: tighten this up.
            LDbl | Floating => matches!(ty, Type::Float(_)),
            Ptr => matches!(ty, Type::Pointer(_)),
            Struct => matches!(ty, Type::Struct { .. } | Type::Named(_)),
            Ellip | Same => unreachable!("Invalid type"),
        }
    }
}

/// The functions `isValidProtoForLibFunc` tests apart from the table
/// (`cabs`, `sincospi_stret`, `size_returning_new`), DEVIATE 3.
fn special_prototype(func: LibFunc) -> bool {
    matches!(
        func,
        LibFunc::Cabs
            | LibFunc::Cabsf
            | LibFunc::Cabsl
            | LibFunc::SincospiStret
            | LibFunc::SincospifStret
            | LibFunc::SizeReturningNew
            | LibFunc::SizeReturningNewHotCold
            | LibFunc::SizeReturningNewAligned
            | LibFunc::SizeReturningNewAlignedHotCold
    )
}

thread_local! {
    /// `buildIndexMap`: the standard names, to the functions.
    static INDEX: crate::hash::HashMap<&'static str, LibFunc> =
        ALL.iter().map(|&func| (func.standard_name(), func)).collect();
}

#[cfg(test)]
#[path = "libfunc_tests.rs"]
mod tests;

/// A library routine a compile states the program links, as the front end's
/// profile spells it: what `TargetLibraryInfo` states of a triple, here of a
/// target and its ABI. The pass that forms a call of it declares it
/// (`getOrInsertFunction`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Routine {
    /// The standard name: `strlen`.
    pub standard: &'static str,
    /// The symbol it is called by, decorated as the ABI does.
    pub symbol: String,
    /// The MIR calling convention number of the ABI's C convention.
    pub calling_convention: u32,
}
