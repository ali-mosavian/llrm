use super::*;

fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(String::from).collect()
}

fn built(line: &str) -> Build {
    match parse(&words(line)).unwrap().1 {
        Job::Build(one) => one,
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_table_parses_and_names_each_field() {
    let table = targets(TARGETS).unwrap();
    assert!(table["x86_64-linux"].datalayout.starts_with("e-m:e"));
    assert!(target_named(&table, "x86_64-unknown-linux-gnu").is_ok());
}

#[test]
fn phase_flags_set_the_stop_and_the_output() {
    let one = built("-O2 -c -o x.o x.c");
    assert_eq!((one.stop, one.output.as_deref(), one.level.as_str()), (Stop::Object, Some("x.o"), "2"));
    assert_eq!(built("-S x.c").stop, Stop::Assembly);
}

#[test]
fn link_flags_stay_off_clang_and_unknown_flags_go_to_it() {
    let one = built("-O2 -DX=1 -Iinc x.c -lm -L/opt -Wl,-z,now");
    assert_eq!(one.clang, words("-O2 -DX=1 -Iinc"));
    assert_eq!(one.link, words("-lm -L/opt -Wl,-z,now"));
}

#[test]
fn preprocess_and_dependency_jobs_pass_through_without_the_target_flag() {
    let (target, job) = parse(&words("-target x86_64-linux -E x.c")).unwrap();
    assert_eq!((target.as_deref(), job), (Some("x86_64-linux"), Job::Passthrough(words("-E x.c"))));
}

#[test]
fn untranslated_features_are_refused_not_dropped() {
    for flag in ["-fsanitize=address", "-fprofile-generate", "-flto", "-g", "-gdwarf-4"] {
        assert!(parse(&words(&format!("{flag} x.c"))).is_err(), "{flag}");
    }
    assert!(parse(&words("-g0 x.c")).is_ok());
}

#[test]
fn unroll_flags_reach_the_backend_too() {
    let one = built("-fno-unroll-loops -c x.c");
    assert_eq!(one.llc, words("-fno-unroll-loops"));
}

#[test]
fn the_link_recipe_expands_its_lists() {
    let table = targets(TARGETS).unwrap();
    let command = link_command(&table["x86_64-linux"], &words("a.o b.o"), &words("-lm"), "prog").unwrap();
    assert_eq!(command, words("cc a.o b.o -lm -o prog"));
}

#[test]
fn clangs_i386_layout_is_m32s_but_double_alignment_is_not() {
    let clang = "e-m:e-p:32:32-p270:32:32-p271:32:32-p272:64:64-i128:128-f64:32:64-f80:32-n8:16:32-S128";
    let m32 = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32";
    assert_eq!(layout_facts(clang), layout_facts(m32));
    assert_ne!(layout_facts(&clang.replace("f64:32:64", "f64:64:64")), layout_facts(m32));
}

/// The pin file grew comment lines after its SHA and every build was refused as
/// "not the pinned clang".
#[test]
fn the_pin_is_the_first_line_alone() {
    assert_eq!(pin().len(), 40, "{:?}", pin());
    assert!(pin().bytes().all(|byte| byte.is_ascii_hexdigit()));
}

#[test]
fn an_environment_placeholder_is_filled_when_set_and_named_when_not() {
    // SAFETY: the variable is this test's own.
    unsafe { std::env::set_var("LLRM_CC_TEST_DIR", "/inc") };
    assert_eq!(with_environment("-I{env:LLRM_CC_TEST_DIR}/x"), "-I/inc/x");
    assert!(unset(&with_environment("{env:LLRM_CC_TEST_UNSET}")).ends_with("LLRM_CC_TEST_UNSET"));
}

/// `-mabi=regparm3` reached clang ("unsupported option '-mabi=' for target
/// i386-unknown-unknown-code16") instead of the backend, so the Nib interop
/// example could not link a C library in the Nib program's convention.
#[test]
fn mabi_goes_to_the_backend_after_the_tables_own() {
    let table = targets(TARGETS).unwrap();
    let one = built("-mabi=regparm3 x.c");
    assert!(one.clang.is_empty());
    let args = llc_args(&table["x86_16-dos"], &one, Path::new("x.ll"), Path::new("x.obj"));
    let last = args.iter().rposition(|arg| arg.starts_with("-mabi=")).unwrap();
    assert_eq!(args[last], "-mabi=regparm3");
}

#[test]
fn the_opt_level_reaches_the_arm64_backend() {
    // With `llc_level = false` on the arm64-macos row -O0..-Os never reached llrm-arm64-llc: a size or speed
    // measured through llrm-cc at any level was the backend's default.
    let table = targets(TARGETS).unwrap();
    for (flag, level) in [("-O0", "-O0"), ("-O1", "-O1"), ("-Os", "-Os")] {
        let one = built(&format!("{flag} -c x.c"));
        let args = llc_args(&table["arm64-macos"], &one, Path::new("x.ll"), Path::new("x.o"));
        assert_eq!(args.first().map(String::as_str), Some(level), "{args:?}");
    }
}
