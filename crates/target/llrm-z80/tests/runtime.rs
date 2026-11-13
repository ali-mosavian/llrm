//! The runtime (`runtime/z80_rt.s`) assembled with sdasz80 and run on
//! ucsim_z80: every routine against what C computes. A routine that is wrong
//! for some input (`__modhi3` returned `-(a % b)` for a positive dividend: its
//! sign test read stale flags) fails here with the input, not in a corpus row.

use std::process::Command;

fn have(tool: &str) -> bool {
    Command::new(tool).arg("-v").output().is_ok()
        || Command::new("which").arg(tool).output().is_ok_and(|o| o.status.success())
}

#[test]
fn the_runtime_computes_what_c_computes() {
    let missing: Vec<_> = ["sdasz80", "sdldz80", "ucsim_z80"].into_iter().filter(|tool| !have(tool)).collect();
    if !missing.is_empty() {
        let optional = std::env::var("Z80_ORACLE_OPTIONAL").is_ok_and(|flag| flag == "1");
        assert!(optional, "{missing:?} not found: the runtime cannot be checked (Z80_ORACLE_OPTIONAL=1 skips it)");
        return;
    }
    let dir = env!("CARGO_MANIFEST_DIR");
    let done = Command::new("python3")
        .args(["-I", &format!("{dir}/tests/runtime/check.py"), &format!("{dir}/runtime/z80_rt.s")])
        .output()
        .expect("python3 runs");
    let text = String::from_utf8_lossy(&done.stdout);
    assert!(done.status.success() && text.contains("ALL OK"), "{text}\n{}", String::from_utf8_lossy(&done.stderr));
}

/// `strlen` is `cpir` over a count that starts at 0: the empty string must be
/// 0 (the count wraps to 0xFFFF) and 299 bytes must not stop at 8 bits.
#[test]
fn strlen_counts_what_c_counts() {
    let missing: Vec<_> = ["sdasz80", "sdcc", "ucsim_z80"].into_iter().filter(|tool| !have(tool)).collect();
    if !missing.is_empty() {
        let optional = std::env::var("Z80_ORACLE_OPTIONAL").is_ok_and(|flag| flag == "1");
        assert!(optional, "{missing:?} not found: strlen cannot be checked (Z80_ORACLE_OPTIONAL=1 skips it)");
        return;
    }
    let dir = env!("CARGO_MANIFEST_DIR");
    let done = Command::new("python3")
        .args(["-I", &format!("{dir}/tests/runtime/strlen.py"), &format!("{dir}/runtime/z80_rt.s")])
        .output()
        .expect("python3 runs");
    let text = String::from_utf8_lossy(&done.stdout);
    assert!(done.status.success() && text.contains("ALL OK"), "{text}\n{}", String::from_utf8_lossy(&done.stderr));
}

/// `__udivhi3` called `__umodhi3` and swapped the pair back: 55 B against
/// SDCC's 55, a tie on bytes. The divide rows must stay wins on both bytes and
/// cycles.
#[test]
fn the_divide_routines_beat_sdcc_on_bytes_and_cycles() {
    if !["sdasz80", "sdcc", "ucsim_z80"].into_iter().all(have) {
        assert!(std::env::var("Z80_ORACLE_OPTIONAL").is_ok_and(|flag| flag == "1"), "sdcc tools not found");
        return;
    }
    let dir = env!("CARGO_MANIFEST_DIR");
    let done = Command::new("python3")
        .args(["-I", &format!("{dir}/tests/runtime/vs_sdcc.py"), &format!("{dir}/runtime/z80_rt.s")])
        .output()
        .expect("python3 runs");
    let text = String::from_utf8_lossy(&done.stdout);
    assert!(done.status.success(), "{text}\n{}", String::from_utf8_lossy(&done.stderr));
    for row in ["udiv8", "umod8", "sdiv8", "smod8", "udiv16", "umod16", "sdiv16", "smod16"] {
        let line = text.lines().find(|line| line.starts_with(row)).unwrap_or_else(|| panic!("no row {row}:\n{text}"));
        assert!(line.ends_with("bytes WIN, T WIN"), "{line}\n{text}");
    }
}

