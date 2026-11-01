//! The functions the old m16 selector handles that the DAG path must keep
//! selecting.
//!
//! The pin has no 16-bit model, so `TargetLowering::x86_16` and `m16.td` are
//! derived from what the old selector covers (SD307); this list is that
//! coverage, one function per operation family. Each must build, legalize and
//! select every node: no refusal and no `CannotSelect`.

use llrm_dagisel::generated::x86_m16 as table;
use llrm_dagisel::interp::Selector;
use llrm_dagisel::x86_addr::AddrMode;
use llrm_dagisel::x86_hooks::{Cpu, Options, X86Hooks};
use llrm_mir::datalayout::DataLayout;

use super::super::actions::{Features, TargetLowering};
use super::super::builder::{BuilderTarget, build_block};
use super::super::dag_select::{do_instruction_selection, legalized};
use super::super::function_lowering::FunctionLoweringInfo;
use super::super::x86_lowering::X86Lowering;
use super::*;
use crate::testing::m16_ir;

const LAYOUT: &str = "target datalayout = \"@m16-datalayout@-ni:2:3\"\n";

/// Builds, legalizes and selects each block of the first function; the error
/// names the stage.
fn selects(body: &str) -> Result<(), String> {
    let text = format!("{layout}@g = global i16 5\n{body}", layout = m16_ir(LAYOUT));
    let module = llrm_mir::parse::module(&text).map_err(|e| format!("parse: {e}"))?;
    let layout = module.datalayout.as_deref().map_or_else(DataLayout::default, |t| DataLayout::parse(t).unwrap());
    let machine = llrm_x86_m16::M16;
    let (id, _, function) = module.functions().next().expect("a function");
    let convention = crate::backend::isel::convention(&module, &layout, id, &machine).map_err(|e| format!("{e:?}"))?;
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let selector = Selector::new(table::TABLE);
    let mut hooks = X86Hooks::new(Options {
        cpu: Cpu::M16,
        opt_none: false,
        opt_for_size: false,
        min_size: false,
        has_frame_pointer: true,
        multiply_early_out: false,
        address_mode: AddrMode::Real16,
    });
    let mut info = FunctionLoweringInfo::new(function);
    for &block in function.layout() {
        let mut built =
            build_block(&module, function, &layout, &BuilderTarget::m16(), &machine, &convention, &mut info, block)
                .map_err(|r| format!("builder: {}", r.0))?;
        legalized(
            &mut built.dag,
            &tli,
            &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false, profile: None },
            &machine,
        )
        .map_err(|r| format!("{r:?}"))?;
        do_instruction_selection(&mut built.dag, &selector, &mut hooks).map_err(|r| format!("{r:?}"))?;
    }
    Ok(())
}

macro_rules! coverage {
    ($($name:ident: $body:expr;)*) => {$(
        #[test]
        fn $name() {
            selects($body).unwrap_or_else(|why| panic!("{why}"));
        }
    )*};
}

coverage! {
    arithmetic: "define i16 @f(i16 %a, i16 %b) {\n %s = add i16 %a, %b\n %t = sub i16 %s, %b\n %u = and i16 %t, 255\n %v = or i16 %u, %a\n %w = xor i16 %v, %b\n ret i16 %w\n}\n";
    shifts: "define i16 @f(i16 %a, i16 %n) {\n %l = shl i16 %a, 3\n %r = lshr i16 %l, %n\n %s = ashr i16 %r, 2\n ret i16 %s\n}\n";
    multiply_by_constants: "define i16 @f(i16 %a) {\n %m = mul i16 %a, 5\n %n = mul i16 %m, 100\n ret i16 %n\n}\n";
    division: "define i16 @f(i16 %a, i16 %b) {\n %q = sdiv i16 %a, %b\n %r = urem i16 %a, %b\n %s = add i16 %q, %r\n ret i16 %s\n}\n";
    global_load_and_store: "define i16 @f(i16 %a) {\n %v = load i16, ptr @g\n store i16 %a, ptr @g\n ret i16 %v\n}\n";
    frame_object: "define i16 @f(i16 %a) {\n %p = alloca i16\n store i16 %a, ptr %p\n %v = load i16, ptr %p\n ret i16 %v\n}\n";
    byte_extensions: "define i16 @f(ptr %p) {\n %b = load i8, ptr %p\n %z = zext i8 %b to i16\n %s = sext i8 %b to i16\n %r = add i16 %z, %s\n ret i16 %r\n}\n";
    branch_on_compare: "define i16 @f(i16 %a, i16 %b) {\n %c = icmp ult i16 %a, %b\n br i1 %c, label %t, label %e\nt:\n ret i16 1\ne:\n ret i16 2\n}\n";
    array_element: "define i16 @f(ptr %p, i16 %i) {\n %q = getelementptr i16, ptr %p, i16 %i\n %v = load i16, ptr %q\n ret i16 %v\n}\n";
}
