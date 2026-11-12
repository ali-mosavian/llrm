//! DEVIATE OS-INLINE-TRIAL: a copy the -Os byte model refuses within its error is inlined in a
//! second compile and kept where the backend writes the chain smaller.

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{self, Level, Request};
use llrm_6502::target::Mos;
use llrm_target::Target;

fn os(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_size").join(format!("{name}.ll"));
    let text = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("target datalayout"))
        .fold(format!("target datalayout = \"{}\"\n", Mos.layout().datalayout), |text, line| text + line + "\n");
    let mut request = Request::new(Level::Os);
    request.cpu = Cpu::Mos6502;
    driver::assembly(&text, &request).unwrap_or_else(|why| panic!("{name}: {why}"))
}

/// memcpy_libcall -Os: the model read the two copies of the private `same` as 52 B larger than its
/// calls and refused them; the object is 58 B larger for it (CODE 577 against 519). `same` is gone
/// once the trial inlines it.
#[test]
fn a_copy_the_model_refuses_within_its_error_is_inlined_when_the_backend_writes_it_smaller() {
    let text = os("memcpy_libcall");
    assert!(!text.contains("same:") && !text.contains("jsr same"), "`same` is still a function\n{text}");
}
