//! `timing::estimate_calling` reads the text once, not once per branch.

use llrm_6502::dagisel::hooks::Cpu;

/// A branch looked its label up with a scan of the whole text: a function of
/// 10,000 lines with 2,000 branches took 22% of the CPU time of memory_expansion
/// at -O1 (54 s) in `estimate_calling` alone. The lines looked at are now the
/// text's, once.
#[test]
fn the_labels_of_a_text_are_found_once_not_once_per_branch() {
    let mut text = String::new();
    for n in 0..2000 {
        text.push_str(&format!("f_b{n}:\n  lda #1\n  sta rc0\n  beq f_b{n}\n  nop\n"));
    }
    let lines = text.lines().count();
    let before = llrm_6502::timing::label_scans();
    llrm_6502::timing::estimate_calling(Cpu::Mos6502, &text, &[], None).expect("prices");
    let scans = llrm_6502::timing::label_scans() - before;
    assert!(scans <= 4 * lines, "{scans} lines looked at for {lines} lines");
}
