//! A constant copy past the size limit of inline pieces is a call at every
//! level: bytes first.

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{self, Level, Request};
use llrm_6502::target::Mos;
use llrm_target::Target;

fn at(
    level: Level,
    bytes: u32,
    callee: &str,
) -> String {
    let text = format!(
        "target datalayout = \"{}\"\n@buf = global [64 x i8] zeroinitializer\ndeclare void @llvm.{callee}.p0.p0.i16(ptr, ptr, i16, i1)\ndefine i16 @main() {{\n  call void @llvm.{callee}.p0.p0.i16(ptr getelementptr inbounds (i8, ptr @buf, i16 3), ptr @buf, i16 {bytes}, i1 false)\n  ret i16 0\n}}\n",
        Mos.layout().datalayout
    );
    let mut request = Request::new(level);
    request.cpu = Cpu::Mos6502;
    driver::assembly(&text, &request).unwrap_or_else(|why| panic!("{level:?}: {why}"))
}

/// memmove_overlap -O1/-O2: the 27-byte move was about 14 word pairs inline
/// (611 B against 536 at -Os, sdcc 591): the pin's limit of eight pieces
/// is for speed, four tuned for size.
#[test]
fn a_constant_move_past_the_size_limit_is_a_call_at_every_level() {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = at(level, 27, "memmove");
        assert!(text.contains("jsr memmove"), "{level:?}: no call\n{text}");
    }
}

/// The same for a copy that cannot overlap.
#[test]
fn a_constant_copy_past_the_size_limit_is_a_call_at_every_level() {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = at(level, 27, "memcpy");
        assert!(text.contains("jsr memcpy"), "{level:?}: no call\n{text}");
    }
}

/// A few bytes stay loads and stores.
#[test]
fn a_four_byte_move_stays_inline() {
    for level in [Level::O2, Level::Os] {
        let text = at(level, 4, "memmove");
        assert!(!text.contains("jsr memmove"), "{level:?}: a call\n{text}");
    }
}
