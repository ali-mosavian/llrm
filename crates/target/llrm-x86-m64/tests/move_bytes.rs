//! The sizes the coalescer prices a remade constant by.

use llrm_target::Target;

/// m64 `jpeg_fixed` -Os grew 42 B: `operand_bytes` (8, the word) was taken for
/// the size an instruction has without a prefix (4), so a dword counted a
/// prefix and every dword zero was remade (`xor` 2 B against a "3 B" copy).
#[test]
fn a_dword_is_the_plain_operand_at_m64() {
    let bytes = llrm_x86_m64::M64.move_bytes().expect("x86 states its encodings");
    assert_eq!(bytes.plain, 4);
    assert_eq!((bytes.copy)(4, bytes.plain), 2);
    assert_eq!((bytes.literal)(0, false, 4, bytes.plain), 2, "a tie with the copy");
    assert_eq!((bytes.copy)(8, bytes.plain), 3, "REX.W");
}
