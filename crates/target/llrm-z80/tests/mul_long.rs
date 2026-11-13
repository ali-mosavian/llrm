//! A `mul i32 %n, 3` at -O2 was refused ("e has no byte halves"): #7893 named
//! a spilled byte by its lane in the wider register, and z80's `e` is not a
//! half of a register the allocator may take for it. The lane naming is the
//! target's to ask for (`spills_named_by_lane`); z80 numbers the value.

#[test]
fn a_long_multiply_by_a_constant_compiles_at_o2() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join(format!("llrm-z80-mul-long-{}.s", std::process::id()));
    let done = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), &format!("{dir}/tests/data/mul_long_by_three.ll")])
        .output()
        .expect("llrm-z80-llc runs");
    std::fs::remove_file(&out).ok();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}

/// `x * 3` on a long expanded through byte lanes: 49 instructions (about
/// 140 B) where `x << 1` and `x +` are two `add hl` / `adc hl` pairs.
/// register_upper_half `n - descend(n-1) + 2*n` was 164 instructions against
/// SDCC's 118.
#[test]
fn a_long_times_three_is_shifts_and_adds() {
    use llrm_z80::compile::text;
    use llrm_z80::driver::{Level, Request};
    let asm = text(include_str!("data/mul_long_by_three.ll"), "m3", 0, &Request::new(Level::O2)).expect("compiles");
    let count = asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count();
    assert!(count <= 30 && asm.contains("adc hl"), "{count} instructions:\n{asm}");
}

/// #7940 decomposed `x * 2^n +- 1` before the size routine could be chosen: in
/// a module with two long multiplies a function tuned for size
/// called `__mulsi3` (14 B a site), and the shifts and adds grew 12 Os cells
/// (mul_constants +227 B, int64_division
/// +96, call_keeps +153 against 7cfc6eabbf~1).
#[test]
fn a_size_function_in_a_module_of_long_multiplies_calls_the_routine_for_x_times_three() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
              define i32 @m(i32 %n, i32 %k, i32 %p, i32 %q) optsize {\n %r = mul i32 %n, 3\n %s = mul i32 %k, 5\n %u = mul i32 %p, 7\n %v = mul i32 %q, 9\n\
              %t = add i32 %r, %s\n %w = add i32 %u, %v\n %x = add i32 %t, %w\n ret i32 %x\n}";
    let asm = llrm_z80::compile::text(ir, "m", 0, &llrm_z80::driver::Request::new(llrm_z80::driver::Level::Os))
        .expect("compiles");
    assert_eq!(asm.matches("call __mulsi3").count(), 4, "{asm}");
}

/// #7940 decomposed `x * 3` for a zero-extended byte too: 21 instructions where
/// the byte-lane multiply of a value whose three high bytes are zero is 15
/// (byte_multiply -O2 +36 B,
/// +38,149 T). A zero-extended word keeps the decomposition (23 against 44).
#[test]
fn a_long_times_three_of_a_zero_extended_byte_keeps_the_byte_lane_multiply() {
    let count = |width: u32| {
        let ir = format!(
            "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
             define i32 @f(i{width} %a) {{\n %x = zext i{width} %a to i32\n %r = mul i32 %x, 3\n ret i32 %r\n}}"
        );
        let asm = llrm_z80::compile::text(&ir, "f", 0, &llrm_z80::driver::Request::new(llrm_z80::driver::Level::O2))
            .expect("compiles");
        asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count()
    };
    assert!(count(8) <= 16, "byte: {} instructions", count(8));
    assert!(count(16) <= 30, "word: {} instructions", count(16));
}

/// The size gate took the speed levels too: in a module of two long multiplies,
/// -O2 no longer decomposed `x * 3` and expanded it through byte lanes (108
/// instructions for two sites, 71 decomposed; div_constants_long -O2 +808 B, 10
/// cells worse). Only a function tuned for size takes the routine.
#[test]
fn a_function_at_o2_in_a_module_of_long_multiplies_keeps_the_decomposition() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
              define i32 @m(i32 %n, i32 %k) {\n %r = mul i32 %n, 3\n %s = mul i32 %k, 5\n %t = add i32 %r, %s\n ret i32 %t\n}";
    let asm = llrm_z80::compile::text(ir, "m", 0, &llrm_z80::driver::Request::new(llrm_z80::driver::Level::O2))
        .expect("compiles");
    let count = asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count();
    assert!(count <= 85, "{count} instructions:\n{asm}");
}

