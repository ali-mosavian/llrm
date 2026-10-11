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

/// The allocator tries other shapes of a body at every level but -O0, and
/// `-f[no-]allocation-search` sets it anywhere. (gcc runs IRA once; turning the
/// search off at -O1 to -Os cuts QCport's compile 26-29% for +0.1% of its bytes
/// and nothing on the 66 m32 programs, but costs the 16-bit bench kernels 5-20%
/// of their instructions: quicksort -O2 +20%, where `Scoped` keeps a loop's
/// address base in a register. The default stays; the switch is there.)
#[test]
fn test_allocation_search_is_on_at_every_level_but_o0_and_a_flag_sets_it_anywhere() {
    for (level, on) in [("-O0", false), ("-O1", true), ("-O2", true), ("-Os", true), ("-O3", true), ("-Omax", true)] {
        assert_eq!(pipeline(&[level]).searches(), on, "{level}");
        assert!(pipeline(&[level, "-fallocation-search"]).searches(), "{level}");
        assert!(!pipeline(&[level, "-fno-allocation-search"]).searches(), "{level}");
    }
}

/// A function is made by the allocator alone and by the spiller's route and the
/// cheaper kept at every level but -O0: that choice, not the search of shapes,
/// is what the single allocation lost at x_dct (+15% clocks), x_ll_arith
/// (+17.6% code) and recmany (+13%) when both were turned off together.
#[test]
fn test_the_routes_are_compared_at_every_level_but_o0() {
    for (level, on) in [("-O0", false), ("-O1", true), ("-O2", true), ("-Os", true), ("-O3", true), ("-Omax", true)] {
        assert_eq!(pipeline(&[level]).compares_routes(), on, "{level}");
        assert_eq!(
            pipeline(&[level, "-fno-allocation-search"]).compares_routes(),
            on,
            "{level}: the search is not the routes"
        );
    }
    assert!(pipeline(&["-O0", "-fallocation-routes"]).compares_routes());
    assert!(!pipeline(&["-O2", "-fno-allocation-routes"]).compares_routes());
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
/// call inlined, -O2 adds inlining, gcse and sibling calls, -O3
/// peeling, unswitching, complete copies of loops that grow the code, and the
/// larger inline threshold. Before, -O1 and -O2 differed by loop copies alone
/// and -O2 let a complete copy grow the code. Pattern fill is the one
/// departure: gcc has it from -O2 (opts.cc:653), clang's O1 pipeline from -O1
/// (PassBuilderPipelines.cpp:562), and -O1 has it by the user's decision of
/// 2026-10-10; -Og does not.
#[test]
fn each_level_selects_gcc_s_passes() {
    // (scalar passes, last call inlined, inlines at all, gcse, sibling calls,
    // fill, peel, unswitch, copies may grow)
    let row = |level: &str| {
        let o = pipeline(&[level]);
        (
            [o.dead, o.promote, o.drop_stores, o.hoist, o.strength],
            o.inline.last,
            o.inline.limit > 0,
            o.forward && o.drop_loads,
            o.sibcalls,
            o.fill,
            o.peel,
            o.unswitch,
            o.limits.grows,
            o.unroll,
        )
    };
    let scalar = [true; 5];
    assert!(!pipeline(&["-O0"]).optimize);
    assert_eq!(row("-O1"), (scalar, true, true, false, false, true, false, false, false, true));
    assert_eq!(row("-O2"), (scalar, true, true, true, true, true, false, false, false, true));
    assert_eq!(row("-O3"), (scalar, true, true, true, true, true, true, true, true, true));
    assert_eq!(pipeline(&["-O"]), pipeline(&["-O1"]));
    assert_eq!(pipeline(&["-Og"]), Options { fill: false, ..pipeline(&["-O1"]) });
    assert_eq!(pipeline(&[]), pipeline(&["-O2"]));
    // `-fipa-cp-clone` is gcc's -O3 (opts.cc:676).
    assert_eq!(
        (
            pipeline(&["-O2"]).inline.cp_clone,
            pipeline(&["-O3"]).inline.cp_clone,
            pipeline(&["-O3", "-fno-ipa-cp-clone"]).inline.cp_clone,
            pipeline(&["-O2", "-fipa-cp-clone"]).inline.cp_clone
        ),
        (false, true, false, true)
    );
    assert_eq!(
        (pipeline(&["-O1"]).inline.limit, pipeline(&["-O2"]).inline.limit, pipeline(&["-O3"]).inline.limit),
        (90, 225, 250)
    );
    let max = pipeline(&["-Omax"]);
    assert_eq!(max.limits, Limits { target_percent: 200, ..Limits::default() });
    assert_eq!((max.inline, max.unroll, max.peel), (Threshold { cp_clone: true, ..Threshold::new(250) }, true, true));
    let os = pipeline(&["-Os"]);
    assert_eq!((os.limits.grows, os.inline, os.unroll), (false, Threshold::default().for_size(), true));
    let oz = pipeline(&["-Oz"]);
    assert_eq!(
        (oz.limits.grows, oz.inline, oz.unroll, oz.peel),
        (false, Threshold::default().for_size(), false, false)
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
    assert_eq!(options.inline, Threshold { cp_clone: true, ..Threshold::new(0) });
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
