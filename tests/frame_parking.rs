//! The frame register holds a value where nothing touches the frame, saved by a
//! `push` before and restored by a `pop` after. A loop of four pointers needs
//! one more base register than bx, si and di: it reloaded one from the frame
//! every trip.

use std::process::Command;

const FOUR_POINTERS: &str = "void acc4(short *a, short *b, short *c, short *d, unsigned n)
{
    unsigned i;
    for (i = 0; i < n; ++i) { *a += *b; *b += *c; *c += *d; *d += *a; }
}
";

fn listing(flags: &[&str]) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("acc4.c");
    std::fs::write(&source, FOUR_POINTERS).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(scratch.path())
        .args(["-O2", "-m16", "-march=i486", "-S", "-o", "acc4.s"])
        .args(flags)
        .arg(&source)
        .output()
        .unwrap();
    assert!(output.status.success(), "{flags:?}: {}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("acc4.s")).unwrap()
}

fn lines<'a>(
    text: &'a str,
    what: &str,
) -> Vec<&'a str> {
    text.lines().map(str::trim).filter(|line| *line == what).collect()
}

/// Reloading a pointer from `[bp-N]` in the loop cost a memory operand per
/// trip.
#[test]
fn test_a_loop_needing_one_more_base_register_than_is_free_holds_a_pointer_in_the_frame_register() {
    let text = listing(&[]);
    assert!(!text.contains("[bp-"), "a pointer was reloaded from the frame:\n{text}");
    assert!(text.contains("[bp]"), "no value in the frame register:\n{text}");
    assert_eq!(lines(&text, "push bp").len(), 2, "the frame's and the parked one:\n{text}");
    assert_eq!(lines(&text, "pop bp").len(), 2, "the parked one's and the frame's:\n{text}");
}

/// Nothing between the push that parks the register and its pop names the
/// frame.
#[test]
fn test_the_parked_stretch_names_no_frame_cell() {
    let text = listing(&[]);
    let mut parked = false;
    let mut seen = 0;
    for line in text.lines().map(str::trim) {
        if line == "push bp" {
            seen += 1;
            parked = seen == 2;
        } else if line == "pop bp" {
            parked = false;
        } else if parked {
            assert!(!line.contains("[bp+") && !line.contains("[bp-"), "a frame cell inside the parked stretch: {line}");
        }
    }
}

/// `-g` never changes the code, so it does not change what is parked.
#[test]
fn test_parking_is_the_same_with_and_without_debug_info() {
    assert_eq!(listing(&[]), listing(&["-g"]));
}

/// Where the stack reaches the data group a pointer through the frame register
/// needs no prefix; where it is not known to, the cell names its data segment.
#[test]
fn test_a_near_cell_through_the_frame_register_names_its_segment_unless_the_stack_is_data() {
    assert!(listing(&["-mno-stack-is-data"]).contains("ds:[bp]"));
    assert!(!listing(&["-mstack-is-data"]).contains("ds:[bp]"));
}

/// A value in the frame register named by an `lea` (`lea cx,[ebp+ebp+9]`) had
/// no byte price: the encoder read the frame register in an address as the
/// frame, and the verified compile of matmul stopped on it.
#[test]
fn test_a_value_in_the_frame_register_is_priced_in_an_address_sum() {
    let scratch = tempfile::tempdir().unwrap();
    let source = format!("{}/bench/matmul/matmul.bas", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .env("LLRM_VERIFY", "1")
        .env("LLRM_CHECK_PARKED", "1")
        .args([
            "--dialect",
            "qb45",
            "--runtime",
            "qb45",
            "-Os",
            "-fno-inline-functions",
            "-fno-inline-functions-called-once",
        ])
        .args(["-o", "a.obj", &source])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
