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
    let on = Applied { options: pipeline::Options { ..Default::default() }, ..Applied::default() };
    let built = names(&on, Phase::Simplify);
    // The rotate slot follows the option, as in the pipeline's own check.
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
    // The rotate slot follows the option, as in the pipeline's own check.
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
    let why = parity::check(
        parity::REGISTRY,
        Phase::Early,
        &["lowerexpect@1177", "decide@1178", "sroa@1179", "earlycse@1180", "unknown@1"],
    )
    .unwrap_err();
    assert!(why.contains("unknown@1"), "{why}");
}

/// The lists `llvm_order` builds at -O1, -O2 and -Os are what the registry
/// says.
#[test]
fn test_the_built_lists_satisfy_the_registry_at_every_level() {
    for options in [pipeline::Options::basic(), pipeline::Options::standard(), pipeline::Options::size()] {
        let on = Applied { options: pipeline::Options { ..options }, ..Applied::default() };
        for phase in [Phase::Early, Phase::Cleanup, Phase::Simplify] {
            let built = names(&on, phase);
            let level = if on.options.pre { parity::Level::O2 } else { parity::Level::O1 };
            assert_eq!(parity::check_phase_at(phase, &built, level), Ok(()), "{phase:?}");
        }
    }
}

/// `dead@677`, `@768`, `@801` and `@837` followed InstCombine in the Simplify
/// slots and changed nothing in 1561 runs each over the 568 vsgcc cells (the
/// `idle` of `LLRM_DEBUG=runs`), 5.7 M Ir of d_faces -O2 with the one at the
/// cleanup slot; only `dead@1242` finds work (48 of 1480 runs).
#[test]
fn test_no_dead_pass_follows_instcombine_in_the_simplify_slots() {
    let on = Applied { options: pipeline::Options { ..pipeline::Options::standard() }, ..Applied::default() };
    let built = names(&on, Phase::Simplify);
    let dead: Vec<_> = built.iter().filter(|name| name.starts_with("dead@")).collect();
    assert!(dead.is_empty(), "{dead:?}");
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
        report.contains("dead@1242 (instcombine) -> instcombine-port") && report.contains("shadow_mssa"),
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
    let why = parity::check_backend(
        &[
            "earlytaildup@1",
            "cmpsub@1",
            "processimpdefs@2",
            "optimizephis@1",
            "deadmi@1",
            "fixupsetcc@5",
            "machinelicm@4",
            "flagscopy@5",
            "callframe@5",
            "fixupbw@14",
            "fixupleas@14",
            "machinecse@4",
            "machinesink@4",
            "peepholeopt@4",
            "machinesched@5",
            "combiner@4",
            "liverangeshrink@4",
            "floatalloc@8",
            "pushfold@6",
            "coalesce@10",
            "latecleanup@13",
            "leaopt@5",
            "jumps@17",
            "copyprop@18",
            "allocate@11",
            "unknown@1",
        ],
        &[],
    )
    .unwrap_err();
    assert!(why.contains("unknown@1"), "{why}");
}

/// A Partial slot that is not run fails like a Translated one, but it is never
/// counted as joined. Held for every Partial row of a registry, so a row that
/// moves to Translated or leaves Partial needs no edit here.
fn partial_rows_are_required_and_never_joined(registry: &[Entry]) {
    for row in registry.iter().filter(|one| one.kind == SlotKind::Partial) {
        let built: Vec<&str> = registry.iter().filter(|one| one.phase == row.phase).map(|one| one.slot).collect();
        assert_eq!(parity::check(registry, row.phase, &built), Ok(()), "{}", row.slot);
        let without: Vec<&str> = built.iter().copied().filter(|one| *one != row.slot).collect();
        let why = parity::check(registry, row.phase, &without).expect_err("a Partial slot left out");
        assert!(why.contains(row.slot), "{why}");
        assert!(parity::joined_slots(registry, &built).all(|one| one != row.slot), "{}", row.slot);
    }
}

