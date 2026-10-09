//! The spill model's forecast, set beside the spill code the allocator leaves:
//! `LLRM_DEBUG=pressure`.

use std::process::Command;

/// A value kept across a call in a function that has too few registers for all
/// it holds.
const KERNEL: &str = "
extern int f(int);
int g(int a, int b, int c, int d, int e, int h, int i)
{
    int x = f(a) + f(b) + f(c) + f(d) + f(e) + f(h) + f(i);
    return x + a + b + c + d + e + h + i;
}
";

/// The channel called `Unit::registers` on a unit that carries none, which #724
/// made a hard error: any compile with `LLRM_DEBUG=spillmodel` panicked, and
/// with it the only check of the model against the allocator (#739).
#[test]
fn test_the_pressure_channel_reports_a_forecast_beside_the_allocators_spills() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), KERNEL).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .env("LLRM_DEBUG", "pressure")
        .args(["-m32", "-O2", "-march=i486", "-S", "-o", "a.s", "a.c"])
        .output()
        .unwrap();
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{errors}");
    // The name the target's default convention gives `g` in OMF, which the
    // report prints.
    let name = llrm_target::Target::calling(&llrm_x86_m32::M32)
        .native()
        .decorated("omf", "g")
        .expect("the default convention decorates for OMF");
    let row = errors
        .lines()
        .find(|line| line.starts_with(&format!("[pressure] {name} forecast")))
        .unwrap_or_else(|| panic!("no row for {name}: {errors}"));
    assert!(row.contains("allocator") && row.contains("reloads"), "{row}");
}
