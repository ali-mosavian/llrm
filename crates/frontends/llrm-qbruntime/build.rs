//! `runtime.toml` as Rust data: every process that asked for a routine's contract parsed the 64 KB table first (8
//! Minstr, 3% of a C program's compile). The rows keep the file's order; `BY_NAME` is sorted for a binary search.

use std::fmt::Write;

fn main() {
    println!("cargo::rerun-if-changed=src/runtime.toml");
    println!("cargo::rerun-if-changed=build.rs");
    let text = std::fs::read_to_string("src/runtime.toml").expect("runtime.toml reads");
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
        let optional_str = |key: &str| {
            option(row, key, if row.contains_key(key) { format!("{:?}", string(row, key)) } else { String::new() })
        };
        let optional_list =
            |key: &str| option(row, key, if row.contains_key(key) { list(row, key) } else { String::new() });
        writeln!(
            out,
            "    Row {{ name: {name:?}, cleanup: {}, control: {:?}, enters_user_code: {}, raises_error: {}, error_handling: {}, writes: {:?}, reads: {:?}, clobbers: {}, established: {}, evidence: {:?}, documented: {}, inputs: {}, direct_writes: {}, direct_reads: {}, captures: {captures} }},",
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
    writeln!(out, "\n/// Each row's contract once built: a program asks of the same routine at every call.\npub static BUILT: [std::sync::OnceLock<crate::Contract>; {}] = [const {{ std::sync::OnceLock::new() }}; {}];", names.len(), names.len()).expect("a string takes text");
    let dir = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR");
    std::fs::write(std::path::Path::new(&dir).join("rows.rs"), out).expect("rows.rs writes");
}
