//! A counter C declares `short` is the index's width in a loop that reads or
//! writes through it, and the address that counter makes is one addressing
//! mode.

mod common;

fn listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(directory)
        // The loops are the premise: at 40 trips the pin's order copies them
        // out.
        .args(["-m32", "-mabi=sysv", "-O2", "-march=i486", "-fno-unroll-loops", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

/// `for (short k = 0; k < n; k++) a[k] = b[k] * 2` was `inc bx; movsx esi,bx;
/// cmp bx,dx; jl` a trip: d_faces executed 18,142 `movsx` against gcc's 5,459,
/// most of them this one. The counter is below `n` when it is stepped, so its
/// sign extension is a counter of the index's width.
#[test]
fn a_short_counter_read_as_an_index_is_not_extended_each_trip() {
    let text =
        listing("void g(float *a, float *b, short n) { short k; for (k = 0; k < n; k++) a[k] = b[k] * 2.0f; }\n");
    let body: Vec<&str> = text.lines().skip_while(|line| !line.starts_with("_g proc")).collect();
    let loop_ = body.iter().skip_while(|line| !line.contains("fld")).take_while(|line| !line.contains("endp")).count();
    assert!(
        loop_ > 0 && !body.iter().any(|line| line.trim_start().starts_with("movsx ") && line.contains(", bx")),
        "{text}"
    );
    assert!(!text.contains("cmp bx") && !text.contains("inc bx"), "the counter is 16 bits wide in the loop:\n{text}");
}

/// tile: `tile[(y + dy) & 63][(x + dx) & 63]` over a global is
/// `[symbol+base+index*2]`, one access, not an index doubled into a register
/// first (`lea esi,[ebx+ebx]` a trip).
#[test]
fn a_global_row_and_a_scaled_counter_are_one_address() {
    let text = listing(
        "short tile[64][64];\nint sum(int y) { int t = 0; short x; for (x = 0; x < 40; x++) t += tile[y & 63][(x + 7) & 63]; return t; }\n",
    );
    // One access a trip: the row, the base and the counter, which is stepped
    // by 2 (the pin's strength reduction), so no `*2` and no doubling.
    let access = text.lines().find(|line| line.contains("movsx") && line.contains("_tile+"));
    assert!(access.is_some_and(|line| line.contains("][") || line.matches('+').count() >= 2), "{text}");
    assert!(!text.contains("lea esi, [ebx+ebx]") && !text.contains("movsx esi, bx"), "{text}");
}
