//! Test support: a function of a MIR fixture as the register allocator
//! receives it, and the allocator phase itself.

use std::cell::RefCell;
use std::rc::Rc;

use crate::abi::runtime::{Reg as Hard, EVERY};
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

/// `name` of a C program's `tests/fixtures/mir/{fixture}`, as `before_regalloc_in`.
pub fn before_regalloc<'a>(fixture: &str, name: &str, cpu_name: &'a str) -> (LirBody, Vec<Box<dyn LIRTransform + 'a>>) {
    before_regalloc_in(Calls::C, fixture, name, cpu_name)
}

/// `name` of `tests/fixtures/mir/{fixture}` for `cpu`, run through every
/// phase before `RegAlloc`; the body, and the phases from `RegAlloc` on.
pub fn before_regalloc_in<'a>(calls: Calls, fixture: &str, name: &str, cpu_name: &'a str) -> (LirBody, Vec<Box<dyn LIRTransform + 'a>>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/fixtures/mir").join(fixture);
    let module = llrm_mir::parse::module(&std::fs::read_to_string(path).unwrap()).expect("parses");
    let clobbered = [Hard::Ax, Hard::Bx, Hard::Cx, Hard::Dx, Hard::Es, Hard::Flags];
    let abi = crate::abi::qb::HirAbi {
        runtime: crate::hir::model::RuntimeProfile::Freestanding,
        objects: Default::default(),
        preserved: match calls {
            Calls::C => EVERY.iter().copied().filter(|one| !clobbered.contains(one)).collect(),
            Calls::Everything => Default::default(),
        },
    };
    let cpu = cpu::profile(cpu_name).unwrap();
    let pool = Rc::new(RefCell::new(Pool::new(0)));
    let selected = isel::selected(&module, name, &abi, &mut pool.borrow_mut(), cpu, &target::BUILT_IN, false, 0).expect("selects");
    let mut made = frame::of(&selected.body, Some(&selected.calls), "", None).unwrap();
    made.floor = made.floor.min(-selected.depth);
    let shared = Rc::new(RefCell::new(made));
    let pinned = selected.body.pins.clone();
    let mut body = selected.body;
    let phases =
        crate::flow::machine(&pinned, Some(shared), Some(pool), Some(&selected.calls), false, ProfileOrName::Profile(cpu), &target::BUILT_IN).unwrap();
    let mut phases = phases.into_iter();
    for mut phase in phases.by_ref() {
        if phase.class_name() == "RegAlloc" {
            return (body, std::iter::once(phase).chain(phases).collect());
        }
        body = phase.transform(body).unwrap();
    }
    panic!("no RegAlloc phase");
}

/// `body` through every phase given, in order.
pub fn through(mut body: LirBody, phases: Vec<Box<dyn LIRTransform + '_>>) -> Result<LirBody, String> {
    for mut phase in phases {
        body = phase.transform(body)?;
    }
    Ok(body)
}
