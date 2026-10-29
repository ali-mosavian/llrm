//! The translated greedy allocator writes its spills as it goes
//! (`InlineSpiller`).

mod common;

use std::process::Command;

/// Eight values rotated in a loop, each started from a constant: the copy at
/// the back edge reads eight spilled sources at once, a constant read by a copy
/// is a literal in place, and the instructions the spiller rewrites keep the
/// live intervals of every other value (`LLRM_CHECK_LIVEINTERVALS`).
const ROTATE: &str = "void rot(int *p, int n)\n{\n    int a = 1, b = 2, c = 3, d = 4, e = 5, f = 6, g = 7, h = 8;\n    int i;\n\
    for (i = 0; i < n; ++i) {\n        int t = a;\n        a = b + p[0];\n        b = c + p[1];\n        c = d + p[2];\n\
        d = e + p[3];\n        e = f + p[4];\n        f = g + p[5];\n        g = h + p[6];\n        h = t + p[7];\n    }\n\
    p[0] = a; p[1] = b; p[2] = c; p[3] = d; p[4] = e; p[5] = f; p[6] = g; p[7] = h;\n}\n";

/// The inline allocation of `rot` fell back to the rounds at -Os (two spill
/// products of the copy needed a register no one could give), and at -O1 and
/// -O2 left an interval that the instructions rewritten no longer matched. Each
/// level must finish inline, with the intervals equal to ones made afresh.
#[test]
fn a_rotation_of_constants_is_allocated_inline_at_every_level() {
    for level in ["-O1", "-O2", "-Os"] {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(scratch.path().join("a.c"), ROTATE).unwrap();
        let output = common::cc::llrm_c()
            .current_dir(scratch.path())
            .env("LLRM_CHECK_LIVEINTERVALS", "1")
            .env("LLRM_GREEDY_CHECK", "1")
            .env("LLRM_GREEDY_TRACE", "1")
            .args(["-m32", "-march=i486", level, "-S", "-o", "a.s", "a.c"])
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{level}: {said}");
        assert!(said.contains("intervals equal afresh"), "{level}: the check compared nothing: {said}");
        assert!(!said.contains("left for the rounds"), "{level}: {said}");
        assert!(!said.contains("greedy-check"), "{level}: {said}");
    }
}

/// tests/run/c/es_across_copy.c at -O2: spilling a far pointer's offset put an
/// index add before its use, which redefined the base register; the base kept
/// its old interval and `LLRM_CHECK_LIVEINTERVALS` found it stale.
#[test]
fn a_register_an_inline_spill_redefines_gets_its_interval_again() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run/c/es_across_copy.c");
    let scratch = tempfile::tempdir().unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .env("LLRM_CHECK_LIVEINTERVALS", "1")
        .env("LLRM_GREEDY_CHECK", "1")
        .args(["-O2", "-march=i486", "-S", "-o", "a.s"])
        .arg(source)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{said}");
    assert!(!said.contains("greedy-check"), "{said}");
}

/// nbody_fixed at -O3 printed 191828 (want 38421): a spilled `pos_x[i] +=
/// vel_x[i]` kept in its slot was written back to `pos_x[i]` as `pos_x[i]`'s
/// old value, because the one-instruction body the inline spiller hands the
/// spiller showed no definition of the value, so its home cell `pos_x[i]`
/// looked current.
const HOME_UPDATE: &str = "/* Six-body Q9 fixed-point integrator, 1000 steps; long is 32-bit, / truncates toward zero. */\n\
\n\
#define BODIES 6\n\
#define ONE 512L\n\
\n\
static long pos_x[BODIES], pos_y[BODIES], vel_x[BODIES], vel_y[BODIES];\n\
\n\
long bench_nbr(void)\n\
{\n\
    long delta_x, delta_y, dist2, falloff, acc_x, acc_y, sum;\n\
    int body, other, step;\n\
\n\
    for (body = 0; body < BODIES; ++body) {\n\
        pos_x[body] = (body * 7 - 15) * ONE;\n\
        vel_x[body] = 0;\n\
        vel_y[body] = 0;\n\
    }\n\
    for (step = 0; step < 1; ++step) {\n\
        for (body = 0; body < BODIES; ++body) {\n\
            acc_x = 0;\n\
            acc_y = 0;\n\
            for (other = 0; other < BODIES; ++other) {\n\
                if (other != body) {\n\
                    delta_x = pos_x[other] - pos_x[body];\n\
                    delta_y = pos_y[other] - pos_y[body];\n\
                    dist2 = delta_x * delta_x + delta_y * delta_y + ONE * ONE;\n\
                    falloff = ONE / (dist2 / (ONE * ONE) + 1);\n\
                    acc_x += (delta_x * falloff) / ONE;\n\
                }\n\
            }\n\
            vel_x[body] += acc_x;\n\
        }\n\
        for (body = 0; body < BODIES; ++body) {\n\
            pos_x[body] += vel_x[body];\n\
            pos_y[body] += vel_y[body];\n\
        }\n\
    }\n\
    sum = 0;\n\
    for (body = 0; body < BODIES; ++body)\n\
        sum += (body + 1) * (pos_x[body] + 3 * pos_y[body] + 5 * vel_x[body] + 7 * vel_y[body]);\n\
    return sum;\n\
}\n\
";

