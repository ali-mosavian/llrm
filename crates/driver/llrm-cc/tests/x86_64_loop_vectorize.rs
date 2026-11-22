//! x86-64 -O2: LoopVectorize runs on the integer loops isel can select
//! (`paddd`, `movups`), because the target now says it has a 128-bit vector
//! register and prices the add, the subtract, the logic ops and the contiguous
//! access (X86TTIImpl rows); every op it does not price keeps its loop scalar
//! (a `<4 x i32>` multiply has no SSE2 form). Before, `getRegisterBitWidth` was
//! 0 and no loop was widened (copy_runs, loop_fusion, fill_nest: 15-59% more
//! instructions than clang). Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

fn assembly(
    target: &str,
    level: &str,
) -> String {
    assembly_of("lv_loops.c", target, level)
}

fn assembly_of(
    source: &str,
    target: &str,
    level: &str,
) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    // Tests run in parallel: a directory of its own for each call.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lv-{}-{call}-{target}-{level}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", target)
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

fn body<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let start = format!("{name} proc near");
    let end = format!("{name} endp");
    text.split(start.as_str()).nth(1).and_then(|rest| rest.split(end.as_str()).next()).expect("a body")
}

#[test]
fn an_integer_add_loop_is_four_lanes_wide_at_o2() {
    let text = assembly("x86_64-linux", "-O2");
    let add = body(&text, "add_arrays");
    assert!(add.contains("paddd") && (add.contains("movups") || add.contains("movaps")), "{add}");
}

/// An unknown trip count and pointers that may overlap: the remainder loop and
/// the runtime check are emitted, and the listing still assembles (no op
/// without a form).
#[test]
fn a_runtime_trip_count_loop_has_a_vector_body_and_a_scalar_remainder() {
    let text = assembly("x86_64-linux", "-O2");
    let copy = body(&text, "copy_plus_one");
    assert!(copy.contains("paddd"), "{copy}");
    assert!(copy.contains("add ") || copy.contains("inc "), "the scalar remainder: {copy}");
}

/// A `<4 x i32>` multiply has no SSE2 `pmulld`: `pmuludq` on the even and the
/// odd lanes, then `punpckldq` (the pin's `LowerMUL`). It was unpriced, so the
/// loop stayed scalar.
#[test]
fn an_int_multiply_loop_uses_pmuludq() {
    let text = assembly("x86_64-linux", "-O2");
    let multiply = body(&text, "multiply_arrays");
    assert!(multiply.contains("pmuludq") && multiply.contains("punpckldq"), "{multiply}");
}

/// A vector multiply by `2^n + 1`, `2^n - 1` or `-(2^n - 1)` is a shift and an
/// add or subtract (`visitMUL` through `isConstOrConstSplat`,
/// `decomposeMulByConstant`): it read only a scalar constant, so a splat stayed
/// `pmuludq` twice with five shuffles (`loop_countdown_sum` +24% instructions
/// against clang).
#[test]
fn a_vector_multiply_by_a_near_power_of_two_is_a_shift_and_an_add() {
    let text = assembly("x86_64-linux", "-O2");
    for (name, shift, op) in
        [("times_three", "pslld", "paddd"), ("times_seven", "pslld", "psubd"), ("times_minus_three", "pslld", "psubd")]
    {
        let body = body(&text, name);
        assert!(!body.contains("pmuludq"), "{name} multiplies: {body}");
        assert!(body.contains(shift) && body.contains(op), "{name} lacks {shift} and {op}: {body}");
    }
}

/// A vector narrower than a register (`<8 x i8>`, `<4 x i16>`, `<2 x i32>`)
/// stopped the compile (`Legalize(Unsupported { op: Load, ty: V8I8, "vector
/// with no register class" })`, no output at all). It is now widened to the
/// register as the pin's `TypeWidenVector` does and its accesses are `movq`, 8
/// bytes, as wide as the vector: the host run in `lv_run.c` puts each buffer
/// against an unmapped page.
#[test]
fn a_vector_narrower_than_a_register_is_widened_and_moved_by_movq() {
    let text = assembly_of("lv_narrow.c", "x86_64-linux", "-O2");
    for (name, op) in [("add_u8x8", "paddb"), ("sub_u16x4", "psubw"), ("xor_u32x2", "pxor")] {
        let body = body(&text, name);
        assert!(body.contains(op) && body.contains("movq"), "{name} lacks {op} and movq: {body}");
        assert!(!body.contains("xmmword") && !body.contains("movups") && !body.contains("movdqu"), "{name}: {body}");
    }
}

