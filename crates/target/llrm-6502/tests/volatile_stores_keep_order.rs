//! Guard for the m68k release-store fix (chain combine and `isSimple`): two
//! volatile stores keep their program order, the data before the flag.

use llrm_6502::driver::{self, Level, Request};
use llrm_6502::target::Mos;
use llrm_target::Target;

#[test]
fn a_volatile_data_store_stays_before_the_volatile_flag_store() {
    let text = format!(
        "target datalayout = \"{}\"\n@data = global [4 x i8] zeroinitializer\n@flag = global i8 0\n\
         define void @publish(i8 %x) {{\n  store volatile i8 %x, ptr getelementptr inbounds (i8, ptr @data, i16 1)\n  \
         store volatile i8 1, ptr @flag\n  ret void\n}}\n",
        Mos.layout().datalayout
    );
    for level in [Level::O0, Level::O1, Level::O2, Level::O3, Level::Os] {
        let asm = driver::assembly(&text, &Request::new(level)).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        let at = |name: &str| asm.lines().position(|line| line.contains(name) && !line.trim_end().ends_with(':'));
        assert!(at("data").is_some() && at("data") < at("flag"), "{level:?}: the flag is stored first:\n{asm}");
    }
}
