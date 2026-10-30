use llrm_analysis::peelsize::Limits;
use llrm_transforms::inline::Threshold;
use llrm_transforms::pipeline::Options;

use super::*;

fn parsed(arguments: &[&str]) -> Result<Flags, String> {
    let argv: Vec<String> = arguments.iter().map(|one| one.to_string()).collect();
    let mut flags = Flags::default();
    let mut at = 0;
    while at < argv.len() {
        assert!(flags.take(&argv, &mut at)?, "{} is not taken", argv[at]);
        at += 1;
    }
    Ok(flags)
}

fn pipeline(arguments: &[&str]) -> Options {
    parsed(arguments).unwrap().pipeline()
}

/// A function is allocated once at every level below -Omax, as gcc's IRA and
/// LLVM's greedy allocator do: no search of other shapes and no second route
/// (d_faces compile -34% at -O2, -32% at -O3, -30% at -Os; no vsgcc row newly
/// below the better of gcc and clang in clocks). `-f[no-]allocation-search` and
#[test]
fn test_a_function_is_allocated_once_below_omax_and_the_flags_set_each_anywhere() {
    for (level, on) in [
        ("-O0", false),
        ("-O1", false),
        ("-Og", false),
        ("-O2", false),
        ("-Os", false),
        ("-O3", false),
        ("-Omax", true),
    ] {
        assert_eq!(pipeline(&[level]).searches(), on, "{level}");
        assert!(pipeline(&[level, "-fallocation-search"]).searches(), "{level}");
        assert!(!pipeline(&[level, "-fno-allocation-search"]).searches(), "{level}");
    }
}

/// Only -Omax tries every shape of a body; the other levels try the one its
/// spills suggest, as the search over all of them cost 2.7x the compile time
/// for +0.04% of QCport's bytes. `-f[no-]allocation-search-all` sets it at any
/// level.
#[test]
fn test_only_omax_searches_every_shape() {
    for level in ["-O1", "-O2", "-O3", "-Os"] {
        assert!(!pipeline(&[level]).searches_all(), "{level}");
        assert!(pipeline(&[level, "-fallocation-search-all"]).searches_all(), "{level}");
    }
    assert!(pipeline(&["-Omax"]).searches_all());
    assert!(!pipeline(&["-Omax", "-fno-allocation-search-all"]).searches_all());
}

/// The passes a level runs are gcc 13.4.0's `default_options_table` (opts.cc
/// 573-694) for the passes this compiler has: -O1 the scalar ones and the last
/// call inlined and sibling calls (the pin's TailCallElim also runs at -O1:
/// PassBuilderPipelines.cpp:1677, #3514), -O2 adds inlining and gcse, -O3
/// peeling, unswitching, complete copies of loops that grow the code, and the
/// larger inline threshold. Before, -O1 and -O2 differed by loop copies alone
/// and -O2 let a complete copy grow the code. Pattern fill is the one
/// departure: gcc has it from -O2 (opts.cc:653), clang's O1 pipeline from -O1
/// (PassBuilderPipelines.cpp:562), and -O1 has it by the user's decision of
/// 2026-10-10; -Og does not.
#[test]
fn each_level_selects_gcc_s_passes() {
    // (scalar passes, last call inlined, inlines at all, pre, sibling calls,
    // fill, peel, unswitch, copies may grow)
    let row = |level: &str| {
        let o = pipeline(&[level]);
        (
            [o.dead, o.promote, o.drop_stores, o.hoist, o.strength],
            o.inline.last,
            o.inline.limit > 0,
            o.pre,
            o.sibcalls,
            o.loopidioms,
            o.peel,
            o.unswitch,
            o.limits.grows,
            o.unroll,
        )
    };
    let scalar = [true; 5];
    assert!(!pipeline(&["-O0"]).optimize);
    // -O1 inlines as the pin's does (225, #3249).
    assert_eq!(row("-O1"), (scalar, true, true, false, true, true, false, false, false, true));
    assert_eq!(row("-O2"), (scalar, true, true, true, true, true, false, false, false, true));
    assert_eq!(row("-O3"), (scalar, true, true, true, true, true, true, true, true, true));
    assert_eq!(pipeline(&["-O"]), pipeline(&["-O1"]));
    assert_eq!(pipeline(&["-Og"]), Options { loopidioms: false, ..pipeline(&["-O1"]) });
    assert_eq!(pipeline(&[]), pipeline(&["-O2"]));
    // `-fipa-cp-clone` is gcc's -O3 (opts.cc:676); the pin's default<O3> has no
    // IPA-CP cloning, so -O3 leaves it off (recchop 1380 lines).
    assert_eq!(
        (
            pipeline(&["-O2"]).inline.cp_clone,
            pipeline(&["-O3"]).inline.cp_clone,
            pipeline(&["-O3", "-fno-ipa-cp-clone"]).inline.cp_clone,
            pipeline(&["-O2", "-fipa-cp-clone"]).inline.cp_clone
        ),
        (false, false, false, true)
    );
    assert_eq!(
        (pipeline(&["-O1"]).inline.limit, pipeline(&["-O2"]).inline.limit, pipeline(&["-O3"]).inline.limit),
        (225, 225, 250)
    );
    let max = pipeline(&["-Omax"]);
    assert_eq!(max.limits, Limits { target_percent: 200, max_unroll_iterations: 64, ..Limits::default() });
    assert_eq!(pipeline(&["-O3"]).limits.max_unroll_iterations, 16, "only -Omax lifts the trip cap");
    assert_eq!((max.inline, max.unroll, max.peel), (Threshold { cp_clone: true, ..Threshold::new(250) }, true, true));
    let os = pipeline(&["-Os"]);
    assert_eq!((os.limits.grows, os.inline, os.unroll), (false, Threshold::default().for_size(), true));
    let oz = pipeline(&["-Oz"]);
    assert_eq!(
        (oz.limits.grows, oz.inline, oz.unroll, oz.peel),
        (false, Threshold::default().for_min_size(), false, false)
    );
    assert!(parsed(&["-O4"]).is_err());
}

