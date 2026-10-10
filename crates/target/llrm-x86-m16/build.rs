//! Generates x86-m16's `REGISTER_INFO` from its `registers.regs`.

fn main() {
    let path = "src/registers.regs";
    println!("cargo:rerun-if-changed={path}");
    let text = std::fs::read_to_string(path).expect("registers.regs");
    let code = llrm_target::registers::source(&text, "iced_x86::Register", &llrm_lir::registers::CLASSES, "rows")
        .unwrap_or_else(|error| panic!("{path}: {error}"));
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("register_info.rs"), code).unwrap();
    // The instruction effects belong to the same description: the register file
    // carries them.
    let family = llrm_iselgen::build::family(std::path::Path::new("../llrm-x86"));
    let forms = llrm_iselgen::build::forms(std::path::Path::new("."), &family);
    println!("cargo:rerun-if-changed=src/instructions/x86.instr");
    std::fs::write(out.join("effects.rs"), llrm_iselgen::build::effect_rows(&forms)).unwrap();
}
