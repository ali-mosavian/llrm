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

#[test]
fn march_and_mtune_name_the_cpu_profiles() {
    let built_in = crate::abi::machine::BUILT_IN.clone();
    for (gcc, cpu) in [("i386", "386"), ("i486", "486"), ("pentium", "P5")] {
        assert_eq!(parsed(&[&format!("-march={gcc}")]).unwrap().machine(built_in.clone()).unwrap().cpu, cpu);
        assert_eq!(parsed(&[&format!("-mtune={gcc}")]).unwrap().machine(built_in.clone()).unwrap().cpu, cpu);
    }
    assert_eq!(parsed(&["--cpu", "K6"]).unwrap().machine(built_in.clone()).unwrap().cpu, "K6");
    assert!(parsed(&["-march=k8"]).unwrap_err().contains("i386, i486 or pentium"));
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
    let machine = |arguments: &[&str]| parsed(arguments).unwrap().machine(crate::abi::machine::BUILT_IN.clone()).unwrap().segments.unwrap().stack_is_data;
    assert!(!machine(&[]));
    assert!(machine(&["-mstack-is-data"]));
    assert!(!machine(&["-mstack-is-data", "-mno-stack-is-data"]));
}

/// Far zero data is stored unless -mfar-bss says the start-up zeroes it: a start-up that does not
/// (Borland's, Open Watcom's) would otherwise read whatever DOS left there.
#[test]
fn far_zero_data_is_stored_unless_the_startup_zeroes_it() {
    let machine = |arguments: &[&str]| parsed(arguments).unwrap().machine(crate::abi::machine::BUILT_IN.clone()).unwrap().far_bss;
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
    let options = flags.driver(crate::abi::machine::BUILT_IN.clone(), std::rc::Rc::new(llrm_x86_code16::Code16), crate::backend::isel::code16());
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

/// `--target` was refused by every frontend ("unrecognized arguments"): a
/// target could not be named at all.
#[test]
fn a_target_is_named_by_its_flag() {
    assert_eq!(parsed(&[]).unwrap().target(), None);
    assert_eq!(parsed(&["--target", "x86-code16"]).unwrap().target(), Some("x86-code16"));
    assert!(parsed(&["--target"]).unwrap_err().contains("expected one argument"));
}
