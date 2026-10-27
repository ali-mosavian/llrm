//! `runtime.toml` as Rust data: every process that asked for a routine's
//! contract parsed the 64 KB table first (8 Minstr, 3% of a C program's
//! compile). The rows keep the file's order; `BY_NAME` is sorted for a binary
//! search.

use std::fmt::Write;

fn main() {
    println!("cargo::rerun-if-changed=../../crates/frontends/qb/runtime.toml");
    println!("cargo::rerun-if-changed=build.rs");
    let text = std::fs::read_to_string("../../crates/frontends/qb/runtime.toml").expect("runtime.toml reads");
    let rows: toml::Table = text.parse().expect("runtime.toml parses");
    let string =
        |row: &toml::Table, key: &str| row[key].as_str().unwrap_or_else(|| panic!("{key} is a str")).to_owned();
    let list = |row: &toml::Table, key: &str| -> String {
        let all: Vec<String> = row[key]
            .as_array()
            .unwrap_or_else(|| panic!("{key} is a list"))
            .iter()
            .map(|one| format!("{:?}", one.as_str().expect("a str")))
            .collect();
        format!("&[{}]", all.join(", "))
    };
    let option = |row: &toml::Table, key: &str, body: String| {
        if row.contains_key(key) { format!("Some({body})") } else { "None".to_owned() }
    };
    let mut out = String::from("pub static ROWS: &[Row] = &[\n");
    let mut names = Vec::new();
    for (name, row) in rows.iter().filter(|(name, _)| name.starts_with("B$")) {
        let row = row.as_table().expect("a row is a table");
        let flag = |key: &str| row[key].as_bool().unwrap_or_else(|| panic!("{key} is a bool"));
        let captures = if row.contains_key("captures") {
            format!("Some({:?})", string(row, "captures"))
        } else {
            "None".to_owned()
        };
        let flags_result = row.get("flags_result").is_some_and(|one| one.as_bool().expect("flags_result is a bool"));
        let calls_back = row.get("calls_back").is_some_and(|one| one.as_bool().expect("calls_back is a bool"));
        let optional_str = |key: &str| {
            option(row, key, if row.contains_key(key) { format!("{:?}", string(row, key)) } else { String::new() })
        };
        let optional_list =
            |key: &str| option(row, key, if row.contains_key(key) { list(row, key) } else { String::new() });
        writeln!(
            out,
            "    Row {{ name: {name:?}, cleanup: {}, control: {:?}, enters_user_code: {}, raises_error: {}, error_handling: {}, writes: {:?}, reads: {:?}, clobbers: {}, established: {}, evidence: {:?}, documented: {}, inputs: {}, direct_writes: {}, direct_reads: {}, captures: {captures}, calls_back: {calls_back}, flags_result: {flags_result} }},",
            row["cleanup"].as_integer().expect("cleanup is an int"),
            string(row, "control"),
            flag("enters_user_code"),
            flag("raises_error"),
            flag("error_handling"),
            string(row, "writes"),
            string(row, "reads"),
            list(row, "clobbers"),
            flag("established"),
            string(row, "evidence"),
            optional_list("documented"),
            optional_list("inputs"),
            optional_str("direct_writes"),
            optional_str("direct_reads"),
        )
        .expect("a string takes text");
        names.push(name.clone());
    }
    out.push_str("];\n\npub static BY_NAME: &[(&str, usize)] = &[\n");
    let mut sorted: Vec<(usize, &String)> = names.iter().enumerate().collect();
    sorted.sort_by(|one, more| one.1.cmp(more.1));
    for (at, name) in sorted {
        writeln!(out, "    ({name:?}, {at}),").expect("a string takes text");
    }
    out.push_str("];\n");
    out.push_str("\n/// The audited stack cleanup per routine, by group: (group, [(routine, bytes)]).\npub static POPS: &[(&str, &[(&str, i64)])] = &[\n");
    if let Some(groups) = rows.get("pops").and_then(|one| one.as_table()) {
        for (group, pops) in groups {
            let pops = pops.as_table().expect("a pops group is a table");
            let all: Vec<String> = pops
                .iter()
                .map(|(name, bytes)| format!("({name:?}, {})", bytes.as_integer().expect("bytes is an int")))
                .collect();
            writeln!(out, "    ({group:?}, &[{}]),", all.join(", ")).expect("a string takes text");
        }
    }
    out.push_str("];\n");
    out.push_str("\n/// Fixed stack blocks with their evidence: (routines, bytes, evidence).\npub static POP_NOTES: &[(&[&str], i64, &str)] = &[\n");
    for one in rows.get("pop").and_then(|one| one.as_array()).into_iter().flatten() {
        let one = one.as_table().expect("a pop entry is a table");
        writeln!(
            out,
            "    ({}, {}, {:?}),",
            list(one, "names"),
            one["bytes"].as_integer().expect("bytes is an int"),
            string(one, "evidence").trim()
        )
        .expect("a string takes text");
    }
    out.push_str("];\n");
    out.push_str("\n/// The cells a program names by EXTDEF and the routines that write each: (cell, family, routines).\npub static WRITERS_ROWS: &[(&str, &str, &[&str])] = &[\n");
    for one in rows.get("writers").and_then(|one| one.as_array()).expect("[[writers]]") {
        let one = one.as_table().expect("a writers entry is a table");
        writeln!(out, "    ({:?}, {:?}, {}),", string(one, "cell"), string(one, "family"), list(one, "routines"))
            .expect("a string takes text");
    }
    out.push_str("];\n");
    writeln!(out, "\n/// Each row's contract once built: a program asks of the same routine at every call.\npub static BUILT: [std::sync::OnceLock<crate::Contract>; {}] = [const {{ std::sync::OnceLock::new() }}; {}];", names.len(), names.len()).expect("a string takes text");
    let dir = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR");
    std::fs::write(std::path::Path::new(&dir).join("rows.rs"), out).expect("rows.rs writes");
}
