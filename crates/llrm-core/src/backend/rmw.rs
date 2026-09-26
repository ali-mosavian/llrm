//! Port of `qbopt/backend/rmw.py`: select exact read-modify-write chains
//! before register allocation.

use std::collections::BTreeSet;
use std::sync::{Arc, LazyLock};

use crate::support::hash::{IndexMap, IndexSet};

use crate::model::ir::{self, Held, Loc, Mem, Operation, Semantics};
use crate::model::lir::{self, Insn};

/// Select private load, integer update, and store chains as one RMW.
///
/// `users` is Python's `Counter`: a missing value counts zero.
pub fn selected(insns: &[Arc<Insn>], users: &IndexMap<u32, i64>) -> Vec<Arc<Insn>> {
    let byte = _fold_byte(insns, users);
    _fold_integer(&byte, users)
}

static _MEMORY_BINARY: LazyLock<BTreeSet<&'static str>> =
    LazyLock::new(|| ["add", "sub", "and", "or", "xor"].into_iter().collect());
static _COMMUTATIVE: LazyLock<BTreeSet<&'static str>> =
    LazyLock::new(|| ["add", "and", "or", "xor"].into_iter().collect());

type Definitions = IndexMap<u32, (usize, Arc<Insn>)>;

/// Select `load; op; store-same-cell` as a memory-destination operation.
fn _fold_integer(insns: &[Arc<Insn>], users: &IndexMap<u32, i64>) -> Vec<Arc<Insn>> {
    let mut definitions: Definitions = IndexMap::default();
    for (index, one) in insns.iter().enumerate() {
        for value in &one.defines {
            definitions.insert(*value, (index, Arc::clone(one)));
        }
    }
    let mut replaced: IndexMap<usize, Arc<Insn>> = IndexMap::default();
    let mut erased: BTreeSet<usize> = BTreeSet::new();
    for (store_at, store) in insns.iter().enumerate() {
        let Some((load_at, operation_at, cell, source, name)) = _integer_chain(insns, store_at, &definitions, users)
        else {
            continue;
        };
        // The cell's read may cross only computations without observable
        // effects; the operation may move down only across zero-byte anchors.
        if insns[load_at + 1..operation_at].iter().any(|one| !_preparation(one)) {
            continue;
        }
        if insns[operation_at + 1..store_at].iter().any(|one| !_anchor(one)) {
            continue;
        }

        let what = Semantics {
            name: Some(name),
            dests: vec![Loc::Mem(cell.clone())],
            sources: vec![Loc::Mem(cell.clone()), source.clone()],
            ..Semantics::new(Operation::Binary)
        };
        let values: Vec<u32> = [Loc::Mem(cell), source]
            .iter()
            .flat_map(|operand| ir::values(operand).into_iter().map(|value| value.value))
            .collect();
        let mut made = (**store).clone();
        made.what = Some(what);
        made.uses = values.into_iter().collect::<IndexSet<u32>>().into_iter().collect();
        made.defines = Vec::new();
        made.widths = Vec::new();
        replaced.insert(store_at, Arc::new(made));
        erased.extend([load_at, operation_at]);
    }
    _rewritten(insns, &replaced, &erased)
}