#[test]
fn a_spilled_update_is_stored_back_from_its_slot_not_its_stale_home() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), HOME_UPDATE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O3", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    for pair in lines.windows(2) {
        let (Some(load), Some(store)) =
            (pair[0].strip_prefix("mov eax, dword ptr "), pair[1].strip_prefix("mov dword ptr "))
        else {
            continue;
        };
        // `pos_x[i]` is updated before its store; `pos_y[i] += 0` may store
        // what it loads.
        assert!(
            !(load.starts_with("_pos_x") && store == format!("{load}, eax")),
            "pos_x stored with its own old value: {} / {}",
            pair[0],
            pair[1]
        );
    }
}

/// tools/test_budgets.py's `nest` axis at 2N (32 loops deep) cost 21 G
/// instructions under `LLRM_GREEDY=1`: the innermost block's frequency
/// overflowed f32 to infinity, every range there weighed infinity (not
/// spillable), 54 ranges found no register, and the function went to the rounds
/// and then to the old allocator.
#[test]
fn a_deep_loop_nest_is_allocated_inline() {
    let depth = 40;
    let counters: Vec<String> = (0..depth).map(|k| format!("i{k}")).collect();
    let mut source = format!("unsigned f(unsigned n) {{\n    unsigned s = 0, {};\n", counters.join(", "));
    for k in 0..depth {
        source += &format!("    for (i{k} = 0; i{k} < n; i{k}++) {{ s += i{k};\n");
    }
    source += &"}".repeat(depth);
    source += "\n    return s;\n}\n";
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .env("LLRM_GREEDY_TRACE", "1")
        .args(["-m32", "-march=i486", "-O2", "-o", "a.o", "a.c"])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{said}");
    assert!(!said.contains("left for the rounds"), "{said}");
}

/// sum_three.bas's SUMTHREE under `LLRM_GREEDY=1` spilled a pointer to the
/// stack (`sub sp, 2`, then `[bp-2]` reloads) to keep three descriptor loads
/// `mov r, [d+2]` in registers before their copies into ES, FS and GS:
/// a load of a cell nothing writes is rematerialized and folded into the copy
/// (`mov es, [d+2]`), so it weighs half and is the one evicted (every BASIC
/// bench row was worse than the old allocator).
///
/// Since LSR became the default (7ee16741c) the loop's induction variables
/// leave two of the pointers spilled (`sub sp, 4`); the pin's llc on the same
/// IR spills three values (`subl $12`), so two slots is the bar.
#[test]
fn a_load_that_can_be_made_again_is_not_kept_in_a_register_at_the_cost_of_a_spill() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/parity/sum_three/sum_three.bas");
    let scratch = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["-O2", "-march=i486", "-S", "-o", "a.s"])
        .arg(source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let body =
        text.split("SUMTHREE proc").nth(1).and_then(|rest| rest.split("SUMTHREE endp").next()).expect("SUMTHREE");
    assert!(!body.contains("sub sp, 6"), "more than two slots for spills: {body}");
    assert!(body.matches("[bp-").count() <= 6, "reloads beyond the pin's: {body}");
}

/// LLVM's order, on by default, allocates with greedy, the allocate slot's
/// translation; before the join it ran the old allocator there.
#[test]
fn the_parity_pipeline_allocates_with_greedy() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), "int f(int a, int b) { return a * b + a; }\n").unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .env("LLRM_GREEDY_TRACE", "1")
        .args(["-m32", "-march=i486", "-O2", "-o", "a.o", "a.c"])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{said}");
    assert!(said.contains("greedy-inline f_"), "{said}");
}