#[test]
fn test_a_partial_slot_is_checked_like_a_translated_one_and_not_counted_joined() {
    let fixture = [Entry {
        slot: "fixture@1",
        phase: Phase::Early,
        llvm: "fixture",
        kind: SlotKind::Partial,
        replaced_by: "",
        note: "criterion: c; exit: e",
    }];
    partial_rows_are_required_and_never_joined(&fixture);
    partial_rows_are_required_and_never_joined(parity::REGISTRY);
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
fn test_the_llvm_order_runs_gvn_once_per_body() {
    let order = runs_of(&Applied::default());
    assert_eq!(parity::check_run_counts(&order, 3, parity::Level::O2), Ok(()), "{order:?}");
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
    let o1 = Applied { options: pipeline::Options { ..pipeline::Options::basic() }, ..Applied::default() };
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
    let on = Applied { options: pipeline::Options { ..pipeline::Options::standard() }, ..Applied::default() };
    let built = names(&on, Phase::Simplify);
    let at = |name: &str| built.iter().position(|one| *one == name).unwrap_or_else(|| panic!("{name} missing"));
    assert!(at("gvn@787") < at("bdce@797") && at("bdce@797") < at("combine@801"), "{built:?}");
}

/// ADCE stands where LLVM runs it, after jump threading and before MemCpyOpt
/// (`PassBuilderPipelines.cpp` l.815).
#[test]
fn test_adce_stands_between_cvp_and_memcpyopt() {
    let on = Applied { options: pipeline::Options { ..Default::default() }, ..Applied::default() };
    let built = names(&on, Phase::Simplify);
    let at = |name: &str| built.iter().position(|one| *one == name).unwrap_or_else(|| panic!("{name} not built"));
    assert_eq!((at("cvp@810") + 1, at("adce@815") + 1), (at("adce@815"), at("memcpyopt@818")));
}

/// LLVM runs SCCP after GVN and before BDCE.
#[test]
fn test_sccp_stands_between_gvn_and_bdce() {
    let on = Applied { options: pipeline::Options { ..pipeline::Options::standard() }, ..Applied::default() };
    let built = names(&on, Phase::Simplify);
    let at = |name: &str| built.iter().position(|one| *one == name).unwrap_or_else(|| panic!("{name} missing"));
    assert_eq!((at("gvn@787") + 1, at("sccp@792") + 1), (at("sccp@792"), at("bdce@797")));
}

/// LSR's translated pieces landed with no registry row, so the count could not
/// show them; DomTree was translated with no analyses row. The slot is
/// Translated now the pass calls the pieces, and `dominators` is a Translated
/// analysis.
#[test]
fn test_lsr_is_a_registered_slot_and_dominators_a_translated_analysis() {
    let lsr = parity::REGISTRY.iter().find(|one| one.slot == "lsr").expect("lsr is registered");
    assert_eq!((lsr.llvm, lsr.kind), ("loop-reduce (LoopStrengthReduce.cpp)", SlotKind::Translated));
    let dominators = parity::ANALYSES.iter().find(|one| one.key == "dominators").expect("dominators is registered");
    assert_eq!(
        (dominators.llvm, dominators.kind, dominators.off),
        ("DominatorTreeAnalysis", SlotKind::Translated, false)
    );
    let shape = parity::ANALYSES.iter().find(|one| one.key == "shape").expect("shape is registered");
    assert_eq!((shape.llvm, shape.kind), ("none", SlotKind::Old), "shape is a bundle, not a translation");
}

/// An analysis the manager computed that no registry row named left the count
/// of old callers without a list to read: `tools/old_analysis_callers.py` kept
/// its own by hand. Every analysis `LLRM_DEBUG=counts` can print has a row, and
/// a row that is not Translated says what replaces it.
#[test]
fn test_every_analysis_the_manager_names_has_a_registry_row() {
    let sources = [
        include_str!("../../../ir/llrm-mir/src/passes.rs"),
        include_str!("../../../ir/llrm-mir/src/callgraph.rs"),
        include_str!("../../../ir/llrm-mir/src/lazyvalueinfo.rs"),
        include_str!("../../llrm-analysis/src/manager.rs"),
        include_str!("../../llrm-analysis/src/bpi.rs"),
        include_str!("../../llrm-analysis/src/blockfreq.rs"),
        include_str!("../../llrm-analysis/src/cfg.rs"),
        include_str!("../../llrm-analysis/src/memoryssa.rs"),
        include_str!("../../llrm-analysis/src/noreturn.rs"),
        include_str!("../../llrm-analysis/src/scev/analysis.rs"),
        include_str!("spill.rs"),
        include_str!("../../../ir/llrm-mir/src/memory.rs"),
    ];
    // An analysis names itself in `const NAME`, a scan that is not one in a
    // `counted("..")` call.
    let named: Vec<&str> = sources
        .iter()
        .flat_map(|source| source.lines())
        .map(|line| line.trim_start().trim_start_matches("crate::passes::"))
        .filter(|line| line.starts_with("const NAME: &'static str = \"") || line.starts_with("counted(\""))
        .filter_map(|line| line.split('"').nth(1))
        .filter(|name| !name.starts_with("program-"))
        .collect();
    assert!(named.len() > 30, "the scan found {} names", named.len());
    let missing: Vec<&&str> =
        named.iter().filter(|name| parity::ANALYSES.iter().all(|one| one.key != **name)).collect();
    assert!(missing.is_empty(), "analyses with no registry row: {missing:?}");
    for one in parity::ANALYSES.iter().filter(|one| one.kind != SlotKind::Translated) {
        assert!(!one.replaced_by.is_empty(), "{} is not Translated and says nothing of its replacement", one.key);
    }
}

/// `tools/old_analysis_callers.py` reads the analyses from the `[parity]
/// analysis` lines; a row printed without its type would leave it nothing to
/// look for.
#[test]
fn test_the_tally_lines_carry_each_analysis_with_its_kind_and_type() {
    let lines = parity::tally_lines();
    assert!(lines.contains(&"[parity] analysis shape old Shape".to_owned()), "{lines:?}");
    assert!(lines.contains(&"[parity] analysis dominators translated Dominators".to_owned()), "{lines:?}");
    assert!(parity::ANALYSES.iter().all(|one| !one.symbol.is_empty()));
}

/// A slot called Old that runs a translated pass went unnoticed: `decide@1178`
/// ran the translated SimplifyCFG (#2907) while its row stayed Old, and the
/// count of record under-reported the work. The same slot called Partial or
/// Translated passes.
#[test]
fn test_an_old_slot_that_runs_a_translated_pass_fails() {
    let row = |kind| Entry {
        slot: "fixture@1",
        phase: Phase::Early,
        llvm: "fixture",
        kind,
        replaced_by: "",
        note: "criterion: c; exit: e",
    };
    let held = [("fixture@1", vec!["simplifycfg"])];
    for kind in [SlotKind::Old, SlotKind::Missing] {
        let why = parity::check_old_slots(&[row(kind)], &held).expect_err("a translated pass in an unfinished row");
        assert!(why.contains("fixture@1") && why.contains("simplifycfg"), "{why}");
    }
    for kind in [SlotKind::Partial, SlotKind::Translated] {
        assert_eq!(parity::check_old_slots(&[row(kind)], &held), Ok(()), "{kind:?}");
    }
    assert_eq!(parity::check_old_slots(&[row(SlotKind::Old)], &[("fixture@1", Vec::new())]), Ok(()));
}

/// The registry is what the order runs: no slot of the built lists, at any
/// level, is Old and holds a translated pass.
#[test]
fn test_no_old_slot_of_the_built_lists_runs_a_translated_pass() {
    for options in [pipeline::Options::basic(), pipeline::Options::standard(), pipeline::Options::size()] {
        let on = Applied { options: pipeline::Options { ..options }, ..Applied::default() };
        for phase in [Phase::Early, Phase::Cleanup, Phase::Simplify, Phase::Late] {
            let passes = pipeline::llvm_order(&on, phase);
            let held: Vec<_> = passes.iter().map(|one| (one.name(), one.translations())).collect();
            let built: Vec<_> = held.iter().map(|one| one.0).collect();
            let level = if on.options.pre { parity::Level::O2 } else { parity::Level::O1 };
            // The slots the options leave out, as `Fixed::linear` names them:
            // the jam is off below -O3.
            let off: Vec<_> = parity::REGISTRY
                .iter()
                .filter(|one| one.phase == phase && !on.options.wanted(one.slot))
                .map(|one| one.slot)
                .collect();
            let checked = parity::check_phase_with(
                phase,
                &built,
                level,
                on.options.sroa,
                on.options.mem2reg,
                on.options.rotate_slot,
                &off,
                &held,
            );
            assert_eq!(checked, Ok(()), "{phase:?} {level:?}");
        }
    }
}

fn recount(registry: &[Entry]) -> parity::Tally {
    let of = |kind| registry.iter().filter(|one| one.kind == kind).count();
    parity::Tally {
        translated: of(SlotKind::Translated),
        partial: of(SlotKind::Partial),
        old: of(SlotKind::Old),
        missing: of(SlotKind::Missing),
    }
}

fn line(
    what: &str,
    count: parity::Tally,
) -> String {
    format!(
        "[parity] {what} translated {} partial {} old {} missing {}",
        count.translated, count.partial, count.old, count.missing
    )
}

/// The registry's tally and the `[parity]` lines `LLRM_DEBUG=counts` prints are
/// the recount of its rows by kind. A pinned number (`assert_eq!(partial, 14)`)
/// made every slot-moving PR edit the same line, so every second one
/// conflicted; the number to read lives in `LLRM_DEBUG=counts`, not in a test.
fn the_counts_are_the_recount_of_the_rows(
    registry: &[Entry],
    analyses: &[parity::AnalysisEntry],
) {
    let all = recount(registry);
    assert_eq!(parity::tally(registry), all);
    assert_eq!(all.translated + all.partial + all.old + all.missing, registry.len());
    let lines = parity::tally_lines_of(registry, analyses);
    assert_eq!(lines[0], line("slots", all) + &format!(" (of {})", registry.len()));
    let phases = [Phase::Early, Phase::Cleanup, Phase::Simplify, Phase::Late, Phase::Module, Phase::Backend];
    let mut by_phase = 0;
    for phase in phases {
        let rows: Vec<Entry> = registry.iter().filter(|one| one.phase == phase).copied().collect();
        by_phase += rows.len();
        let name = format!("{phase:?}").to_lowercase();
        let wanted = line(&format!("phase {name}"), recount(&rows));
        assert!(lines.contains(&wanted), "{wanted}\n{lines:?}");
    }
    assert_eq!(by_phase, registry.len(), "a phase is missing from this test's list");
    let of = |kind| analyses.iter().filter(|one| one.kind == kind).count();
    let analyses_count = parity::Tally {
        translated: of(SlotKind::Translated),
        partial: of(SlotKind::Partial),
        old: of(SlotKind::Old),
        missing: of(SlotKind::Missing),
    };
    assert!(lines.contains(&line("analyses", analyses_count)), "{lines:?}");
}

#[test]
fn test_the_registry_counts_are_the_recount_of_its_rows() {
    the_counts_are_the_recount_of_the_rows(parity::REGISTRY, parity::ANALYSES);
    let first = parity::report().lines().next().unwrap_or_default().to_owned();
    let all = recount(parity::REGISTRY);
    assert_eq!(
        first,
        format!(
            "slots: {} translated, {} partial, {} old, {} missing",
            all.translated, all.partial, all.old, all.missing
        )
    );
}

/// Moving a row's kind moves the recount and nothing pinned: the same
/// invariants hold for the registry before and after, with no literal to edit.
#[test]
fn test_moving_a_row_changes_the_recount_and_no_literal() {
    let row = |slot, kind| Entry {
        slot,
        phase: Phase::Simplify,
        llvm: "fixture",
        kind,
        replaced_by: "",
        note: "criterion: c; exit: e",
    };
    let before = [row("a@1", SlotKind::Old), row("b@2", SlotKind::Translated)];
    let after = [row("a@1", SlotKind::Partial), row("b@2", SlotKind::Translated)];
    for registry in [&before, &after] {
        the_counts_are_the_recount_of_the_rows(registry, &[]);
    }
    assert_eq!(recount(&before), parity::Tally { translated: 1, partial: 0, old: 1, missing: 0 });
    assert_eq!(recount(&after), parity::Tally { translated: 1, partial: 1, old: 0, missing: 0 });
    assert_eq!(parity::tally(&after), recount(&after));
}

/// Marking a wired row Old without the runtime changing is the drift the
/// check exists for: take the real built lists, mark one slot that holds a
/// translated pass Old in a copy of the registry, and the check names it.
#[test]
fn test_marking_a_wired_row_old_is_caught_on_the_real_lists() {
    let on = Applied { options: pipeline::Options { ..pipeline::Options::standard() }, ..Applied::default() };
    let mut checked = 0;
    for phase in [Phase::Early, Phase::Cleanup, Phase::Simplify, Phase::Late] {
        let passes = pipeline::llvm_order(&on, phase);
        let held: Vec<_> = passes.iter().map(|one| (one.name(), one.translations())).collect();
        assert_eq!(parity::check_old_slots(parity::REGISTRY, &held), Ok(()), "{phase:?}");
        for (slot, translations) in held.iter().filter(|one| !one.1.is_empty()) {
            let marked: Vec<Entry> = parity::REGISTRY
                .iter()
                .map(|one| Entry { kind: if one.slot == *slot { SlotKind::Old } else { one.kind }, ..*one })
                .collect();
            if marked.iter().all(|one| one.slot != *slot) {
                continue;
            }
            let why = parity::check_old_slots(&marked, &held).expect_err("a wired row marked Old");
            assert!(why.contains(slot) && why.contains(translations[0]), "{slot}: {why}");
            checked += 1;
        }
    }
    assert!(checked > 0, "no slot of the built lists holds a translated pass: the scan checks nothing");
}

/// The three late InstCombines of the pin's default pipeline (l.1403, l.1464,
/// l.1499) had no slot: a `not` of a condition that SimplifyCFG (l.1438) made
/// before a branch stayed, 30 QCport objects larger once `fold`, which had
/// cleaned it, went. They run in a compile of a body, once each.
#[test]
fn test_the_late_instcombine_slots_run_in_a_compile() {
    let lines = runs_of(&Applied::default());
    for slot in ["combine@1403", "combine@1464", "combine@1499"] {
        let ran = lines.iter().filter(|line| line.contains(&format!("r01-{slot}"))).count();
        assert!(ran >= 1, "{slot} did not run: {lines:?}");
    }
}

/// The `.rs` file stems under `crates`.
fn source_stems() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../crates");
    let mut stems = Vec::new();
    let mut dirs = vec![root];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).expect("a readable directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name != "target") {
                    dirs.push(path);
                }
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && let Some(stem) = path.file_stem()
            {
                stems.push(stem.to_string_lossy().into_owned());
            }
        }
    }
    stems
}

