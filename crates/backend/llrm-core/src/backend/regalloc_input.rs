//! Test support: a function of a MIR fixture as the register allocator
//! receives it, and the allocator phase itself.

use std::cell::RefCell;
use std::rc::Rc;

use crate::abi::runtime::{EVERY, Reg as Hard};
use crate::backend::constpool::Pool;
use crate::backend::cpu::{self, ProfileOrName};
use crate::backend::{frame, isel, target};
use crate::model::lir::LirBody;
use crate::model::passes::LIRTransform;

/// What a call leaves alone.
pub enum Calls {
    /// llrm-c's: everything but ax, bx, cx, dx, es and the flags.
    C,
    /// A Nib or BASIC program's: nothing.
    Everything,
}

/// `name` of a C program's `tests/check/mir/{fixture}`, as
/// `before_regalloc_in`.
pub fn before_regalloc<'a>(
    fixture: &str,
    name: &str,
    cpu_name: &'a str,
) -> (LirBody, Vec<Box<dyn LIRTransform + 'a>>) {
    before_regalloc_in(Calls::C, fixture, name, cpu_name)
}

/// `name` of `tests/check/mir/{fixture}` for `cpu`, run through every
/// phase before `RegAlloc` as production runs them (the spiller included);
/// the body, and the phases from `RegAlloc` on.
pub fn before_regalloc_in<'a>(
    calls: Calls,
    fixture: &str,
    name: &str,
    cpu_name: &'a str,
) -> (LirBody, Vec<Box<dyn LIRTransform + 'a>>) {
    before_phase_skipping(calls, fixture, name, cpu_name, "RegAlloc", &[])
}

/// `before_regalloc`, without the spiller: the allocator is handed the pressure
/// the spiller takes away in production. For tests of the allocator on its own,
/// which must hold for any input it is given; a test whose premise only holds
/// here guards code the production pipeline may not reach.
pub fn before_regalloc_unspilled<'a>(
    fixture: &str,
    name: &str,
    cpu_name: &'a str,
) -> (LirBody, Vec<Box<dyn LIRTransform + 'a>>) {
    before_phase_skipping(Calls::C, fixture, name, cpu_name, "RegAlloc", &["SsaSpill"])
}

/// `name` of `tests/check/mir/{fixture}` for `cpu`, run through every
/// phase before the one of class `phase`; the body, and the phases from it on.
pub fn before_phase<'a>(
    calls: Calls,
    fixture: &str,
    name: &str,
    cpu_name: &'a str,
    phase_class: &str,
) -> (LirBody, Vec<Box<dyn LIRTransform + 'a>>) {
    before_phase_skipping(calls, fixture, name, cpu_name, phase_class, &[])
}

/// `before_phase`, leaving out the phases of the classes in `skipped`.
pub fn before_phase_skipping<'a>(
    calls: Calls,
    fixture: &str,
    name: &str,
    cpu_name: &'a str,
    phase_class: &str,
    skipped: &[&str],
) -> (LirBody, Vec<Box<dyn LIRTransform + 'a>>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/check/mir").join(fixture);
    let module = llrm_mir::parse::module(&std::fs::read_to_string(path).unwrap()).expect("parses");
    let clobbered = [Hard::Ax, Hard::Bx, Hard::Cx, Hard::Dx, Hard::Es, Hard::Flags];
    let abi = crate::abi::qb::HirAbi {
        runtime: crate::hir::model::RuntimeProfile::Freestanding,
        objects: Default::default(),
        preserved: match calls {
            Calls::C => EVERY.iter().copied().filter(|one| !clobbered.contains(one)).collect(),
            Calls::Everything => Default::default(),
        },
        stack_check: None,
    };
    let cpu = cpu::profile(cpu_name).unwrap();
    let pool = Rc::new(RefCell::new(Pool::new(0)));
    let selected = isel::selected(
        &module,
        name,
        &abi,
        &mut pool.borrow_mut(),
        cpu,
        &target::BUILT_IN,
        isel::m16(),
        &llrm_x86_m16::M16,
        false,
        0,
    )
    .expect("selects");
    let mut made = frame::of(&selected.body, Some(&selected.calls), "", None).unwrap();
    made.floor = made.floor.min(-selected.depth);
    let shared = Rc::new(RefCell::new(made));
    let pinned = selected.body.pins.clone();
    let mut body = selected.body;
    let phases = crate::flow::machine(
        &pinned,
        Some(shared),
        Some(pool),
        Some(&selected.calls),
        false,
        ProfileOrName::Profile(cpu),
        &target::BUILT_IN,
        true,
    )
    .unwrap();
    let mut phases = phases.into_iter();
    for mut phase in phases.by_ref() {
        if phase.class_name() == phase_class {
            return (body, std::iter::once(phase).chain(phases).collect());
        }
        if skipped.contains(&phase.class_name()) {
            continue;
        }
        body = phase.transform(body).unwrap();
    }
    panic!("no {phase_class} phase");
}

/// `body` through every phase given, in order.
pub fn through(
    mut body: LirBody,
    phases: Vec<Box<dyn LIRTransform + '_>>,
) -> Result<LirBody, String> {
    for mut phase in phases {
        body = phase.transform(body)?;
    }
    Ok(body)
}
