//! A function from IR to assembly text: the shared DAG stages, then the
//! m68k machine function, allocation, expansion and printing.

use llrm_mir::module::Module;

use crate::dagisel::function::select_function;
use crate::dagisel::hooks::Cpu;
use crate::expand::{Line, finish};
use crate::mc;
use crate::mfunc::{emit, expand_cmov};
use crate::regalloc::{
    allocate_spilling, branch_fold, eliminate_phis, fold_reserved_copies, machine_sink, remove_identity_copies,
    two_address,
};

/// A line of the function: a label or an instruction.
#[derive(Clone, Debug)]
pub enum Item {
    Label(String),
    Inst(mc::Inst),
    /// Bytes placed as they are: code assembled elsewhere, or data.
    Bytes(Vec<u8>),
}

/// What the compiler may do beyond the pin's output.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Use the instructions of [`crate::overlay`] (`addq`, `subq`, `tst`).
    pub overlay: bool,
}

impl Options {
    /// The pin's output, instruction for instruction: what `llc` and `llvm-mc`
    /// can be compared with.
    pub const PINNED: Options = Options { overlay: false };
}

impl Default for Options {
    fn default() -> Options {
        Options { overlay: true }
    }
}

/// The function after selection, allocation and expansion: its label, then
/// each block's label where a branch names it, and the instructions.
pub fn items(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<Vec<Item>, String> {
    items_with(module, name, cpu, Options::default())
}

/// [`items`] with `options`.
pub fn items_with(
    module: &Module,
    name: &str,
    cpu: Cpu,
    options: Options,
) -> Result<Vec<Item>, String> {
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let number =
        module.functions().filter(|(_, _, one)| !one.is_declaration()).position(|(one, _, _)| one == id).unwrap_or(0);
    let blocks = select_function(module, name, cpu)?;
    let mut function = emit(name, &blocks)?;
    expand_cmov(&mut function)?;
    machine_sink(&mut function);
    eliminate_phis(&mut function);
    fold_reserved_copies(&mut function);
    two_address(&mut function);
    let (assignment, slots) = allocate_spilling(&mut function, 0)?;
    let slots: Vec<i32> = slots.into_iter().map(|(slot, _)| slot).collect();
    remove_identity_copies(&mut function, &assignment);
    branch_fold(&mut function);
    let mut out = vec![Item::Label(name.to_owned())];
    for line in finish(&function, &assignment, number, &slots, options.overlay)? {
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
    text_with(module, name, cpu, Options::default())
}

/// [`text`] with `options`.
pub fn text_with(
    module: &Module,
    name: &str,
    cpu: Cpu,
    options: Options,
) -> Result<String, String> {
    let mut out = String::new();
    for item in items_with(module, name, cpu, options)? {
        match item {
            Item::Label(label) => out.push_str(&format!("{label}:\n")),
            Item::Inst(inst) => out.push_str(&format!("\t{}\n", mc::print(&inst)?)),
            Item::Bytes(bytes) => out.push_str(&format!("\t.byte\t{}\n", bytes.len())),
        }
    }
    Ok(out)
}
