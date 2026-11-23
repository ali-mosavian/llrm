//! `-mcmodel=large`/`medium` kept an external tail call as `bra sym`, whose
//! PC16 relocation cannot reach past 32 KiB (the exec.c link failed).

use llrm_m68k::compile::CodeModel;
use llrm_m68k::driver::{Level, assembly_in, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare i32 @ext(i32, i32)
define i32 @f(i32 %a, i32 %b) {
  %r = tail call i32 @ext(i32 %b, i32 %a)
  ret i32 %r
}";

#[test]
fn an_external_tail_call_is_absolute_in_the_far_models_and_a_branch_in_small() {
    let cpu = cpu_of("68020").unwrap();
    for level in [Level::O1, Level::Os] {
        let small = assembly_in(IR, cpu, level, false, None, CodeModel::Small).unwrap();
        assert!(small.contains("bra") && small.contains("ext"), "{level:?}\n{small}");
        for model in [CodeModel::Medium, CodeModel::Large] {
            let far = assembly_in(IR, cpu, level, false, None, model).unwrap();
            assert!(far.contains("lea\text, %a0") && far.contains("jmp\t(%a0)"), "{level:?} {model:?}\n{far}");
            assert!(!far.contains("bra\text") && !far.contains("jra"), "{level:?} {model:?}\n{far}");
        }
    }
}
