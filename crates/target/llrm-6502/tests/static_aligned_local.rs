//! A local aligned past what the soft stack keeps (`MAX_ALLOCA_ALIGN`) was
//! refused up front. In a function that cannot be re-entered it
//! is a zero-filled global that `.align` places (`staticlocals`); where it
//! cannot be, it is still refused by name.

mod common;

use llrm_6502::driver::{Level, Request, assembly};

fn program(
    attrs: &str,
    call: &str,
) -> String {
    format!(
        "target datalayout = \"{}\"\n\ndeclare void @use(ptr)\n\ndefine i8 @probe(i8 %n) {attrs} {{\nentry:\n  %x = alloca [16 x i8], align 16\n  call void @use(ptr %x)\n  %i = ptrtoint ptr %x to i16\n  %m = and i16 %i, 15\n  %r = trunc i16 %m to i8\n  {call}\n  ret i8 %r\n}}\n",
        common::pipeline::datalayout()
    )
}

#[test]
fn an_over_aligned_local_of_a_norecurse_function_is_an_aligned_global() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = assembly(&program("norecurse", ""), &Request::new(level))
            .unwrap_or_else(|error| panic!("{level:?}: {error}"));
        assert!(text.contains(".align 16\ndataprobe_local0:"), "{level:?}: no aligned global:\n{text}");
    }
}

#[test]
fn an_over_aligned_local_that_stays_on_the_stack_is_refused_by_name() {
    // the call to itself: not re-entrant-free, so `staticlocals` leaves the
    // stack object
    let recursive = program("", "%t = call i8 @probe(i8 %n)");
    for level in [Level::O0, Level::O1, Level::O2, Level::Os] {
        let error = assembly(&recursive, &Request::new(level)).unwrap_err();
        assert!(error.contains("aligned to 16"), "{level:?}: {error}");
    }
}

/// The address `use` saw, on sim65 behind a 3-byte BSS object that leaves the
/// BSS cursor odd: the mid-end folded `ptr & 15` to 0 and the object was at an
/// address that is not (2026-11-26: refused instead, then an unaligned `.res`).
#[test]
fn the_static_of_an_over_aligned_local_is_at_an_aligned_address() {
    let compiled = assembly(&program("norecurse", ""), &Request::new(Level::O2)).unwrap();
    let functions: String =
        compiled.lines().filter(|line| !line.contains(" = $")).map(|line| format!("{line}\n")).collect();
    let extra = format!(
        ".segment \"BSS\"
pad: .res 3
.segment \"CODE\"
{functions}
use:
  lda rc2
  and #15
  sta seen
  rts
.segment \"BSS\"
seen: .res 1
.segment \"CODE\"
"
    );
    let main = "  lda #0\n  jsr probe\n  cmp seen\n  beq same\n  lda #1\n  rts\nsame:\n  lda #0\n";
    assert_eq!(common::run_with("static-aligned-local", main, &extra), 0);
}
