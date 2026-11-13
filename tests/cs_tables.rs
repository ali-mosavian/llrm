//! At m16 a read-only table one procedure reads follows that procedure in its
//! code segment and is read through `cs:`; before, a switch's 12-byte lookup
//! table and every jump table sat in `_DATA`, which the bench counts as data
//! (bench/switch: 12 B over the bar of 0, six cells).

mod common;

fn listing(source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("t.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m16", "-O2", "-S", "-o", "t.s", "t.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(scratch.path().join("t.s")).unwrap()
}

/// What the listing puts in the near data segment, before the code.
fn data(text: &str) -> &str {
    let start = text.find(".data").map_or(0, |at| at + ".data".len());
    &text[start..text.find(".code").expect("a code segment")]
}

const LOOKUP: &str = "long weight(long k)\n{ switch (k) {\n\
    case 0: return 3; case 1: return 14; case 2: return 15; case 3: return 92;\n\
    case 4: return 65; case 5: return 35; case 6: return 89; case 7: return 79;\n\
    case 8: return 32; case 9: return 38; case 10: return 46; case 11: return 26;\n\
    default: return 1; } }\n";

const JUMPS: &str = "long step(int op, long acc)\n{ switch (op) {\n\
    case 0: acc += 3; break; case 1: acc *= 2; break; case 2: acc -= 7; break;\n\
    case 3: acc ^= 0x55; break; case 4: acc = acc << 1 | 1; break; case 5: acc /= 3; break;\n\
    default: acc += 1000; break; }\n return acc; }\n";

#[test]
fn test_a_switch_lookup_table_is_not_in_data() {
    let text = listing(LOOKUP);
    assert!(data(&text).trim().is_empty(), "the lookup table is in _DATA ({} B of data): {text}", data(&text).len());
    assert!(text.contains("byte ptr cs:G$"), "the table is not read through cs: {text}");
}

#[test]
fn test_a_jump_table_is_not_in_data() {
    let text = listing(JUMPS);
    assert!(data(&text).trim().is_empty(), "the jump table is in _DATA: {text}");
    assert!(text.contains("jmp word ptr cs:$J"), "the jump is not through cs: {text}");
}

#[test]
fn test_a_table_whose_address_is_taken_stays_in_data() {
    let text = listing(
        "extern void take(const char *);\n\
         long pick(long k) { const char *t = \"\\3\\16\\17\\134\\101\\43\"; take(t); return t[k % 6]; }\n",
    );
    assert!(data(&text).contains("db "), "an address that escapes cannot be in code: {text}");
}

#[test]
fn test_a_table_two_procedures_read_stays_in_data() {
    let text = listing(&format!("{LOOKUP}{}", LOOKUP.replace("weight", "heavy")));
    assert!(data(&text).contains("db "), "a table shared by two procedures is in _DATA: {text}");
    assert!(!text.contains("cs:G$"), "a table shared by two segments is read through one's cs: {text}");
}
