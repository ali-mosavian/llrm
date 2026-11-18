//! `mulprice CONSTANT...`: the priced model for each constant on the 68020:
//! chain and `muls.l` (bytes, clocks) and the decision at -O1 (strict), -Os
//! (size) and -O2 (rate).
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::dagisel::mulprice::{chain, decomposes, multiply};
use llrm_m68k::timing::Price;

fn main() {
    println!("| x* | chain B/clk | muls.l B/clk | -Os | -O1..-O3 rate |");
    for c in std::env::args().skip(1).map(|a| a.parse::<u32>().expect("a constant")) {
        let cpu = Cpu::M68020;
        let fmt = |v: Option<(usize, u32)>| v.map_or("none".to_owned(), |(b, c)| format!("{b} / {c}"));
        let pick = |p| if decomposes(cpu, p, 32, u128::from(c)) { "chain" } else { "muls.l" };
        println!(
            "| {c} | {} | {} | {} | {} |",
            fmt(chain(cpu, c)),
            fmt(Some(multiply(cpu))),
            pick(Price::Size),
            pick(Price::Rate)
        );
    }
}
