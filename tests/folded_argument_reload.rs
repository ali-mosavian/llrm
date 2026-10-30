//! An incoming argument the allocator would reload is read where it is used.

mod common;

/// x_isqrt: `n` is read at the entry and again by every trip of the outer loop,
/// where the loop's own values hold the registers.
const SOURCE: &str = r#"extern void report(long value);
typedef unsigned long u32;
typedef long i32;
static u32 isqrt(u32 x) { u32 r = 0, bit = 1ul << 30; while (bit > x) bit >>= 2; while (bit) { if (x >= r + bit) { x -= r + bit; r = (r >> 1) + bit; } else r >>= 1; bit >>= 2; } return r; }
long bench_x_isqrt(i32 n)
{
    u32 s = 3; i32 i; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 1103515245ul + 12345ul; sum += isqrt(s >> 2); }
    return sum;
}
long (*volatile bench_x_isqrt_via)(i32 n) = bench_x_isqrt;
int main(void) { report((*bench_x_isqrt_via)(1500)); return 0; }
"#;

/// The loop compared `ebx` with `n` through `mov ecx, [esp+32]; cmp ebx, ecx`:
/// the allocator made the argument's load again in a new register. The pin
/// folds a load that is made again for one instruction into it
/// (`reMaterializeFor`, `canFoldAsLoad`): `cmp ebx, [esp+32]`.
#[test]
fn a_load_made_again_for_one_instruction_is_that_instructions_memory_operand() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", "-O2", "--isel=dag", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("a.s")).unwrap();
    let folded = regex::Regex::new(r"(?m)^\s+cmp \w+, dword ptr \[esp\+\d+\]$").unwrap();
    assert!(folded.is_match(&text), "no compare reads the argument from its cell:\n{text}");
}