/// `-Omax` was no level: "unknown optimization level", so a build that meant
/// "everything on" had to say `-O3`.
#[test]
fn test_omax_is_every_pass_on_with_the_widest_budgets() {
    assert_eq!(pipeline(&["-Omax"]), Options::aggressive());
    assert!(pipeline(&["-Omax", "-fno-unroll-loops"]).limits == Options::aggressive().limits);
    assert!(parsed(&["-Omaximum"]).is_err());
}

/// `-fno-inline-functions` was no inlining at all, the last call of a function
/// included; gcc's leaves `-finline-functions-called-once` on and so does this,
/// which is the spelling for none.
#[test]
fn test_no_inline_functions_leaves_called_once_on_as_gcc_does() {
    assert!(pipeline(&["-O2", "-fno-inline-functions"]).inline.last);
    assert!(!pipeline(&["-O2", "-fno-inline-functions-called-once"]).inline.last);
    assert!(!pipeline(&["-O2", "-fno-inline-functions-called-once", "-fno-inline-functions"]).inline.last);
    assert_eq!(
        pipeline(&["-O2", "-fno-inline-functions-called-once", "-fno-inline-functions"]).inline,
        llrm_transforms::inline::Threshold::none()
    );
}

#[test]
fn a_pass_option_overrides_the_level_wherever_it_stands() {
    let options = pipeline(&["-fno-unroll-loops", "-O3", "-funswitch-loops", "-fno-inline-functions", "-fno-gcse"]);
    assert!(!options.unroll && options.unswitch && !options.forward && !options.drop_loads);
    assert_eq!(options.inline, Threshold::new(0));
    assert_eq!(pipeline(&["-O2", "-fno-peel-loops", "-fpeel-loops"]).peel, true);
    assert_eq!(pipeline(&["-O2", "-fno-inline-functions", "-finline-functions"]).inline, Threshold::default());
    let error = parsed(&["-fno-vectorize"]).unwrap_err();
    assert!(error.contains("-fno-vectorize") && error.contains("-funroll-loops"), "{error}");
}

/// A target names the CPUs gcc's `-march`/`-mtune` take in its `timings.times`,
/// not the flag parser.
#[test]
fn march_and_mtune_name_the_cpu_profiles() {
    use llrm_target::Target;
    let on = |arguments: &[&str]| {
        parsed(arguments).unwrap().machine(&llrm_x86_m16::M16, llrm_x86_m16::machine::BUILT_IN.clone())
    };
    for (gcc, cpu) in [("i386", "386"), ("i486", "486"), ("pentium", "P5"), ("athlon", "K7")] {
        assert_eq!(on(&[&format!("-march={gcc}")]).unwrap().cpu, cpu);
        assert_eq!(on(&[&format!("-mtune={gcc}")]).unwrap().cpu, cpu);
    }
    // -mtune prices for its CPU where -march names another.
    assert_eq!(on(&["-march=i386", "-mtune=pentium"]).unwrap().cpu, "P5");
    assert_eq!(on(&["-mtune=pentium", "-march=i386"]).unwrap().cpu, "P5");
    let error = on(&["-march=k8"]).unwrap_err();
    assert!(error.contains("-march=k8") && error.contains(&llrm_x86_m16::M16.marches().join(", ")), "{error}");
}