/// The vectors an unrolled vector loop stores are constants (`<0, 1, ..> + <8,
/// ..>` per trip folds, #10074): with the sub-register vectors widened,
/// `copy_runs`' init loops are stored constants and not `paddb`/`pmullw` at run
/// time.
#[test]
fn the_vectors_a_fully_unrolled_ramp_loop_stores_are_constants() {
    let text = assembly_of("lv_narrow.c", "x86_64-linux", "-O2");
    let fill = body(&text, "fill_ramp");
    assert!(!fill.contains("paddb") && !fill.contains("pmullw") && !fill.contains("paddw"), "{fill}");
}

#[test]
fn the_32_bit_target_and_o1_keep_every_loop_scalar() {
    assert!(!assembly("x86_32-linux", "-O2").contains("xmm"));
    assert!(!assembly("x86_64-linux", "-O1").contains("xmm"));
}

/// A sum reached `llvm.vector.reduce.add` and the backend refused it
/// (`CannotSelect("VecReduceAdd")`, no output at all), so reductions were kept
/// scalar. The total is now folded from the spilled lanes (`expandVecReduce`).
#[test]
fn a_sum_loop_is_vector_with_a_folded_total() {
    let text = assembly("x86_64-linux", "-O2");
    let sum = body(&text, "sum_array");
    assert!(sum.contains("paddd"), "{sum}");
}

/// A vector sum is folded by shuffles, as the pin's `ExpandReductions` does
/// (`pshufd` for the 64- and 32-bit halves, `psrld` and `psrlw` for the 16- and
/// 8-bit ones, then `movd`): it was spilled to the stack and every lane loaded
/// (`[rsp-24]`, two `add`s and a `mov`).
#[test]
fn a_vector_total_is_folded_by_shuffles_and_not_spilled_to_the_stack() {
    let text = assembly("x86_64-linux", "-O2");
    for (name, shift) in
        [("sum_array", None), ("sum_longs", None), ("sum_shorts", Some("psrld")), ("sum_bytes", Some("psrlw"))]
    {
        let sum = body(&text, name);
        assert!(sum.contains("pshufd") && (sum.contains("movd") || sum.contains("movq")), "{name}: {sum}");
        assert!(!sum.contains("rsp"), "{name} spills the vector: {sum}");
        if let Some(shift) = shift {
            assert!(sum.contains(shift), "{name} lacks {shift}: {sum}");
        }
    }
}

/// `pshufd` was a tied two-address form (`movaps xmm1, xmm0; pshufd xmm1, xmm1,
/// 0xee`), a copy before each fold step where the pin writes a fresh register
/// (`pshufd xmm1, xmm0, 0xee`): countdown_sum 462,091 against 442,611 Ir.
#[test]
fn a_vector_total_shuffles_into_a_fresh_register_with_no_copy() {
    let text = assembly("x86_64-linux", "-O2");
    let sum = body(&text, "sum_array");
    let lines: Vec<&str> = sum.lines().map(str::trim).collect();
    let shuffles: Vec<usize> = (0..lines.len()).filter(|at| lines[*at].starts_with("pshufd")).collect();
    assert!(!shuffles.is_empty(), "{sum}");
    for at in shuffles {
        assert!(!lines[at - 1].starts_with("movaps"), "a copy before {}: {sum}", lines[at]);
    }
}

/// LSR charged a constant in a setup by `getIntImmCost` (a free 0 costs
/// nothing), so the strlen loop started at 0 and copied the index before
/// bumping it (`mov ecx, eax; inc eax; cmp [rdi+rcx]`, 4 a trip) where clang 20
/// starts at -1 and indexes with the bumped register (3 a trip): strlen_idiom
/// 2,895 against 2,759 Ir.
#[test]
fn a_loop_to_the_terminator_indexes_with_the_bumped_register() {
    let text = assembly_of("lsr_strlen.c", "x86_64-linux", "-O2");
    let len = body(&text, "len");
    assert!(len.contains("[rdi+rax") && !len.contains("[rdi+rcx"), "{len}");
}

