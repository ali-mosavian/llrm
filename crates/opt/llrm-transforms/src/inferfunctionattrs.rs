//! A library function the module only declares (`sin`, `strlen`, `memcmp`)
//! gets the attributes the C library promises of it, as LLVM's
//! `InferFunctionAttrsPass`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc`
//! [`InferFunctionAttrs.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/InferFunctionAttrs.cpp)
//! and `inferNonMandatoryLibFuncAttrs` of `BuildLibCalls.cpp`. A declaration
//! whose name and prototype are a [`LibFunc`] the target's runtime has
//! ([`TargetLibraryInfo`]) takes the attributes of its case in
//! `libfunc/attrs.rs` (generated from the pin by `tools/gen/gen_libattrs.py`):
//! `nounwind`, `willreturn`, `nofree`, `nosync`, `nocallback`, what it does to
//! memory (`memory(errnomem: write)` for `sin`: it sets `errno` and touches
//! nothing else) and what each pointer parameter is. A call of `sin(x)` twice
//! is then one call to the optimizer's CSE.
//!
//! # Example
//! ```text
//!   declare double @sin(double)
//!   ==>
//!   declare double @sin(double) nofree nosync nounwind willreturn nocallback memory(errnomem: write)
//! ```
//!
//! DEVIATE (anything MIR or Rust lacks):
//! 1. `allocsize` and `allockind` keep their name only (the attribute list
//!    parser drops their arguments), `allocalign` and `allocptr` are not kept,
//!    and `alloc-family` is a string attribute: no pass here reads them.
//! 2. `-fno-builtin` and `optnone` are the function's own flags: a declaration
//!    that has `optnone` is left alone, as the pin's.
//! 3. `getRtLibUseGOT` (`nonlazybind`) is no module flag here.
//! 4. `isLibFreeFunction` is whether the case set `allockind(free)`.
//!
//! # Runs
//! Once, at the start of the module pipeline
//! (`PassBuilderPipelines.cpp#L1169`).

use llrm_mir::context::GlobalId;
use llrm_mir::facts::Fact;
use llrm_mir::libfunc::attrs::infer;
use llrm_mir::libfunc::infer::{AllocKind, Setters};
use llrm_mir::libfunc::{Library, TargetLibraryInfo};
use llrm_mir::module::{Function, Module};
use llrm_mir::opcode::Attribute;
use llrm_mir::passes::{ModuleAnalyses, ModulePass};
use llrm_mir::types::Type;

pub struct InferFunctionAttrs;

impl ModulePass for InferFunctionAttrs {
    fn name(&self) -> &'static str {
        "inferattrs"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        module: &mut Module,
        analyses: &mut ModuleAnalyses,
    ) -> Vec<GlobalId> {
        let library: std::rc::Rc<dyn Library> = analyses
            .program()
            .target
            .library()
            .unwrap_or_else(
                || {
                    let bits = if module
                        .datalayout
                        .as_deref()
                        .is_some_and(|text| text.split('-').any(|one| one.starts_with("p:16")))
                    {
                        16
                    } else {
                        32
                    };
                    std::rc::Rc::new(llrm_mir::libfunc::Generic { int_bits: bits, size_t_bits: bits })
                },
            );
        let tli = TargetLibraryInfo::new(&*library);
        let mut changed = Vec::new();
        for at in 0..module.globals.len() {
            let global = &module.globals[at];
            let Some(function) = global.function() else { continue };
            if !function.is_declaration() || function.attrs.contains(&Attribute::Flag("optnone".to_owned())) {
                continue;
            }
            let Some(name) = global.name.as_deref().filter(|name| !name.starts_with("llvm.")) else { continue };
            let Some(lib) = tli.get_lib_func(&module.context.types, library.standard(name), function.ty) else {
                continue;
            };
            if !tli.has(lib) {
                continue;
            }
            let (returns_void, parameters) = match module.context.types.get(function.ty) {
                Type::Function { returns, parameters, .. } => {
                    (module.context.types.is_void(*returns), parameters.len())
                }
                _ => continue,
            };
            let llrm_mir::module::GlobalKind::Function(function) = &mut module.globals[at].kind else { continue };
            let mut declaration = Declaration::new(function, returns_void, parameters);
            if infer(lib, &mut declaration) {
                declaration.does_not_free_memory_unless_freeing();
            }
            if declaration.changed {
                changed.push(GlobalId(at as u32));
            }
        }
        changed
    }
}

/// How much of one location a function may touch: bit 0 reads, bit 1 writes.
const REF: u8 = 1;
const MOD: u8 = 2;
const MOD_REF: u8 = 3;

