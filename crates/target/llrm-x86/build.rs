//! The table of iced's `Code` names: its five thousand names hashed here, once,
//! so that no compile formats them (`select::_code` built a map of owned names
//! in the first compile of every process: five million instructions, a quarter
//! of a compile of one small function).

use std::io::Write;

use iced_x86::Code;

/// Two hashes of a name, 128 bits between them: the same as
/// `select::_name_hash`.
fn hash(name: &str) -> (u64, u64) {
    let (mut first, mut second) = (0xcbf2_9ce4_8422_2325_u64, 0x9e37_79b9_7f4a_7c15_u64);
    for byte in name.bytes() {
        first = (first ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        second = (second.rotate_left(5) ^ u64::from(byte)).wrapping_mul(0xff51_afd7_ed55_8ccd);
    }
    (first, second)
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let mut table: Vec<(u64, u64, u16)> = Code::values()
        .enumerate()
        .map(|(at, code)| {
            let (first, second) = hash(&format!("{code:?}").to_uppercase());
            (first, second, u16::try_from(at).expect("iced's codes fit u16"))
        })
        .collect();
    table.sort_unstable();
    assert!(table.windows(2).all(|pair| (pair[0].0, pair[0].1) != (pair[1].0, pair[1].1)), "two names hash alike");
    let mut text = format!(
        "pub const CODE_COUNT: usize = {};\npub static CODE_NAMES: [(u64, u64, u16); {}] = [\n",
        table.len(),
        table.len()
    );
    for (first, second, at) in &table {
        text.push_str(&format!("    ({first:#x}, {second:#x}, {at}),\n"));
    }
    text.push_str("];\n");
    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("code_names.rs");
    std::fs::File::create(out).expect("OUT_DIR is writable").write_all(text.as_bytes()).expect("written");
}
