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
}
