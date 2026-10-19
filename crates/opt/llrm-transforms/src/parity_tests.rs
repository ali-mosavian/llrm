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
    // The rotate slot is Old unless `LLRM_ROTATE` is set, as in the
    // pipeline's own check.
    let rotate = pipeline::Options::default().rotate_slot;
    let mut registry: Vec<Entry> = parity::REGISTRY
        .iter()
        .map(|one| Entry { kind: if !rotate && one.slot == "rotate@728" { SlotKind::Old } else { one.kind }, ..*one })
        .collect();
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
    registry.pop();
    let without: Vec<_> = built.iter().copied().filter(|one| *one != "earlycse@649").collect();
    // The rotate slot is Old unless `LLRM_ROTATE` is set, as in the
    // pipeline's own check.
    let rotate = pipeline::Options::default().rotate_slot;
    let registry: Vec<Entry> = parity::REGISTRY
        .iter()
        .map(|one| Entry { kind: if !rotate && one.slot == "rotate@728" { SlotKind::Old } else { one.kind }, ..*one })
        .collect();
    let why = parity::check(&registry, Phase::Simplify, &without).expect_err("earlycse@649 left out");
    assert!(why.contains("earlycse@649"), "{why}");
}

/// A slot the parity pipeline runs must be in the registry, or it has no kind.
#[test]
fn test_a_slot_that_runs_and_is_not_registered_fails() {
    let why = parity::check(parity::REGISTRY, Phase::Early, &["sroa@1179", "earlycse@1180", "unknown@1"]).unwrap_err();
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
            let level = if on.options.pre { parity::Level::O2 } else { parity::Level::O1 };
            assert_eq!(parity::check_phase_at(phase, &built, level), Ok(()), "{phase:?}");
        }
    }
}

/// The MemorySSA entry carries its dependency on alias analysis, is asked by
/// default (EarlyCSE, LICM), and what is open of keeping it updated is named.
#[test]
fn test_the_registry_records_mssa_with_its_dependency_and_the_open_vocabulary() {
    let mssa = parity::ANALYSES.iter().find(|one| one.key == "mssa").expect("mssa is registered");
    assert_eq!(
        (mssa.llvm, mssa.kind, mssa.off, mssa.deps),
        ("MemorySSAAnalysis", SlotKind::Translated, false, &["aa"][..])
    );
    let report = parity::report();
    assert!(
        report.contains("fold@677 (instcombine) -> instcombine-port") && report.contains("shadow_mssa"),
        "{report}"
    );
    assert!(parity::TODO.iter().any(|one| one.contains("LICM")), "LICM still drops what it updates");
    assert!(parity::TODO.iter().all(|one| !one.contains("not a PreservedAnalyses variant")), "the variant exists now");
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
    let why =
        parity::check_backend(&["floatalloc@8", "coalesce@10", "jumps@17", "allocate@11", "unknown@1"]).unwrap_err();
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
    // jumps@17, allocate@11 (greedy by default under LLVM's order) and
    // merge@818 (MemCpyOpt's memsets), dse@820 (stage 1) and the two
    // jumpthreading slots (stage S1), indvars@738, coalesce@10 and the five
    // combine@N slots.
    assert_eq!(count.partial, 13);
    assert!(parity::report().contains("jumps@17") && parity::report().contains("13 partial"));
}

/// A Partial slot without its criterion and exit says what is done and not what
/// is left.
#[test]
fn test_a_partial_slot_states_its_criterion_and_exit() {
    for one in parity::REGISTRY.iter().filter(|one| one.kind == SlotKind::Partial) {
        assert!(one.note.contains("criterion:") && one.note.contains("exit:"), "{}: {:?}", one.slot, one.note);
    }
}