/// constraint-elimination and shrink-wrap were labelled Missing while
/// `constraintelim.rs` and `shrinkwrap.rs` existed: the table undercounted the
/// port and sent work to translate a pass twice (a duplicate PR, #4457).
#[test]
fn test_a_missing_entry_whose_pass_has_a_source_file_fails() {
    let stems = source_stems();
    parity::check_missing_has_no_file(parity::REGISTRY, &stems).unwrap();
    let mut registry = parity::REGISTRY.to_vec();
    registry.push(Entry {
        slot: "constraint-elimination",
        phase: Phase::Simplify,
        llvm: "constraint-elimination",
        kind: SlotKind::Missing,
        replaced_by: "",
        note: "",
    });
    let why = parity::check_missing_has_no_file(&registry, &stems).unwrap_err();
    assert!(why.contains("constraint-elimination"), "{why}");
}

/// `decide@809` ran after JumpThreading and CVP: 19.1 M Ir on d_faces -O2 for
/// 3 or 4 of 65 objects, +5 to +24 B. The slot is gone.
#[test]
fn test_the_simplify_list_has_no_decide_after_jump_threading() {
    let on = Applied { options: pipeline::Options { ..Default::default() }, ..Applied::default() };
    let built = names(&on, Phase::Simplify);
    assert!(!built.contains(&"decide@809"), "decide@809 runs again: {built:?}");
    assert!(built.contains(&"jumpthreading@809") && built.contains(&"cvp@810"));
}

