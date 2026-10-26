//! A constant multiply that only an address reads is `lea` and the address's
//! own scale.

mod common;

use std::process::Command;

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

/// x_dct and bintree at -Os made `imul r, r, 12` (13 to 42 clocks on a 486) for
/// `base + 12 * i`, where `lea` by 3 and the address's scale of 4 is the same
/// bytes and one clock; at -O2 it was `lea` and a `shl`.
#[test]
fn a_factor_of_a_lea_and_an_address_scale_is_one_lea_at_every_level() {
    for level in ["-Os", "-O2"] {
        let text = listing(FACTORS, level);
        assert!(!text.contains("imul") && !text.contains("shl "), "{level}: {text}");
        assert!(text.contains("lea eax, [eax+eax*2]"), "{level}: {text}");
        assert!(text.contains("lea eax, [eax+eax*4]"), "{level}: {text}");
        assert!(text.contains("lea eax, [eax+eax*8]"), "{level}: {text}");
        assert!(text.contains("*4]") && text.contains("*8]"), "{level}: {text}");
    }
}

/// A multiply by 7 is no `lea` factor times a scale: tuned for size the `imul`
/// is 3 bytes against 9, and for clocks the chain beats it.
#[test]
fn a_factor_that_is_no_lea_times_a_scale_is_priced_by_its_channel() {
    let source = "int a7(char *a, int i) { return a[7 * i]; }\n";
    assert!(listing(source, "-Os").contains("imul eax, eax, 7"));
    let text = listing(source, "-O2");
    assert!(!text.contains("imul"), "{text}");
}