/// `--cpu` was a spelling of its own beside gcc's `-march`: one spelling.
#[test]
fn the_old_cpu_spelling_is_gone() {
    let argv = vec!["--cpu".to_owned(), "486".to_owned()];
    assert_eq!(Flags::default().take(&argv, &mut 0), Ok(false));
}

#[test]
fn output_and_assembly() {
    let flags = parsed(&["-S", "-o", "out.asm"]).unwrap();
    assert!(flags.assembly);
    assert_eq!(flags.output, Some(PathBuf::from("out.asm")));
    assert_eq!(parsed(&["--output=a.obj"]).unwrap().output, Some(PathBuf::from("a.obj")));
}

/// The stack is apart from the data group unless -mstack-is-data says it
/// is not: qcport's sound IRQ calls C on a stack of its own.
#[test]
fn stack_is_data_only_when_asked() {
    let machine = |arguments: &[&str]| {
        parsed(arguments)
            .unwrap()
            .machine(&llrm_x86_m16::M16, llrm_x86_m16::machine::BUILT_IN.clone())
            .unwrap()
            .segments
            .unwrap()
            .stack_is_data
    };
    assert!(!machine(&[]));
    assert!(machine(&["-mstack-is-data"]));
    assert!(!machine(&["-mstack-is-data", "-mno-stack-is-data"]));
}

/// Far zero data is stored unless -mfar-bss says the start-up zeroes it: a
/// start-up that does not (Borland's, Open Watcom's) would otherwise read
/// whatever DOS left there.
#[test]
fn far_zero_data_is_stored_unless_the_startup_zeroes_it() {
    let machine = |arguments: &[&str]| {
        parsed(arguments).unwrap().machine(&llrm_x86_m16::M16, llrm_x86_m16::machine::BUILT_IN.clone()).unwrap().far_bss
    };
    assert!(!machine(&[]));
    assert!(machine(&["-mfar-bss"]));
    assert!(!machine(&["-mfar-bss", "-mno-far-bss"]));
}

/// `-mbss-begin` marks the start of the near uninitialised data in every
/// object; off unless asked.
#[test]
fn the_bss_begin_marker_is_off_unless_asked() {
    let machine = |arguments: &[&str]| {
        parsed(arguments)
            .unwrap()
            .machine(&llrm_x86_m16::M16, llrm_x86_m16::machine::BUILT_IN.clone())
            .unwrap()
            .bss_begin
    };
    assert!(!machine(&[]));
    assert!(machine(&["-mbss-begin"]));
    assert!(!machine(&["-mbss-begin", "-mno-bss-begin"]));
}

/// gcc's run-time check names: each sanitizer alone, `undefined` all of
/// them, `-fno-sanitize` and `-ftrapv` as gcc reads them.
#[test]
fn sanitizers_take_gccs_names() {
    let sanitize = |arguments: &[&str]| parsed(arguments).unwrap().sanitize;
    let all = Sanitize { bounds: true, integer_divide_by_zero: true, signed_integer_overflow: true, stack: false };
    assert_eq!(sanitize(&[]), Sanitize::default());
    assert_eq!(sanitize(&["-fsanitize=undefined"]), all);
    assert_eq!(
        sanitize(&["-fsanitize=bounds,integer-divide-by-zero"]),
        Sanitize { signed_integer_overflow: false, ..all }
    );
    assert_eq!(sanitize(&["-fsanitize=undefined", "-fno-sanitize=bounds"]), Sanitize { bounds: false, ..all });
    assert_eq!(sanitize(&["-ftrapv"]), Sanitize { signed_integer_overflow: true, ..Sanitize::default() });
    assert!(parsed(&["-fsanitize=address"]).is_err());
    // A check that costs code on every call is asked for by name, never by
    // `undefined`.
    assert!(
        sanitize(&["-fsanitize=stack"]).stack && !sanitize(&["-fsanitize=undefined"]).stack && !sanitize(&[]).stack
    );
    assert!(!sanitize(&["-fsanitize=stack", "-fno-sanitize=stack"]).stack);
}

