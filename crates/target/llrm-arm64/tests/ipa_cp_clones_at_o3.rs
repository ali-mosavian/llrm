//! gcc's `-fipa-cp-clone` is on at -O3 only. arm64 made no clones: queens -O3
//! ran 113,742 instructions where its clones of `place` (its row known, called
//! once, so inlined into `bench_queens`) run 59,543 (gcc 71,673).

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn instructions_of_bench_queens(level: Level) -> usize {
    let asm = driver::assembly(include_str!("data/ipa_cp_queens.ll"), Platform::Linux, level).expect("lowers");
    let body = asm.lines().skip_while(|line| *line != "bench_queens:").skip(1);
    let is_symbol = |line: &&str| line.ends_with(':') && !line.starts_with(".L") && !line.starts_with('\t');
    body.take_while(|line| !is_symbol(line)).filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count()
}

#[test]
fn place_is_cloned_for_its_constant_row_at_o3_and_not_at_o2() {
    let (o2, o3) = (instructions_of_bench_queens(Level::O2), instructions_of_bench_queens(Level::O3));
    assert!(o2 < 100, "-O2 `bench_queens` has {o2} instructions");
    assert!(o3 > 200, "-O3 `bench_queens` has {o3} instructions, so `place` was not cloned");
}