/// Two loops multiplying a long accumulator by 31 and by 7, at -Os: both calls
/// kept `__mulsi3` (main 109 B plus the 57 B routine, 166 B) where the shifts
/// and adds are 148 B (switch_table_holes -Os +17 B and +969,464 T). The calls
/// save less than the routine costs.
#[test]
fn a_size_function_whose_calls_save_less_than_the_routine_costs_keeps_the_shifts_and_adds() {
    use llrm_z80::compile::text;
    use llrm_z80::driver::{Level, Request};
    let asm =
        text(include_str!("data/two_loops_long_multiply.ll"), "main", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(!asm.contains("__mulsi3"), "{asm}");
}

/// compound_assignment_types -Os: `main` is 1 B smaller with the shifts and
/// adds, but they call the runtime's shift routines (+46 B) where the
/// multiplies called `__mulsi3`: priced by the function alone the shifts won
/// and the program grew 2 B. The module is priced with the runtime routines it
/// reaches, and the form chosen is the cheaper of the two (a tie goes to the
/// shifts, which are faster).
#[test]
fn a_function_keeps_the_call_when_the_shifts_pull_in_more_runtime_than_the_routine() {
    use llrm_z80::compile::{module_prices, text};
    use llrm_z80::driver::{Level, Request};
    let ir = include_str!("data/compound_assignment_types.ll");
    let request = Request::new(Level::Os);
    let (calls, shifts) = module_prices(ir, &request).expect("prices");
    let asm = text(ir, "main", 0, &request).expect("compiles");
    let called = asm.contains("call __mulsi3");
    assert!(
        if called { calls < shifts } else { shifts <= calls },
        "calls {calls} B, shifts {shifts} B, called {called}:\n{asm}"
    );
}

/// call_keeps -O2 priced the all-calls form as a gain (-368 B for 1115 of 11124
/// priced clocks) and ran +5970 T of 57813: the static loop weights run about
/// 5x under the measured ones, so a rate of 1 B = 3 T misjudged it.
/// -O1 compares forms by relative regret, which the scale does not touch, and
/// takes the routines; -O2 keeps the shifts and adds inline.
#[test]
fn the_routines_replace_long_shifts_and_multiplies_at_o1_and_not_at_o2() {
    let calls = |level| {
        let asm = llrm_z80::compile::text(
            include_str!("data/call_keeps.ll"),
            "churn",
            0,
            &llrm_z80::driver::Request::new(level),
        )
        .expect("compiles");
        (
            asm.matches("call __ashlsi3").count() + asm.matches("call __lshrsi3").count(),
            asm.matches("call __mulsi3").count(),
        )
    };
    let (o1, o2) = (calls(llrm_z80::driver::Level::O1), calls(llrm_z80::driver::Level::O2));
    // The long shifts are inline at O1 too since `add hl, hl; adc hl, hl`
    // carries in the flag (they cost the regret-based comparison less there
    // than the routine); the multiplies are still the routine.
    assert!(o1.1 > 0, "O1 calls the multiply routine: {o1:?}");
    assert_eq!(o2.0, 0, "O2 keeps the shifts inline: {o2:?}");
}

/// div_constants_long -O1 took `call __ashlsi3` for xorshift steps (-14 B, +10%
/// T measured): the module's static clocks were a hot loop elsewhere, so each
/// step's 500 T of routine were nothing and the routine, linked once for the
/// steps, paid its bytes. The module price keeps the calls here; each step's
/// own bytes and clocks inline it.
#[test]
fn small_long_shift_functions_beside_a_hot_loop_stay_inline_at_o1() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n".to_owned()
        + include_str!("data/shift_steps_beside_hot_loop.ll");
    let asm = llrm_z80::compile::text(&ir, "step1", 0, &llrm_z80::driver::Request::new(llrm_z80::driver::Level::O1))
        .expect("compiles");
    assert!(!asm.contains("call __ashlsi3") && !asm.contains("call __lshrsi3"), "{asm}");
}
