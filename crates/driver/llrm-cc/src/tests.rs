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

/// `-fpeel-loops` was refused by name once `Options::peel` was deleted, which
/// broke a command line that gcc and clang take: it is the backend's, accepted
/// and ignored, like `-fno-peel-loops`.
#[test]
fn fpeel_loops_and_fno_peel_loops_compile() {
    for flag in ["-fpeel-loops", "-fno-peel-loops"] {
        assert_eq!(built(&format!("{flag} -O2 -c x.c")).llc, words(flag), "{flag}");
    }
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
    assert!(one.clang.iter().all(|flag| !flag.starts_with("-fexperimental-assignment-tracking")), "{:?}", one.clang);
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

/// With `llc_level = false` on an arm64 row -O0..-Os never reached
/// llrm-arm64-llc: arm64-linux and arm64-glibc ran every -O level through the
/// -O2 mid-end (116 qemu counts that were four copies of one). A row that takes
/// no level must say why.
fn rows_dropping_the_level(table: &str) -> Vec<String> {
    let table = targets(table).unwrap();
    let mut bad = vec![];
    for (name, row) in &table {
        for level in ["-O0", "-O1", "-O2", "-Os"] {
            let one = built(&format!("{level} -c x.c"));
            let args = llc_args(row, &one, Path::new("x.ll"), Path::new("x.o"));
            let reaches = args.first().map(String::as_str) == Some(level);
            let reasoned = row.llc_level_reason.as_deref().is_some_and(|why| !why.is_empty());
            if !reaches && !reasoned {
                bad.push(format!("{name} at {level}: {args:?}"));
            }
        }
    }
    bad
}

#[test]
fn the_opt_level_reaches_every_backend_or_the_row_says_why() {
    let bad = rows_dropping_the_level(TARGETS);
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn a_row_without_a_level_and_without_a_reason_is_found() {
    let at = TARGETS.find("[arm64-linux]").unwrap();
    let (head, rest) = TARGETS.split_at(at);
    let row = format!("{head}{}", rest.replacen("llc_level = true", "llc_level = false", 1));
    let bad = rows_dropping_the_level(&row);
    assert!(bad.iter().any(|line| line.starts_with("arm64-linux")), "{bad:#?}");
    let said = row.replacen("llc_level = false", "llc_level = false\nllc_level_reason = \"takes no level\"", 1);
    assert!(rows_dropping_the_level(&said).is_empty());
}

/// `-fno-inline-functions-called-once` (tests/run/c headers, 38 programs carry
/// `-fno-inline-functions`) reached llrm-wasm-llc, which answered
/// "unrecognized argument" and refused the compile; the pin's clang takes the
/// switches itself and writes `noinline` into the IR.
#[test]
fn a_pass_switch_stays_away_from_a_backend_that_takes_none() {
    let table = targets(TARGETS).unwrap();
    let one = built("-fno-inline-functions-called-once -fno-unroll-loops -c x.c");
    let args = |name: &str| llc_args(&table[name], &one, Path::new("x.ll"), Path::new("x.o"));
    for name in ["wasm32", "z80-sdcc"] {
        assert!(args(name).iter().all(|flag| !flag.starts_with("-fno-")), "{name}: {:?}", args(name));
    }
    // m68k's llc takes them (its program headers' `-fno-inline-functions` made
    // 19 programs fail)
    for name in ["x86_32-linux", "m68k-68000", "m68k-68020"] {
        let kept = args(name);
        assert!(kept.contains(&"-fno-inline-functions-called-once".to_owned()), "{name}: {kept:?}");
        assert!(kept.contains(&"-fno-unroll-loops".to_owned()), "{name}: {kept:?}");
    }
    let precise = built("-fexcess-precision=standard -c x.c");
    let wasm = &table["wasm32"];
    let wasm_args = llc_args(wasm, &precise, Path::new("x.ll"), Path::new("x.o"));
    assert!(wasm_args.iter().all(|flag| !flag.starts_with("-fexcess-precision")), "{wasm_args:?}");
    assert!(clang_flags(wasm, &precise).contains(&"-fexcess-precision=standard".to_owned()));
    let x86 = &table["x86_32-linux"];
    assert!(
        llc_args(x86, &precise, Path::new("x.ll"), Path::new("x.o"))
            .contains(&"-fexcess-precision=standard".to_owned())
    );
}

#[test]
fn a_row_that_takes_no_pass_switch_says_why() {
    let table = targets(TARGETS).unwrap();
    for (name, row) in &table {
        let reasoned = row.llc_passes_reason.as_deref().is_some_and(|why| !why.is_empty());
        assert!(row.llc_passes || reasoned, "{name}");
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

/// The tests that spawned llrm-cc pass `-fobject-format=elf` and `-Omax`; clang
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
    // dosbatch asked `llrm-cc --os-layer`, which a build without llrm-cc lacks:
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

/// A C global `sp` collided with cc65's own `sp` at link. The cc65 rows name
/// the platform whose C symbols take cc65's `_`; the sim65 rows keep bare
/// names.
#[test]
fn the_cc65_rows_take_the_platform_with_cc65s_symbol_prefix() {
    let table = targets(TARGETS).unwrap();
    let one = built("-Os -S x.c");
    for (row, cpu) in [("6502-cc65", "--cpu=6502"), ("65c02-cc65", "--cpu=65c02")] {
        let args = llc_args(&table[row], &one, Path::new("x.ll"), Path::new("x.s"));
        assert_eq!(args[..3], ["-Os", "--platform=sim65-cc65", cpu], "{row}: {args:?}");
    }
}

/// `-ffp-contract=off` was put on every machine that fuses a multiply-add, so
/// arm64 never formed `fmadd` where the pin's clang and gcc do. No row turns
/// contraction off; a user who wants unfused results says so.
#[test]
fn no_row_turns_contraction_off_by_itself_and_the_flag_still_reaches_clang() {
    let table = targets(TARGETS).unwrap();
    for (name, target) in &table {
        assert!(!target.clang.iter().any(|flag| flag.starts_with("-ffp-contract")), "{name}");
    }
    let one = built("-ffp-contract=off -c x.c");
    assert!(one.clang.iter().any(|flag| flag == "-ffp-contract=off"), "{:?}", one.clang);
}

/// The z80 row names its own backend program and the C sizes of its
/// description. A row with the default `llrm-llc` handed the msp430 IR to the
/// x86 backend.
#[test]
fn the_z80_row_selects_its_own_backend() {
    let table = targets(TARGETS).unwrap();
    assert_eq!(table["z80-sdcc"].llc_program, "llrm-z80-llc");
    let args = llc_args(&table["z80-sdcc"], &built("-O2 -S x.c"), Path::new("x.ll"), Path::new("x.s"));
    assert!(args.contains(&"-S".to_owned()) && args.contains(&"-O2".to_owned()), "{args:?}");
    assert_eq!(table["z80-sdcc"].c_abi.as_ref().map(|c| (c.int, c.long, c.pointer)), Some((2, 4, 2)));
}

/// Every build spawned `clang --version` just to check the pin: ~6M
/// instructions on a 31M compile of grep. A clang unchanged in path, size and
/// mtime is now judged once; a changed or non-pin one is asked again.
#[test]
fn an_unchanged_pin_clang_is_asked_its_version_once() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("llrm-cc-pin-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let count = dir.join("count");
    let clang = dir.join("clang");
    let script = |said: &str| {
        std::fs::write(&clang, format!("#!/bin/sh\necho x >> {}\necho {said}\n", count.display())).unwrap();
        std::fs::set_permissions(&clang, std::fs::Permissions::from_mode(0o755)).unwrap();
    };
    let spawns = || std::fs::read_to_string(&count).unwrap_or_default().lines().count();
    script(pin());
    assert!(check_pin(&clang).is_ok());
    assert!(check_pin(&clang).is_ok());
    assert_eq!(spawns(), 1, "the second check spawned again");
    script(&format!("{} changed", pin()));
    assert!(check_pin(&clang).is_ok());
    assert_eq!(spawns(), 2, "a clang of another size was trusted unasked");
    script("not-a-pin");
    assert!(check_pin(&clang).is_err(), "a replaced non-pin clang passed on the old stamp");
    assert!(check_pin(&clang).is_err());
    assert_eq!(spawns(), 4);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The shared flags keep their meaning for every other target: x86_16 reads
/// `-mcmodel` for near and far code, so clang gets it there, and no backend of
/// a row without `llc_mcmodel` is handed a flag it never took. The m68k row's
/// clang is not given the flag (it refuses it) and its backend is.
#[test]
fn mcmodel_goes_to_the_m68k_backend_and_not_to_its_clang() {
    let table = targets(TARGETS).unwrap();
    let one = built("-O2 -mcmodel=medium -c x.c");
    let m68k = &table["m68k-68000"];
    assert!(llc_args(m68k, &one, Path::new("x.ll"), Path::new("x.o")).iter().any(|arg| arg == "-mcmodel=medium"));
    assert!(!clang_flags(m68k, &one).iter().any(|arg| arg == "-mcmodel=medium"));
    for (name, row) in table.iter().filter(|(name, _)| !name.starts_with("m68k-")) {
        assert!(!row.llc_mcmodel, "{name}");
        assert!(clang_flags(row, &one).iter().any(|arg| arg == "-mcmodel=medium"), "{name}: clang lost the flag");
        assert!(
            !llc_args(row, &one, Path::new("x.ll"), Path::new("x.o")).iter().any(|arg| arg == "-mcmodel=medium"),
            "{name}"
        );
    }
}

/// clang marks every loop `llvm.loop.unroll.disable` at -O1 (its `UnrollLoops`
/// is off), so the pin's -O1 never unrolls and x_life O1 ran 64k taken `jne`
/// against gcc's 16k (+151k clk). The m32 and m64 rows take `-funroll-loops` at -O1,
/// unless the command line states either spelling; other rows and levels are
/// as before.
#[test]
fn m32_rows_unroll_at_o1_unless_told_otherwise() {
    let table = targets(TARGETS).unwrap();
    let flags = |row: &str, line: &str| {
        let one = built(line);
        (clang_flags(&table[row], &one), llc_args(&table[row], &one, Path::new("x.ll"), Path::new("x.o")))
    };
    let (clang, llc) = flags("x86_32-linux", "-O1 -c x.c");
    assert!(clang.iter().any(|f| f == "-funroll-loops") && llc.iter().any(|f| f == "-funroll-loops"));
    for line in ["-O2 -c x.c", "-Os -c x.c", "-O1 -fno-unroll-loops -c x.c"] {
        let (clang, llc) = flags("x86_32-linux", line);
        assert!(!clang.iter().any(|f| f == "-funroll-loops") && !llc.iter().any(|f| f == "-funroll-loops"), "{line}");
    }
    // #9536: the m64 row takes it at -O1 as well, with the same exceptions.
    for line in ["-O1 -c x.c", "-O2 -c x.c", "-O1 -fno-unroll-loops -c x.c"] {
        let (clang, llc) = flags("x86_64-linux", line);
        let unrolls = line.starts_with("-O1 -c");
        assert!(
            clang.iter().any(|f| f == "-funroll-loops") == unrolls && llc.iter().any(|f| f == "-funroll-loops") == unrolls,
            "m64 {line}"
        );
    }
    let (clang, _) = flags("x86_16-dos", "-O1 -c x.c");
    assert!(!clang.iter().any(|f| f == "-funroll-loops"), "m16 as before");
}

/// The m68k rows take `-funroll-loops` at -O1 (M68K-UNROLL-O1): float_in_place_update kept a two-trip float
/// loop, 170 B at -O1 against gcc's 16. Other levels are as before, and a command line that names either
/// spelling is obeyed.
#[test]
fn the_m68k_rows_unroll_at_o1_unless_told_otherwise() {
    let table = targets(TARGETS).unwrap();
    let clang = |row: &str, line: &str| clang_flags(&table[row], &built(line));
    for row in ["m68k-68000", "m68k-68010", "m68k-68020", "m68k-68030", "m68k-68040", "m68k-68060"] {
        assert!(clang(row, "-O1 -S x.c").iter().any(|f| f == "-funroll-loops"), "{row}");
        for line in ["-O2 -S x.c", "-Os -S x.c", "-O1 -fno-unroll-loops -S x.c"] {
            assert!(!clang(row, line).iter().any(|f| f == "-funroll-loops"), "{row} {line}");
        }
    }
}

/// The 6502 rows take `-funroll-loops` at -O1 as the m32 rows do (6502-UNROLL-O1): float_in_place_update
/// kept a two-trip float loop at -O1, 9441 B against 19 B at -Os. Other levels are as before, and
/// a command line that names either spelling is obeyed.
#[test]
fn the_6502_rows_unroll_at_o1_unless_told_otherwise() {
    let table = targets(TARGETS).unwrap();
    let clang = |row: &str, line: &str| clang_flags(&table[row], &built(line));
    for row in ["6502-sim65", "65c02-sim65", "6502-cc65", "65c02-cc65"] {
        assert!(clang(row, "-O1 -S x.c").iter().any(|f| f == "-funroll-loops"), "{row}");
        for line in ["-O2 -S x.c", "-Os -S x.c", "-O1 -fno-unroll-loops -S x.c"] {
            assert!(!clang(row, line).iter().any(|f| f == "-funroll-loops"), "{row} {line}");
        }
    }
}

/// The pin's -O1 pipeline has no JumpThreadingPass (JUMPTHREAD-O1), so clang's
/// IR keeps a latch branch on a compare tested one block up (bsearch -O1: 668
/// clk against gcc's 614; shift_int32_variable m32 -O1: 8942 clk against gcc's
/// 7784; arm64 mandel -O1: 153,637 instructions against gcc's 139,398, the
/// second `b.hi` on one compare; m64 mandel -O1: 192,827 instructions against
/// gcc's 176,067, the same). The m68k, m32, m64 and arm64 rows give their
/// backend `-fthread-jumps` at -O1, unless the command line states either
/// spelling; no other row, and no other level, does.
#[test]
fn m68k_x86_and_arm64_rows_thread_jumps_at_o1_unless_told_otherwise() {
    let table = targets(TARGETS).unwrap();
    let llc = |row: &str, line: &str| llc_args(&table[row], &built(line), Path::new("x.ll"), Path::new("x.o"));
    let has = |args: &[String]| args.iter().any(|f| f == "-fthread-jumps");
    let threads = |row: &str| {
        row.starts_with("m68k-") || row.starts_with("x86_32-") || row.starts_with("x86_64-") || row.starts_with("arm64-")
    };
    for row in table.keys().filter(|row| threads(row)) {
        assert!(has(&llc(row, "-O1 -c x.c")), "{row}");
        for line in ["-O2 -c x.c", "-Os -c x.c", "-O1 -fno-thread-jumps -c x.c"] {
            assert!(!has(&llc(row, line)), "{row} {line}");
        }
    }
    for row in table.keys().filter(|row| !threads(row)) {
        assert!(!has(&llc(row, "-O1 -c x.c")), "{row} as before");
    }
}

/// A dist read the description files from the checkout it was built in, so
/// deleting or moving the checkout failed every compile (2345 of 3911 cells of
/// a post-merge A/B). Every description targets.toml names is embedded: the
/// table parses with no file read.
#[test]
fn every_description_a_target_names_is_embedded() {
    let table: BTreeMap<String, Target> = toml::from_str(TARGETS).unwrap();
    let mut wanted: Vec<(String, String)> = table
        .values()
        .filter_map(|one| {
            one.description
                .clone()
                .map(|name| (name, one.description_file.clone().unwrap_or_else(|| "datalayout.toml".into())))
        })
        .collect();
    for name in
        TARGETS.match_indices("{datalayout:").map(|(at, _)| TARGETS[at + 12..].split('}').next().unwrap().to_owned())
    {
        wanted.push((name, "datalayout.toml".into()));
    }
    assert!(wanted.len() > 8);
    for (name, file) in wanted {
        assert!(
            DESCRIPTIONS.iter().any(|(one, f, _)| *one == name && *f == file),
            "llrm-{name}/{file} is read from the checkout at run time: add it to DESCRIPTIONS"
        );
    }
}

/// A dist whose checkout was moved failed every DOS compile with `flat.h file
/// not found`: each header a recipe names under `{root}/toolchain/clang` is
/// embedded, so the driver can write them out when the checkout's own directory
/// is gone.
#[test]
fn every_toolchain_header_a_recipe_includes_is_embedded() {
    let names: Vec<&str> = TOOLCHAIN_FILES.iter().map(|(name, _)| *name).collect();
    for header in ["flat.h", "borland.h", "floatm.h", "watcom.h"] {
        assert!(names.contains(&header), "{header} missing from the embedded toolchain: {names:?}");
    }
    assert!(TOOLCHAIN_FILES.iter().all(|(_, bytes)| !bytes.is_empty()));
}

/// An m16 compile on a box with only `LLRM_PIN_CLANG` died with "clang's
/// datalayout `e-m:e-p:32:32...` lays types out differently" and no word of the
/// variable that picks the fork's clang. The message names it when it is unset,
/// and only then.
#[test]
fn a_layout_mismatch_names_the_variable_of_the_forks_clang_when_it_is_unset() {
    let table = targets(TARGETS).unwrap();
    let row = &table["x86_16-dos"];
    let name = row.clang_env.as_deref().unwrap();
    let unset = layout_mismatch("f.c", "e-m:e-p:32:32", row, |_| None);
    assert!(unset.contains(name), "{unset}");
    let set = layout_mismatch("f.c", "e-m:e-p:32:32", row, |_| Some("clang".into()));
    assert!(!set.contains(name), "{set}");
}

/// Every `-nostdinc` compile spawned `clang -print-resource-dir` (~1 process
/// per compile) although the dir follows from the binary's path.
#[test]
fn resource_dir_comes_from_the_clang_path_without_a_spawn() {
    let root = std::env::temp_dir().join(format!("llrm-cc-resource-{}", std::process::id()));
    std::fs::create_dir_all(root.join("bin")).unwrap();
    std::fs::create_dir_all(root.join("lib/clang/24/include")).unwrap();
    std::fs::write(root.join("bin/clang"), "").unwrap();
    let spawns = std::cell::Cell::new(0);
    let found = resource_include(&root.join("bin/clang"), || {
        spawns.set(spawns.get() + 1);
        None
    });
    assert_eq!((found, spawns.get()), (Some(root.canonicalize().unwrap().join("lib/clang/24")), 0));
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn resource_dir_spawns_only_when_the_layout_is_missing() {
    let root = std::env::temp_dir().join(format!("llrm-cc-resource-fallback-{}", std::process::id()));
    std::fs::create_dir_all(root.join("bin")).unwrap();
    std::fs::write(root.join("bin/clang"), "").unwrap();
    let found = resource_include(&root.join("bin/clang"), || Some(PathBuf::from("/spawned")));
    assert_eq!(found, Some(PathBuf::from("/spawned")));
    std::fs::remove_dir_all(&root).unwrap();
}

/// Several versions under `lib/clang` leave the pick to clang (the spawn); a
/// symlinked `clang` resolves to its real install, as clang itself does.
#[test]
fn resource_dir_defers_to_clang_when_ambiguous_and_follows_symlinks() {
    let root = std::env::temp_dir().join(format!("llrm-cc-resource-multi-{}", std::process::id()));
    std::fs::create_dir_all(root.join("real/bin")).unwrap();
    std::fs::create_dir_all(root.join("link")).unwrap();
    std::fs::create_dir_all(root.join("real/lib/clang/23/include")).unwrap();
    std::fs::write(root.join("real/bin/clang"), "").unwrap();
    std::os::unix::fs::symlink(root.join("real/bin/clang"), root.join("link/clang")).unwrap();
    let one = resource_include(&root.join("link/clang"), || None);
    assert_eq!(one, Some(root.canonicalize().unwrap().join("real/lib/clang/23")));
    std::fs::create_dir_all(root.join("real/lib/clang/24/include")).unwrap();
    let many = resource_include(&root.join("link/clang"), || Some(PathBuf::from("/spawned")));
    assert_eq!(many, Some(PathBuf::from("/spawned")));
    std::fs::remove_dir_all(&root).unwrap();
}

/// A 6502 program linked from one C source kept every external function
/// nothing calls: float_return was 5945 B against llvm-mos's 90 (its linker
/// drops them). The sweep is the default of that link only: not of `-S` or
/// `-c` (other units may name the globals), of several sources, nor of a row
/// whose backend lacks it.
#[test]
fn one_source_linked_for_the_6502_sweeps_what_main_does_not_reach() {
    let table = targets(TARGETS).unwrap();
    let args = |name: &str, line: &str| llc_args(&table[name], &built(line), Path::new("x.ll"), Path::new("x.o"));
    let on = "-O2 x.c -o x";
    for name in ["6502-cc65", "65c02-cc65"] {
        assert!(args(name, on).contains(&"-fwhole-program".to_owned()), "{name}");
        for line in
            ["-O2 -S x.c", "-O2 -c x.c", "-O2 x.c y.c -o x", "-O2 x.c y.o -o x", "-O2 -fno-whole-program x.c -o x"]
        {
            assert!(!args(name, line).contains(&"-fwhole-program".to_owned()), "{name}: {line}");
        }
    }
    // identity elsewhere: the same command line on every other row, bit for bit
    for (name, row) in table.iter().filter(|(name, _)| !name.contains("cc65")) {
        assert!(!row.whole_program, "{name}");
        assert!(!args(name, on).contains(&"-fwhole-program".to_owned()), "{name}");
    }
}
