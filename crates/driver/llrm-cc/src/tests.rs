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
    for flag in ["-fsanitize=address", "-fprofile-generate", "-flto"] {
        assert!(parse(&words(&format!("{flag} x.c"))).is_err(), "{flag}");
    }
    assert!(parse(&words("-g0 x.c")).is_ok());
}

/// `-fsanitize=stack` and `bounds` were refused with "not translated yet",
/// so stackover.c, stackdeep.c, es_across_copy.c and idioms.c (8 configs)
/// could not compile under llrm-cc; the backend takes them.
#[test]
fn the_stack_and_bounds_checks_reach_the_backend() {
    let one = built("-fsanitize=stack -fsanitize=bounds -c x.c");
    assert_eq!(one.llc, words("-fsanitize=stack -fsanitize=bounds"));
    assert!(parse(&words("-fsanitize=signed-integer-overflow x.c")).is_err());
}

/// Open Watcom's `char` is unsigned; clang's default on i386 is signed, so
/// watcom_far_sum.c (`*p = 251` read back through a far `char`) printed 1229
/// for 1485 under llrm-cc.
#[test]
fn the_watcom_abi_makes_char_unsigned() {
    let unsigned = |args: &str| built(args).clang.iter().any(|one| one == "-funsigned-char");
    assert!(unsigned("-mabi=watcom -c x.c"));
    assert!(!unsigned("-mabi=sysv -c x.c"));
    assert!(!unsigned("-mabi=watcom -fsigned-char -c x.c"));
}

/// The header Nib generates for `-mabi=watcom` says `extern short __far
/// __watcall weight(...)`; the rows without watcom.h (x86_16-dos) did not know
/// `__watcall` ("expected ';'"), so the interop example's C library did not
/// compile. The backend's convention is the Watcom one: the keyword adds
/// nothing for clang.
#[test]
fn the_watcom_abi_defines_the_watcall_keyword_away() {
    let defines = |args: &str| built(args).clang.iter().any(|one| one == "-D__watcall=");
    assert!(defines("-mabi=watcom -c x.c"));
    assert!(!defines("-mabi=sysv -c x.c"));
}

/// `-g` was refused ("debug info is not carried yet"), so no program could
/// carry debug info: clang gets `-g`, the backend the flag that names the
/// format.
#[test]
fn debug_flags_reach_clang_and_the_backend() {
    let one = built("-gcodeview -c x.c");
    assert!(one.clang.iter().any(|flag| flag == "-g"), "{:?}", one.clang);
    assert!(one.clang.iter().any(|flag| flag == "-fexperimental-assignment-tracking=disabled"), "{:?}", one.clang);
    assert_eq!(one.llc, words("-gcodeview"));
    assert!(built("-g0 -c x.c").clang.iter().all(|flag| flag != "-g"));
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
    // With `llc_level = false` on the arm64-macos row -O0..-Os never reached
    // llrm-arm64-llc: a size or speed measured through llrm-cc at any level
    // was the backend's default.
    let table = targets(TARGETS).unwrap();
    for (flag, level) in [("-O0", "-O0"), ("-O1", "-O1"), ("-Os", "-Os")] {
        let one = built(&format!("{flag} -c x.c"));
        let args = llc_args(&table["arm64-macos"], &one, Path::new("x.ll"), Path::new("x.o"));
        assert_eq!(args.first().map(String::as_str), Some(level), "{args:?}");
    }
}

/// The Watcom front end's tests pass the backend's switches (`-march`,
/// `-fno-inline-functions-called-once`, `-mcode-segment=`) with the source;
/// clang took them all and the backend saw none, so a program built with them
/// came out as without.
#[test]
fn the_backends_switches_reach_the_backend() {
    let one =
        built("-march=i486 -fno-inline-functions-called-once -fno-jump-tables -mcode-segment=X -fno-inline=f -c x.c");
    assert_eq!(
        one.llc,
        words("-march=i486 -fno-inline-functions-called-once -fno-jump-tables -mcode-segment=X -fno-inline=f")
    );
    assert_eq!(one.clang, words("-march=i486"));
}

/// `--dump d` was taken for an input: "cannot specify -o with several
/// inputs" and no stage written.
#[test]
fn dump_reaches_the_backend_with_its_directory() {
    let one = built("--dump d -c x.c");
    assert_eq!(one.llc, words("--dump d"));
    assert_eq!(one.inputs.len(), 1);
    assert_eq!(built("--dump=d -c x.c").llc, words("--dump=d"));
}

