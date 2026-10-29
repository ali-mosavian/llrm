//! At -Os a loop that keeps stores of computed values keeps its zero stores
//! too, unless a memset or an initializer has set the object already.

mod common;

fn kept_listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(directory)
        .args(["-m16", "-march=i386", "-Os", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(directory.join("a.s")).unwrap();
    text.lines().skip_while(|line| !line.contains("_g@3 proc")).collect::<Vec<_>>().join("\n")
}

const STATICS: &str = "extern void use(float *a, float *b, float *v, float *w);
float a[6], b[6], v[6], w[6];
void g(void)
{
    int i;
    for (i = 0; i < 6; ++i) {
        a[i] = (float)(i * 7 - 15);
        b[i] = (float)(i * 5 - 12);
        v[i] = 0.0f;
        w[i] = 0.0f;
    }
    use(a, b, v, w);
}
";

const INITIALIZED: &str = "extern void use(float *a, float *b, float *v, float *w);
void g(void)
{
    float a[6], b[6], v[6] = {0, 0, 0, 0, 0, 0}, w[6] = {0, 0, 0, 0, 0, 0};
    int i;
    use(a, b, v, w);
    for (i = 0; i < 6; ++i) {
        a[i] = (float)(i * 7 - 15);
        b[i] = (float)(i * 5 - 12);
        v[i] = 0.0f;
        w[i] = 0.0f;
    }
    use(a, b, v, w);
}
";

/// nbody_single.bas m16 -Os: two fills (13 bytes each) to save two stores of
/// 6 left the loop its other stores; 15 bytes over the loop.
#[test]
fn at_os_zero_stores_beside_computed_ones_are_not_two_fills() {
    let text = kept_listing(STATICS);
    assert!(!text.contains("rep stos"), "{text}");
}

/// nbody_single.nib m16 -Os: the arrays were set before the loop, whose zero
/// stores a fill makes dead; keeping them was 11 bytes over.
#[test]
fn at_os_zero_stores_of_a_set_object_are_still_filled() {
    let text = kept_listing(INITIALIZED);
    assert!(!text.contains("xor edx, edx"), "{text}");
}
