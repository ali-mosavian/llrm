//! Generates x86-m32's `REGISTER_INFO` from its `registers.regs`.

fn main() {
    let path = "src/registers.regs";
    println!("cargo:rerun-if-changed={path}");
    let text = std::fs::read_to_string(path).expect("registers.regs");
    let code = llrm_target::registers::source(&text, "iced_x86::Register", &llrm_lir::registers::CLASSES)
        .unwrap_or_else(|error| panic!("{path}: {error}"));
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("register_info.rs"), code).unwrap();
}
