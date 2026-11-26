//! DEVIATE OS-INLINE-TRIAL: a copy the -Os byte model refuses within its error
//! is inlined in a second compile and kept where the backend writes the chain
//! smaller.

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{self, Level, Request};
use llrm_6502::target::Mos;
use llrm_target::Target;

fn source(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_size").join(format!("{name}.ll"));
    let text = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("target datalayout"))
        .fold(format!("target datalayout = \"{}\"\n", Mos.layout().datalayout), |text, line| text + line + "\n");
    text
}

fn at(
    level: Level,
    name: &str,
) -> String {
    let text = source(name);
    let mut request = Request::new(level);
    request.cpu = Cpu::Mos6502;
    driver::assembly(&text, &request).unwrap_or_else(|why| panic!("{name}: {why}"))
}

fn os(name: &str) -> String {
    at(Level::Os, name)
}

/// memcpy_libcall -Os: the model read the two copies of the private `same` as
/// 52 B larger than its calls and refused them; the object is 58 B larger for
/// it (CODE 577 against 519). `same` is gone once the trial inlines it.
#[test]
fn a_copy_the_model_refuses_within_its_error_is_inlined_when_the_backend_writes_it_smaller() {
    let text = os("memcpy_libcall");
    assert!(!text.contains("same:") && !text.contains("jsr same"), "`same` is still a function\n{text}");
}

/// memcpy_libcall -O2/-O3: the byte rate refused the copies of `same` (the
/// model prices the copy, not the constants it folds in `main`) and the module
/// came out 100 B larger than inlined.
#[test]
fn a_copy_the_byte_rate_refuses_is_inlined_at_speed_when_the_backend_writes_it_smaller() {
    for level in [Level::O2, Level::O3] {
        let text = at(level, "memcpy_libcall");
        assert!(!text.contains("same:") && !text.contains("jsr same"), "{level:?}: `same` is still a function\n{text}");
    }
}

/// double_to_uint64 -O2: the trial inlines `show` and keeps nothing. The kept
/// side was selected twice, as the cut chain to price it and again to write it
/// (and a third time for the unroll trial's price): 13 selections
/// for 5 functions. It is priced from the assembly the output and the unroll
/// trial use. At -O2 both sides are finished, each rolled and unrolled: four
/// builds, each selected once (18).
#[test]
fn a_trial_that_keeps_nothing_selects_the_kept_side_once() {
    let mut request = Request::new(Level::O2);
    request.cpu = Cpu::Mos6502;
    let text = source("double_to_uint64");
    let functions = driver::size_audit(&text, &request).unwrap().len();
    let before = driver::selected();
    driver::assembly(&text, &request).unwrap();
    let selected = driver::selected() - before;
    assert!(selected <= 4 * functions, "{selected} selections for {functions} functions");
}

/// loop_fusion -O2/-O3: the rate refused `clear` and `total`, and the unroll
/// trial priced their loops as calls (+1,388 B, put back): 1,190 B and 92,367
/// clocks. Inlined and unrolled the whole program folds (133 B, 2,825 clocks)
/// once the stores of the arrays no body loads any more are removed after the
/// late unroll: the listing then has no call and no array.
#[test]
fn a_refused_copy_and_the_unroll_it_enables_are_priced_together() {
    for level in [Level::O2, Level::O3] {
        let text = at(level, "loop_fusion");
        for gone in ["jsr clear", "jsr total", "clear:", "total:", "data__r_a_"] {
            assert!(!text.contains(gone), "{level:?}: `{gone}` is still in the listing\n{text}");
        }
    }
}

/// local_initializers at -O1 kept `sum` as a call (316 B; the pin inlines it at
/// cost 5 under its threshold of 225, llvm-mos 96 B): the -O1 rule keeps a
/// modeled shrink only if it is an eighth of the caller, and no trial priced
/// the finished chain. With the chain priced by the backend, and the loops it
/// brings unrolled, the program folds to constants (72 B).
#[test]
fn a_callee_the_pin_inlines_at_o1_is_inlined_when_the_finished_chain_is_smaller() {
    let text = driver::assembly(include_str!("data/local_initializers_o1.ll"), &Request::new(Level::O1)).unwrap();
    let calls = text.lines().filter(|line| line.trim() == "jsr sum").count();
    assert_eq!(calls, 0, "{calls} calls to sum stay:\n{text}");
}
