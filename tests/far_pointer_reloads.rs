//! A far pointer live across a call is reloaded where the call needs the
//! registers, not before every use.

mod common;

const KERNEL: &str = "
typedef struct { float x, y, z, d; } P;
extern float g(float);
void f(P far *fr, float *m)
{
    int i;
    fr[0].x = -(m[3]+m[0]); fr[0].y = -(m[7]+m[4]); fr[0].z = -(m[11]+m[8]); fr[0].d = -(m[15]+m[12]);
    fr[1].x = -(m[3]-m[0]); fr[1].y = -(m[7]-m[4]); fr[1].z = -(m[11]-m[8]); fr[1].d = -(m[15]-m[12]);
    fr[2].x = -(m[3]-m[1]); fr[2].y = -(m[7]-m[5]); fr[2].z = -(m[11]-m[9]); fr[2].d = -(m[15]-m[13]);
    fr[3].x = -(m[3]+m[1]); fr[3].y = -(m[7]+m[5]); fr[3].z = -(m[11]+m[9]); fr[3].d = -(m[15]+m[13]);
    for (i = 0; i < 4; i++) {
        float d = 1.0f / g(fr[i].x*fr[i].x + fr[i].y*fr[i].y + fr[i].z*fr[i].z);
        fr[i].x *= d; fr[i].y *= d; fr[i].z *= d; fr[i].d *= d;
    }
}
";

/// r_set_frustum (QCport -Os): both halves of the far pointer parameter were
/// spilled whole because `trySplit` refused a value the spiller can make again,
/// and `les bx, [bp+6]` was reloaded before each of the 24 stores: 29 `les`
/// here (1 with the old allocator), +304 `les` over QCport.
#[test]
fn test_a_far_pointer_live_across_a_call_is_not_reloaded_before_every_use() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output =
        common::cc::llrm_c().current_dir(scratch.path()).args(["-Os", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let loads = text.lines().filter(|line| line.trim_start().starts_with("les ")).count();
    assert!(loads <= 8, "{loads} `les` for one far pointer (8 allowed; 21 before the split rematerializes)\n{text}");
}

/// The same function after the split: the offset half of the pointer kept `bx`
/// (its class from the loop's `[bx+si+k]`) in every child, so the 24 stores
/// before the loop reloaded it from a slot, `mov bx, [bp-6]` x23; a register
/// made by a split takes the class of its own instructions.
#[test]
fn test_the_stores_before_the_loop_do_not_reload_the_offset_from_a_slot() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output =
        common::cc::llrm_c().current_dir(scratch.path()).args(["-Os", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let reloads =
        text.lines().filter(|line| line.trim_start().starts_with("mov ") && line.contains(", word ptr [bp-")).count();
    assert!(
        reloads <= 2,
        "{reloads} reloads from the frame (2 allowed; 15 before a split takes its own class)\n{text}"
    );
}

/// The loop calls a function between the uses of the pointer, so it cannot stay
/// in `bx`: split across the call it is fetched once before and once after it.
/// Every gap of the block was a regmask gap (the instructions that read `es`
/// without naming it counted as regmasks), no split was possible and the
/// pointer was fetched before each use: 6 `les` in this function.
#[test]
fn test_a_pointer_live_across_a_call_is_fetched_once_on_each_side_of_it() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), KERNEL).unwrap();
    let output =
        common::cc::llrm_c().current_dir(scratch.path()).args(["-Os", "-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let loads = text.lines().filter(|line| line.trim_start().starts_with("les ")).count();
    assert!(loads <= 3, "{loads} `les` (3 allowed; 6 when every gap is a regmask gap)\n{text}");
}
