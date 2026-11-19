//! `llrm_dagisel::instr::flag` is the module the generator writes for the
//! targets that carry a copy: a flag added to one and not the other would shift
//! every descriptor's bits.

#[test]
fn the_shared_flag_module_is_what_the_generator_renders() {
    let shared = include_str!("../../llrm-dagisel/src/instr.rs");
    let rendered = llrm_dagisel_gen::emit::render_flags();
    assert!(shared.contains(rendered.trim_end()), "instr.rs `flag` differs from emit::render_flags()");
}