/// `MemoryEffects`: argument memory, inaccessible memory, `errno`, anything
/// else.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Memory([u8; 4]);

const ARG: usize = 0;
const INACCESSIBLE: usize = 1;
const ERRNO: usize = 2;

impl Memory {
    const NONE: Self = Self([0; 4]);

    fn of(attrs: &[Attribute]) -> Self {
        let mut stated = Self([MOD_REF; 4]);
        for attr in attrs {
            match attr {
                _ if Fact::of_attribute(attr) == Some(Fact::ReadNone) => stated = stated.and(Self::NONE),
                _ if Fact::of_attribute(attr) == Some(Fact::ReadOnly) => stated = stated.and(Self([REF; 4])),
                _ if Fact::of_attribute(attr) == Some(Fact::WriteOnly) => stated = stated.and(Self([MOD; 4])),
                Attribute::Memory(locations) => {
                    let access = |one: &str| match one {
                        "read" => REF,
                        "write" => MOD,
                        "readwrite" => MOD_REF,
                        _ => 0,
                    };
                    let other = locations.iter().find(|(at, _)| at.is_none()).map_or(0, |(_, one)| access(one));
                    let mut each = [other; 4];
                    for (at, one) in locations {
                        match at.as_deref() {
                            Some("argmem") => each[ARG] = access(one),
                            Some("inaccessiblemem") => each[INACCESSIBLE] = access(one),
                            Some("errnomem") => each[ERRNO] = access(one),
                            _ => {}
                        }
                    }
                    stated = stated.and(Self(each));
                }
                _ => {}
            }
        }
        stated
    }

    fn and(
        self,
        other: Self,
    ) -> Self {
        Self(std::array::from_fn(|at| self.0[at] & other.0[at]))
    }

    /// `memory(...)` as the pin prints it: the access to everything else
    /// first, then each named location that differs.
    fn attribute(self) -> Attribute {
        let word = |bits: u8| match bits {
            0 => "none",
            REF => "read",
            MOD => "write",
            _ => "readwrite",
        };
        let other = self.0[3];
        let mut locations = Vec::new();
        if other != 0 || self.0[..3].iter().all(|&one| one == 0) {
            locations.push((None, word(other).to_owned()));
        }
        for (at, name) in [(ARG, "argmem"), (INACCESSIBLE, "inaccessiblemem"), (ERRNO, "errnomem")] {
            if self.0[at] != other {
                locations.push((Some(name.to_owned()), word(self.0[at]).to_owned()));
            }
        }
        Attribute::Memory(locations)
    }
}

/// A library declaration the setters change, and whether any did.
struct Declaration<'f> {
    function: &'f mut Function,
    returns_void: bool,
    parameters: usize,
    changed: bool,
    alloc_kind: u64,
}

impl<'f> Declaration<'f> {
    fn new(
        function: &'f mut Function,
        returns_void: bool,
        parameters: usize,
    ) -> Self {
        let wanted = parameters.max(function.parameter_attrs.len());
        function.parameter_attrs.resize_with(wanted, Vec::new);
        Self { function, returns_void, parameters, changed: false, alloc_kind: 0 }
    }

    fn has(
        &self,
        flag: &str,
    ) -> bool {
        self.function.attrs.iter().any(|one| matches!(one, Attribute::Flag(name) if name == flag))
    }

    fn add(
        &mut self,
        flag: &str,
    ) {
        if !self.has(flag) {
            self.function.attrs.push(Attribute::Flag(flag.to_owned()));
            self.changed = true;
        }
    }

    fn add_param(
        &mut self,
        arg: usize,
        flag: &str,
    ) {
        let Some(attrs) = self.function.parameter_attrs.get_mut(arg) else { return };
        if !attrs.iter().any(|one| matches!(one, Attribute::Flag(name) if name == flag)) {
            attrs.push(Attribute::Flag(flag.to_owned()));
            self.changed = true;
        }
    }

    fn add_ret(
        &mut self,
        flag: &str,
    ) {
        if !self.function.return_attrs.iter().any(|one| matches!(one, Attribute::Flag(name) if name == flag)) {
            self.function.return_attrs.push(Attribute::Flag(flag.to_owned()));
            self.changed = true;
        }
    }

    /// `setMemoryEffects`: what the function may do, and what the case allows.
    fn memory(
        &mut self,
        allowed: Memory,
    ) {
        let before = Memory::of(&self.function.attrs);
        let after = before.and(allowed);
        if before == after {
            return;
        }
        self.function
            .attrs
            .retain(
                |one| !matches!(one, Attribute::Memory(_))
                    && !matches!(
                        Fact::of_attribute(one),
                        Some(Fact::ReadNone | Fact::ReadOnly | Fact::WriteOnly)
                    ),
            );
        self.function.attrs.push(after.attribute());
        self.changed = true;
    }