/// `-fstack-usage` and `-Wstack-usage=N` were no options: the stack a program
/// could use was never reported, and `-f` named every flag a pass.
#[test]
fn test_stack_usage_options_reach_the_driver() {
    let mut flags = Flags::default();
    for argument in ["-fstack-usage", "-Wstack-usage=512"] {
        let argv = vec![argument.to_owned()];
        assert!(flags.take(&argv, &mut 0).unwrap(), "{argument}");
    }
    let options = flags.driver(
        llrm_x86_m16::machine::BUILT_IN.clone(),
        std::rc::Rc::new(llrm_x86_m16::M16),
        crate::backend::isel::m16(),
    );
    assert!(options.stack_usage);
    assert_eq!(options.stack_limit, Some(512));
    assert!(Flags::default().take(&["-Wstack-usage=lots".to_owned()], &mut 0).is_err());
}

/// `--memory-model=real16|flat32|flat64` is the long form of `-m16|-m32|-m64`.
#[test]
fn the_memory_model_has_a_long_form_for_each_m_flag() {
    for (model, mode) in [("real16", 16), ("flat32", 32), ("flat64", 64)] {
        assert_eq!(parsed(&[&format!("--memory-model={model}")]).unwrap().mode(), Some(mode));
        assert_eq!(parsed(&["--memory-model", model]).unwrap().mode(), Some(mode));
    }
    assert!(parsed(&["--memory-model=huge"]).unwrap_err().contains("expected real16, flat32 or flat64"));
    assert!(parsed(&["--memory-model"]).is_err());
}

/// Two spellings of different models were taken silently, the last one winning.
#[test]
fn two_memory_models_that_differ_are_refused_and_equal_ones_are_not() {
    assert!(parsed(&["-m16", "--memory-model=flat32"]).unwrap_err().contains("conflicts with -m16"));
    assert!(parsed(&["--memory-model=flat64", "-m32"]).unwrap_err().contains("conflicts with -m64"));
    assert_eq!(parsed(&["-m32", "--memory-model=flat32"]).unwrap().mode(), Some(32));
}

/// `-mcmodel=` names real mode's code model; absent, the target's own applies.
#[test]
fn mcmodel_names_one_of_three_code_models() {
    assert_eq!(parsed(&[]).unwrap().code_model(), None);
    for model in ["small", "medium", "large"] {
        assert_eq!(parsed(&["-m16", &format!("-mcmodel={model}")]).unwrap().code_model(), Some(model));
    }
    assert!(parsed(&["-mcmodel=huge"]).unwrap_err().contains("expected small, medium or large"));
}

#[test]
fn clocks_per_byte_limits_the_growth_an_inline_may_buy() {
    assert_eq!(pipeline(&[]).limits.milliclocks_per_byte, 16_000);
    assert_eq!(pipeline(&["--clocks-per-byte", "2"]).limits.milliclocks_per_byte, 2000);
    assert_eq!(pipeline(&["--clocks-per-byte=0.25"]).limits.milliclocks_per_byte, 250);
    assert!(parsed(&["--clocks-per-byte", "-1"]).is_err());
    assert!(parsed(&["--clocks-per-byte", "lots"]).is_err());
}

/// `-m16`, `-m32` and `-m64` name the target as gcc's do; `--target NAME` was a
/// spelling of its own.
#[test]
fn a_target_is_named_by_gccs_m_flag() {
    assert_eq!(parsed(&[]).unwrap().mode(), None);
    assert_eq!(parsed(&["-m16"]).unwrap().mode(), Some(16));
    assert_eq!(parsed(&["-m32"]).unwrap().mode(), Some(32));
    assert_eq!(parsed(&["-m64"]).unwrap().mode(), Some(64));
    let argv = vec!["--target".to_owned(), "x86-m32".to_owned()];
    assert_eq!(Flags::default().take(&argv, &mut 0), Ok(false));
    // The machine flags that begin with -m stay their own.
    assert_eq!(parsed(&["-mstack-is-data"]).unwrap().mode(), None);
}

#[test]
fn fwrapv_makes_signed_arithmetic_wrap() {
    assert!(parsed(&["-fwrapv"]).unwrap().wrapv);
    assert!(!parsed(&["-fwrapv", "-fno-wrapv"]).unwrap().wrapv && !parsed(&[]).unwrap().wrapv);
}

/// -Omax is every pass on: it had `unswitch` off while -O3 had it on.
#[test]
fn omax_unswitches_loops() {
    assert!(pipeline(&["-Omax"]).unswitch);
    assert!(!pipeline(&["-Omax", "-fno-unswitch-loops"]).unswitch);
}

