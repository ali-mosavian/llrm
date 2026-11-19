//! The price of a run: `main`'s clocks through its calls, not the sum of every
//! function. `fill_nest` was priced 783,928 clocks for a run of 436,825: the
//! function main had inlined was counted again as the copy kept for the world,
//! and a trial that stopped inlining it looked 382,304 cheaper (measured
//! 8,855).

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::weights::run_cost;

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|one| (*one).to_owned()).collect()
}

const LEAF: &str = "leaf:\n  lda #1\n  rts\n";
const MAIN: &str = "main:\n  jsr leaf\n  rts\n";
const SPARE: &str = "spare:\n  lda #2\n  lda #3\n  rts\n";

#[test]
fn a_call_costs_its_callee_and_an_uncalled_function_adds_no_clocks() {
    let pieces: Vec<String> = [MAIN, LEAF, SPARE].iter().map(|one| (*one).to_owned()).collect();
    let weights = vec![vec![1u64]; 3];
    let (clocks, bytes) = run_cost(Cpu::Mos6502, &names(&["main", "leaf", "spare"]), &pieces, &weights).unwrap();
    let (leaf_only, _) = run_cost(Cpu::Mos6502, &names(&["main", "leaf"]), &pieces[..2], &weights[..2]).unwrap();
    assert_eq!(clocks, leaf_only, "the function nothing calls ran");
    assert_eq!(leaf_only, 6 + (2 + 6) + 6, "jsr, the callee's lda and rts, rts");
    let (_, bytes_without) = run_cost(Cpu::Mos6502, &names(&["main", "leaf"]), &pieces[..2], &weights[..2]).unwrap();
    assert!(bytes > bytes_without, "the bytes of every function count");
}

/// `memmove_overlap` was kept at 1.6 clk/B: its rolled form called `__mulsi3`
/// in loops and the call was priced as a `jsr` alone, so a trial that dropped
/// the loops looked 10,708 clocks cheaper (measured 5,707).
#[test]
fn a_call_of_a_runtime_routine_costs_the_routine() {
    let pieces = vec!["main:\n  jsr __mulsi3\n  rts\n".to_owned()];
    let (clocks, _) = run_cost(Cpu::Mos6502, &names(&["main"]), &pieces, &[vec![1u64]]).unwrap();
    assert!(clocks > 1000, "a 32-bit multiply priced at {clocks} clocks");
}