/// `__umodsi3` of a 22-bit dividend by a 17-bit divisor (loop_interchange's
/// `% 100003`) took 13.6k T: the remainder and the quotient both lived in the
/// frame and all 32 steps ran; with the remainder in registers and the leading
/// zeros skipped it was 8.1k. SDCC aligns the divisor under the dividend and
/// takes one step per quotient bit (3.1k here); so does this: 2.5k.
#[test]
fn a_long_remainder_takes_one_step_per_quotient_bit() {
    if !["sdasz80", "ucsim_z80"].into_iter().all(have) {
        assert!(std::env::var("Z80_ORACLE_OPTIONAL").is_ok_and(|flag| flag == "1"), "sdcc tools not found");
        return;
    }
    let dir = env!("CARGO_MANIFEST_DIR");
    let done = Command::new("python3")
        .args(["-I", &format!("{dir}/tests/runtime/divlong_clocks.py"), &format!("{dir}/runtime/z80_rt.s"), "3000"])
        .output()
        .expect("python3 runs");
    let text = String::from_utf8_lossy(&done.stdout);
    assert!(done.status.success() && text.contains("ALL OK"), "{text}\n{}", String::from_utf8_lossy(&done.stderr));
}

/// The runtime had no `memcpy`, `memmove` or `memset`: 7 programs of
/// tests/run/c linked to nothing. Counts of 0, 1 and 300, overlap both ways and
/// the result register against C's meaning.
#[test]
fn the_memory_routines_move_what_c_moves() {
    let missing: Vec<_> = ["sdasz80", "sdldz80", "ucsim_z80"].into_iter().filter(|tool| !have(tool)).collect();
    if !missing.is_empty() {
        let optional = std::env::var("Z80_ORACLE_OPTIONAL").is_ok_and(|flag| flag == "1");
        assert!(
            optional,
            "{missing:?} not found: the memory routines cannot be checked (Z80_ORACLE_OPTIONAL=1 skips it)"
        );
        return;
    }
    let dir = env!("CARGO_MANIFEST_DIR");
    let done = Command::new("python3")
        .args(["-I", &format!("{dir}/tests/runtime/memops.py"), &format!("{dir}/runtime/z80_rt.s")])
        .output()
        .expect("python3 runs");
    let text = String::from_utf8_lossy(&done.stdout);
    assert!(done.status.success() && text.contains("ALL OK"), "{text}\n{}", String::from_utf8_lossy(&done.stderr));
}

/// The runtime exported `_strlen`, `_memcpy`, `_memmove` and `_memset`: a
/// program defining its own `strlen` failed to link (2 programs of
/// tests/run/c). Its exports are `__` names, which C reserves.
#[test]
fn the_runtime_exports_no_name_a_program_may_define() {
    let clashing: Vec<_> = llrm_z80::RUNTIME
        .lines()
        .filter_map(|line| line.trim().strip_prefix(".globl"))
        .flat_map(|names| names.split(','))
        .map(str::trim)
        .filter(|name| name.starts_with('_') && !name.starts_with("__"))
        .collect();
    assert!(clashing.is_empty(), "{clashing:?} are C library names a program may define");
}

/// The programs harness left a `z80prog-*` tree in the temporary directory
/// per run: /tmp is a tmpfs with a fixed inode count, and the trees helped
/// fill it until the landing runner stopped (PREFLIGHT: /tmp has 7% free).
#[test]
fn the_programs_harness_leaves_no_temporary_tree_behind() {
    if !have("sdasz80") {
        return;
    }
    let dir = env!("CARGO_MANIFEST_DIR");
    let tmp = tempfile::tempdir().unwrap();
    let scratch = tmp.path().join(format!("z80-harness-tmp-{}", std::process::id()));
    std::fs::create_dir_all(scratch.join("src")).expect("a scratch directory");
    // Another process's tree in the shared temporary directory is not this run's: the harness works in `scratch`.
    // (`tempfile::env`, not the std call: `tmp_paths_per_call` bans scratch files there, and this is a decoy, not scratch.)
    let stray = tempfile::env::temp_dir().join(format!("z80prog-stray-{}", std::process::id()));
    std::fs::create_dir_all(&stray).expect("a stray tree");
    // `--llc`: run.py's default is a build in one person's scratch directory, which a clean machine has none of
    // (the test failed there with "cannot ask ... for its runtime stamp") and a stale one tests an old compiler.
    let done = Command::new("python3")
        .args(["-I", &format!("{dir}/tests/programs/run.py"), "--llc", env!("CARGO_BIN_EXE_llrm-z80-llc"), "--src"])
        .arg(scratch.join("src"))
        .env("TMPDIR", &scratch)
        .output()
        .expect("python3 runs");
    std::fs::remove_dir_all(&stray).ok();
    let left: Vec<_> = std::fs::read_dir(&scratch)
        .expect("the scratch directory")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name != "src")
        .collect();
    std::fs::remove_dir_all(&scratch).ok();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    assert!(left.is_empty(), "left behind: {left:?}");
}