fn _integer_chain(
    insns: &[Arc<Insn>],
    store_at: usize,
    definitions: &Definitions,
    users: &IndexMap<u32, i64>,
) -> Option<(usize, usize, Mem, Loc, String)> {
    let store = &insns[store_at];
    let (cell, result) = match &store.what {
        Some(what)
            if what.op == Operation::Move
                && what.name.as_deref() == Some("mov")
                && matches!(what.dests.as_slice(), [Loc::Mem(_)])
                && matches!(what.sources.as_slice(), [Loc::Held(_)]) =>
        {
            let (Loc::Mem(cell), Loc::Held(result)) = (&what.dests[0], &what.sources[0]) else {
                unreachable!()
            };
            (cell.clone(), *result)
        }
        _ => return None,
    };
    if !_plain(store) || _volatile(store) || users.get(&result.value).copied().unwrap_or(0) != 1 {
        return None;
    }

    let (operation_at, operation) = definitions.get(&result.value)?;
    let operation_at = *operation_at;
    if operation_at >= store_at || !_plain(operation) || _volatile(operation) {
        return None;
    }
    let (name, made, left, right) = match &operation.what {
        Some(what)
            if what.op == Operation::Binary
                && what.name.is_some()
                && matches!(what.dests.as_slice(), [Loc::Held(_)])
                && matches!(what.sources.as_slice(), [Loc::Held(_), Loc::Held(_) | Loc::Imm(_)]) =>
        {
            let (Loc::Held(made), Loc::Held(left)) = (&what.dests[0], &what.sources[0]) else {
                unreachable!()
            };
            (what.name.clone().expect("checked"), *made, *left, what.sources[1].clone())
        }
        _ => return None,
    };
    if !_MEMORY_BINARY.contains(name.as_str())
        || made != result
        || operation.defines != [result.value]
        || operation.symbol == Some(true)
        || ![1, 2, 4].contains(&cell.width)
        || cell.width != result.width
    {
        return None;
    }

    let mut candidates: Vec<(Held, Loc)> = vec![(left, right.clone())];
    if _COMMUTATIVE.contains(name.as_str()) {
        if let Loc::Held(right) = &right {
            candidates.push((*right, Loc::Held(left)));
        }
    }
    for (old, source) in candidates {
        let source_width = match &source {
            Loc::Held(one) => one.width,
            Loc::Imm(one) => one.width,
            _ => unreachable!("matched as Held or Imm"),
        };
        if source_width != cell.width || users.get(&old.value).copied().unwrap_or(0) != 1 {
            continue;
        }
        let Some((load_at, load)) = definitions.get(&old.value) else {
            continue;
        };
        let load_at = *load_at;
        if load_at >= operation_at || !_plain(load) || _volatile(load) {
            continue;
        }
        if let Some(what) = &load.what {
            if let (Operation::Move, Some("mov"), [Loc::Held(loaded)], [Loc::Mem(loaded_cell)]) =
                (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
            {
                if *loaded == old
                    && load.defines == [old.value]
                    && *loaded_cell == cell
                    && loaded_cell.width == old.width
                    && old.width == cell.width
                {
                    return Some((load_at, operation_at, cell, source, name));
                }
            }
        }
    }
    None
}

fn _fold_byte(insns: &[Arc<Insn>], users: &IndexMap<u32, i64>) -> Vec<Arc<Insn>> {
    let mut definitions: Definitions = IndexMap::default();
    for (index, one) in insns.iter().enumerate() {
        for value in &one.defines {
            definitions.insert(*value, (index, Arc::clone(one)));
        }
    }
    let mut replaced: IndexMap<usize, Arc<Insn>> = IndexMap::default();
    let mut erased: BTreeSet<usize> = BTreeSet::new();
    for (store_at, store) in insns.iter().enumerate() {
        let Some((load_at, mask, erase)) = _chain(insns, store_at, &definitions, users) else {
            continue;
        };
        // No read, write, branch, call, trap-like opaque operation or source
        // ownership boundary may intervene.  Pure integer preparation of the
        // mask is allowed; it remains and produces the byte source register.
        if insns[load_at + 1..store_at.max(load_at + 1)]
            .iter()
            .enumerate()
            .map(|(offset, one)| (load_at + 1 + offset, one))
            .any(|(index, one)| !erase.contains(&index) && !_pure(one))
        {
            continue;
        }
        let Some(what) = &store.what else {
            panic!("AssertionError")
        };
        let Loc::Mem(cell) = &what.dests[0] else {
            panic!("AssertionError")
        };
        let what = Semantics {
            name: Some("or".to_owned()),
            dests: vec![Loc::Mem(cell.clone())],
            sources: vec![Loc::Mem(cell.clone()), Loc::Held(mask)],
            ..Semantics::new(Operation::Binary)
        };
        let values: Vec<u32> = ir::values(&Loc::Mem(cell.clone())).iter().map(|value| value.value).collect();
        let mut made = (**store).clone();
        made.what = Some(what);
        made.uses = values
            .into_iter()
            .chain([mask.value])
            .collect::<IndexSet<u32>>()
            .into_iter()
            .collect();
        made.defines = Vec::new();
        made.widths = Vec::new();
        replaced.insert(store_at, Arc::new(made));
        erased.extend(erase);
        erased.insert(load_at);
    }
    _rewritten(insns, &replaced, &erased)
}

fn _chain(
    insns: &[Arc<Insn>],
    store_at: usize,
    definitions: &Definitions,
    users: &IndexMap<u32, i64>,
) -> Option<(usize, Held, BTreeSet<usize>)> {
    let store = &insns[store_at];
    let (cell, narrowed) = match &store.what {
        Some(what)
            if what.op == Operation::Move
                && what.name.as_deref() == Some("mov")
                && matches!(what.dests.as_slice(), [Loc::Mem(_)])
                && matches!(what.sources.as_slice(), [Loc::Held(_)]) =>
        {
            let (Loc::Mem(cell), Loc::Held(narrowed)) = (&what.dests[0], &what.sources[0]) else {
                unreachable!()
            };
            (cell.clone(), *narrowed)
        }
        _ => return None,
    };
    // Python's chained `cell.width != narrowed.width == 1`.
    if cell.width != narrowed.width && narrowed.width == 1 || !_plain(store) {
        return None;
    }
    let narrowed_definition = definitions.get(&narrowed.value);
    if narrowed_definition.is_none() || users.get(&narrowed.value).copied().unwrap_or(0) != 1 {
        return None;
    }
    let (narrowed_at, narrowed_from) = narrowed_definition.expect("checked");
    let narrowed_at = *narrowed_at;
    let joined = match &narrowed_from.what {
        Some(what)
            if what.op == Operation::Extend
                && what.name.as_deref() == Some("movzx")
                && matches!(what.dests.as_slice(), [Loc::Held(_)])
                && matches!(what.sources.as_slice(), [Loc::Held(_)]) =>
        {
            let (Loc::Held(narrowed_result), Loc::Held(joined)) = (&what.dests[0], &what.sources[0]) else {
                unreachable!()
            };
            // The source-side byte view is deliberately not a distinct SSA
            // value.  `movzx v15:word, v14:byte` followed by `mov [m],
            // v15:byte` is the C truncation after integer promotion.
            if narrowed_result.value != narrowed.value
                || narrowed_result.width != 2
                || narrowed.width != 1
                || joined.width != 1
                || !_plain(narrowed_from)
            {
                return None;
            }
            *joined
        }
        _ => return None,
    };
    let joined_definition = definitions.get(&joined.value);
    if joined_definition.is_none() || users.get(&joined.value).copied().unwrap_or(0) != 1 {
        return None;
    }
    let (joined_at, joined_from) = joined_definition.expect("checked");
    let joined_at = *joined_at;
    let (left, right) = match &joined_from.what {
        Some(what)
            if what.op == Operation::Binary
                && what.name.as_deref() == Some("or")
                && matches!(what.dests.as_slice(), [Loc::Held(_)])
                && matches!(what.sources.as_slice(), [Loc::Held(_), Loc::Held(_)]) =>
        {
            let (Loc::Held(joined_result), Loc::Held(left), Loc::Held(right)) =
                (&what.dests[0], &what.sources[0], &what.sources[1])
            else {
                unreachable!()
            };
            // Python's chained `left.width != right.width == 2`.
            if joined_result.value != joined.value
                || joined_result.width != 2
                || joined.width != 1
                || left.width != right.width && right.width == 2
                || !_plain(joined_from)
            {
                return None;
            }
            (*left, *right)
        }
        _ => return None,
    };
    let candidates = [
        (_byte_load(definitions, users, left), right),
        (_byte_load(definitions, users, right), left),
    ];
    for (loaded, mask_wide) in candidates {
        let Some((load_at, loaded_cell)) = loaded else {
            continue;
        };
        let mask_definition = definitions.get(&mask_wide.value);
        if mask_definition.is_none() || users.get(&mask_wide.value).copied().unwrap_or(0) != 1 {
            continue;
        }
        let (mask_at, mask_from) = mask_definition.expect("checked");
        let mask = match &mask_from.what {
            Some(what)
                if what.op == Operation::Extend
                    && what.name.as_deref() == Some("movzx")
                    && matches!(what.dests.as_slice(), [Loc::Held(_)])
                    && matches!(what.sources.as_slice(), [Loc::Held(_)]) =>
            {
                let (Loc::Held(mask_result), Loc::Held(mask)) = (&what.dests[0], &what.sources[0]) else {
                    unreachable!()
                };
                if *mask_result != mask_wide || mask.width != 1 || !_plain(mask_from) {
                    continue;
                }
                *mask
            }
            _ => continue,
        };
        if loaded_cell != cell {
            continue;
        }
        // The chain values are private.  Do not erase the mask's own
        // definition: it is the byte source of the final OR.
        return Some((load_at, mask, [narrowed_at, joined_at, *mask_at].into_iter().collect()));
    }
    None
}

fn _byte_load(definitions: &Definitions, users: &IndexMap<u32, i64>, value: Held) -> Option<(usize, Mem)> {
    let definition = definitions.get(&value.value);
    if definition.is_none() || users.get(&value.value).copied().unwrap_or(0) != 1 {
        return None;
    }
    let (at, one) = definition.expect("checked");
    if let Some(what) = &one.what {
        if let (Operation::Extend, Some("movzx"), [Loc::Held(result)], [Loc::Mem(cell)]) =
            (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice())
        {
            if *result == value && value.width == 2 && cell.width == 1 && _plain(one) {
                return Some((*at, cell.clone()));
            }
        }
    }
    None
}

fn _plain(one: &Insn) -> bool {
    !(!one.clobbers.is_empty()
        || !one.clobbers_high.is_empty()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
        || !one.spread.is_empty()
        || one.group.is_some()
        || one.frame_adjust
        || one.spill_reload
        || one.spill_store
        || one.rematerialized)
}

fn _volatile(one: &Insn) -> bool {
    one.op.as_ref().is_some_and(|op| op.volatile)
}

fn _anchor(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing)
        && one.defines.is_empty()
        && one.uses.is_empty()
        && _plain(one)
}

