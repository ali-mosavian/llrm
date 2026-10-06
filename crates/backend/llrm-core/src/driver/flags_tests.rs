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

#[test]
fn each_level_selects_its_pipeline() {
    let o2 = Options::default();
    assert_eq!(pipeline(&[]), o2);
    assert_eq!(pipeline(&["-O2"]), o2);
    assert_eq!(pipeline(&["-O0"]), Options { optimize: false, ..o2.clone() });
    let o1 = Options { unroll: false, peel: false, unswitch: false, ..o2.clone() };
    assert_eq!(pipeline(&["-O1"]), o1);
    assert_eq!(pipeline(&["-O"]), o1);
    assert_eq!(pipeline(&["-Og"]), o1);
    let o3 = pipeline(&["-O3"]);
    assert_eq!(o3.limits, Limits { max_unrolled_operations: 400, ..Limits::default() });
    assert_eq!((o3.inline, o3.unroll, o3.peel), (Threshold::new(250), true, true));
    let os = pipeline(&["-Os"]);
    assert_eq!((os.limits.grows, os.inline, os.unroll), (false, Threshold::default().for_size(), true));
    let oz = pipeline(&["-Oz"]);
    assert_eq!((oz.limits.grows, oz.inline, oz.unroll, oz.peel), (false, Threshold::default().for_size(), false, false));
    assert!(parsed(&["-O4"]).is_err());
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

/// A target names the CPUs gcc's `-march`/`-mtune` take in its `timings.times`, not the flag parser.
#[test]
fn march_and_mtune_name_the_cpu_profiles() {
    use llrm_target::Target;
    let on = |arguments: &[&str]| parsed(arguments).unwrap().machine(&llrm_x86_m16::M16, llrm_x86_m16::machine::BUILT_IN.clone());
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
    let machine = |arguments: &[&str]| parsed(arguments).unwrap().machine(&llrm_x86_m16::M16, llrm_x86_m16::machine::BUILT_IN.clone()).unwrap().segments.unwrap().stack_is_data;
    assert!(!machine(&[]));
    assert!(machine(&["-mstack-is-data"]));
    assert!(!machine(&["-mstack-is-data", "-mno-stack-is-data"]));
}

/// Far zero data is stored unless -mfar-bss says the start-up zeroes it: a start-up that does not
/// (Borland's, Open Watcom's) would otherwise read whatever DOS left there.
#[test]
fn far_zero_data_is_stored_unless_the_startup_zeroes_it() {
    let machine = |arguments: &[&str]| parsed(arguments).unwrap().machine(&llrm_x86_m16::M16, llrm_x86_m16::machine::BUILT_IN.clone()).unwrap().far_bss;
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
    assert_eq!(sanitize(&["-fsanitize=bounds,integer-divide-by-zero"]), Sanitize { signed_integer_overflow: false, ..all });
    assert_eq!(sanitize(&["-fsanitize=undefined", "-fno-sanitize=bounds"]), Sanitize { bounds: false, ..all });
    assert_eq!(sanitize(&["-ftrapv"]), Sanitize { signed_integer_overflow: true, ..Sanitize::default() });
    assert!(parsed(&["-fsanitize=address"]).is_err());
    // A check that costs code on every call is asked for by name, never by `undefined`.
    assert!(sanitize(&["-fsanitize=stack"]).stack && !sanitize(&["-fsanitize=undefined"]).stack && !sanitize(&[]).stack);
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
    let options = flags.driver(llrm_x86_m16::machine::BUILT_IN.clone(), std::rc::Rc::new(llrm_x86_m16::M16), crate::backend::isel::m16());
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

/// `-m16`, `-m32` and `-m64` name the target as gcc's do; `--target NAME` was a spelling of its own.
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
