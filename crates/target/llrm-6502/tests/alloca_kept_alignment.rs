//! The 6502 states what its frames keep (`Machine::kept_stack_align` 1, `max_alloca_align` 2). Unset, the mid-end took an
//! `alloca ... align 2` at its word: it folded `ptr & 1` to 0 for a local on an odd frame (a one-byte frame above it),
//! where the address is odd (2026-11-09: on sim65 `use` saw bit 0 = 1 while `probe` returned 0).

mod common;

use llrm_6502::driver::{Level, Request, assembly, machine};
use llrm_mir::program::Program;
use llrm_transforms::pipeline::{Applied, Options, applied};

fn probe() -> String {
    format!(
        "target datalayout = \"{}\"\n{}",
        common::pipeline::datalayout(),
        "\ndeclare void @use(ptr)\n\ndefine i8 @probe() {\nentry:\n  %x = alloca i16, align 2\n  call void @use(ptr %x)\n  %i = ptrtoint ptr %x to i16\n  %m = and i16 %i, 1\n  %r = trunc i16 %m to i8\n  ret i8 %r\n}\n"
    )
}

/// The IR after the mid-end on the 6502 machine.
fn mid_end(ir: &str) -> Result<String, String> {
    let module = llrm_mir::parse::module(ir).map_err(|error| error.to_string())?;
    let mut program = Program::new(vec![module], machine())?;
    applied(&mut program, &Applied { options: Options::speed(), only: None, dump: None })?;
    Ok(llrm_mir::print::module(&program.modules[0]))
}

#[test]
fn an_align_2_local_is_not_assumed_aligned_by_the_mid_end() {
    let text = mid_end(&probe()).expect("compiles");
    assert!(!text.contains("ret i8 0"), "ptr & 1 folded to 0 from the alloca's align:\n{text}");
}

#[test]
fn a_local_aligned_past_the_accepted_alignment_is_refused_by_name() {
    for level in [Level::O0, Level::O2, Level::Os] {
        let error = assembly(&probe().replace("align 2", "align 4"), &Request::new(level)).unwrap_err();
        assert!(error.contains("aligned to 4"), "{level:?}: {error}");
    }
}

#[test]
fn an_align_2_local_on_an_odd_frame_has_the_address_bit_the_code_returns() {
    let compiled = assembly(&probe(), &Request::new(Level::O2)).unwrap();
    let functions: String =
        compiled.lines().filter(|line| !line.contains(" = $")).map(|line| format!("{line}\n")).collect();
    // `g` has a one-byte frame; `use` keeps the low bit of the pointer it is
    // given.
    let extra = format!(
        "{functions}
g:
  lda rc0
  bne g1
  dec rc1
g1:
  dec rc0
  jsr probe
  tax
  inc rc0
  bne g2
  inc rc1
g2:
  txa
  rts
use:
  lda rc2
  and #1
  sta seen
  rts
.segment \"BSS\"
seen: .res 1
.segment \"CODE\"
"
    );
    let main = "  lda #0\n  sta rc0\n  lda #$F0\n  sta rc1\n  jsr g\n  cmp seen\n  beq same\n  lda #1\n  rts\nsame:\n  lda #0\n";
    assert_eq!(common::run_with("alloca-align-two", main, &extra), 0);
}

/// The clamp left `load/store ... align 2` on an `align 1` object: the access claimed an alignment nothing has, a load of it could not be
/// speculated, and `c >= '0' && c <= '9'` kept two branches (isdigit 28 -> 50 bytes, `tti::a_range_test_of_a_word_...`).
#[test]
fn an_access_to_a_clamped_local_states_what_the_local_keeps() {
    let ir = format!(
        "target datalayout = \"{}\"\n\ndeclare void @use(ptr)\n\ndefine i16 @f(i16 %v) {{\nentry:\n  %x = alloca i16, align 2\n  store i16 %v, ptr %x, align 2\n  call void @use(ptr %x)\n  %r = load i16, ptr %x, align 2\n  ret i16 %r\n}}\n",
        common::pipeline::datalayout()
    );
    let text = mid_end(&ir).expect("compiles");
    assert!(!text.contains("align 2"), "an access still states align 2 on an align 1 local:\n{text}");
}
