//! Test support: the machine-phase list and the LIR a fixture has reached just
//! before a chosen phase.
//!
//! # How it works
//! `before_phase_skipping` parses a MIR fixture under `tests/check/mir`,
//! selects one function for the 16-bit target (`isel::selected`), lays out its
//! frame (`frame::of`) and builds the machine phase list with `flow::machine`.
//! It runs the phases in order until the one whose class name is `phase_class`,
//! skipping any named in `skipped`, and returns the body so far and the
//! remaining phases starting with that one.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart LR
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   f["<b>fixture.mir</b>"]:::mid
//!   i["<b>isel</b>"]:::mid
//!   fr["<b>frame</b>"]:::mid
//!   run["<b>phase 1 .. k-1</b><br/><small>run here, body returned</small>"]:::mid
//!   rest["<b>phase k .. last</b><br/><small>returned, unrun</small>"]:::mid
//!   f --> i
//!   i --> fr
//!   fr --> run
//!   run --> rest
//! ```
//!
//! *Where the fixture's phases stop*
//!
//! The wrappers fix the usual arguments. `before_regalloc` and
//! `before_regalloc_in` stop before `RegAlloc` with the spiller run, as in
//! production. `before_regalloc_unspilled` leaves out `SsaSpill`, handing the
//! allocator the pressure the spiller removes, for tests of the allocator that
//! must hold for any input. `Calls` states what a call leaves alone: C's
//! convention keeps everything but ax, bx, cx, dx, es and the flags; a Nib or
//! BASIC program's keeps nothing. `through` runs a body through given phases.
//!
//! # Example
//! None: a test-support module with no transformation of its own. Its callers
//! show the LIR it hands over; read `lirtext.rs` for the text format.
//!
//! # Used by
//! Tests of [`crate::backend::allocate`], [`crate::backend::ssaspill`],
//! [`crate::backend::loopslots`] and the jumps and total-allocation test files.
//! Compiled only under `cfg(test)`.

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
    // The fixtures are m16 and state no datalayout of their own: the target's
    // is stated once, in its description.
    let text = std::fs::read_to_string(path).unwrap();
    let layout = llrm_x86_m16::DATALAYOUT_TOML
        .lines()
        .find_map(|line| line.strip_prefix("datalayout = \"")?.strip_suffix('"'))
        .expect("datalayout.toml states a datalayout");
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{layout}\"\n{text}")).expect("parses");
    let clobbered = [Hard::Ax, Hard::Bx, Hard::Cx, Hard::Dx, Hard::Es, Hard::Flags];
    let abi = crate::abi::qb::HirAbi {
        runtime: crate::hir::model::RuntimeProfile::Freestanding,
        objects: Default::default(),
        preserved: match calls {
            Calls::C => EVERY.iter().copied().filter(|one| !clobbered.contains(one)).collect(),
            Calls::Everything => Default::default(),
        },
        stack_check: None,
        hosted: false,
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
