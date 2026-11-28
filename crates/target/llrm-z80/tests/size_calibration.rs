//! `function_bytes`, the estimate the SZ2 inline trial decides by, against the
//! bytes the z80 backend writes, over the front-end IR of the 6502's
//! `tests/data_size` (16-bit, one `msp430` triple: the corpus both targets
//! are measured on). `LLRM_SIZE_FIT=1` prints one row a function for
//! `tools/size_fit.py`.
//!
//! The band is on the aggregate and the median program. `LLRM_SIZE_DIR` and
//! `LLRM_SIZE_LEVEL` audit another directory of `.ll` files at another level
//! (the fit over the corpus, `tools/z80_size_fit.py`) and only print the rows.

use llrm_target::Target;
use llrm_z80::driver::{self, Level, Request};
use llrm_z80::target::Z80;

/// estimate / written.
const BAND: std::ops::RangeInclusive<f64> = 0.7..=1.25;

fn audits() -> Vec<(String, i64, i64)> {
    let dir = std::env::var_os("LLRM_SIZE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../llrm-6502/tests/data_size"));
    let level = std::env::var("LLRM_SIZE_LEVEL").map_or(Level::Os, |text| Level::parse(&text).expect("a level"));
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ll"))
        .collect();
    files.sort();
    assert!(files.len() >= 80 || std::env::var_os("LLRM_SIZE_DIR").is_some(), "{} fixtures", files.len());
    files
        .iter()
        .filter_map(|path| {
            let name = path.file_stem().unwrap().to_str().unwrap().to_string();
            let text = std::fs::read_to_string(path)
                .unwrap()
                .lines()
                .filter(|line| !line.starts_with("target datalayout"))
                .fold(format!("target datalayout = \"{}\"\n", Z80.layout().datalayout), |text, line| {
                    text + line + "\n"
                });
            // a program the backend refuses is no row of the audit
            let rows = match driver::size_audit(&text, &Request::new(level)) {
                Ok(rows) => rows,
                Err(why) => {
                    if std::env::var_os("LLRM_SIZE_FIT").is_some() {
                        eprintln!("refused {name}: {}", why.lines().next().unwrap_or(""));
                    }
                    return None;
                }
            };
            if std::env::var_os("LLRM_SIZE_FIT").is_some() {
                for row in &rows {
                    println!("row {name} {} {} {} {}", row.name, row.model, row.written, row.counts);
                }
            }
            Some((name, rows.iter().map(|row| row.model).sum(), rows.iter().map(|row| row.written).sum()))
        })
        .collect()
}

#[test]
fn the_estimate_tracks_the_bytes_written() {
    let rows = audits();
    if std::env::var_os("LLRM_SIZE_DIR").is_some() {
        return;
    }
    assert!(rows.len() >= 60, "{} programs audited", rows.len());
    let aggregate = rows.iter().map(|row| row.1).sum::<i64>() as f64 / rows.iter().map(|row| row.2).sum::<i64>() as f64;
    let mut ratios: Vec<f64> = rows.iter().map(|row| row.1 as f64 / row.2 as f64).collect();
    ratios.sort_by(f64::total_cmp);
    let median = ratios[ratios.len() / 2];
    let failures: Vec<String> = [("aggregate", aggregate), ("median program", median)]
        .into_iter()
        .filter(|(_, ratio)| !BAND.contains(ratio))
        .map(|(what, ratio)| format!("z80 -Os {what}: estimate / written {ratio:.2}, band {BAND:?}"))
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}
