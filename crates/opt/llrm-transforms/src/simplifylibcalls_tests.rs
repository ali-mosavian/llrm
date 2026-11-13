//! `SimplifyLibCalls` reads `int` and `size_t` from the target, not from a
//! literal.

use llrm_mir::libfunc::Routine;

use super::SimplifyLibCalls;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16-n8:16:32\"\n\n";

fn run(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let routines = vec![Routine { standard: "puts", symbol: "puts".into(), calling_convention: 0 }];
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(SimplifyLibCalls::with_routines(routines));
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// At m16 `int` is 16 bits. A literal 32 made the library table reject the
/// program's own `i16 @puts` and `i16 @printf`, so `printf("hi\n")` stayed a
/// `printf` call (and a formed `puts` returned `i32`).
#[test]
fn test_printf_of_a_plain_string_is_puts_when_int_is_sixteen_bits() {
    let after = run("@s = private constant [4 x i8] c\"hi\\0A\\00\"\n\
         declare i16 @printf(ptr, ...)\n\
         define void @f() {\nb0:\n  %r = call i16 (ptr, ...) @printf(ptr @s)\n  ret void\n}\n");
    assert!(after.contains("@puts"), "{after}");
    assert!(!after.contains("call i16 (ptr, ...) @printf"), "{after}");
}

/// A library that counts how often the table asks it about a function.
struct Counting(std::rc::Rc<std::cell::Cell<usize>>);

impl llrm_mir::libfunc::Library for Counting {
    fn int_bits(&self) -> u32 {
        16
    }

    fn size_t_bits(&self) -> u32 {
        16
    }

    fn availability(
        &self,
        func: llrm_mir::libfunc::LibFunc,
    ) -> llrm_mir::libfunc::Availability {
        self.0.set(self.0.get() + 1);
        llrm_mir::libfunc::Generic::for_pointer(16).availability(func)
    }
}

struct Counted(std::rc::Rc<std::cell::Cell<usize>>);

impl llrm_mir::target::Machine for Counted {
    fn library(&self) -> Option<std::rc::Rc<dyn llrm_mir::libfunc::Library>> {
        Some(std::rc::Rc::new(Counting(self.0.clone())))
    }

    fn foreign_span(
        &self,
        a: (i64, i64),
        b: (i64, i64),
        c: i64,
    ) -> Option<(i64, i64)> {
        llrm_mir::target::Neutral.foreign_span(a, b, c)
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        llrm_mir::target::Neutral.spaces()
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        llrm_mir::target::Neutral.costs()
    }

    fn registers(&self) -> i64 {
        0
    }

    fn call_registers(&self) -> i64 {
        0
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        llrm_mir::target::Neutral.address_forms()
    }
}

fn table_reads(functions: usize) -> usize {
    let mut text = String::from("declare i16 @printf(ptr, ...)\n");
    for at in 0..functions {
        text += &format!("define void @f{at}() {{\nb0:\n  ret void\n}}\n");
    }
    let mut module = parsed(&format!("{HEAD}{text}"));
    let count = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(SimplifyLibCalls::with_routines(Vec::new()));
    manager.run_module(&mut module, std::rc::Rc::new(Counted(count.clone()))).unwrap();
    count.get()
}

/// The table was built for every function: 50 functions asked the library
/// about every routine 50 times, the compile time of a many-function module.
#[test]
fn test_the_library_table_is_built_once_per_module() {
    let one = table_reads(1);
    assert!(one > 0);
    assert_eq!(table_reads(50), one);
}

/// `memcpy`, `memmove`, `memset` and `mempcpy` calls stayed library calls
/// (the arms were missing: `optimizeMemCpy` and the others), so the DAG never
/// saw an intrinsic to expand inline and `mempcpy`/`memccpy` never did.
#[test]
fn test_the_memory_functions_become_their_intrinsics() {
    let after = run("declare ptr @memcpy(ptr, ptr, i16)\n\
         declare ptr @memmove(ptr, ptr, i16)\n\
         declare ptr @memset(ptr, i16, i16)\n\
         declare ptr @mempcpy(ptr, ptr, i16)\n\
         define ptr @f(ptr %d, ptr %s, i16 %n, i16 %c) {\nb0:\n\
         \x20 %a = call ptr @memcpy(ptr %d, ptr %s, i16 %n)\n\
         \x20 %b = call ptr @memmove(ptr %a, ptr %s, i16 %n)\n\
         \x20 %m = call ptr @memset(ptr %b, i16 %c, i16 %n)\n\
         \x20 %p = call ptr @mempcpy(ptr %m, ptr %s, i16 %n)\n\
         \x20 ret ptr %p\n}\n");
    for gone in ["call ptr @memcpy", "call ptr @memmove", "call ptr @memset", "call ptr @mempcpy"] {
        assert!(!after.contains(gone), "{gone} stayed:\n{after}");
    }
    for made in ["llvm.memcpy.p0.p0.i16", "llvm.memmove.p0.p0.i16", "llvm.memset.p0.i16"] {
        assert!(after.contains(made), "no {made}:\n{after}");
    }
    assert!(after.contains("getelementptr inbounds"), "mempcpy's result is dst + n:\n{after}");
}

/// A `memccpy` of a constant count from a constant string up to a byte in it
/// is a `memcpy` of the bytes through the stop byte, the result the byte after
/// it (`optimizeMemCCpy`); a byte not in the first `n` gives null.
#[test]
fn test_memccpy_of_a_constant_string_is_a_memcpy_through_the_stop_byte() {
    let after = run("@s = private constant [6 x i8] c\"hello\\00\"\n\
         declare ptr @memccpy(ptr, ptr, i16, i16)\n\
         define ptr @f(ptr %d) {\nb0:\n\
         \x20 %r = call ptr @memccpy(ptr %d, ptr @s, i16 108, i16 5)\n\
         \x20 ret ptr %r\n}\n");
    assert!(!after.contains("call ptr @memccpy"), "{after}");
    assert!(after.contains("llvm.memcpy.p0.p0.i16(ptr align 1 %d, ptr align 1 @s, i16 3"), "{after}");
}

fn printf_of(
    global: &str,
    argument: &str,
) -> String {
    run(&format!(
        "{global}\ndeclare i16 @printf(ptr, ...)\n\
         define void @f() {{\nb0:\n  %i = getelementptr i8, ptr @s, i16 1\n  \
         %r = call i16 (ptr, ...) @printf({argument})\n  ret void\n}}\n"
    ))
}

/// `getConstantStringInfo` follows a constant offset into the global: the
/// pin turns `printf(@s + 1)` into `puts("hi")`; a direct-global-only read
/// left the `printf` call.
#[test]
fn test_printf_of_a_constant_gep_into_a_string_is_puts() {
    let after = printf_of("@s = private constant [6 x i8] c\"xhi\\0A\\00\\00\"", "ptr %i");
    assert!(after.contains("@puts"), "{after}");
    assert!(!after.contains("call i16 (ptr, ...) @printf"), "{after}");
}

/// The same through a constant-expression GEP operand.
#[test]
fn test_printf_of_a_constant_expression_gep_is_puts() {
    let after =
        printf_of("@s = private constant [6 x i8] c\"xhi\\0A\\00\\00\"", "ptr getelementptr (i8, ptr @s, i16 1)");
    assert!(after.contains("@puts"), "{after}");
    assert!(!after.contains("call i16 (ptr, ...) @printf"), "{after}");
}

/// An array with no NUL is the whole tail for `TrimAtNul`: the pin folds
/// `printf(c"hi\n")` to `puts("hi")`; ours returned none and kept the call.
#[test]
fn test_printf_of_an_array_without_a_nul_is_puts() {
    let after = printf_of("@s = private constant [3 x i8] c\"hi\\0A\"", "ptr @s");
    assert!(after.contains("@puts"), "{after}");
    assert!(!after.contains("call i16 (ptr, ...) @printf"), "{after}");
}

/// A `zeroinitializer` string is the empty string: the pin drops
/// `printf("")` (result 0); ours kept the call.
#[test]
fn test_printf_of_a_zeroinitializer_string_is_dropped() {
    let after = printf_of("@s = private constant [4 x i8] zeroinitializer", "ptr @s");
    assert!(!after.contains("call i16 (ptr, ...) @printf"), "{after}");
}