/// The module that implements each `Translated` slot. A name starting with
/// `codegen:` is a file or directory under `llrm-codegen/src`.
const IMPLEMENTED_BY: &[(&str, &str)] = &[
    ("lowerexpect@1177", "lowerexpect"),
    ("sroa@1179", "sroa"),
    ("earlycse@1180", "earlycse"),
    ("promote@1241", "mem2reg"),
    ("sroa@646", "sroa"),
    ("earlycse@649", "earlycse"),
    ("cvp@669", "cvp"),
    ("jumptableswitch@672", "jumptableswitch"),
    ("tailrec@688", "tailcallelim"),
    ("reassociate@696", "reassociate"),
    ("constraint-elimination@699", "constraintelim"),
    ("loop-instsimplify@715", "loopinstsimplify"),
    ("loop-simplifycfg@716", "loopsimplifycfg"),
    ("licm@724", "licm"),
    ("rotate@728", "looprotate"),
    ("licm@730", "licm"),
    ("trivialunswitch@732", "simpleloopunswitch"),
    ("loopidiom@737", "loopidiom"),
    ("loopdeletion@749", "loopdeletion"),
    ("sroa@776", "sroa"),
    ("mldst-motion@783", "mldstmotion"),
    ("gvn@787", "gvn"),
    ("sccp@792", "sccp"),
    ("bdce@797", "bdce"),
    ("dfajumpthreading@806", "dfajumpthreading"),
    ("cvp@810", "cvp"),
    ("adce@815", "adce"),
    ("memcpyopt@818", "memcpyopt"),
    ("licm@823", "licm"),
    ("infer-alignment@1362", "inferalignment"),
    ("sroa@1494", "sroa"),
    ("infer-alignment@1498", "inferalignment"),
    ("licm@1508", "licm"),
    ("float2int@1594", "float2int"),
    ("lower-constant-intrinsics@1597", "lowerconstantintrinsics"),
    ("instsimplify@1662", "instsimplify"),
    ("divrempairs@1667", "divrempairs"),
    ("tailrec@1677", "tailcallelim"),
    ("jumpthreading@668", "jumpthreading"),
    ("unroll@753", "loopunrollpass"),
    ("jumpthreading@809", "jumpthreading"),
    ("loop-interchange@1631", "loopinterchange"),
    ("loop-unroll-and-jam@1474", "loopunrollandjam"),
    ("cmpsub@1", "codegen:backend/cmpsub"),
    ("optimizephis@1", "codegen:backend/optimizephis"),
    ("deadmi@1", "codegen:backend/deadmi"),
    ("processimpdefs@2", "codegen:processimpdefs"),
    ("machinelicm@4", "codegen:backend/machinelicm"),
    ("combiner@4", "codegen:backend/combiner"),
    ("liverangeshrink@4", "codegen:backend/liverangeshrink"),
    ("fixupsetcc@5", "codegen:backend/fixupsetcc"),
    ("callframe@5", "codegen:backend/callframe"),
    ("flagscopy@5", "codegen:backend/flagscopy"),
    ("leaopt@5", "codegen:backend/leaopt"),
    ("floatalloc@8", "codegen:backend/fpstack"),
    ("latecleanup@13", "codegen:backend/latecleanup"),
    ("fixupbw@14", "codegen:backend/fixupbw"),
    ("fixupleas@14", "codegen:backend/fixupleas"),
    ("copyprop@18", "codegen:backend/machinecp"),
    ("lsr", "lsr_pass"),
    ("canonfreeze", "canonfreeze"),
];

