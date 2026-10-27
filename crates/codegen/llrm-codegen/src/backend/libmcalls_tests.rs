use llrm_mir::module::Module;

const SOURCE: &str = "declare double @llvm.floor.f64(double)
declare float @llvm.ceil.f32(float)
define double @f(double %x, float %y) {
  %a = call double @llvm.floor.f64(double %x)
  %b = call float @llvm.ceil.f32(float %y)
  %c = fpext float %b to double
  %d = fadd double %a, %c
  ret double %d
}
";

fn expanded(libcalls: &[(&str, u32, &str)]) -> String {
    let mut module: Module = llrm_mir::parse::module(SOURCE).expect("parses");
    let libcalls: Vec<_> = libcalls.iter().map(|(a, b, c)| (a.to_string(), *b, c.to_string())).collect();
    super::libmcalls::expanded(&mut module, &libcalls).expect("expands");
    assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new(), "{}", llrm_mir::print::module(&module));
    llrm_mir::print::module(&module)
}

/// `floor(x)` of a C program reached the backend as `llvm.floor.f64`, which no
/// selector took: `@f_: @llvm.floor.f64`, no object. Each is the call of the
/// routine its type names, with the symbol the target spells it.
#[test]
fn test_a_rounding_intrinsic_is_a_call_of_its_library_routine() {
    let after = expanded(&[("floor", 0, "_floor"), ("ceilf", 0, "_ceilf")]);
    assert!(after.contains("call double @_floor(double %x)"), "{after}");
    assert!(after.contains("call float @_ceilf(float %y)"), "{after}");
    assert!(!after.contains("call double @llvm.floor"), "{after}");
}

/// A target that names no routine keeps the intrinsic.
#[test]
fn test_without_the_routine_the_intrinsic_stays() {
    let after = expanded(&[("floor", 0, "_floor")]);
    assert!(after.contains("call float @llvm.ceil.f32"), "{after}");
}