/// Whether the destination read may move across one source computation.
fn _preparation(one: &Insn) -> bool {
    if _anchor(one) {
        return true;
    }
    if !_plain(one) || one.what.is_none() || _volatile(one) {
        return false;
    }
    let what = one.what.as_ref().expect("checked");
    if ![
        Operation::Move,
        Operation::Address,
        Operation::Binary,
        Operation::Unary,
        Operation::Extend,
        Operation::Multiply,
    ]
    .contains(&what.op)
    {
        return false;
    }
    if what.dests.iter().any(|operand| matches!(operand, Loc::Mem(_) | Loc::Reg(_))) {
        return false;
    }
    let memory_sources: Vec<&Loc> = what.sources.iter().filter(|operand| matches!(operand, Loc::Mem(_))).collect();
    memory_sources.is_empty() || (what.op == Operation::Move && memory_sources.len() == 1)
}

fn _rewritten(insns: &[Arc<Insn>], replaced: &IndexMap<usize, Arc<Insn>>, erased: &BTreeSet<usize>) -> Vec<Arc<Insn>> {
    let mut out = Vec::new();
    for (index, one) in insns.iter().enumerate() {
        if let Some(made) = replaced.get(&index) {
            out.push(Arc::clone(made));
        } else if erased.contains(&index) {
            // Retain source ownership and anchors while removing the virtual
            // ranges before allocation.  lir.anchor intentionally keeps
            // definitions for post-allocation cleanup, so clear them here.
            let mut anchored = (*lir::anchor(Arc::clone(one))).clone();
            anchored.defines = Vec::new();
            anchored.uses = Vec::new();
            anchored.widths = Vec::new();
            out.push(Arc::new(anchored));
        } else {
            out.push(Arc::clone(one));
        }
    }
    out
}

