//! A constant multiply that only an address reads is `lea` and the address's
//! own scale.

mod common;

fn listing(
    source: &str,
    level: &str,
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(directory)
        .args(["-m32", "-mabi=sysv", "-march=i486", level, "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

const FACTORS: &str = "int a3(int *a, int i) { return a[3 * i]; }
int a5(int *a, int i) { return a[5 * i]; }
int a9(int *a, int i) { return a[9 * i]; }
int c12(char *a, int i) { return a[12 * i]; }
int c24(char *a, int i) { return a[i * 24]; }
";

/// Whether the listing has `lea r, [s+s*scale]`, `s` any one register.
fn has_lea_of_itself(
    text: &str,
    scale: u32,
) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        let inner = line.strip_prefix("lea ").and_then(|rest| rest.split_once('[')).map(|(_, rest)| rest);
        inner
            .and_then(|rest| rest.strip_suffix(&format!("*{scale}]")))
            .is_some_and(|pair| pair.split_once('+').is_some_and(|(base, index)| base == index))
    })
}

/// x_dct and bintree at -Os made `imul r, r, 12` (13 to 42 clocks on a 486) for
/// `base + 12 * i`, where `lea` by 3 and the address's scale of 4 is the same
/// bytes and one clock; at -O2 it was `lea` and a `shl`.
///
/// llrm-cc: the index arrives in `ecx` (clang's IR), not `eax`, so the `lea`
/// is of whichever register holds it; the pin's `llc` on the same IR emits
/// the same `lea` and address scale (pin-shape).
#[test]
fn a_factor_of_a_lea_and_an_address_scale_is_one_lea_at_every_level() {
    for level in ["-Os", "-O2"] {
        let text = listing(FACTORS, level);
        assert!(!text.contains("imul") && !text.contains("shl "), "{level}: {text}");
        for scale in [2, 4, 8] {
            assert!(has_lea_of_itself(&text, scale), "{level}: no lea by {scale}: {text}");
        }
        assert!(text.contains("*4]") && text.contains("*8]"), "{level}: {text}");
    }
}

/// A multiply by 7 is no `lea` factor times a scale. At -O2 the pin's `llc` on
/// the same IR makes it `lea r, [i*8]` and `sub r, i`, and so do we. At -Os the
/// pin's `lea; sub` is 21 B for `a[7 * i]` against gcc's 13 B `imul`, and ours
/// keeps the `imul` (DEVIATE MUL-OS, `Tuning::imul_at_size`, owner x86-m32,
/// exit 2026-11-20; #7579): this test still asked the -Os of before it, and was
/// not run in CI (it needs the pin's clang).
///
/// llrm-cc: the register that holds the index is `eax`, as in the pin's
/// listing (pin-shape).
#[test]
fn a_factor_that_is_no_lea_times_a_scale_is_the_pins_lea_and_sub_at_speed_and_an_imul_at_size() {
    let source = "int a7(char *a, int i) { return a[7 * i]; }\n";
    let speed = listing(source, "-O2");
    assert!(!speed.contains("imul"), "-O2: {speed}");
    assert!(speed.contains("*8]") && speed.lines().any(|line| line.trim().starts_with("sub ")), "-O2: {speed}");
    let size = listing(source, "-Os");
    assert!(size.contains("imul") && !size.lines().any(|line| line.trim().starts_with("sub ")), "-Os: {size}");
}