/// C programs run on ucsim through the crate's own compiler, against their
/// `.out` files (`tests/programs/c`): a verdict other than PASS fails.
#[test]
fn the_programs_of_this_crate_run_and_print_what_c_does() {
    let clang = std::env::var("LLRM_PIN_CLANG")
        .unwrap_or_else(|_| format!("{}/scratch/llvm-build/bin/clang", std::env::var("HOME").unwrap_or_default()));
    if !have("sdasz80") || !have("sdldz80") || !have("ucsim_z80") || !std::path::Path::new(&clang).exists() {
        return;
    }
    let dir = env!("CARGO_MANIFEST_DIR");
    let done = Command::new("python3")
        .args(["-I", &format!("{dir}/tests/programs/run.py"), "--llc", env!("CARGO_BIN_EXE_llrm-z80-llc")])
        .args(["--clang", &clang, "--src", &format!("{dir}/tests/programs/c"), "--jobs", "2"])
        .output()
        .expect("python3 runs");
    let text = String::from_utf8_lossy(&done.stdout);
    // every program passes: the tally line is `PASS n of n`, with no refusal
    // beside it
    let all = text
        .lines()
        .last()
        .and_then(|last| last.strip_prefix("PASS "))
        .is_some_and(|rest| rest.split_once(" of ").is_some_and(|(passed, total)| passed == total));
    assert!(done.status.success() && all, "{text}");
}

/// `bench/` programs run whole on ucsim through the crate's compiler, so a pass that moves clocks (#9540's phi order:
/// frames +9.2M T for +13 B) is measured by T-states, not by object bytes alone. A bench program that stops linking
/// or running leaves no row, and the whole-program T of every cell with it.
#[test]
fn a_bench_program_gets_a_whole_program_row_of_bytes_and_t_states() {
    let clang = std::env::var("LLRM_PIN_CLANG")
        .unwrap_or_else(|_| format!("{}/scratch/llvm-build/bin/clang", std::env::var("HOME").unwrap_or_default()));
    if !have("sdasz80") || !have("sdldz80") || !have("ucsim_z80") || !std::path::Path::new(&clang).exists() {
        return;
    }
    let dir = env!("CARGO_MANIFEST_DIR");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("rows.tsv");
    let done = Command::new("python3")
        .args(["-I", &format!("{dir}/tests/programs/sweep.py"), env!("CARGO_BIN_EXE_llrm-z80-llc")])
        .arg(&out)
        .args(["--clang", &clang, "--ours-only", "--no-softfloat", "--no-library-checks", "--jobs", "1", "--src", &format!("{dir}/../../../bench/recchop"), "recchop"])
        .env("TMPDIR", tmp.path())
        .output()
        .expect("python3 runs");
    let rows = std::fs::read_to_string(&out).unwrap_or_default();
    let cols: Vec<&str> = rows.lines().next().unwrap_or("").split('\t').collect();
    let measured = cols.len() == 4 && cols[1] == "ours" && cols[2].parse::<u64>().is_ok_and(|b| b > 0) && cols[3].parse::<u64>().is_ok_and(|t| t > 0);
    assert!(measured, "no row: {rows:?}\n{}{}", String::from_utf8_lossy(&done.stdout), String::from_utf8_lossy(&done.stderr));
}

/// The corpus byte counts charge a program for the runtime routines it reaches.
/// The graph missed a fall-through (`__modsi3` into `__udivmodsi4`) and a jump
/// into a local label (`__udivdi3` into `udivmoddi`, under `__moddi3`),
/// so a program with a long `%` was counted without the divide.
#[test]
fn the_runtime_graph_reaches_a_routine_by_falling_or_jumping_into_it() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let done = Command::new("python3")
        .args(["-I", &format!("{dir}/tests/runtime/graph.py"), &format!("{dir}/runtime/z80_rt.s")])
        .output()
        .expect("python3 runs");
    let text = String::from_utf8_lossy(&done.stdout);
    assert!(done.status.success() && text.contains("ALL OK"), "{text}\n{}", String::from_utf8_lossy(&done.stderr));
}
