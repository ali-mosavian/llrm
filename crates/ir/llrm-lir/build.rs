//! Generates `RegId`'s constants: one per register iced names, under iced's
//! names.

fn main() {
    let mut code = String::from("#[allow(non_upper_case_globals)]\nimpl RegId {\n");
    for register in iced_x86::Register::values() {
        let name = format!("{register:?}");
        if name.starts_with("DontUse") {
            continue;
        }
        code.push_str(&format!("    pub const {name}: RegId = RegId(Register::{name} as u8);\n"));
    }
    code.push_str("}\n");
    // The dword of each general register's family (`root`), by iced's number: a
    // table, as the answer is asked in every pass.
    code.push_str(&format!("static ROOT_OF: [u8; {}] = [\n", iced_x86::Register::values().len()));
    for register in iced_x86::Register::values() {
        let whole = register.full_register();
        let root = if register.is_gpr() && whole.is_gpr64() { whole.full_register32() } else { register };
        // The registers iced marks unusable are no family's, and are not named
        // (they are deprecated variants).
        let root = if format!("{root:?}").starts_with("DontUse") { iced_x86::Register::None } else { root };
        code.push_str(&format!("    Register::{root:?} as u8,\n"));
    }
    code.push_str("];\n");
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("reg_consts.rs"), code).unwrap();
}
