use crate::parity::{self, Entry, SlotKind};
use crate::pipeline::{self, Applied, Phase};

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
    assert!(parity::TODO.iter().any(|one| one.contains("CFGAnalyses")));
}
