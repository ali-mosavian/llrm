//! A switch of ten cases to distinct blocks reaches the object as the pin's
//! jump table: `adrp`/`add` of the table, an `adr` of the base block, the
//! entry load and `br`. The expected text is the pin's `llc -O2` on the same
//! IR.

use std::process::Command;

const HEADER: &str = "target triple = \"arm64-apple-macosx\"\n\
target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn dag(body: &str) -> Result<String, String> {
    dag_with(body, &[])
}

fn dag_with(
    body: &str,
    env: &[(&str, &str)],
) -> Result<String, String> {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, format!("{HEADER}{body}")).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .envs(env.iter().copied())
        .args(["-S", "--no-opt", "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    if said.status.success() {
        Ok(std::fs::read_to_string(&output).unwrap())
    } else {
        Err(String::from_utf8_lossy(&said.stderr).into_owned())
    }
}

fn switch_ir() -> String {
    let mut ir = String::new();
    for i in 0..10 {
        ir += &format!("declare void @f{i}()\n");
    }
    ir += "define void @f(i32 %x) {\n  switch i32 %x, label %d [\n";
    for i in 0..10 {
        ir += &format!("    i32 {i}, label %l{i}\n");
    }
    ir += "  ]\n";
    for i in 0..10 {
        ir += &format!("l{i}:\n  call void @f{i}()\n  ret void\n");
    }
    ir + "d:\n  ret void\n}\n"
}

/// The DAG path refused the ten-case switch ("has no arm64 instruction yet" for
/// `MOVaddrJT`), so the hand selector compiled it as a compare tree.
#[test]
fn a_ten_case_switch_is_a_jump_table_in_the_const_section() {
    let text = dag(&switch_ir()).expect("the DAG path takes the switch");
    assert!(text.contains("adrp") && text.contains("@PAGE") && text.contains("br x"), "{text}");
    assert!(text.contains("__TEXT,__const"), "{text}");
}

/// The table in the object: each entry, added to the base the `adr` names,
/// is the address of the block that calls the function of that case. The
/// text's own relocations (the `bl` of each block) say which block that is.
fn entries_reach_their_calls(platform: llrm_arm64::machine::Platform) {
    use llrm_arm64::driver::{self, IselMode, Level};
    use llrm_object::{Role, Target};
    driver::set_isel_mode(IselMode::Dag);
    let header = format!("target datalayout = \"{}\"\n", platform.datalayout());
    let object = driver::object(&format!("{header}{}", switch_ir()), platform, "t", Level::O0).expect("compiles");
    let text = &object.sections[0];
    let table = object.sections.iter().find(|one| one.role == Role::ROData).expect("a read-only section");
    let word = |at: usize| u32::from_le_bytes(text.image[at..at + 4].try_into().unwrap());
    let adr = (0..text.image.len() / 4)
        .map(|n| n * 4)
        .find(|&at| word(at) & 0x9F00_0000 == 0x1000_0000)
        .expect("an adr of the base");
    let immediate = ((word(adr) >> 5) & 0x7_FFFF) << 2 | (word(adr) >> 29) & 3;
    let distance = ((immediate as i32) << 11) >> 11;
    let base = (adr as i64 + i64::from(distance)) as usize;
    assert_eq!(table.image.len(), 10, "ten one-byte entries");
    for (case, &entry) in table.image.iter().enumerate() {
        let target = base + 4 * usize::from(entry);
        let call = text.relocs.iter().find(|one| one.at == target).unwrap_or_else(|| panic!("no call at {target}"));
        let Target::Symbol(symbol) = call.target else { panic!("{call:?}") };
        assert!(
            object.symbols[symbol].name.ends_with(&format!("f{case}")),
            "case {case}: {:?}",
            object.symbols[symbol]
        );
    }
}

/// The table was dropped from the object (the address was a symbol nothing
/// defined) and the entries were never resolved against the laid-out blocks.
#[test]
fn the_entries_of_a_darwin_table_reach_the_block_of_their_case() {
    entries_reach_their_calls(llrm_arm64::machine::Platform::Darwin);
}

#[test]
fn the_entries_of_a_linux_table_reach_the_block_of_their_case() {
    entries_reach_their_calls(llrm_arm64::machine::Platform::Linux);
}

/// The jump of `text`: the line numbers of its `ldrb` and `adr`, the register
/// it loads into, the register of the table, the register of the address.
fn jump_lines(text: &str) -> (usize, usize, String, String, String) {
    let at = |start: &str| text.lines().position(|line| line.trim_start().starts_with(start)).expect(text);
    let (load, adr) = (at("ldrb"), at("adr x"));
    let load_line = text.lines().nth(load).unwrap().trim_start().trim_start_matches("ldrb ");
    let loaded = load_line.split(',').next().unwrap().replace('w', "x");
    let table = load_line.split('[').nth(1).unwrap().split(',').next().unwrap().to_owned();
    let address = text.lines().nth(adr).unwrap().trim_start().trim_start_matches("adr ").split(',').next().unwrap();
    (load, adr, loaded, table, address.to_owned())
}

/// `adr` ran before the load and took the table's own register, so the load
/// read through the entry's address (a wrong jump); the offset and the address
/// must also be two registers, or the sum adds the address to itself.
#[test]
fn the_address_of_a_jump_does_not_clobber_what_the_load_reads() {
    for env in [&[][..], &[("LLRM_ARM64_GREEDY", "1")][..]] {
        let text = dag_with(&switch_ir(), env).expect("the DAG path takes the switch");
        let (load, adr, loaded, table, address) = jump_lines(&text);
        assert!(load < adr || address != table, "{env:?}\n{text}");
        assert_ne!(loaded, address, "{env:?}\n{text}");
    }
}