/// Every source line of the module `name`: its file and its directory.
fn module_text(name: &str) -> String {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let (src, name) = match name.strip_prefix("codegen:") {
        Some(rest) => (manifest.join("../../codegen/llrm-codegen/src"), rest),
        None => (manifest.join("src"), name),
    };
    let mut text = std::fs::read_to_string(src.join(format!("{name}.rs"))).unwrap_or_default();
    if let Ok(dir) = std::fs::read_dir(src.join(name)) {
        for file in dir.flatten() {
            text += &std::fs::read_to_string(file.path()).unwrap_or_default();
        }
    }
    text
}

/// The first `Translated` slot whose module is not named
/// in `implemented_by` or does not cite the pin, as `reason`.
fn untranslated_slot(
    registry: &[Entry],
    implemented_by: &[(&str, &str)],
    text: &dyn Fn(&str) -> String,
) -> Result<(), String> {
    for one in registry.iter().filter(|one| one.kind == SlotKind::Translated) {
        let Some((_, module)) = implemented_by.iter().find(|(slot, _)| *slot == one.slot) else {
            return Err(format!("{}: Translated, but no module is named for it", one.slot));
        };
        if !text(module).contains("d1106deb71cc") {
            return Err(format!("{}: Translated, but module `{module}` cites no pin source", one.slot));
        }
    }
    Ok(())
}

