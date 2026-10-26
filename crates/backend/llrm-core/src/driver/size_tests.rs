//! `size_marked`: the `optsize` attribute at -Os and -Oz.

use super::flags::Level;

/// The functions of the program that carry `optsize`, by name.
fn marked(level: Level) -> Vec<bool> {
    let layout = llrm_x86_m16::layout().datalayout;
    let text = format!(
        "target datalayout = \"{layout}\"\ndefine void @f() {{\nb0:\n  ret void\n}}\n\ndefine void @g() {{\nb0:\n  ret void\n}}\n"
    );
    let module = llrm_mir::parse::module(&text).expect("parses");
    let cpu = crate::backend::cpu::profile("486").expect("a cpu");
    let mut program = llrm_mir::program::Program::new(vec![module], cpu.target()).expect("a program");
    super::size_marked(&mut program, &level.options());
    program
        .modules
        .iter()
        .flat_map(|module| module.globals.iter())
        .filter_map(|global| match &global.kind {
            llrm_mir::GlobalKind::Function(function) if !function.is_declaration() => {
                Some(llrm_mir::memory::has(&function.attrs, "optsize"))
            }
            _ => None,
        })
        .collect()
}

/// -Os never marked a function `optsize` (clang does): the DAG selector's
/// memcpy limit (4 stores at optsize, 8 otherwise) read none, so a 24-byte
/// struct copy came out as six `mov`s where the pin's is `rep movsl`, and
/// every pass reading the attribute chose as for speed.
#[test]
fn test_every_function_is_marked_optsize_at_os_and_oz_and_at_no_other_level() {
    assert_eq!(marked(Level::Os), [true, true]);
    assert_eq!(marked(Level::Oz), [true, true]);
    for level in [Level::O0, Level::O1, Level::O2, Level::O3, Level::Omax] {
        assert_eq!(marked(level), [false, false]);
    }
}
