use crate::parity::Phase;
use crate::parity::{self, Entry, SlotKind};
use crate::pipeline::{self, Applied};

fn names(
    applied: &Applied,
    phase: Phase,
) -> Vec<&'static str> {
    pipeline::llvm_order(applied, phase).iter().map(|one| one.name()).collect()
}

/// A slot marked Translated that the parity pipeline does not run passed
/// unnoticed: the registry said joined and the stand-in kept running.
#[test]
fn test_a_translated_slot_that_is_not_run_under_the_parity_switch_fails() {
    let on = Applied { options: pipeline::Options { llvm_order: true, ..Default::default() }, ..Applied::default() };
    let built = names(&on, Phase::Simplify);
    let mut registry = parity::REGISTRY.to_vec();
    registry.push(Entry {
        slot: "ghost@999",
        phase: Phase::Simplify,
        llvm: "ghost",
        kind: SlotKind::Translated,
        replaced_by: "",
        note: "",
    });
    let why = parity::check(&registry, Phase::Simplify, &built).expect_err("a Translated slot nobody runs");
    assert!(why.contains("ghost@999"), "{why}");
    let without: Vec<_> = built.iter().copied().filter(|one| *one != "earlycse@649").collect();
    let why = parity::check(parity::REGISTRY, Phase::Simplify, &without).expect_err("earlycse@649 left out");
    assert!(why.contains("earlycse@649"), "{why}");
}

/// A slot the parity pipeline runs must be in the registry, or it has no kind.
#[test]
fn test_a_slot_that_runs_and_is_not_registered_fails() {
    let why = parity::check(parity::REGISTRY, Phase::Early, &["earlycse@1180", "unknown@1"]).unwrap_err();
    assert!(why.contains("unknown@1"), "{why}");
}

/// The lists `llvm_order` builds at -O1, -O2 and -Os are what the registry
/// says.
#[test]
fn test_the_built_lists_satisfy_the_registry_at_every_level() {
    for options in [pipeline::Options::basic(), pipeline::Options::standard(), pipeline::Options::size()] {
        let on = Applied { options: pipeline::Options { llvm_order: true, ..options }, ..Applied::default() };
        for phase in [Phase::Early, Phase::Cleanup, Phase::Simplify] {
            let built = names(&on, phase);
            assert_eq!(parity::check_phase(phase, &built), Ok(()), "{phase:?}");
        }
    }
}

/// The MemorySSA entry carries its dependency on alias analysis, and the
/// preserved-and-updated vocabulary is a named TODO until the manager has it.
#[test]
fn test_the_registry_records_mssa_with_its_dependency_and_the_open_vocabulary() {
    let mssa = parity::ANALYSES.iter().find(|one| one.key == "mssa").expect("mssa is registered");
    assert_eq!(
        (mssa.llvm, mssa.kind, mssa.off, mssa.deps),
        ("MemorySSAAnalysis", SlotKind::Translated, true, &["aa"][..])
    );
    let report = parity::report();
    assert!(report.contains("gvn@787 (gvn) -> gvn-port") && report.contains("shadow_mssa"), "{report}");
    assert!(parity::TODO.iter().any(|one| one.contains("kept updated by the pass")));
    assert!(parity::TODO.iter().all(|one| !one.contains("CFGAnalyses")), "the CFG set exists now");
    let loops = parity::ANALYSES.iter().find(|one| one.key == "loops").expect("loops is registered");
    assert_eq!((loops.llvm, loops.kind, loops.off), ("LoopAnalysis", SlotKind::Translated, false));
}

/// A Translated machine slot that `flow::machine_with` does not run passed
/// unnoticed, and a phase that runs with no slot had no kind.
#[test]
fn test_a_translated_backend_slot_that_is_not_run_fails() {
    let mut registry = parity::REGISTRY.to_vec();
    registry.push(Entry {
        slot: "ghost@99",
        phase: Phase::Backend,
        llvm: "ghost",
        kind: SlotKind::Translated,
        replaced_by: "",
        note: "",
    });
    let built: Vec<&str> = parity::REGISTRY
        .iter()
        .filter(|one| one.phase == Phase::Backend && one.kind != SlotKind::Missing)
        .map(|one| one.slot)
        .collect();
    assert_eq!(parity::check(parity::REGISTRY, Phase::Backend, &built), Ok(()));
    let why = parity::check(&registry, Phase::Backend, &built).expect_err("a Translated slot nobody runs");
    assert!(why.contains("ghost@99"), "{why}");
    let why = parity::check_backend(&["floatalloc@8", "jumps@17", "allocate@11", "unknown@1"]).unwrap_err();
    assert!(why.contains("unknown@1"), "{why}");
}

