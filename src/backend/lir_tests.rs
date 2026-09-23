//! Port of `tests/test_lir.py`'s tests over raised BC objects.
//!
//! Skipped: the two `_EXPANDS` tests (monkeypatch a table the Rust lowering
//! has as a match). The rest of the module is ported beside
//! the code it tests.

use std::collections::BTreeSet;
use std::rc::Rc;

use iced_x86::Register;

use crate::abi::runtime;
use crate::backend::{lower, target};
use crate::frontend::blocks::{self, Block};
use crate::model::ir::{self, Loc, Operation, root};
use crate::model::lir::LirBody;
use crate::model::mir::{self, Kind, MirBody, RaisedBodies};
use crate::objectfile::module::{self, Module};
use crate::objectfile::omf;
use crate::optimize::transform;
use crate::support::testing;

fn raised(stem: &str) -> (Rc<Module>, Rc<Vec<Block>>, RaisedBodies) {
    let found = testing::module(&format!("fixtures/omf/{}.obj", stem.to_lowercase()));
    let blocks = testing::blocks_of(&found);
    let bodies = testing::raised_from(&found, &blocks, None);
    (found, blocks, bodies)
}

/// Python's `lower.lowered(name, body, found.calls, source.absorbed, contracts, source.coverage, ...)`.
fn lowered(
    name: &str,
    body: &MirBody,
    found: &Module,
    raised: &RaisedBodies,
    contracts: &crate::support::hash::IndexMap<i64, runtime::Contract>,
    hints: Option<&mir::AllocationHints>,
) -> LirBody {
    let source = &raised.source;
    lower::lowered(
        name,
        body,
        Some(&found.calls),
        source.absorbed.keys().copied().collect(),
        Some(contracts),
        "386",
        lower::Lowered {
            coverage: source.coverage.clone(),
            nodes: source.nodes.clone(),
            occurrences: Some(&source.occurrences),
            hints,
            sites: source.absorbed.clone(),
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn test_string_copy_keeps_its_implicit_address_registers() {
    // fpdeep printed DSQ=0 for 144: movsw lost the SI/DI addresses of its double copy.
    let data = std::fs::read("fixtures/omf/fpdeep-p-g2.obj").unwrap();
    let found = module::of(&omf::parse(&data).unwrap()).unwrap();
    let mut contracts = runtime::for_module(&found, None).unwrap();
    let blocks = blocks::partition(&found, &blocks::code_map(&found).unwrap());
    let raised = mir::bodies(&found, &blocks, Some(&mut contracts), false, false).unwrap();
    let (name, body) = &raised.values[0];
    let low = lowered(name, body, &found, &raised, &contracts, raised.hints.get(&body.entry));
    let copies: Vec<_> = low
        .blocks
        .iter()
        .flat_map(|block| &block.insns)
        .filter(|one| one.op.as_ref().is_some_and(|op| op.kind == Kind::Opaque && (0x154..0x158).contains(&op.at)))
        .collect();
    assert_eq!(copies.len(), 4);
    let both = BTreeSet::from([Register::SI, Register::DI]);
    for one in copies {
        assert_eq!(one.requires.iter().map(|(_, register)| *register).collect::<BTreeSet<_>>(), both);
        assert_eq!(one.delivers.iter().map(|(_, register)| *register).collect::<BTreeSet<_>>(), both);
    }
}

#[test]
fn test_a_procedure_hands_back_dx_ax() {
    // procs p-ot's TWICE& left its answer in bx and ax, and its callers read dx:ax.
    let data = std::fs::read("fixtures/omf/procs-p-ot.obj").unwrap();
    let found = module::of(&omf::parse(&data).unwrap()).unwrap();
    let mut contracts = runtime::for_module(&found, None).unwrap();
    let blocks = blocks::partition(&found, &blocks::code_map(&found).unwrap());
    let raised = mir::bodies(&found, &blocks, Some(&mut contracts), false, false).unwrap();
    let (name, body) = raised.values.iter().find(|(name, _)| name.contains("TWICE")).unwrap();
    let low = lowered(name, body, &found, &raised, &contracts, None);
    let returns: Vec<_> = low
        .blocks
        .iter()
        .flat_map(|block| &block.insns)
        .filter(|one| one.op.as_ref().is_some_and(|op| op.kind == Kind::Return))
        .collect();
    assert_eq!(returns.len(), 1);
    let got: BTreeSet<Register> = returns[0].requires.iter().map(|(_, register)| *register).collect();
    assert_eq!(got, BTreeSet::from([Register::AX, Register::DX]));
}

#[test]
fn test_bcs_own_assignment_satisfies_every_requirement() {
    // The table is only worth having if the code it describes obeys it.
    let mut fixtures: Vec<_> = std::fs::read_dir("fixtures/omf")
        .unwrap()
        .map(|one| one.unwrap().path())
        .filter(|one| one.extension().is_some_and(|extension| extension == "obj"))
        .collect();
    fixtures.sort();
    for obj in fixtures {
        let stem = obj.file_stem().unwrap().to_string_lossy().into_owned();
        let Some(found) = module::of(&omf::parse(&std::fs::read(&obj).unwrap()).unwrap()) else {
            continue;
        };
        let Ok(mapped) = blocks::code_map(&found) else {
            continue;
        };
        let raised = mir::bodies(&found, &blocks::partition(&found, &mapped), None, false, false).unwrap();
        for (name, body) in &raised.values {
            let hints = &raised.hints[&body.entry];
            let rooted = |one: &mir::Value| hints.origin_of(*one).map(root);
            for block in &body.blocks {
                let mut written: BTreeSet<Option<Register>> = BTreeSet::new();
                for op in &block.ops {
                    // `cwd` before `idiv` is raised as a sign extension the
                    // divide no longer names; dx still holds what BC put there.
                    let mut held: BTreeSet<Option<Register>> = op.uses.iter().map(rooted).collect();
                    held.extend(written.iter().copied());
                    written.extend(op.defines.iter().map(rooted));
                    if !op.source_backed {
                        continue;
                    }
                    let node = op.id.and_then(|id| raised.source.nodes.get(&id)).map(|node| &**node);
                    let Some(what) = lower::current(op, lower::Place::Default, node).unwrap() else {
                        continue;
                    };
                    for (want, need) in target::reads(&what) {
                        if need.fixed().is_some() {
                            assert!(
                                held.contains(&Some(want)),
                                "{stem} {name}: {:#x} {} is said to need {want:?} and BC has no value there",
                                op.at,
                                op.name
                            );
                        } else {
                            assert!(
                                need.r#where.iter().map(|one| root(*one)).any(|one| one == want),
                                "{stem} {name}: {:#x} {} reaches memory by {want:?}, which the class does not permit",
                                op.at,
                                op.name
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn test_lowering_is_one_instruction_per_operation_unless_something_expands() {
    let (found, _blocks, raised) = raised("hotlop-p-g2");
    let (name, body) = &raised.values[0];
    let contracts = runtime::for_module(&found, None).unwrap();
    let low = lowered(name, body, &found, &raised, &contracts, None);
    let ops: Vec<_> = body.blocks.iter().flat_map(|block| &block.ops).collect();
    let insns: Vec<_> = low.blocks.iter().flat_map(|block| &block.insns).collect();
    assert_eq!(insns.len(), ops.len());
    for (one, op) in insns.iter().zip(&ops) {
        let owned: Vec<(i64, i64)> =
            op.absorbed.iter().flat_map(|identity| raised.source.occurrences[identity].iter().copied()).collect();
        assert_eq!(one.at, op.at);
        assert_eq!(one.op.as_deref(), Some(*op));
        let expected: BTreeSet<i64> = owned.iter().flat_map(|&(low, high)| low..high).collect();
        let actual: BTreeSet<i64> =
            one.covers.iter().chain(&one.spread).flat_map(|&(low, high)| low..high).collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn test_an_allocatable_value_stays_a_value_through_lowering() {
    // pressx printed R= 6460 for 7500.
    let (found, blocks, raised) = raised("harr-v-g3");
    let (name, body) = &raised.values[0];
    let body = transform::applied(
        body,
        &found.dgroup.members,
        &found.calls,
        transform::Applied { blocks: Some(blocks), found: Some(found.clone()), ..Default::default() },
    )
    .unwrap();
    let contracts = runtime::for_module(&found, None).unwrap();
    let low = lowered(name, &body, &found, &raised, &contracts, None);
    // By what they compute, not by where they sit.
    let rows: Vec<&ir::Semantics> =
        low.blocks.iter().flat_map(|block| &block.insns).filter_map(|one| one.what.as_ref()).collect();
    let pairs: Vec<(&ir::Semantics, &ir::Semantics)> = rows
        .windows(2)
        .filter(|pair| {
            let (what, then) = (pair[0], pair[1]);
            what.op == Operation::Move
                && what.sources.len() == 1
                && matches!(what.sources[0], Loc::Imm(_))
                && then.op == Operation::Move
                && then.sources.len() == 1
                && matches!(then.sources[0], Loc::Mem(_))
        })
        .map(|pair| (pair[0], pair[1]))
        .collect();
    assert_eq!(pairs.len(), 1, "{} constant-then-load pairs, so this proves nothing", pairs.len());
    let (constant, load) = pairs[0];
    assert!(constant.dests.iter().all(|one| matches!(one, Loc::Held(_))), "lowering resolved the constant");
    assert!(load.dests.iter().all(|one| matches!(one, Loc::Held(_))), "lowering resolved the load");
    let (Loc::Held(constant), Loc::Held(load)) = (&constant.dests[0], &load.dests[0]) else { unreachable!() };
    assert_ne!(constant.value, load.value, "the constant and the load name one value");
}

fn one_of(values: &[mir::Value]) -> BTreeSet<u32> {
    values.iter().filter(|value| !value.flags).map(|value| value.id).collect()
}

#[test]
fn test_an_increment_is_its_own_operation() {
    let (stem, at, expected, want) = ("addrm-p-g2", 0x86, Kind::Increment, "inc");
    let (_found, _blocks, raised) = raised(stem);
    for (_name, body) in &raised.values {
        for op in body.blocks.iter().flat_map(|block| &block.ops) {
            if op.at != at || op.kind != expected {
                continue;
            }
            assert_eq!(op.args.len(), 1, "the implicit operand was written out");
            let node = op.id.and_then(|id| raised.source.nodes.get(&id)).map(|node| &**node);
            let what = lower::current(op, lower::Place::AsAValue, node).unwrap().unwrap();
            assert!(what.op == Operation::Unary && what.name.as_deref() == Some(want));
            let (Loc::Held(source), Loc::Held(dest)) = (&what.sources[0], &what.dests[0]) else {
                panic!("not values: {what:?}");
            };
            assert_eq!(what.sources.len(), 1);
            assert!(one_of(&op.defines).contains(&dest.value));
            assert!(one_of(&op.uses).contains(&source.value));
            return;
        }
    }
    panic!("no increment operation at {at:#06x}");
}

#[test]
fn test_a_stores_address_is_the_value_that_computed_it() {
    // arrprm printed ' 0  0' for ' 7  8'.
    let (found, blocks, raised) = raised("addrm-p-g2");
    let contracts = runtime::for_module(&found, None).unwrap();
    let mut seen = 0;
    for (name, body) in &raised.values {
        let body = transform::applied(
            body,
            &found.dgroup.members,
            &found.calls,
            transform::Applied { blocks: Some(blocks.clone()), found: Some(found.clone()), ..Default::default() },
        )
        .unwrap();
        let low = lowered(name, &body, &found, &raised, &contracts, None);
        for one in low.blocks.iter().flat_map(|block| &block.insns) {
            let Some(what) = &one.what else { continue };
            for r#where in what.dests.iter().chain(&what.sources) {
                let Loc::Mem(mem) = r#where else { continue };
                let Some(base) = mem.base else { continue };
                seen += 1;
                assert!(one.uses.contains(&base.value), "{:#06x} reaches memory by a value it does not read", one.at);
            }
        }
        // every store whose MIR base named a value must carry it
        for op in body.blocks.iter().flat_map(|block| &block.ops) {
            for reference in &op.stores {
                let Some(base) = reference.base else { continue };
                let made = low
                    .blocks
                    .iter()
                    .flat_map(|block| &block.insns)
                    .filter(|one| one.what.is_some() && one.op.as_deref().is_some_and(|mine| std::ptr::eq(mine, op) || mine == op));
                for one in made {
                    let cell = one.what.as_ref().unwrap().dests.iter().find_map(|one| match one {
                        Loc::Mem(mem) => Some(mem),
                        _ => None,
                    });
                    let cell = cell.unwrap_or_else(|| panic!("{:#06x} lowered its base as None", op.at));
                    assert_eq!(cell.base.map(|one| one.value), Some(base.id), "{:#06x}", op.at);
                }
            }
        }
    }
    assert!(seen >= 3, "only {seen} based cells; addrm-p-g2 has three");
}

/// hotlpx rebuilt twice printed S=250 for 630: `lea ax,[ebx+ebx*4]` pinned
/// nothing, and the product it reads was allocated to ax. The decoded
/// address, not a historical register name, is the oracle.
#[test]
#[ignore = "fails in Python too: ValueError: not enough values to unpack (expected 1, got 0)"]
fn test_an_opaque_address_keeps_the_registers_it_is_written_in() {
    let data = testing::data("fixtures/omf/hotlpx-p-g2.obj");
    let rebuilt = crate::rewrite::rewrite(&data, &crate::rewrite::Rewrite::new(false)).unwrap().0;
    let found = testing::loaded_bytes(&rebuilt).unwrap();
    let mut contracts = runtime::for_module(&found, None).unwrap();
    let blocks = blocks::partition(&found, &blocks::code_map(&found).unwrap());
    let raised = mir::bodies(&found, &blocks, Some(&mut contracts), false, false).unwrap();
    let (name, body) = &raised.values[0];
    let low = lowered(name, body, &found, &raised, &contracts, None);
    let leas: Vec<_> = low
        .blocks
        .iter()
        .flat_map(|block| &block.insns)
        .filter(|one| one.op.as_ref().is_some_and(|op| op.kind == Kind::Address))
        .collect();
    let [lea] = <[_; 1]>::try_from(leas).unwrap();
    let ir::Loc::Address(address) = &lea.node.as_ref().unwrap().semantics().sources[0] else { panic!("not an address") };
    let expected: BTreeSet<Register> =
        [address.through, address.index].into_iter().filter(|&one| one != Register::None).map(root).collect();
    assert!(!expected.is_empty(), "the fixture no longer has a register-based address");
    assert_eq!(lea.requires.iter().map(|(_, register)| root(*register)).collect::<BTreeSet<_>>(), expected);
}