/// A byte sum into an `i64` (VF 16) kept its total as eight registers and
/// interleaved twice: 16 accumulators spilled to the stack,
/// mem_length_boundaries_m32 186,155 instructions to 142,319 scalar and gcc's
/// 137,355. One register of lanes takes the eight pieces of a trip.
#[test]
fn a_byte_sum_into_a_wide_total_does_not_spill() {
    let text = assembly_of("lv_bytesum.c", "x86_64-linux", "-O2");
    let sum = body(&text, "sum_bytes_wide");
    assert!(sum.contains("punpcklbw") || sum.contains("pmovzx"), "{sum}");
    assert!(!sum.contains("rsp"), "{sum}");
}

/// The widened loops compute what the scalar ones do: sums and xors folded from
/// vector lanes, and forward and backward copies that overlap by 1 to 5 bytes,
/// every length under 100, run on the host. The count-down copy sat 31 bytes
/// low at IC 2 (`bad 340`).
#[test]
fn overlapping_forward_and_backward_copies_keep_their_bytes_at_o2() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lv-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut objects = Vec::new();
    for source in ["lv_run.c", "lv_loops.c", "lv_lanes.c", "lv_casts.c", "lv_narrow.c"] {
        let object = dir.join(format!("{source}.o"));
        let built = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_64-linux")
            .env("LLRM_LLC", std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set"))
            .args(["-O2", "-w", "-c", "-o"])
            .arg(&object)
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c").join(source))
            .output()
            .unwrap();
        assert!(built.status.success(), "{source}: {}", String::from_utf8_lossy(&built.stderr));
        objects.push(object);
    }
    let exe = dir.join("a.out");
    let linked = Command::new("gcc").args(["-static", "-no-pie", "-o"]).arg(&exe).args(&objects).output().unwrap();
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));
    let ran = Command::new(&exe).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "bad 0");
}

/// The vector loop of a 64-lane row (VF 8, IC 2: four trips) was kept as a
/// loop: the unroller found no size for a
/// `<8 x i16>` store ("no cost for store") and never fully unrolled it. The pin
/// unrolls it to one trip of eight stores per row.
#[test]
fn a_vector_loop_of_four_trips_is_fully_unrolled() {
    let text = assembly("x86_64-linux", "-O2");
    let fill = body(&text, "fill_rows");
    let stores = fill.matches("movaps xmmword ptr [").count() + fill.matches("movups xmmword ptr [").count();
    assert!(stores >= 8, "{fill}");
}

/// A vector constant the folder left as `bitcast (<16 x i8> .. to i128)` or
/// `bitcast (i128 N to <4 x i32>)` stopped the compile
/// (`CannotSelect("Truncate")`, "a vector constant that is not a list of
/// integers"): both build now.
#[test]
fn loops_that_leave_vector_image_constants_compile_at_o2_and_o3() {
    for level in ["-O2", "-O3"] {
        let text = assembly("x86_64-linux", level);
        assert!(text.contains("stack_copy") && text.contains("square_sum"), "{level}");
    }
}

/// A `<8 x i16>` multiply is `pmullw`: the loop was kept scalar because the
/// multiply had no price.
#[test]
fn a_short_multiply_loop_uses_pmullw() {
    let text = assembly("x86_64-linux", "-O2");
    let multiply = body(&text, "multiply_shorts");
    assert!(multiply.contains("pmullw"), "{multiply}");
}

/// A shift by a constant is `pslld`/`psrad`/`psrld` (`VSHLI`, `VSRAI`,
/// `VSRLI`): a loop with one stayed scalar, its shift unpriced.
#[test]
fn a_loop_of_constant_shifts_uses_the_immediate_forms() {
    let text = assembly("x86_64-linux", "-O2");
    let shifts = body(&text, "shift_ints");
    assert!(shifts.contains("pslld") && shifts.contains("psrad") && shifts.contains("psrld"), "{shifts}");
}

