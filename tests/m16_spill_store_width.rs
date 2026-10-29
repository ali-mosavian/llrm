//! A spill store is the width of its slot.

mod common;

const SOURCE: &str = r#"extern int input_read(char *buffer, int count);
static unsigned char nxt[80 * 32], classof[256], hit[80];

long scan(long *checksum)
{
    char buffer[1024];
    int s = 0, n, i, inline_hit = 0;
    long lines = 0, sum = 0, pos = 0;

    while ((n = input_read(buffer, 1024)) > 0) {
        for (i = 0; i < n; ++i) {
            unsigned char b = (unsigned char)buffer[i];
            s = nxt[s * 32 + classof[b]];
            if (hit[s]) {
                inline_hit = 1;
                sum = ((sum << 1) + pos + hit[s]) & 0x1FFFFFFFL;
            }
            if (b == 10) {
                lines += inline_hit;
                inline_hit = 0;
            }
            ++pos;
        }
    }
    *checksum = sum;
    return lines + inline_hit;
}
"#;

/// grep.c under the DAG selector hung: a word value defined by a dword zero
/// was spilled as `mov dword ptr [bp-20], 0` into a 2-byte slot and zeroed the
/// loop bound beside it.
#[test]
fn test_a_stored_cell_does_not_cover_the_next_cell() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.c"), SOURCE).unwrap();
    let output = common::cc::llrm_cc()
        .current_dir(scratch.path())
        .args(["a.c", "-m16", "-march=i386", "-O2", "--isel=dag", "-S", "-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let asm = String::from_utf8_lossy(&output.stdout);
    // the cells `[bp-N]` the function touches, with the widest access to each
    let mut cells: Vec<(i32, i32)> = Vec::new();
    for line in asm.lines() {
        let Some(at) = line.find("ptr [bp-") else { continue };
        let width = if line[..at].ends_with("dword ") {
            4
        } else if line[..at].ends_with("word ") {
            2
        } else {
            continue;
        };
        let offset: i32 = line[at + 8..].split(']').next().unwrap().parse().unwrap();
        match cells.iter_mut().find(|cell| cell.0 == offset) {
            Some(cell) => cell.1 = cell.1.max(width),
            None => cells.push((offset, width)),
        }
    }
    for &(offset, width) in &cells {
        for &(other, _) in &cells {
            assert!(
                !(other < offset && other > offset - width),
                "[bp-{offset}] of {width} bytes covers [bp-{other}]:\n{asm}"
            );
        }
    }
}
