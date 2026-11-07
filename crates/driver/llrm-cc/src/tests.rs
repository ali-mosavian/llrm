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
/// against gcc's 16k (+151k clk). The m32 rows take `-funroll-loops` at -O1,
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
    let (clang, llc) = flags("x86_64-linux", "-O1 -c x.c");
    assert!(
        !clang.iter().any(|f| f == "-funroll-loops") && !llc.iter().any(|f| f == "-funroll-loops"),
        "m64 as before"
    );
    let (clang, _) = flags("x86_16-dos", "-O1 -c x.c");
    assert!(!clang.iter().any(|f| f == "-funroll-loops"), "m16 as before");
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
