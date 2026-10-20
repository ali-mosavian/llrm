//! Compile LLVM IR to an object.

#[global_allocator]
static ALLOCATOR: llrm_alloc::Slab = llrm_alloc::Slab;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(llrm_core::support::debug::run_main(|| llrm_driver::llc::main(&argv)));
}