/// A Partial slot that is not run fails like a Translated one, but it is never
/// counted as joined: `jumps@17` is TailMergeBlocks only.
#[test]
fn test_a_partial_slot_is_checked_like_a_translated_one_and_not_counted_joined() {
    let built: Vec<&str> =
        parity::REGISTRY.iter().filter(|one| one.phase == Phase::Backend).map(|one| one.slot).collect();
    let without: Vec<&str> = built.iter().copied().filter(|one| *one != "jumps@17").collect();
    let why = parity::check(parity::REGISTRY, Phase::Backend, &without).expect_err("jumps@17 left out");
    assert!(why.contains("jumps@17"), "{why}");
    assert!(parity::joined_slots(parity::REGISTRY, &built).all(|one| one != "jumps@17"));
    let count = parity::tally(parity::REGISTRY);
    // jumps@17 and allocate@11 (greedy by default under LLVM's order).
    assert_eq!(count.partial, 2);
    assert!(parity::report().contains("jumps@17") && parity::report().contains("2 partial"));
}

/// A Partial slot without its criterion and exit says what is done and not what
/// is left.
#[test]
fn test_a_partial_slot_states_its_criterion_and_exit() {
    for one in parity::REGISTRY.iter().filter(|one| one.kind == SlotKind::Partial) {
        assert!(one.note.contains("criterion:") && one.note.contains("exit:"), "{}: {:?}", one.slot, one.note);
    }
}

const TWO_FUNCTIONS: &str = "define i16 @h(ptr %p) {\nb0:\n  store i16 1, ptr %p\n  %v = load i16, ptr %p\n  ret i16 %v\n}\n\ndefine i16 @f(i16 %x) {\nb0:\n  %a = add i16 %x, 1\n  %b = add i16 %x, 1\n  %c = mul i16 %a, %b\n  ret i16 %c\n}\n\ndefine i16 @g(i16 %x) {\nb0:\n  %a = mul i16 %x, 3\n  ret i16 %a\n}\n";

fn runs_of(applied: &Applied) -> Vec<String> {
    let mut module = crate::testing::parsed(TWO_FUNCTIONS);
    let mut manager = llrm_mir::passes::PassManager::default();
    for stage in pipeline::function_stages(applied) {
        manager.add(stage);
    }
    pipeline::capturing_runs(|| {
        manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap()
    })
    .1
}

/// d_faces -O2 ran `gvn` 35 times over 12 bodies in the rounds (the fixed point
/// repeats a body until nothing changes), clang 11: once per function. Under
/// the LLVM order it is once per body.
#[test]
fn test_the_llvm_order_runs_gvn_once_per_body_and_the_rounds_do_not() {
    let order = runs_of(&Applied::default());
    assert_eq!(parity::check_run_counts(&order, 3), Ok(()), "{order:?}");
    let rounds = runs_of(&Applied {
        options: pipeline::Options { llvm_order: false, ..Default::default() },
        ..Applied::default()
    });
    assert!(
        parity::check_run_counts(&rounds, 3).is_err(),
        "premise: the rounds run gvn more than once per body: {rounds:?}"
    );
}

/// `LLRM_DEBUG=counts` said nothing of who asked for an analysis under the
/// parity pipeline (the plan page called the counter "not built yet"). The
/// default order tells the counts its Translated slots and a `callers` line
/// splits each analysis between them and the old passes; the rounds have none.
#[test]
fn test_the_default_order_prints_callers_lines_split_by_joined_slots() {
    llrm_mir::counts::per_pass();
    let _ = runs_of(&Applied::default());
    let callers = llrm_mir::counts::callers();
    assert!(
        callers.iter().any(|line| line.starts_with("[counts] callers ") && !line.contains(" 0 joined ")),
        "no analysis was computed by a joined slot under the default order: {callers:?}"
    );
}
