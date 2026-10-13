//! Generates `RegId`'s constants: one per register iced names, under iced's
//! names.

fn main() {
    let mut code = String::from("#[allow(non_upper_case_globals)]\nimpl RegId {\n");
    for register in iced_x86::Register::values() {
        let name = format!("{register:?}");
        if name.starts_with("DontUse") {
            continue;
        }
        code.push_str(&format!("    pub const {name}: RegId = RegId(Register::{name});\n"));
    }
    code.push_str("}\n");
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("reg_consts.rs"), code).unwrap();
}
