//! A function from IR to assembly text: the shared DAG stages, then the
//! m68k machine function, allocation, expansion and printing.

use llrm_mir::module::Module;

use crate::dagisel::function::select_function;
use crate::dagisel::hooks::Cpu;
use crate::expand::finish;
use crate::mc;
use crate::mfunc::emit;
use crate::regalloc::{Allocate, LinearScan, fold_reserved_copies, two_address};

/// The instructions of function `name`, one per line without indentation.
pub fn instructions(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<Vec<String>, String> {
    let blocks = select_function(module, name, cpu)?;
    let mut function = emit(name, &blocks)?;
    fold_reserved_copies(&mut function);
    two_address(&mut function);
    let assignment = LinearScan.allocate(&function)?;
    finish(&function, &assignment)?.iter().map(mc::print).collect()
}

/// The function as `llc` prints it, less the directives: the label, then each
/// instruction after a tab.
pub fn text(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<String, String> {
    let mut out = format!("{name}:\n");
    for line in instructions(module, name, cpu)? {
        out.push('\t');
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}