/// A splat of a variable was four stores through the stack (`BUILD_VECTOR`
/// expanded); it is `movd` + `pshufd` (and `movq` + `punpcklqdq` for 64-bit
/// lanes), as the pin's `LowerBUILD_VECTOR` makes it.
#[test]
fn a_splat_of_a_variable_is_movd_and_pshufd() {
    let text = assembly("x86_64-linux", "-O2");
    let ints = body(&text, "fill_value");
    assert!(ints.contains("movd xmm") && ints.contains("pshufd"), "{ints}");
    assert!(!ints.contains("[rsp"), "{ints}");
    let longs = body(&text, "fill_long_value");
    assert!(longs.contains("movq xmm") && longs.contains("punpcklqdq"), "{longs}");
}

/// A `<16 x i8>` multiply is two `pmullw` of the unpacked halves, masked and
/// packed: the loop was kept scalar (one `imul` and two byte stores a byte)
/// because the byte multiply had no price and no lowering.
#[test]
fn a_byte_multiply_by_a_constant_is_pmullw_and_packuswb() {
    let text = assembly("x86_64-linux", "-O2");
    let pattern = body(&text, "multiply_bytes");
    assert!(pattern.contains("pmullw") && pattern.contains("packuswb") && pattern.contains("pand"), "{pattern}");
    assert!(!pattern.contains("imul"), "{pattern}");
}

