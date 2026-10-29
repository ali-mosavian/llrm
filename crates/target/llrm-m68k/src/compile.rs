//! A function from IR to assembly text: the shared DAG stages, then the
//! m68k machine function, allocation, expansion and printing.

use llrm_mir::module::Module;

use crate::dagisel::function::select_function;
use crate::dagisel::hooks::Cpu;
use crate::expand::{Line, finish};
use crate::mc;
use crate::mfunc::emit;
use crate::regalloc::{Allocate, GraphColoring, eliminate_phis, fold_reserved_copies, two_address};

/// A line of the function: a label or an instruction.
pub enum Item {
    Label(String),
    Inst(mc::Inst),
}

/// The function after selection, allocation and expansion: its label, then
/// each block's label where a branch names it, and the instructions.
pub fn items(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<Vec<Item>, String> {
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let number =
        module.functions().filter(|(_, _, one)| !one.is_declaration()).position(|(one, _, _)| one == id).unwrap_or(0);
    let blocks = select_function(module, name, cpu)?;
    let mut function = emit(name, &blocks)?;
    eliminate_phis(&mut function);
    fold_reserved_copies(&mut function);
    two_address(&mut function);
    let assignment = GraphColoring.allocate(&function)?;
    let mut out = vec![Item::Label(name.to_owned())];
    for line in finish(&function, &assignment, number)? {
        out.push(match line {
            Line::Label(at) => Item::Label(format!(".LBB{number}_{at}")),
            Line::Inst(inst) => Item::Inst(inst),
        });
    }
    Ok(out)
}

/// The function as `llc` prints it, less the directives: the label, then each
/// instruction after a tab, and a block's label where a branch names it.
pub fn text(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<String, String> {
    let mut out = String::new();
    for item in items(module, name, cpu)? {
        match item {
            Item::Label(label) => out.push_str(&format!("{label}:\n")),
            Item::Inst(inst) => out.push_str(&format!("\t{}\n", mc::print(&inst)?)),
        }
    }
    Ok(out)
}