/// The registry marked indvars@753 `Translated` while its module translates
/// only `rewriteLoopExitValues` and `widenIV`; a slot is Translated only
/// where its module cites the pin source it translates.
#[test]
fn test_every_translated_slot_names_a_module_that_cites_the_pin() {
    untranslated_slot(parity::REGISTRY, IMPLEMENTED_BY, &module_text).expect("the registry");
}

#[test]
fn test_a_translated_slot_whose_module_cites_no_pin_source_is_refused() {
    let mut registry = parity::REGISTRY.to_vec();
    registry.push(Entry {
        slot: "standin@1",
        phase: Phase::Simplify,
        llvm: "standin",
        kind: SlotKind::Translated,
        replaced_by: "",
        note: "",
    });
    let table: Vec<(&str, &str)> = IMPLEMENTED_BY.iter().copied().chain([("standin@1", "standin")]).collect();
    let text = |module: &str| if module == "standin" { "gcc's pass".to_string() } else { module_text(module) };
    let why = untranslated_slot(&registry, &table, &text).expect_err("a stand-in under a pin name");
    assert!(why.contains("standin@1"), "{why}");
    let why = untranslated_slot(&registry, IMPLEMENTED_BY, &module_text).expect_err("an unmapped slot");
    assert!(why.contains("no module is named"), "{why}");
}