/// A byte multiplied by a byte in memory (`mul byte ptr [..]`) stopped the
/// compile to an object with `Semantics(op=MULTIPLY ... width=1)`: the
/// one-operand `mul` took no memory byte.
#[test]
fn a_byte_multiply_from_memory_assembles() {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("lv-mulb-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args(["-O2", "-w", "-c", "-o"])
        .arg(dir.join("a.o"))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/lv_loops.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}

/// The splat constant of a vector loop is read from the pool before the loop:
/// it was reloaded (`movaps xmm, $K`) on every trip, because machine LICM left
/// every xmm value in the loop.
#[test]
fn a_vector_constant_is_loaded_before_the_loop() {
    let text = assembly("x86_64-linux", "-O2");
    let add = body(&text, "add_five");
    let (before, inside) = add.split_once("align 16").expect("a loop");
    assert!(before.contains("$K"), "{add}");
    assert!(!inside.contains("$K"), "{add}");
}

/// The zero a reduction starts from is `xorps xmm, xmm` (`V_SET0`, an
/// `isBuildVectorAllZeros` build the pin leaves legal): it was a 16-byte
/// constant-pool load (`movaps xmm0, $K`) and a 16-byte `.rodata.cst16` entry.
#[test]
fn a_vector_of_zeros_is_an_xor_and_not_a_pool_load() {
    let text = assembly("x86_64-linux", "-O2");
    let sum = body(&text, "sum_array");
    assert!(sum.contains("xorps xmm") || sum.contains("pxor xmm"), "{sum}");
    assert!(!sum.contains("$K"), "{sum}");
}

/// A byte loop whose accumulator starts from an argument and whose operand is a
/// loop-invariant byte stopped the compile: `insertelement <16 x i8>` (the seed
/// in lane 0 of a zero vector, the broadcast) had no lowering (`CannotSelect("
/// InsertVectorElt")`), and the `i1` condition of a vector `select` was
/// zero-extended to a vector (panic "a vector type"). The seed is now a `movd`
/// and the select a branch around the register copy. known_loads_walks.
#[test]
fn a_byte_accumulator_seeded_from_an_argument_compiles_with_movd_and_a_vector_select() {
    for level in ["-O2", "-O3"] {
        let text = assembly("x86_64-linux", level);
        let toggle = body(&text, "xor_toggle_count");
        assert!(toggle.contains("movd xmm"), "{level}: {toggle}");
    }
}

/// A lane written at a constant position (`insertelement` at 5, 3 and 2) had no
/// lowering at all (`Unsupported { op: InsertVectorElt, .. "convert node to
/// libcall" }`): it is a store of the vector, a store of the lane and a load
/// (`ExpandINSERT_VECTOR_ELT`). The host run in `lv_run.c` checks the bytes.
#[test]
fn a_lane_written_at_a_constant_position_assembles_as_a_stack_round_trip() {
    let text = assembly_of("lv_lanes.c", "x86_64-linux", "-O2");
    let set = body(&text, "set_lanes");
    assert!(set.contains("mov byte ptr [rsp") && set.contains("mov word ptr [rsp"), "{set}");
}

/// A vector widened to or narrowed from more than one register had no lowering
/// (`Unsupported { op: Truncate, ty: V4I16, "vector with no register class" }`,
/// `"split extending load"`, no `punpcklwd`): `punpck` with zero or with
/// the sign, `pand` and `packuswb`. The host run in `lv_run.c` checks every
/// width pair on edge values.
#[test]
fn vectors_widened_or_narrowed_across_registers_use_unpack_and_pack() {
    let text = assembly_of("lv_casts.c", "x86_64-linux", "-O2");
    assert!(body(&text, "z8_32").contains("punpcklwd"), "{text}");
    assert!(body(&text, "t32_8").contains("packuswb"), "{text}");
    assert!(body(&text, "s32_64").contains("psrad"), "{text}");
}

/// A byte table computed in 32-bit lanes stayed a scalar loop because `trunc
/// <16 x i32> to <16 x i8>` had no price (mem_length_boundaries_m32: 233,551
/// instructions to clang's 137,355): VF 16, `pand` and `packuswb`.
#[test]
fn a_byte_table_computed_in_32_bit_lanes_is_a_vector_loop_with_packuswb() {
    let text = assembly("x86_64-linux", "-O2");
    let pattern = body(&text, "pattern_bytes");
    assert!(pattern.contains("packuswb") && pattern.contains("paddd"), "{pattern}");
}

/// The scalar remainder of a vector loop is not unrolled at run time: the
/// unroller could not see the loop ID the vectorizer had just made, so the
/// remainder got a 0-3 trip prologue and a 4-wide body (loop_countdown_sum:
/// three loops in `down_sum` where the pin has two).
#[test]
fn a_vector_loop_remainder_is_not_unrolled_at_run_time() {
    let text = assembly("x86_64-linux", "-O2");
    let sum = body(&text, "down_sum");
    assert!(sum.contains("paddd"), "vectorized: {sum}");
    assert_eq!(sum.matches("align 16").count(), 2, "a vector loop and a rolled remainder: {sum}");
}

/// The pin keeps a 32-trip table fold at -O3 (386 units against its threshold
/// of 300), gcc and clang 20 unroll it whole: loop_interchange +17.6% over the
/// bar. m64 takes 450 at -O3.
#[test]
fn a_32_trip_table_fold_is_unrolled_whole_at_o3() {
    let text = assembly("x86_64-linux", "-O3");
    let fold = body(&text, "table_fold");
    assert!(!fold.contains("align 16"), "a loop is left: {fold}");
}

/// A packed accumulator is the tied operand of the `paddd` that adds to it: the
/// peephole's recurrence commute knew only the integer ops, so each trip of
/// `down_sum`'s vector loop copied two accumulators (6 copies a trip, 4 now;
/// the pin has 4).
#[test]
fn a_packed_accumulator_is_added_in_place() {
    let text = assembly("x86_64-linux", "-O2");
    let sum = body(&text, "down_sum");
    let (_, rest) = sum.split_once("align 16").expect("a vector loop");
    let vector = rest.split_once("jne").expect("its back edge").0;
    let copies = vector.lines().filter(|line| line.trim_start().starts_with("movaps")).count();
    assert!(copies <= 4, "the loop copies {copies} registers: {vector}");
}

/// `descent` carries a loaded `short` round its loop and indexes with it: the
/// load was `movzx` and every trip extended it again (`movzx ecx, cx`). The
/// extension is on the load now (gcc's `ree`).
#[test]
fn a_loop_carried_short_is_extended_on_its_load_and_not_in_every_trip() {
    let text = assembly_of("ree_loops.c", "x86_64-linux", "-O3");
    let descent = body(&text, "descent");
    let widened = descent
        .lines()
        .filter(|line| (line.contains("movsx") || line.contains("movzx")) && !line.contains("ptr"))
        .count();
    assert_eq!(widened, 0, "a register-to-register extension stays in the loop: {descent}");
}

/// `differ` of memory_expansion (a byte compare that leaves at the first
/// difference) stayed a scalar loop, 1.69 M instructions against clang's 0.93
/// M: no loop of two blocks was planned and x86 priced no vector compare. The
/// pin's vector loop compares 16 bytes (`pcmpeqb`) and leaves on any set lane
/// (`pmovmskb`).
#[test]
fn an_early_exit_byte_compare_is_sixteen_lanes_wide() {
    let text = assembly_of("lv_early_exit.c", "x86_64-linux", "-O3");
    let differ = body(&text, "differ_arrays");
    assert!(differ.contains("pcmpeqb"), "{differ}");
    assert!(differ.contains("pmovmskb"), "{differ}");
}

/// The any-of of a vector compare is its sign-bit mask (`combineBitcastvxi1`):
/// bytes `pmovmskb` as they are, words packed to bytes first, doublewords
/// `movmskps`; an unsigned order flips the sign bits and a missing order (`<`)
/// swaps the operands. Before, the vectorized loop refused to select
/// (`CannotSelect(SetCC)`).
#[test]
fn an_early_exit_compare_of_each_lane_width_is_gathered_by_a_sign_mask() {
    let text = assembly_of("lv_early_exit_widths.c", "x86_64-linux", "-O3");
    let words = body(&text, "first_below");
    assert!(words.contains("pcmpgtw") && words.contains("packsswb") && words.contains("pmovmskb"), "{words}");
    assert!(words.contains("pxor"), "an unsigned `<` flips the sign bits: {words}");
    let dwords = body(&text, "first_differs");
    assert!(dwords.contains("pcmpeqd") && dwords.contains("movmskps"), "{dwords}");
    let bytes = body(&text, "first_greater");
    assert!(bytes.contains("pcmpgtb") && bytes.contains("pmovmskb"), "{bytes}");
}

/// A loop that returns the index of the first difference as an `int`: the
/// induction is cast inside the loop (`trunc`) and only the exit phi reads it.
/// Before, the loop stayed scalar (`recipes: An instruction the vectorizer does
/// not widen yet`): no `pmovmskb` in `first_index32`.
#[test]
fn an_early_exit_that_returns_the_cast_index_is_a_vector_loop() {
    let text = assembly_of("lv_early_exit_index.c", "x86_64-linux", "-O3");
    for name in ["first_index32", "first_index16"] {
        let words = body(&text, name);
        assert!(words.contains("pmovmskb"), "{name}: {words}");
    }
}

/// `(long)a[i] * ((i & 15) + 1)` over an `i64` counter was a `<8 x i64>` chain
/// (`pand`, `paddq` x4 and a `pshufd`/`packssdw` ladder down to the `i16`
/// multiply): 51 instructions per 8 elements, scroll +3.8k against gcc.
/// The chain fits 16 bits, so the counter is a `<8 x i16>` `paddw` and the
/// multiply `pmullw`/`pmulhw`.
#[test]
fn a_masked_counter_weight_is_computed_in_sixteen_bit_lanes() {
    let text = assembly("x86_64-linux", "-O2");
    let sum = body(&text, "weighted_sum");
    assert!(sum.contains("paddw") && sum.contains("pmullw") && sum.contains("pmulhw"), "{sum}");
    // One `pshufd` ends the reduction; the wide chain had six and a `packssdw`
    // ladder.
    assert!(
        !sum.contains("packssdw") && sum.matches("pshufd").count() <= 1,
        "the counter is narrowed by shuffles: {sum}"
    );
}

/// tile -O2 (14,774 instructions against clang's 7,727): the vector `trunc` of
/// `splat + <0, 3, ..>` stayed `<8 x i64>` (the combiner skipped vector casts),
/// 22 instructions per 8 stores. It is `paddw` on the truncated splat: no
/// `paddq`.
#[test]
fn a_masked_i64_counter_stored_as_short_is_added_in_sixteen_bit_lanes() {
    let text = assembly("x86_64-linux", "-O2");
    let init = body(&text, "init_tile");
    assert!(init.contains("paddw") && init.contains("pand"), "{init}");
    // One `pshufd` splats the counter; the wide chain had 32 and a `paddq` per
    // vector.
    assert!(!init.contains("paddq") && init.matches("pshufd").count() <= 1, "the add is done in 64-bit lanes: {init}");
}
