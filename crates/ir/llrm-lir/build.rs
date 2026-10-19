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
    // The dword of each general register's family (`root`), by iced's number: a
    // table, as the answer is asked in every pass.
    code.push_str(&format!("static ROOT_OF: [Register; {}] = [\n", iced_x86::Register::values().len()));
    for register in iced_x86::Register::values() {
        let whole = register.full_register();
        let root = if register.is_gpr() && whole.is_gpr64() { whole.full_register32() } else { register };
        code.push_str(&format!("    Register::{root:?},\n"));
    }
    code.push_str("];\n");
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("reg_consts.rs"), code).unwrap();
}
