//! `memcmp` of a constant size becomes loads and compares on a target that
//! states its load sizes.

use super::ExpandMemCmp;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"\n\ndeclare i32 @memcmp(ptr, ptr, i64)\n\n";

/// x86-64's `enableMemCmpExpansion` over a machine that states nothing else.
struct Long;

impl llrm_mir::target::Machine for Long {
    fn tti(&self) -> Option<std::rc::Rc<dyn llrm_mir::tti::Tti>> {
        Some(std::rc::Rc::new(llrm_x86::tti::X86::of_cpu("x86-64").expect("x86-64")))
    }

    fn library(&self) -> Option<std::rc::Rc<dyn llrm_mir::libfunc::Library>> {
        Some(std::rc::Rc::new(Lp64))
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        llrm_mir::target::Neutral.spaces()
    }

    fn foreign_span(
        &self,
        a: (i64, i64),
        b: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        llrm_mir::target::Neutral.foreign_span(a, b, width)
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

/// `int` 32 bits, `size_t` 64.
struct Lp64;

impl llrm_mir::libfunc::Library for Lp64 {
    fn int_bits(&self) -> u32 {
        32
    }

    fn size_t_bits(&self) -> u32 {
        64
    }

    fn availability(
        &self,
        func: llrm_mir::libfunc::LibFunc,
    ) -> llrm_mir::libfunc::Availability {
        llrm_mir::libfunc::Generic::for_pointer(64).availability(func)
    }
}

fn run(body: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{body}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(ExpandMemCmp);
    manager.run_module(&mut module, std::rc::Rc::new(Long)).unwrap();
    printed(&module)
}

/// `memcmp(a, b, 8) == 0` stayed a call; it is one `i64` load from each side
/// and one compare.
#[test]
fn test_eight_byte_equality_is_one_i64_load_pair_and_no_call() {
    let after = run("define i1 @f(ptr %a, ptr %b) {\nb0:\n  %c = call i32 @memcmp(ptr %a, ptr %b, i64 8)\n  \
         %e = icmp eq i32 %c, 0\n  ret i1 %e\n}\n");
    assert!(!after.contains("call i32 @memcmp"), "{after}");
    assert_eq!(after.matches("load i64").count(), 2, "{after}");
    assert_eq!(after.matches("icmp ne i64").count(), 1, "{after}");
}

/// A three-way compare of 3 bytes is a chain of blocks of byte-swapped loads
/// ending in `res_block`, not a call.
#[test]
fn test_three_way_compare_of_three_bytes_is_a_block_chain() {
    let after = run("define i32 @f(ptr %a, ptr %b) {\nb0:\n  %c = call i32 @memcmp(ptr %a, ptr %b, i64 3)\n  \
         ret i32 %c\n}\n");
    assert!(!after.contains("call i32 @memcmp"), "{after}");
    assert!(after.contains("res_block:") && after.contains("llvm.bswap.i16"), "{after}");
}
