//! A 16-bit offset counter that a masked address reads is the one counter of
//! the loop at m32, as it is at m16.

use std::process::Command;

/// QBasic's flat PEEK/POKE offset `(x * 2 + y * 640) AND &HFFFF&` of 16-bit
/// INTEGERs, y in 0..99 and x in 0..159: the offset starts at `y * 640` (at
/// most 63360, inside 16 bits unsigned) and walks by 2, so `movsx; and`, now
/// one `zext`, is the wide counter itself. The loop kept that counter beside a
/// second one for the array element (TSC's copy loop: three counters where
/// m16 had one register); it has one offset counter now.
const SOURCE: &str = "DEFINT A-Z
DIM SHARED tmpshit(159, 99) AS INTEGER
DIM SHARED vbase AS LONG
vbase = &HA0000
FOR y = 0 TO 99
  FOR x = 0 TO 159
    POKE vbase + ((x * 2 + y * 640) AND &HFFFF&), tmpshit(x, y)
  NEXT
NEXT
";

#[test]
fn test_a_masked_offset_counter_and_the_array_walk_share_one_register_at_m32() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("T.BAS"), SOURCE).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .current_dir(scratch.path())
        .args(["T.BAS", "--dialect", "qb45", "-fqb-runtime=llrm", "-O2", "-m32", "-march=i486", "-S", "-o", "t.s"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = std::fs::read_to_string(scratch.path().join("t.s")).unwrap();
    let from = text.find("$QB$MAIN proc").expect("the main program");
    let body: Vec<&str> =
        text[from..].lines().take_while(|line| !line.contains("$QB$MAIN endp")).map(str::trim).collect();
    let steps = body.iter().filter(|line| line.starts_with("add e") && line.ends_with(", 2")).count();
    assert_eq!(steps, 1, "one offset counter steps by 2: {body:#?}");
    // A zero-extending load of the array element is not an extended counter.
    let extended =
        |line: &&str| (line.starts_with("movsx e") || line.starts_with("movzx e")) && !line.contains("ptr [");
    assert!(!body.iter().any(|line| extended(&line)), "{body:#?}");
}
