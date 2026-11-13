//! The tables instruction selection reads are built once, not per call.

use llrm_arm64::dag::pipeline::Aarch64Sched;
use llrm_arm64::reg;
use llrm_codegen::backend::selectiondag::schedule::SchedInfo;
use llrm_support::alloc_count::{Counting, counted};

#[global_allocator]
static COUNTING: Counting = Counting;

/// `pin_register` built the table of every pin register (a name search over
/// the 256 ids for each, 100 K instructions) on each call; the scheduler asks
/// once per register of every unit: 1.8 M of rectwo's 11 M instructions in
/// isel, 3.5 M with the per-block copy.
#[test]
fn asking_for_a_pin_register_builds_no_table() {
    let sched = Aarch64Sched;
    assert!(sched.pin_register(reg::W1).is_some(), "the premise: it answers");
    let (_, made) = counted(|| {
        for _ in 0..50 {
            std::hint::black_box(sched.pin_register(reg::W1));
        }
    });
    assert_eq!(made, 0, "{made} allocations for 50 questions");
}

/// The action table was built for every block selected (17 in rectwo): a
/// function of many blocks builds it once, and a second compile builds none.
#[test]
fn the_action_table_is_built_once_however_many_blocks_are_selected() {
    use llrm_arm64::driver;
    use llrm_arm64::machine::Platform;

    let blocks: String = (0..12)
        .map(|n| format!("b{n}:\n  %x{n} = load volatile i32, ptr %p\n  %c{n} = icmp eq i32 %x{n}, {n}\n  br i1 %c{n}, label %b{next}, label %done\n", next = n + 1))
        .collect();
    let ir = format!(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
define i32 @f(ptr %p, i32 %a) {{\nentry:\n  br label %b0\n{blocks}b12:\n  br label %done\ndone:\n  ret i32 %a\n}}\n"
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers");
    let first = llrm_arm64::dag::pipeline::action_tables_built();
    assert_eq!(first, 1, "{first} tables for the blocks of one function");
    driver::assembly(&ir, Platform::Linux, true).expect("lowers again");
    assert_eq!(llrm_arm64::dag::pipeline::action_tables_built(), first, "a second compile built a table");
}

/// Making the selection hooks decoded the table's complex patterns again for
/// every block (a vector and a name match per pattern: 0.33 M of rectwo's isel
/// in 17 blocks).
#[test]
fn making_the_selection_hooks_decodes_no_table() {
    use llrm_arm64::dag::globals::Globals;
    use llrm_arm64::dag::hooks::Aarch64Hooks;

    let globals = Globals::default();
    drop(Aarch64Hooks::new(globals.clone(), false, false));
    let (_, made) = counted(|| {
        for _ in 0..20 {
            drop(std::hint::black_box(Aarch64Hooks::new(globals.clone(), false, false)));
        }
    });
    assert_eq!(made, 0, "{made} allocations for 20 hooks");
}
