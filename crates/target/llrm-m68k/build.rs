//! Generates m68k's register ids and `REGISTER_INFO` from its
//! `registers.regs`.

fn main() {
    let path = "src/registers.regs";
    println!("cargo:rerun-if-changed={path}");
    let text = std::fs::read_to_string(path).expect("registers.regs");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ids = llrm_target::registers::constants(&text, "RegId").unwrap_or_else(|error| panic!("{path}: {error}"));
    std::fs::write(out.join("register_ids.rs"), ids).unwrap();
    let info = llrm_target::registers::source_with(&text, "RegId", "reg", &llrm_lir::registers::CLASSES, "rows")
        .unwrap_or_else(|error| panic!("{path}: {error}"));
    std::fs::write(out.join("register_info.rs"), info).unwrap();
    timing_tables();
}

#[path = "src/timing/csv.rs"]
mod csv;

/// The 68020 timing tables (`src/timing/m68020/*.csv`) as statics: parsed here
/// once, not by every compile at its first price.
fn timing_tables() {
    use std::fmt::Write;
    let dir = "src/timing/m68020";
    println!("cargo:rerun-if-changed={dir}");
    println!("cargo:rerun-if-changed=src/timing/csv.rs");
    let read = |name: &str| std::fs::read_to_string(format!("{dir}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut out = String::new();

    // instruction rows 8.2.7 to 8.2.17: (instruction key, operands key, mark,
    // cache cell)
    let mut rows = Vec::new();
    for n in 7..=17 {
        for line in read(&format!("t8.2.{n}.csv")).lines().skip(1) {
            let f = csv::split_csv(line);
            rows.push((csv::normalize(&f[0]), csv::normalize(&f[1]), f[2].clone(), f[4].clone()));
        }
    }
    out.push_str("pub(super) static ROWS: &[(&str, &str, &str, &str)] = &[\n");
    for r in &rows {
        writeln!(out, "    ({:?}, {:?}, {:?}, {:?}),", r.0, r.1, r.2, r.3).unwrap();
    }
    out.push_str("];\n");
    // the first row of each (instruction, operands), sorted for a binary search
    let mut index: Vec<(&str, &str, usize)> = Vec::new();
    for (at, r) in rows.iter().enumerate() {
        if !index.iter().any(|i| i.0 == r.0 && i.1 == r.1) {
            index.push((&r.0, &r.1, at));
        }
    }
    index.sort();
    out.push_str("pub(super) static ROW_INDEX: &[(&str, &str, usize)] = &[\n");
    for i in &index {
        writeln!(out, "    ({:?}, {:?}, {}),", i.0, i.1, i.2).unwrap();
    }
    out.push_str("];\n");

    // effective address tables 8.2.1 to 8.2.5: (mode key, cache cell)
    for (name, file) in
        [("FETCH", 1), ("FETCH_IMMEDIATE", 2), ("CALCULATE", 3), ("CALCULATE_IMMEDIATE", 4), ("JUMP", 5)]
    {
        writeln!(out, "pub(super) static {name}: &[(&str, &str)] = &[").unwrap();
        for line in read(&format!("t8.2.{file}.csv")).lines().skip(1) {
            let f = csv::split_csv(line);
            writeln!(out, "    ({:?}, {:?}),", csv::normalize(&f[0]), f[2]).unwrap();
        }
        out.push_str("];\n");
    }

    // the move matrix 8.2.6, and its first column and row by label
    let moves: Vec<Vec<String>> = read("t8.2.6-cache.csv").lines().map(csv::split_csv).collect();
    out.push_str("pub(super) static MOVES: &[&[&str]] = &[\n");
    for row in &moves {
        writeln!(out, "    &{:?},", row.iter().map(String::as_str).collect::<Vec<_>>()).unwrap();
    }
    out.push_str("];\n");
    let label = |text: &str| csv::normalize(text.split(" or ").next().unwrap_or(text));
    for (name, labels) in [
        ("MOVE_COLUMNS", moves[0].iter().map(|l| label(l)).collect::<Vec<_>>()),
        ("MOVE_ROWS", moves[1..].iter().map(|r| label(&r[0])).collect::<Vec<_>>()),
    ] {
        let mut first: Vec<(&str, usize)> = Vec::new();
        for (at, l) in labels.iter().enumerate() {
            if !first.iter().any(|f| f.0 == l) {
                first.push((l, at));
            }
        }
        first.sort();
        writeln!(out, "pub(super) static {name}: &[(&str, usize)] = &[").unwrap();
        for f in &first {
            writeln!(out, "    ({:?}, {}),", f.0, f.1).unwrap();
        }
        out.push_str("];\n");
    }
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("m68020_tables.rs"), out).unwrap();
}
