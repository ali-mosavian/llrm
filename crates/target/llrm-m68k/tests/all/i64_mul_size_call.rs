//! A module built for size calls `__muldi3` for its `i64` products when the
//! calls save more than the routine costs (134 B at the 68000):
//! int64_divmod_edges -Os was 1354 B of 16-bit partial products against gcc's
//! 838. One site stays inline (sfm16_float -Os: -64 B in the object, +72 B
//! linked). Speed keeps the partial products.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const HEAD: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

/// `sites` functions, each the product of two variable `i64`s.
fn products(
    sites: usize,
    attrs: &str,
) -> String {
    let mut ir = HEAD.to_owned();
    for n in 0..sites {
        ir += &format!("define i64 @f{n}(i64 %a, i64 %b) {attrs} {{ %r = mul i64 %a, %b ret i64 %r }}\n");
    }
    ir
}

fn calls(
    cpu: Cpu,
    sites: usize,
    attrs: &str,
) -> bool {
    let options = Options { mid_end: false, ..Options::default() };
    let module = llrm_mir::parse::module(&products(sites, attrs)).expect("parses");
    let text = text_with(&module, "f0", cpu, options).expect("compiles");
    text.contains("__muldi3")
}

#[test]
fn a_size_module_with_several_i64_products_calls_muldi3() {
    for cpu in [Cpu::M68000, Cpu::M68010] {
        assert!(calls(cpu, 3, "optsize"), "{cpu:?}: partial products at -Os");
    }
}

#[test]
fn a_single_i64_product_stays_inline_at_size() {
    for cpu in [Cpu::M68000, Cpu::M68010] {
        assert!(!calls(cpu, 1, "optsize"), "{cpu:?}: a call for one site pays the routine for nothing");
    }
}

#[test]
fn a_speed_built_module_keeps_the_partial_products() {
    for cpu in [Cpu::M68000, Cpu::M68010] {
        assert!(!calls(cpu, 3, ""), "{cpu:?}: a call at speed");
    }
}

/// fixed_point -Os: the widening product of `smul.fix` is the same `__muldi3`,
/// found at no `mul i64` of the source (-656 B linked at the 68000).
#[test]
fn fixed_point_products_are_sites_of_the_same_call() {
    let mut ir = format!("{HEAD}declare i32 @llvm.smul.fix.i32(i32, i32, i32 immarg)\n");
    for n in 0..3 {
        ir += &format!(
            "define i32 @f{n}(i32 %a, i32 %b) optsize {{ %r = call i32 @llvm.smul.fix.i32(i32 %a, i32 %b, i32 16) ret i32 %r }}\n"
        );
    }
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let options = Options { mid_end: false, ..Options::default() };
    let text = text_with(&module, "f0", Cpu::M68000, options).expect("compiles");
    assert!(text.contains("__muldi3"), "{text}");
}
