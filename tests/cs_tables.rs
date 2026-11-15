//! At m16 a read-only table one procedure reads follows that procedure in its
//! code segment and is read through `cs:`; before, a switch's 12-byte lookup
//! table and every jump table sat in `_DATA`, which the bench counts as data
//! (bench/switch: 12 B over the bar of 0, six cells).

mod common;

fn listing(source: &str) -> String {
    listing_at("-O2", source)
}

fn listing_at(level: &str, source: &str) -> String {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("t.c"), source).unwrap();
    let output = common::cc::llrm_c()
        .current_dir(scratch.path())
        .args(["-m16", level, "-S", "-o", "t.s", "t.c"])
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

/// At -Os the move cost +1 B per load site (QCport -Os grew, user rule: no growth), so the
/// table stays in `_DATA`; -O1 and up still move it.
#[test]
fn test_a_switch_lookup_table_stays_in_data_at_os_and_moves_at_o1() {
    let os = listing_at("-Os", LOOKUP);
    assert!(data(&os).contains("db "), "-Os moved the table into code: {os}");
    assert!(!os.contains("cs:G$"), "-Os reads a table through cs: {os}");
    let o1 = listing_at("-O1", LOOKUP);
    assert!(data(&o1).trim().is_empty(), "-O1 left the table in _DATA: {o1}");
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

/// A string a pointer in `_DATA` names was moved after its one reading
/// procedure: the pointer, followed through DS, read garbage
/// (run/c/table_named_by_data).
#[test]
fn test_a_table_a_data_pointer_names_stays_in_data() {
    let text = listing(
        "const char *names[2] = {\"abc\", \"def\"};\n\
         __attribute__((noinline)) long first(int i) { return \"abc\"[i]; }\n\
         __attribute__((noinline)) long second(int i) { return names[0][i]; }\n",
    );
    assert!(!text.contains("cs:G$"), "a table a pointer in data names is read through cs: {text}");
}

/// A Nib program is laid out apart from assembly (`lay_out`), which wrote the
/// table again in `_DATA`: 12 B of data beside the 12 in code, and the label
/// defined twice.
#[test]
fn test_a_nib_table_is_defined_once_and_not_in_data() {
    let scratch = tempfile::tempdir().unwrap();
    let source = format!("{}/bench/switch/switch.nib", env!("CARGO_MANIFEST_DIR"));
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
        .current_dir(scratch.path())
        .args([&source, "-m16", "-O2", "--procedure-segments", "-S", "-o", "t.s"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("t.s")).unwrap();
    assert!(!data(&text).contains(" label "), "the Nib table is in _DATA: {text}");
    assert_eq!(text.matches("switch$table$0 label byte").count(), 1, "{text}");
}

/// A table of 16-bit entries (aligned, so not a lookup of bytes) moves too.
#[test]
fn test_a_word_lookup_table_is_not_in_data() {
    let text = listing(
        "int w(int k){ switch(k){case 0:return 300;case 1:return 1400;case 2:return 2500;case 3:return 920;\n\
         case 4:return 650;case 5:return 3500;case 6:return 890;case 7:return 790;case 8:return 320;\n\
         case 9:return 380;default:return 1;}}\n",
    );
    assert!(data(&text).trim().is_empty(), "the word table is in _DATA: {text}");
    assert!(text.contains("word ptr cs:G$"), "{text}");
}
