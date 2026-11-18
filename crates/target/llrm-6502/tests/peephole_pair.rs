//! The priced text and the kept text ran the whole pass list each, though the
//! passes before `drop_known_loads` are the same for both: every function paid
//! for them twice (four times where a return block was copied). `run_pair`
//! runs them once and gives what two `run_with` calls give.

use llrm_6502::asm::Asm;
use llrm_6502::peephole::{prefix_runs, run_pair, run_with};

/// Known loads differ between the two texts; the return block is copied.
const CODE: [&str; 3] = [
    "  lda #3\n  sta rc4\n  ldx #3\n  stx rc5\n  lda #3\n  sta rc6\n  rts\n",
    "  ldx rc6\n  bne b21\nb23:\n  lda #0\n  sta rc7\n  jmp b3\nb21:\n  lda #0\n  sta rc7\n  tay\nb2:\n  lda (rc4),y\n  clc\n  adc rc7\n  sta rc7\n  iny\n  cpy rc6\n  bne b2\nb3:\n  lda rc7\n  rts\n",
    "  lda #1\n  sta rc4\n  lda rc5\n  sta rc6\n  lda rc6\n  rts\n",
];

fn two_runs(text: &str) -> (String, String) {
    let (mut priced, mut kept) = (Asm::parse(text), Asm::parse(text));
    run_with(&mut priced, false);
    run_with(&mut kept, true);
    (priced.text(), kept.text())
}

#[test]
fn a_pair_is_what_two_runs_are() {
    for text in CODE {
        let (priced, kept) = run_pair(&Asm::parse(text));
        assert_eq!((priced.text(), kept.text()), two_runs(text), "{text}");
    }
    let (priced, kept) = two_runs(CODE[0]);
    assert_ne!(priced, kept, "the premise: the known load is the difference");
}

#[test]
fn a_pair_runs_the_shared_passes_once() {
    let before = prefix_runs();
    run_pair(&Asm::parse(CODE[0]));
    assert_eq!(prefix_runs() - before, 1, "no return block is copied: one run for both texts");
    let before = prefix_runs();
    run_pair(&Asm::parse(CODE[1]));
    assert_eq!(prefix_runs() - before, 2, "a copied return block makes a second text to run them on");
}