    /// The step after the switch: a function that does not free and is no
    /// `realloc` states `nofree`.
    fn does_not_free_memory_unless_freeing(&mut self) {
        let freeing = self.alloc_kind & (AllocKind::FREE | AllocKind::REALLOC) != 0;
        if !freeing {
            self.does_not_free_memory();
        }
    }
}

impl Setters for Declaration<'_> {
    fn does_not_access_memory(&mut self) {
        self.memory(Memory::NONE);
    }

    fn is_cold(&mut self) {
        self.add("cold");
    }

    fn no_return(&mut self) {
        self.add("noreturn");
    }

    fn only_reads_memory(&mut self) {
        self.memory(Memory([REF; 4]));
    }

    fn only_reads_memory_arg(
        &mut self,
        arg: usize,
    ) {
        self.add_param(arg, "readonly");
    }

    fn only_writes_memory(&mut self) {
        self.memory(Memory([MOD; 4]));
    }

    fn only_writes_memory_arg(
        &mut self,
        arg: usize,
    ) {
        self.add_param(arg, "writeonly");
    }

    fn only_accesses_arg_memory(&mut self) {
        self.memory(Memory([MOD_REF, 0, 0, 0]));
    }

    fn only_accesses_inaccessible_mem_or_arg_mem(&mut self) {
        self.memory(Memory([MOD_REF, MOD_REF, 0, 0]));
    }

    fn only_accesses_inaccessible_mem_or_errno_mem(&mut self) {
        self.memory(Memory([0, MOD_REF, MOD, 0]));
    }

    fn only_accesses_inaccessible_mem_or_arg_mem_or_errno_mem(&mut self) {
        self.memory(Memory([MOD_REF, MOD_REF, MOD, 0]));
    }

    fn only_writes_errno_memory(&mut self) {
        self.memory(Memory([0, 0, MOD, 0]));
    }

    fn only_writes_arg_mem_or_errno_mem(&mut self) {
        self.memory(Memory([MOD, 0, MOD, 0]));
    }

    fn does_not_throw(&mut self) {
        self.add("nounwind");
    }

    fn does_not_callback(&mut self) {
        self.add("nocallback");
    }

    fn ret_does_not_alias(&mut self) {
        self.add_ret("noalias");
    }

    fn does_not_capture(
        &mut self,
        arg: usize,
    ) {
        self.add_param(arg, "nocapture");
    }

    fn does_not_alias(
        &mut self,
        arg: usize,
    ) {
        self.add_param(arg, "noalias");
    }

    fn does_not_sync(&mut self) {
        self.add("nosync");
    }

    fn ret_no_undef(&mut self) {
        if !self.returns_void {
            self.add_ret("noundef");
        }
    }

    fn args_no_undef(&mut self) {
        for arg in 0..self.parameters {
            self.add_param(arg, "noundef");
        }
    }

    fn arg_no_undef(
        &mut self,
        arg: usize,
    ) {
        self.add_param(arg, "noundef");
    }

    fn ret_and_args_no_undef(&mut self) {
        self.ret_no_undef();
        self.args_no_undef();
    }

    fn returned_arg(
        &mut self,
        arg: usize,
    ) {
        self.add_param(arg, "returned");
    }

    fn does_not_free_memory(&mut self) {
        self.add("nofree");
    }

    fn will_return(&mut self) {
        self.add("willreturn");
    }

    fn aligned_alloc_param(
        &mut self,
        _: usize,
    ) {
        // DEVIATE 1.
    }

    fn allocated_pointer_param(
        &mut self,
        _: usize,
    ) {
        // DEVIATE 1.
    }

    fn alloc_size(
        &mut self,
        _: usize,
        _: Option<usize>,
    ) {
        self.add("allocsize");
    }

    fn alloc_family(
        &mut self,
        family: &str,
    ) {
        let has = self.function.attrs.iter().any(|one| matches!(one, Attribute::Str(key, _) if key == "alloc-family"));
        if !has {
            self.function.attrs.push(Attribute::Str("alloc-family".to_owned(), Some(family.to_owned())));
            self.changed = true;
        }
    }

    fn alloc_kind(
        &mut self,
        kind: u64,
    ) {
        self.alloc_kind |= kind;
        self.add("allockind");
    }
}

#[cfg(test)]
#[path = "inferfunctionattrs_tests.rs"]
mod tests;
