//! A QB program compiled by llrm-qb for wasm32, linked with the C runtime built
//! by clang and llrm-wasm-llc (runtime/wasm/build.sh) and run in node on the
//! headless device (runtime/wasm/headless.mjs). A machine without
//! clang for wasm32, wasm-ld, node, or a built llrm-wasm-llc fails here by
//! name; it does not skip.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// `cargo build -p llrm-wasm --bin llrm-wasm-llc` puts it next to llrm-qb.
fn llc() -> PathBuf {
    let found = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).with_file_name("llrm-wasm-llc");
    assert!(found.exists(), "qb_wasm32 needs {}: cargo build -p llrm-wasm --bin llrm-wasm-llc", found.display());
    found
}

fn need(tool: &str) {
    let found = Command::new("which").arg(tool).output().is_ok_and(|out| out.status.success());
    assert!(found, "qb_wasm32 needs `{tool}` on the PATH");
}

/// `source` built into a module; the directory it is in.
fn built(
    name: &str,
    source: &str,
) -> PathBuf {
    built_with(name, source, "")
}

/// `built`, with `flags` for llrm-qb.
fn built_with(
    name: &str,
    source: &str,
    flags: &str,
) -> PathBuf {
    for tool in ["clang", "wasm-ld", "node"] {
        need(tool);
    }
    let dir = std::env::temp_dir().join(format!("{name}_wasm32_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bas = dir.join(format!("{name}.bas"));
    std::fs::write(&bas, source).unwrap();
    let made = Command::new(root().join("runtime/wasm/build.sh"))
        .arg(&bas)
        .arg(dir.join(format!("{name}.wasm")))
        .arg(dir.join("obj"))
        .env("LLRM_QB", env!("CARGO_BIN_EXE_llrm-qb"))
        .env("LLRM_QB_FLAGS", flags)
        .env("LLRM_WASM_LLC", llc())
        .output()
        .expect("runtime/wasm/build.sh runs");
    assert!(made.status.success(), "build.sh: {}", String::from_utf8_lossy(&made.stderr));
    dir
}

/// What the harness printed and its exit status, standard error included.
fn run_raw(
    dir: &Path,
    name: &str,
    flags: &[&str],
) -> std::process::Output {
    // A program that spins without asking the host anything is never stopped by
    // the harness's own limit.
    Command::new("timeout")
        .args(["-k", "5", "60", "node"])
        .arg(root().join("runtime/wasm/headless.mjs"))
        .arg(dir.join(format!("{name}.wasm")))
        .args(["--limit-ms", "30000", "--out"])
        .arg(dir.join("frames"))
        .args(flags)
        .current_dir(dir)
        .output()
        .expect("node runs")
}

/// The text screen the program left, and its exit code.
fn run(
    dir: &Path,
    name: &str,
    flags: &[&str],
) -> (String, Option<i32>) {
    let ran = run_raw(dir, name, flags);
    (String::from_utf8_lossy(&ran.stdout).into_owned(), ran.status.code())
}

/// `PRINT "hello"`: the first QB program to run on wasm32. Before `--wasm32`
/// the compiler had no route to the wasm backend, and its irreducible loops,
/// variadic runtime entries and `unreachable` after a runtime error did not
/// compile or ran off the end of a function.
#[test]
fn print_hello_runs_on_wasm32() {
    let dir = built("hello", "PRINT \"hello\"\nEND\n");
    let (text, code) = run(&dir, "hello", &[]);
    assert_eq!(text.trim_end(), "hello");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// OPEN, PRINT#, LINE INPUT#, NAME and KILL: the OS layer denied every file
/// (and left `llrm_os_open` and fifteen more unresolved), so grep, jpeg, stft,
/// fieldbuf and open_modes could not instantiate.
#[test]
fn files_are_written_read_renamed_and_killed() {
    let dir = built(
        "files",
        "OPEN \"t.dat\" FOR OUTPUT AS #1\nPRINT #1, \"hello\"; 42\nCLOSE #1\n\
         NAME \"t.dat\" AS \"u.dat\"\nOPEN \"u.dat\" FOR INPUT AS #1\nLINE INPUT #1, a$\nCLOSE #1\n\
         PRINT a$\nOPEN \"v.dat\" FOR OUTPUT AS #2\nPRINT #2, 1\nCLOSE #2\nKILL \"u.dat\"\nEND\n",
    );
    let (text, code) = run(&dir, "files", &[]);
    assert_eq!(text.trim_end(), "hello 42");
    assert_eq!(code, Some(0));
    assert!(!dir.join("u.dat").exists() && !dir.join("t.dat").exists(), "KILL and NAME left files");
    assert_eq!(std::fs::read_to_string(dir.join("v.dat")).unwrap(), " 1 \r\n");
    std::fs::remove_dir_all(dir).ok();
}

/// COMMAND$ is the arguments after the program's name, a blank apart, upper
/// case; the harness gives them with `--arg`.
#[test]
fn the_command_line_is_the_harness_arguments() {
    let dir = built("cmdline", "PRINT COMMAND$\nEND\n");
    let (text, code) = run(&dir, "cmdline", &["--arg", "ab", "--arg", "cd"]);
    assert_eq!(text.trim_end(), "AB CD");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// DIM and COLOR call the runtime's block entries (B$DDIM, B$COLR), C varargs
/// on a host that take the block's words in the order BASIC pushes them. Built
/// as for DOS they were called in the Microsoft order and the first DIM died
/// with "Subscript out of range".
#[test]
fn block_entries_are_called_as_a_host_calls_them() {
    let dir = built("dims", "DIM a(1 TO 2)\na(2) = 5\nCOLOR 4\nPRINT a(2)\nEND\n");
    let (text, code) = run(&dir, "dims", &[]);
    assert_eq!(text.trim_end(), " 5");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// PLAY waits for the clock tick to take its notes off the queue. `music_tick`
/// was a weak no-op in tick.c, bound locally by the wasm object writer, so
/// play.c's never ran: the queue never drained and PLAY spun until the time
/// limit (Gorillas hung on its first throw).
#[test]
fn play_finishes_on_wasm32() {
    let dir = built("play", "PLAY \"T255 L64 CDEFG\"\nPRINT \"played\"\nEND\n");
    let (text, code) = run(&dir, "play", &[]);
    assert_eq!(text.trim_end(), "played");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// The harness wrote its frame colours and typed keys at `__heap_base - 1024`
/// and `- 64`: the top of the program's stack, under its outermost frames. A
/// frame taken while a program waited for a key corrupted its locals (Gorillas
/// lost the player names on the next turn and later trapped in MAKECITYSCAPE).
#[test]
fn a_frame_and_a_key_leave_the_programs_locals_alone() {
    // A SUB with 120 string temporaries: a frame of about a kilobyte under the
    // program's entry, the range written.
    let temps: String =
        (1..=120).map(|n| format!("  t{n}$ = LEFT$(keep$, {}) + a$ + RIGHT$(keep$, 2)\n", 1 + n % 5)).collect();
    let source = format!(
        "DECLARE SUB Echo (a$)\nEcho \"alpha\"\nEND\nSUB Echo (a$)\n  keep$ = a$ + \"!\"\n  INPUT \"key\"; k$\n{temps}  PRINT keep$; LEFT$(t120$, 3)\nEND SUB\n"
    );
    let dir = built("locals", &source);
    let (text, code) = run(&dir, "locals", &["--dump-at", "300=mid", "--keys", "x\\r@600"]);
    assert!(text.trim_end().ends_with("alpha!aal"), "{text:?}");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// ON ERROR cannot land on wasm32 (no unwinding): an error that reaches a
/// handler stops the program with a named failure, not by running on past it or
/// by a bare trap. Gorillas' SCREEN 9 and PALETTE probes raise nothing, so it
/// never lands.
#[test]
fn an_error_that_reaches_a_handler_stops_with_a_named_failure() {
    let dir =
        built("onerror", "ON ERROR GOTO Handler\nERROR 5\nPRINT \"went on\"\nEND\nHandler:\nPRINT \"handled\"\nEND\n");
    let ran = run_raw(&dir, "onerror", &[]);
    let (out, err) = (String::from_utf8_lossy(&ran.stdout), String::from_utf8_lossy(&ran.stderr));
    assert!(!out.contains("went on") && !out.contains("handled"), "{out:?}");
    assert!(err.contains("ON ERROR landed: longjmp is not supported on wasm32"), "{err}");
    assert_ne!(ran.status.code(), Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// Unbounded recursion under `-fsanitize=stack`: the text handed to the wasm
/// backend carried the attribute twice and the reader refused it, so the
/// program did not build; and once built, the engine's own stack overflow
/// ended the harness with a JS RangeError instead of QB's "Out of stack
/// space".
#[test]
fn unbounded_recursion_ends_with_out_of_stack_space() {
    let dir = built_with(
        "deep",
        "DECLARE FUNCTION Deep% (n AS INTEGER)\nPRINT \"start\"\nPRINT Deep%(30000)\nPRINT \"unreachable\"\n\
         FUNCTION Deep% (n AS INTEGER)\n    Deep% = Deep%(n + 1) + 1\nEND FUNCTION\n",
        "-O2 -fsanitize=stack",
    );
    let (text, code) = run(&dir, "deep", &[]);
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("start"));
    assert!(lines.next().is_some_and(|line| line.starts_with("Out of stack space in line")), "{text}");
    assert_eq!(code, Some(255));
    std::fs::remove_dir_all(dir).ok();
}

/// The host sounds what PLAY plays, from the tones the module hands its host
/// (`llrm_host_tone`, a wrap of `dev_tone`). Without the wrap the host heard
/// nothing. The sequence is the x86-32 host build's own for this tune.
#[test]
fn play_hands_its_tones_to_the_host() {
    let dir = built("tune", "PLAY \"T120 L16 CDE\"\nPRINT \"played\"\nEND\n");
    let (text, code) = run(&dir, "tune", &[]);
    assert_eq!(text.trim_end(), "played");
    assert_eq!(code, Some(0));
    let log = std::fs::read_to_string(dir.join("frames/tones.json")).expect("the host heard no tone");
    let hertz: Vec<u32> =
        log.split("\"hertz\":").skip(1).map(|rest| rest.split(['}', ',']).next().unwrap().parse().unwrap()).collect();
    assert_eq!(hertz, [524, 0, 588, 0, 0]);
    std::fs::remove_dir_all(dir).ok();
}

/// The MID$ statement has a C runtime entry (`B$SMID`): its call was left
/// unresolved on every flat target, so a module using it did not instantiate
/// (jpeg, stft). Over a variable string, a fixed one, and from an
/// overlapping source (rt/mid.asm copies forward, a byte at a time: the first
/// byte spreads).
#[test]
fn the_mid_statement_overwrites_in_place() {
    let dir = built(
        "midstmt",
        "a$ = \"hello world\"\nMID$(a$, 7, 3) = \"WORLD\"\nMID$(a$, 1) = \"J\"\nDIM f AS STRING * 5\nf = \"abcde\"\n\
         MID$(f, 4, 9) = \"XYZ\"\nb$ = \"abcdef\"\nMID$(b$, 2) = b$\nPRINT a$; \"|\"; f; \"|\"; b$\nEND\n",
    );
    let (text, code) = run(&dir, "midstmt", &[]);
    assert_eq!(text.trim_end(), "Jello WORld|abcXY|aaaaaa");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// `READ` into a `STRING * 5` called B$RDSD, which wrote a descriptor over the
/// fixed buffer (garbage on screen); a fixed string is read by B$RDFS now.
#[test]
fn read_fills_a_fixed_length_string_padded_and_cut() {
    let dir = built(
        "fixed",
        "DIM s AS STRING * 5\nDIM t AS STRING * 2\nREAD s, t\nPRINT \"[\"; s; \"][\"; t; \"]\"\nDATA ab, xyz\nEND\n",
    );
    let (text, code) = run(&dir, "fixed", &[]);
    assert_eq!(text.trim_end(), "[ab   ][xy]");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// EXP, LOG and `^` compile to calls of `exp`, `log` and `log2`/`pow`; the host
/// supplied only sin, cos, atan and lrint, so the module did not instantiate
/// ("function import requires a callable": data_after_checked, log_exp_digits,
/// range_power).
#[test]
fn exp_log_and_power_run_on_wasm32() {
    let dir =
        built("mathfns", "DIM x AS DOUBLE\nx = 3\nPRINT EXP(x - 3); LOG(x - 2); 2# ^ x; x ^ 2#; SQR(x + 1)\nEND\n");
    let (text, code) = run(&dir, "mathfns", &[]);
    assert_eq!(text.trim_end(), " 1  0  8  9  2");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// `^` was the x87 sequence (`log2`, a multiply, `exp2`) on wasm32, which has
/// no extended float: the product lost its low bits. It is a call of the
/// runtime's `pow` now, with the errors of B$POW4: 5 for 0 to a negative power
/// and a negative base with a fractional exponent, 6 past 2^32767.5, none for a
/// result only past a DOUBLE (range_power).
#[test]
fn power_is_a_library_call_that_raises_the_errors_of_b_pow4() {
    for (name, body, message) in [
        ("powzero", "x# = 0: y# = -1: PRINT x# ^ y#", "Illegal function call"),
        ("powroot", "x# = -8: y# = 1 / 3: PRINT x# ^ y#", "Illegal function call"),
        ("powbig", "x# = 2: y# = 32768: PRINT x# ^ y#", "Overflow"),
    ] {
        let dir = built(name, &format!("{body}\nEND\n"));
        let (text, code) = run(&dir, name, &[]);
        assert!(text.starts_with(message), "{name}: {text:?}");
        assert_eq!(code, Some(255), "{name}");
        std::fs::remove_dir_all(dir).ok();
    }
    let dir = built("powdouble", "x# = 10: y# = 500: z# = x# ^ y#\nPRINT \"ok\"\nEND\n");
    let (text, code) = run(&dir, "powdouble", &[]);
    assert_eq!((text.trim_end(), code), ("ok", Some(0)));
    std::fs::remove_dir_all(dir).ok();
    let dir = built("powsign", "x# = -2: y# = 3: z# = 0: PRINT x# ^ y#; z# ^ y#; z# ^ z#; 2# ^ -y#\nEND\n");
    let (text, code) = run(&dir, "powsign", &[]);
    assert_eq!(text.trim_end(), "-8  0  1  .125");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// The words of `text`, each a number where it reads as one: `D` is QB's
/// exponent letter for a DOUBLE.
fn words(text: &str) -> Vec<(String, Option<f64>)> {
    text.split_whitespace().map(|word| (word.to_owned(), word.replace('D', "E").parse().ok())).collect()
}

/// LIMIT W17: a target without an extended float computes `^` in doubles, so
/// its digits differ from the x87 programs' in the last place. The four
/// programs below printed those digits and failed on them; they stay within
/// `RELATIVE_BOUND` (measured: 6.5e-16, three ulp, printing included) of the
/// x87 outputs, the numbers compared and not the text, and `^` is the
/// library call, not the sequence.
#[test]
fn power_programs_stay_within_a_few_ulps_of_the_x87_digits() {
    const RELATIVE_BOUND: f64 = 1e-15;
    for name in ["range_power_digits", "pow_unrolled", "pow_stack_depth", "log_exp_digits"] {
        let source_path = root().join(format!("tests/run/qb/{name}.bas"));
        let source = std::fs::read_to_string(&source_path).unwrap();
        // The first alternative of `' flags:`, without the x86 options.
        let flags = source
            .lines()
            .find_map(|line| line.strip_prefix("' flags:"))
            .map(|line| line.split('|').next().unwrap().split_whitespace().collect::<Vec<_>>().join(" "))
            .unwrap_or_default();
        let dir = built_with(name, &source, &flags);
        let (text, code) = run(&dir, name, &[]);
        assert_eq!(code, Some(0), "{name}: {text}");
        let expected = std::fs::read_to_string(source_path.with_extension("out")).unwrap();
        // The harness keeps the last rows of the screen.
        let kept: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
        let rows: Vec<&str> = expected.lines().filter(|line| !line.trim().is_empty()).collect();
        let tail = &rows[rows.len() - kept.len()..];
        let (got, want) = (words(&kept.join(" ")), words(&tail.join(" ")));
        assert_eq!(got.len(), want.len(), "{name}:\n{text}");
        for ((got_text, got), (want_text, want)) in got.iter().zip(&want) {
            match (got, want) {
                (Some(got), Some(want)) => {
                    assert!((got - want).abs() <= RELATIVE_BOUND * want.abs(), "{name}: {got} for {want}")
                }
                _ => assert_eq!(got_text, want_text, "{name}"),
            }
        }
        let ir = dir.join("module.ll");
        let made = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
            .arg(&source_path)
            .args(["--wasm32", "-o"])
            .arg(dir.join("module.o"))
            .args(flags.split_whitespace())
            .env("LLRM_WASM32_TEXT", &ir)
            .output()
            .expect("llrm-qb runs");
        assert!(made.status.success(), "{name}: {}", String::from_utf8_lossy(&made.stderr));
        let text = std::fs::read_to_string(&ir).unwrap();
        if name != "log_exp_digits" {
            assert!(text.contains("@B_LPOW(") && !text.contains("llvm.exp2"), "{name}: the sequence, not the call");
        }
        std::fs::remove_dir_all(dir).ok();
    }
}

/// CURRENCY as text (currency_text_pds71) did not instantiate: B$PECY, B$PSCY,
/// B$STCY, B$RDCY, B$FMKC and B$FCVC had no flat definition.
#[test]
fn currency_text_matches_the_microsoft_output_on_wasm32() {
    let source = std::fs::read_to_string(root().join("tests/run/qb/currency_text_pds71.bas")).unwrap();
    let dir = built("curtext", &source);
    let (text, code) =
        run(&dir, "curtext", &["--script", "12.5\\r-0.0001, 922337203685477.5807\\r3\\r", "--gap", "30"]);
    let want = std::fs::read_to_string(root().join("tests/run/qb/currency_text_pds71.out")).unwrap();
    let trimmed = |text: &str| text.lines().map(|line| line.trim_end().to_owned()).collect::<Vec<_>>();
    assert_eq!(trimmed(&text), trimmed(&want));
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// ON ERROR lands on wasm32 with exception handling: the runtime throws and the
/// function's `catch_all` takes it. With the `setjmp` stub the program ended at
/// the first error with "ON ERROR landed: longjmp is not supported". The pad
/// branches back into the function (RESUME NEXT, RESUME 0), which a `catch`
/// cannot do, so the function runs inside a dispatch loop.
#[test]
fn on_error_lands_with_wasm_exceptions() {
    let source = "ON ERROR GOTO h\nPRINT \"a\"\nERROR 5\nPRINT \"b\"\nERROR 6\nPRINT \"c\"\nEND\n\
        h:\nPRINT \"err\"; ERR\nRESUME NEXT\n";
    let dir = built_with("eh_land", source, "--wasm-exceptions");
    let (text, code) = run(&dir, "eh_land", &[]);
    assert_eq!(text.split_whitespace().collect::<Vec<_>>(), ["a", "err", "5", "b", "err", "6", "c"], "{text}");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// A currency read from text is exact (a double holds 16 digits, not 19), the
/// fifth place rounds half away from zero, and a magnitude past the type is
/// Overflow.
#[test]
fn currency_reads_exactly_and_overflows_past_the_type_on_wasm32() {
    let dir = built(
        "cyexact",
        "DIM c AS CURRENCY, d AS CURRENCY, e AS CURRENCY\nREAD c, d, e\nPRINT c; d; e\nDATA 922337203685477.5807, -922337203685477.5808, 1.00005\nEND\n",
    );
    let (text, code) = run(&dir, "cyexact", &[]);
    assert_eq!(
        text.split_whitespace().collect::<Vec<_>>(),
        ["922337203685477.5807", "-922337203685477.5808", "1.0001"]
    );
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
    let dir = built("cyover", "DIM c AS CURRENCY\nREAD c\nPRINT c\nDATA 922337203685477.5808\nEND\n");
    let (text, code) = run(&dir, "cyover", &[]);
    assert!(text.starts_with("Overflow"), "{text:?}");
    assert_eq!(code, Some(255));
    std::fs::remove_dir_all(dir).ok();
}

/// DATESERIAL did not instantiate (B$DTSR). A month or day out of range
/// carries, and a year of 0 to 99 is 19xx.
#[test]
fn dateserial_counts_days_from_1899_12_30_and_carries_on_wasm32() {
    let dir = built(
        "dateser",
        "' dialect: vbdos\nPRINT DATESERIAL(1992, 2, 29); DATESERIAL(1992, 14, 1); DATESERIAL(1992, 3, 0); DATESERIAL(95, 1, 1); DATESERIAL(1900, 1, 1)\nEND\n",
    );
    let (text, code) = run(&dir, "dateser", &[]);
    assert_eq!(text.split_whitespace().collect::<Vec<_>>(), ["33663", "34001", "33663", "34700", "2"]);
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// ON LOCAL ERROR did not instantiate (B$OEGP). A handler that is never needed
/// is registered and the program runs on.
#[test]
fn on_local_error_registers_and_the_program_runs_on_wasm32() {
    let dir = built(
        "localreg",
        "' dialect: pds71\nDECLARE SUB top ()\ntop\nPRINT \"done\"\nEND\nSUB top\nON LOCAL ERROR GOTO th\nPRINT \"in\"\nEXIT SUB\nth:\nRESUME NEXT\nEND SUB\n",
    );
    let (text, code) = run(&dir, "localreg", &[]);
    assert_eq!(text.split_whitespace().collect::<Vec<_>>(), ["in", "done"]);
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// A procedure's ON LOCAL ERROR handler takes the error and resumes: the
/// ownf_local and resumelocals_local programs did not link (B$OEGP) and then
/// ended at the first error.
#[test]
fn on_local_error_lands_in_its_handler_on_wasm32() {
    let dir = built_with(
        "locland",
        "' dialect: pds71\nDECLARE SUB top ()\ntop\nPRINT \"done\"\nEND\nSUB top\nON LOCAL ERROR GOTO th\nPRINT \"in\"\nERROR 5\nPRINT \"after\"\nEXIT SUB\nth:\nPRINT \"err\"; ERR\nRESUME NEXT\nEND SUB\n",
        "--wasm-exceptions",
    );
    let (text, code) = run(&dir, "locland", &[]);
    assert_eq!(text.split_whitespace().collect::<Vec<_>>(), ["in", "err", "5", "after", "done"], "{text}");
    assert_eq!(code, Some(0));
    std::fs::remove_dir_all(dir).ok();
}

/// A module's `catch_all` catches what an import throws, so the host's
/// endings (`END`, a fatal error) are traps: thrown, `END` inside a function
/// with a handler landed in the handler and the program went on.
#[test]
fn a_host_ending_gets_through_a_catch_all() {
    need("node");
    let probe = root().join("runtime/wasm/probe_end.mjs");
    let out = Command::new("node").arg(&probe).output().expect("node");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "escaped", "{}", String::from_utf8_lossy(&out.stderr));
}
