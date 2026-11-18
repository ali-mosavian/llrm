//! `function_bytes`, the estimate the SZ2 inline trial decides by, against the
//! bytes the 6502 backend writes.
//!
//! It priced every operation as one native byte whatever its width: over the 86
//! programs of tests/data_size (the front-end IR of tools/6502/pass82.txt,
//! `tools/6502/size_fixtures.sh`) the module came to 0.39 of the written bytes
//! at -Os (48,705 B against 126,491 B), an `i64` shift 1/6, and the
//! `memcpy_libcall` trial priced two copies of a 200-trip loop above the
//! callee's two calls: the object was 58 B larger for not inlining them.
//!
//! The band is on the aggregate and the median program; the programs it is far
//! off on (soft float, `i64` multiply and divide, variable `i64` shifts:
//! routines the table has no class for) are the residue to be priced next.

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{self, Level, Request};
use llrm_6502::target::Mos;
use llrm_target::Target;

/// estimate / written.
const BAND: std::ops::RangeInclusive<f64> = 0.7..=1.25;

fn audits(cpu: Cpu) -> Vec<(String, i64, i64)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_size");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ll"))
        .collect();
    files.sort();
    assert!(files.len() >= 80, "{} fixtures", files.len());
    files
        .iter()
        .map(|path| {
            let name = path.file_stem().unwrap().to_str().unwrap().to_string();
            // The fixtures carry no datalayout: the target's own, whatever it is this month.
            let text = std::fs::read_to_string(path)
                .unwrap()
                .lines()
                .filter(|line| !line.starts_with("target datalayout"))
                .fold(format!("target datalayout = \"{}\"\n", Mos.layout().datalayout), |text, line| text + line + "\n");
            let mut request = Request::new(Level::Os);
            request.cpu = cpu;
            let rows = driver::size_audit(&text, &request).unwrap_or_else(|why| panic!("{name}: {why}"));
            if cpu == Cpu::Mos6502 && std::env::var_os("LLRM_SIZE_FIT").is_some() {
                for row in &rows {
                    println!("row {name} {} {} {} {}", row.name, row.model, row.written, row.counts);
                }
            }
            (name, rows.iter().map(|row| row.model).sum(), rows.iter().map(|row| row.written).sum())
        })
        .collect()
}

#[test]
fn the_estimate_tracks_the_bytes_written_at_both_cpus() {
    let mut failures = Vec::new();
    for (name, cpu) in [("6502", Cpu::Mos6502), ("65C02", Cpu::W65c02)] {
        let rows = audits(cpu);
        let aggregate =
            rows.iter().map(|row| row.1).sum::<i64>() as f64 / rows.iter().map(|row| row.2).sum::<i64>() as f64;
        let mut ratios: Vec<f64> = rows.iter().map(|row| row.1 as f64 / row.2 as f64).collect();
        ratios.sort_by(f64::total_cmp);
        let median = ratios[ratios.len() / 2];
        for (what, ratio) in [("aggregate", aggregate), ("median program", median)] {
            if !BAND.contains(&ratio) {
                failures.push(format!("{name} -Os {what}: estimate / written {ratio:.2}, band {BAND:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// `function_bytes` read a call's operands from the second, so the first
/// argument was dropped and the callee (the last operand) priced as a word: a
/// call of `f(i32)` cost what a call of `f(i8)` did. Every argument is priced,
/// the callee not (CALL-ARGS-SKIP).
#[test]
fn a_call_is_priced_by_all_of_its_arguments_and_not_its_callee() {
    let model = |ty: &str| {
        let ir = format!(
            "target datalayout = \"{}\"\ndeclare void @f({ty})\ndefine void @m() {{\n  call void @f({ty} 5)\n  ret void\n}}\n",
            Mos.layout().datalayout
        );
        let mut request = Request::new(Level::Os);
        request.cpu = Cpu::Mos6502;
        let rows = driver::size_audit(&ir, &request).unwrap_or_else(|why| panic!("{ty}: {why}"));
        rows.iter().find(|row| row.name == "m").unwrap_or_else(|| panic!("no m among {} rows", rows.len())).model
    };
    let costs = llrm_6502::opcosts::trial_bytes(Cpu::Mos6502);
    assert_eq!(model("i32") - model("i8"), 3 * costs.argument, "three more argument bytes in the call");
}