/// indvars@738 and coalesce@10 ran translated code under the order at defaults
/// but counted as Old, so the count of record hid the progress.
#[test]
fn test_indvars_and_coalesce_that_run_translations_are_partial_with_an_exit() {
    for slot in ["indvars@738", "coalesce@10"] {
        let one = parity::REGISTRY.iter().find(|one| one.slot == slot).expect(slot);
        assert_eq!(one.kind, SlotKind::Partial, "{slot}");
        assert!(one.note.contains("exit:"), "{slot}: {:?}", one.note);
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
    assert_eq!(parity::check_run_counts(&order, 3, parity::Level::O2), Ok(()), "{order:?}");
    let rounds = runs_of(&Applied {
        options: pipeline::Options { llvm_order: false, ..Default::default() },
        ..Applied::default()
    });
    assert!(
        parity::check_run_counts(&rounds, 3, parity::Level::O2).is_err(),
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

/// The pin's `default<O1>` has no GVN, and the -O1 list had it: `gvn@787` ran
/// over every body at -O1 where LLVM runs none. At -O1 the order builds no
/// GVN and the registry does not require the Translated slot; -O2 keeps one
/// per body.
#[test]
fn test_the_llvm_order_has_no_gvn_at_o1_as_the_pin_has_none() {
    let o1 =
        Applied { options: pipeline::Options { llvm_order: true, ..pipeline::Options::basic() }, ..Applied::default() };
    let lines = runs_of(&o1);
    assert_eq!(parity::check_run_counts(&lines, 3, parity::Level::O1), Ok(()), "{lines:?}");
    let o2 = runs_of(&Applied::default());
    let gvn = |lines: &[String]| lines.iter().filter(|line| line.contains("-gvn@787")).count();
    assert_eq!(gvn(&lines), 0, "-O1 ran gvn: {lines:?}");
    assert!(gvn(&o2) > 0, "premise: the -O2 order runs gvn: {o2:?}");
}

/// The scoreboard's recount printed all zeros once the tally stopped reading
/// the kinds: a hand-made registry must come out as counted by hand.
#[test]
fn test_the_tally_lines_count_a_registry_by_kind_and_by_phase() {
    let one =
        |slot, phase, kind| Entry { slot, phase, llvm: "x", kind, replaced_by: "", note: "criterion: c; exit: e" };
    let registry = [
        one("a", Phase::Early, SlotKind::Translated),
        one("b", Phase::Simplify, SlotKind::Translated),
        one("c", Phase::Simplify, SlotKind::Old),
        one("d", Phase::Backend, SlotKind::Partial),
        one("e", Phase::Backend, SlotKind::Missing),
    ];
    let lines = parity::tally_lines_of(&registry, &[]);
    assert_eq!(lines[0], "[parity] slots translated 2 partial 1 old 1 missing 1 (of 5)");
    assert!(lines.contains(&"[parity] phase early translated 1 partial 0 old 0 missing 0".to_owned()), "{lines:?}");
    assert!(lines.contains(&"[parity] phase simplify translated 1 partial 0 old 1 missing 0".to_owned()), "{lines:?}");
    assert!(lines.contains(&"[parity] phase backend translated 0 partial 1 old 0 missing 1".to_owned()), "{lines:?}");
    assert!(lines.contains(&"[parity] analyses translated 0 partial 0 old 0 missing 0".to_owned()), "{lines:?}");
}

/// The real registry: every slot is counted once overall and once in its phase.
#[test]
fn test_the_tally_lines_of_the_real_registry_add_up() {
    let lines = parity::tally_lines();
    let numbers = |line: &str| -> Vec<usize> {
        line.split_whitespace().filter_map(|word| word.trim_matches(|c| c == '(' || c == ')').parse().ok()).collect()
    };
    let total = numbers(&lines[0]);
    assert_eq!(total[0] + total[1] + total[2] + total[3], parity::REGISTRY.len(), "{lines:?}");
    assert_eq!(total[4], parity::REGISTRY.len());
    let by_phase: usize = lines
        .iter()
        .filter(|line| line.starts_with("[parity] phase "))
        .map(|line| numbers(line).iter().sum::<usize>())
        .sum();
    assert_eq!(by_phase, parity::REGISTRY.len(), "{lines:?}");
    assert!(lines.iter().any(|line| line.starts_with("[parity] analyses ")), "{lines:?}");
}

/// LLVM runs BDCE after GVN and before the second InstCombine; the joined
/// slot was missing from the -O2 list, so its output never reached later slots.
#[test]
fn test_bdce_stands_between_gvn_and_the_next_fold() {
    let on = Applied {
        options: pipeline::Options { llvm_order: true, ..pipeline::Options::standard() },
        ..Applied::default()
    };
    let built = names(&on, Phase::Simplify);
    let at = |name: &str| built.iter().position(|one| *one == name).unwrap_or_else(|| panic!("{name} missing"));
    assert!(at("gvn@787") < at("bdce@797") && at("bdce@797") < at("fold@801"), "{built:?}");
}

/// ADCE stands where LLVM runs it, after jump threading and before MemCpyOpt
/// (`PassBuilderPipelines.cpp` l.815).
#[test]
fn test_adce_stands_between_cvp_and_memcpyopt() {
    let on = Applied { options: pipeline::Options { llvm_order: true, ..Default::default() }, ..Applied::default() };
    let built = names(&on, Phase::Simplify);
    let at = |name: &str| built.iter().position(|one| *one == name).unwrap_or_else(|| panic!("{name} not built"));
    assert_eq!((at("cvp@810") + 1, at("adce@815") + 1), (at("adce@815"), at("memcpyopt@818")));
}

/// LLVM runs SCCP after GVN and before BDCE.
#[test]
fn test_sccp_stands_between_gvn_and_bdce() {
    let on = Applied {
        options: pipeline::Options { llvm_order: true, ..pipeline::Options::standard() },
        ..Applied::default()
    };
    let built = names(&on, Phase::Simplify);
    let at = |name: &str| built.iter().position(|one| *one == name).unwrap_or_else(|| panic!("{name} missing"));
    assert_eq!((at("gvn@787") + 1, at("sccp@792") + 1), (at("sccp@792"), at("bdce@797")));
}

/// LSR's translated pieces landed with no registry row, so the count could not
/// show them; DomTree was translated with no analyses row. The slot is Old
/// while no pass calls the pieces, and `dominators` is a Translated analysis.
#[test]
fn test_lsr_is_a_registered_slot_and_dominators_a_translated_analysis() {
    let lsr = parity::REGISTRY.iter().find(|one| one.slot == "lsr").expect("lsr is registered");
    assert_eq!((lsr.llvm, lsr.kind), ("loop-reduce (LoopStrengthReduce.cpp)", SlotKind::Old));
    assert!(!lsr.replaced_by.is_empty(), "an Old slot names who replaces it");
    let dominators = parity::ANALYSES.iter().find(|one| one.key == "dominators").expect("dominators is registered");
    assert_eq!(
        (dominators.llvm, dominators.kind, dominators.off),
        ("DominatorTreeAnalysis", SlotKind::Translated, false)
    );
    assert!(parity::ANALYSES.iter().all(|one| one.key != "shape"), "shape is a bundle, not a translation");
}