/// `huge_array.c` (flags `-m32`) failed at m32 with "expected ';' after top
/// level declarator": the flat header emptied `near` and `far` but not `huge`.
#[test]
fn the_flat_header_empties_every_segment_keyword() {
    let flat =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../toolchain/clang/flat.h")).unwrap();
    for keyword in ["__near", "__far", "__huge", "near", "far", "huge"] {
        assert!(flat.lines().any(|line| line.trim() == format!("#define {keyword}")), "{keyword}");
    }
}

/// The tests that spawned llrm-c pass `-fobject-format=elf` and `-Omax`; clang
/// refused both ("unknown argument", "invalid integral value 'max'").
#[test]
fn the_object_format_goes_to_the_backend_and_omax_is_clangs_o3() {
    let one = built("-fobject-format=elf -Omax -c x.c");
    assert_eq!(one.llc, words("-fobject-format=elf"));
    assert_eq!(one.clang, words("-O3"));
    assert_eq!(one.level, "max");
}

/// `-fverify-ir` went to clang ("unknown argument") and the backend never
/// checked the IR it was given.
#[test]
fn fverify_ir_reaches_the_backend() {
    let one = built("-fverify-ir -c x.c");
    assert_eq!(one.llc, words("-fverify-ir"));
    assert!(one.clang.is_empty(), "{:?}", one.clang);
}

/// The m16 datalayout is stated in the target's `datalayout.toml` and again in
/// every m16 row of the table, and clang's own (the fork's `I8086.h`). A row
/// that drifted compiled with another `long double` layout than the backend
/// laid out: clang refused `struct { char c; long double v; short s; }` with a
/// static initializer ("cannot compile this static initializer yet") while the
/// default `f80` was 128 bits.
#[test]
fn every_m16_row_states_the_targets_datalayout() {
    let target: toml::Table =
        include_str!("../../../target/llrm-x86-m16/src/machines/datalayout.toml").parse().unwrap();
    let stated = target["datalayout"].as_str().unwrap();
    let table = targets(TARGETS).unwrap();
    for name in ["x86_16-dos", "x86_16-bcc", "x86_16-wat"] {
        let row = table[name].datalayout.replace("-ni:2:3", "");
        assert_eq!(row, stated, "{name}");
    }
    assert!(stated.contains("-f80:16-"), "{stated}");
}

/// `-Og` reached the backend as `-Og`, so the loops of o1_fill.rs stayed
/// loops: clang's -Og is its -O1, which has LoopIdiomRecognize.
#[test]
fn og_reaches_the_backend_as_o1_like_clang() {
    assert_eq!(built("-Og -c x.c").level, "1");
}

#[test]
fn os_layer_answers_without_llrm_c_or_clang() {
    // dosbatch asked `llrm-c --os-layer`, which a build without llrm-c lacks:
    // every C program of the behavioural stage failed to link.
    let said = os_layer(&words("--os-layer start -m32")).unwrap();
    assert_eq!(said.unwrap().trim(), "start.asm");
    assert!(os_layer(&words("-O2 x.c")).is_none());
    assert!(os_layer(&words("--os-layer start")).unwrap().is_err());
}

/// The 6502 rows name their own backend and platform, and the level reaches it.
/// A row with the default `llrm-llc` handed the msp430 IR to the x86 backend.
#[test]
fn the_6502_rows_select_their_own_backend_and_cpu() {
    let table = targets(TARGETS).unwrap();
    let one = built("-Os -S x.c");
    for (row, cpu) in [("6502-sim65", "--cpu=6502"), ("65c02-sim65", "--cpu=65c02")] {
        assert_eq!(table[row].llc_program, "llrm-6502-llc", "{row}");
        let args = llc_args(&table[row], &one, Path::new("x.ll"), Path::new("x.s"));
        assert_eq!(args[..3], ["-Os", "--platform=sim65", cpu], "{row}: {args:?}");
        assert!(args.contains(&"-S".to_owned()), "{row}: {args:?}");
        assert_eq!(table[row].c_abi.as_ref().map(|c| (c.int, c.pointer)), Some((2, 2)), "{row}");
    }
    assert_eq!(table["x86_64-linux"].llc_program, "llrm-llc");
}
