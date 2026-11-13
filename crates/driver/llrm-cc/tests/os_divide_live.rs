//! m32 -Os: a narrow divide by a constant keeps its `div` (DIV-OPTSIZE-1)
//! unless more than four values are live across it (DIV-LIVE). matmul's init
//! nest, `(i + j) % 3` over seven live values, was `mov ax,si; mov cx,3; div
//! cx` with four copies and two reloads around it: 1148 B, 1140 B with the
//! multiply. Needs the Open Watcom tree (tools/ow-fetch.sh) and the m16 clang
//! (`LLRM_CLANG_M16`).

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

const NEST: &str = "unsigned long f(unsigned short seed)
{
    short a[8][8], b[8][8];
    long c[8][8];
    unsigned short i, j, k;
    unsigned long checksum = 0;
    for (i = 0; i < 8; ++i)
        for (j = 0; j < 8; ++j) {
            a[i][j] = (short)(i * 3 + j + 1 + seed);
            b[i][j] = (short)(i == j ? 2 : (i + j) % 3);
        }
    for (i = 0; i < 8; ++i)
        for (j = 0; j < 8; ++j) {
            long total = 0;
            for (k = 0; k < 8; ++k) total += (long)a[i][k] * b[k][j];
            c[i][j] = total;
        }
    for (i = 0; i < 8; ++i)
        for (j = 0; j < 8; ++j) checksum += (unsigned long)c[i][j] * (i * 8 + j + 1);
    return checksum;
}
";

const LONE: &str = "unsigned short r(unsigned short a) { return a % 3; }\n";

fn compile(
    name: &str,
    source: &str,
    flag: &str,
) -> (String, u64) {
    assert!(llrm_cc::ow_present(), "the Open Watcom tree is missing: run tools/ow-fetch.sh; this test does not skip");
    assert!(
        std::env::var_os("LLRM_CLANG_M16").is_some(),
        "LLRM_CLANG_M16 is not set: this test needs it and does not skip"
    );
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("divlive-{name}-{}", std::process::id()));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.c"), source).unwrap();
    let run = |mode: &str, out: &str| {
        let result = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
            .env("LLRM_CC_TARGET", "x86_32-linux")
            .env(
                "LLRM_LLC",
                std::env::var_os("LLRM_LLC").unwrap_or_else(|| target_dir::of(&root).join("release/llrm-llc").into()),
            )
            .args([flag, "-w", mode, "-march=i486", "-mabi=sysv", "-o"])
            .arg(dir.join(out))
            .arg(dir.join("d.c"))
            .output()
            .unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    };
    run("-S", "d.s");
    run("-c", "d.o");
    (std::fs::read_to_string(dir.join("d.s")).unwrap(), std::fs::metadata(dir.join("d.o")).unwrap().len())
}

#[test]
fn test_m32_os_narrow_divide_with_five_live_values_is_a_multiply() {
    let (listing, size) = compile("nest", NEST, "-Os");
    assert!(!listing.contains("div cx"), "the init nest divides:\n{listing}");
    assert!(size <= 1144, "object is {size} B; the divide gave 1148, the multiply 1140");
}

#[test]
fn test_m32_os_lone_narrow_divide_stays_a_divide() {
    let (listing, _) = compile("lone", LONE, "-Os");
    assert!(listing.contains("div cx"), "DIV-OPTSIZE-1: a lone divide keeps its div:\n{listing}");
}