/// The pin's default pipelines hold `LoopInterchange` at every level but -O0
/// (`enable-loopinterchange`, PassBuilderPipelines.cpp#L219); it was on at -O3
/// and -Omax only, gcc's choice.
#[test]
fn every_optimized_level_interchanges_loops_and_the_flag_overrides_it() {
    for level in ["-O1", "-O2", "-Os", "-O3", "-Omax"] {
        assert!(pipeline(&[level]).interchange, "{level}");
        assert!(!pipeline(&[level, "-fno-loop-interchange"]).interchange, "{level}");
    }
}

/// The pin's default pipelines have `LoopUnrollAndJam` off
/// (`enable-unroll-and-jam`); llrm runs it at -O3 and -Omax (DEVIATE D1 of its
/// parity row), and `-floop-unroll-and-jam` overrides either way.
#[test]
fn only_o3_and_omax_unroll_and_jam_and_the_flag_overrides_it() {
    for level in ["-O1", "-O2", "-Os"] {
        assert!(!pipeline(&[level]).unroll_and_jam, "{level}");
        assert!(pipeline(&[level, "-floop-unroll-and-jam"]).unroll_and_jam, "{level}");
    }
    for level in ["-O3", "-Omax"] {
        assert!(pipeline(&[level]).unroll_and_jam, "{level}");
        assert!(!pipeline(&[level, "-fno-loop-unroll-and-jam"]).unroll_and_jam, "{level}");
    }
}

/// `-disable-lsr` (LLVM's TargetPassConfig option, also `-mllvm -disable-lsr`)
/// turns off the loop strength reduction alone: no flag turned off `lsr`
/// without `indvars`, `window` and the other address passes, so an on/off
/// diff of its asm was not possible.
#[test]
fn test_disable_lsr_turns_off_the_lsr_slot_alone() {
    assert!(pipeline(&["-O2"]).lsr);
    for arguments in [&["-O2", "-disable-lsr"][..], &["-O2", "-mllvm", "-disable-lsr"], &["-O2", "-fno-lsr"]] {
        let options = pipeline(arguments);
        assert!(!options.lsr && options.strength, "{arguments:?}");
    }
    assert!(parsed(&["-mllvm", "-nonsense"]).is_err());
}

/// DEVIATE 1: `-fno-inline=NAME` names a function a source cannot mark
/// `noinline`; each is kept, in order, and a bare `-fno-inline=` is no name.
#[test]
fn test_no_inline_names_each_function_and_is_not_the_inline_functions_pass() {
    let flags = parsed(&["-O2", "-fno-inline=BenchFib", "-fno-inline=Helper"]).unwrap();
    assert_eq!(flags.no_inline, ["BenchFib", "Helper"]);
    assert_eq!(flags.pipeline().inline, pipeline(&["-O2"]).inline, "no function is left out of inlining by it");
    assert!(parsed(&["-fno-inline="]).is_err());
    assert!(parsed(&["-fno-inline-functions"]).unwrap().no_inline.is_empty());
}

/// `--isel=` takes `dag`, `old` and `shadow`, inline or as the next argument;
/// anything else is an error. With none, a target gets the DAG selector where
/// it selects (x86 at every width) and the old one elsewhere.
#[test]
fn test_isel_names_the_selector() {
    assert_eq!(parsed(&[]).unwrap().isel, None);
    assert_eq!(parsed(&["--isel=dag"]).unwrap().isel, Some(Selector::Dag));
    assert_eq!(parsed(&["--isel=shadow"]).unwrap().isel, Some(Selector::Shadow));
    assert_eq!(parsed(&["--isel", "old"]).unwrap().isel, Some(Selector::Old));
    assert!(parsed(&["--isel=fast"]).unwrap_err().contains("unknown --isel=fast"));
    assert_eq!(Selector::for_target(&llrm_x86_m16::M16), Selector::Dag);
    assert_eq!(Selector::for_target(&llrm_x86_m32::M32), Selector::Dag);
    assert_eq!(Selector::for_target(&llrm_x86_m64::M64), Selector::Dag);
}

/// `-fallocation-routes` chose between the allocator alone and a second route
/// through the spiller; the second route is gone, so the flag is refused as
/// unknown, where it used to be taken and do nothing.
#[test]
fn test_the_removed_allocation_routes_flags_are_not_flags() {
    for flag in ["-fallocation-routes", "-fno-allocation-routes"] {
        let argv = vec![flag.to_owned()];
        let mut at = 0;
        let refused = Flags::default().take(&argv, &mut at).expect_err(&format!("{flag} is still taken"));
        assert!(refused.contains("unknown option"), "{refused}");
    }
}