fn _pure(one: &Insn) -> bool {
    if !_plain(one) || one.what.is_none() {
        return false;
    }
    // The replacement moves its selected memory read to the original store;
    // integer-only mask preparation is the deliberately narrow safe region.
    let what = one.what.as_ref().expect("checked");
    !what.dests.iter().chain(&what.sources).any(|operand| matches!(operand, Loc::Mem(_)))
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_rmw.py`.

    use std::sync::Arc;

    use crate::support::hash::IndexMap;

    use super::selected;
    use crate::model::ir::{Held, Loc, Mem, Operation, Semantics};
    use crate::model::lir::Insn;
    use crate::model::mir::Op;

    fn _insn(at: i64, what: Semantics, defines: Vec<u32>, uses: Vec<u32>, volatile: bool) -> Arc<Insn> {
        let mut op = Op::new(at, None, "", Vec::new(), Vec::new());
        op.volatile = volatile;
        let mut one = Insn::new(at, Some((at, at)), Some(what), defines, uses);
        one.op = Some(Arc::new(op));
        Arc::new(one)
    }

    fn _users(insns: &[Arc<Insn>]) -> IndexMap<u32, i64> {
        let mut users = IndexMap::default();
        for value in insns.iter().flat_map(|one| one.uses.iter()) {
            *users.entry(*value).or_insert(0) += 1;
        }
        users
    }

    fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
    }

    fn _chain(name: &str, old_on_left: bool, volatile: bool) -> Vec<Arc<Insn>> {
        let base = Held { value: 1, width: 2 };
        let cell = Loc::Mem(Mem { base: Some(base), ..Mem::new(None, 4) });
        let old = Held { value: 2, width: 4 };
        let source = Held { value: 3, width: 4 };
        let result = Held { value: 4, width: 4 };
        let (left, right) = if old_on_left { (old, source) } else { (source, old) };
        vec![
            _insn(
                0,
                semantics(Operation::Move, "mov", vec![Loc::Held(old)], vec![cell.clone()]),
                vec![old.value],
                vec![base.value],
                false,
            ),
            _insn(
                1,
                semantics(Operation::Move, "mov", vec![Loc::Held(source)], vec![Loc::Mem(Mem::new(None, 4))]),
                vec![source.value],
                Vec::new(),
                false,
            ),
            _insn(
                2,
                semantics(Operation::Binary, name, vec![Loc::Held(result)], vec![Loc::Held(left), Loc::Held(right)]),
                vec![result.value],
                vec![left.value, right.value],
                false,
            ),
            _insn(3, semantics(Operation::Nothing, "", Vec::new(), Vec::new()), Vec::new(), Vec::new(), false),
            _insn(
                4,
                semantics(Operation::Move, "mov", vec![cell], vec![Loc::Held(result)]),
                Vec::new(),
                vec![result.value, base.value],
                volatile,
            ),
        ]
    }

    #[test]
    fn test_private_integer_update_selects_a_memory_destination() {
        // nbody used a temporary for the old field and stored its sum back.
        let insns = _chain("add", true, false);

        let selected = selected(&insns, &_users(&insns));

        let cell = Loc::Mem(Mem { base: Some(Held { value: 1, width: 2 }), ..Mem::new(None, 4) });
        assert_eq!(
            selected.last().unwrap().what,
            Some(semantics(
                Operation::Binary,
                "add",
                vec![cell.clone()],
                vec![cell, Loc::Held(Held { value: 3, width: 4 })],
            ))
        );
        assert!(selected.last().unwrap().defines.is_empty());
        assert_eq!(selected.last().unwrap().uses, vec![1, 3]);
        assert_eq!(selected[0].what.as_ref().unwrap().op, Operation::Nothing);
        assert_eq!(selected[2].what.as_ref().unwrap().op, Operation::Nothing);
    }

    #[test]
    fn test_noncommutative_update_requires_the_loaded_cell_on_the_left() {
        let insns = _chain("sub", false, false);

        assert_eq!(selected(&insns, &_users(&insns)), insns);
    }

    #[test]
    fn promoted_byte_update_folds_as_python_does() {
        // No Python unit test covers `_fold_byte`; expected text is Python's
        // output for the same chain.
        use crate::model::ir::Imm;
        use crate::support::pyrepr::{self, Repr};

        let cell = Loc::Mem(Mem { base: Some(Held { value: 1, width: 2 }), ..Mem::new(None, 1) });
        let held = |value, width| Loc::Held(Held { value, width });
        let insn = |at, what, defines, uses| _insn(at, what, defines, uses, false);
        let insns = vec![
            insn(0, semantics(Operation::Extend, "movzx", vec![held(2, 2)], vec![cell.clone()]), vec![2], vec![1]),
            insn(
                1,
                semantics(Operation::Move, "mov", vec![held(3, 1)], vec![Loc::Imm(Imm { value: 7, width: 1, address: None })]),
                vec![3],
                vec![],
            ),
            insn(2, semantics(Operation::Extend, "movzx", vec![held(4, 2)], vec![held(3, 1)]), vec![4], vec![3]),
            insn(3, semantics(Operation::Binary, "or", vec![held(5, 2)], vec![held(2, 2), held(4, 2)]), vec![5], vec![2, 4]),
            insn(4, semantics(Operation::Extend, "movzx", vec![held(6, 2)], vec![held(5, 1)]), vec![6], vec![5]),
            insn(5, semantics(Operation::Move, "mov", vec![cell], vec![held(6, 1)]), vec![], vec![6, 1]),
        ];

        let printed: Vec<String> = selected(&insns, &_users(&insns))
            .iter()
            .map(|one| format!("{} {} {}", one.what.repr(), pyrepr::tuple(&one.defines), pyrepr::tuple(&one.uses)))
            .collect();

        let nothing = "Semantics(op=<Operation.NOTHING: 'nothing'>, name='', dests=(), sources=(), target=None, \
                       indirect=False) () ()";
        let mem = "Mem(addr=None, width=1, through=0, offset=0, disp_width=0, base=Held(value=1, width=2), \
                   stack_argument=False, selector=None, index=None, scale=1, index_through=0)";
        assert_eq!(
            printed,
            [
                nothing.to_owned(),
                "Semantics(op=<Operation.MOVE: 'move'>, name='mov', dests=(Held(value=3, width=1),), \
                 sources=(Imm(value=7, width=1, address=None),), target=None, indirect=False) (3,) ()"
                    .to_owned(),
                nothing.to_owned(),
                nothing.to_owned(),
                nothing.to_owned(),
                format!(
                    "Semantics(op=<Operation.BINARY: 'binary'>, name='or', dests=({mem},), sources=({mem}, \
                     Held(value=3, width=1)), target=None, indirect=False) () (1, 3)"
                ),
            ]
        );
    }

    #[test]
    fn test_volatile_update_retains_its_explicit_load_and_store() {
        let insns = _chain("add", true, true);

        assert_eq!(selected(&insns, &_users(&insns)), insns);
    }
}
