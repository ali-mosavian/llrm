//! The words `attrs.rs` is written in: what BuildLibCalls' `setX(F, ...)`
//! helpers do to a declaration, as a trait the pass implements over a
//! function.

/// `AllocFnKind`: what an allocation function does.
pub struct AllocKind;

impl AllocKind {
    pub const ALLOC: u64 = 1;
    pub const REALLOC: u64 = 2;
    pub const FREE: u64 = 4;
    pub const UNINITIALIZED: u64 = 8;
    pub const ZEROED: u64 = 16;
    pub const ALIGNED: u64 = 32;
}

/// BuildLibCalls' `setX(Function &F, ...)`. Each adds what the function does
/// not state already and leaves what it does; an argument number counts from 0.
pub trait Setters {
    fn does_not_access_memory(&mut self);
    fn is_cold(&mut self);
    fn no_return(&mut self);
    fn only_reads_memory(&mut self);
    fn only_reads_memory_arg(
        &mut self,
        arg: usize,
    );
    fn only_writes_memory(&mut self);
    fn only_writes_memory_arg(
        &mut self,
        arg: usize,
    );
    fn only_accesses_arg_memory(&mut self);
    fn only_accesses_inaccessible_mem_or_arg_mem(&mut self);
    fn only_accesses_inaccessible_mem_or_errno_mem(&mut self);
    fn only_accesses_inaccessible_mem_or_arg_mem_or_errno_mem(&mut self);
    fn only_writes_errno_memory(&mut self);
    fn only_writes_arg_mem_or_errno_mem(&mut self);
    fn does_not_throw(&mut self);
    fn does_not_callback(&mut self);
    fn ret_does_not_alias(&mut self);
    fn does_not_capture(
        &mut self,
        arg: usize,
    );
    fn does_not_alias(
        &mut self,
        arg: usize,
    );
    fn does_not_sync(&mut self);
    fn ret_no_undef(&mut self);
    fn args_no_undef(&mut self);
    fn arg_no_undef(
        &mut self,
        arg: usize,
    );
    fn ret_and_args_no_undef(&mut self);
    fn returned_arg(
        &mut self,
        arg: usize,
    );
    fn does_not_free_memory(&mut self);
    fn will_return(&mut self);
    fn aligned_alloc_param(
        &mut self,
        arg: usize,
    );
    fn allocated_pointer_param(
        &mut self,
        arg: usize,
    );
    fn alloc_size(
        &mut self,
        element: usize,
        count: Option<usize>,
    );
    fn alloc_family(
        &mut self,
        family: &str,
    );
    fn alloc_kind(
        &mut self,
        kind: u64,
    );
}
