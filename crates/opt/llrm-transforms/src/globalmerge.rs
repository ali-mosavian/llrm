//! Merges internal globals into one struct, so that code addresses them as
//! offsets from one base instead of one `adrp`+`add` each (LLVM's
//! `GlobalMerge`).
//!
//! # How it works
//! Variables of one address space and section are candidates when they are
//! local, initialized, not named by anything but instructions, and smaller
//! than the target's offset reach. Zero-initialized ones (`.bss`) are grouped
//! apart from the rest. Within a group, the globals each function uses
//! together are found; those used with another global are merged, in order of
//! size, into packed structs that stay within the reach. Every use becomes a
//! `getelementptr inbounds` of the struct, and the originals go.
//!
//! # Example
//! ```text
//!   @a = internal global i32 1      @_MergedGlobals = private global <{ i32, i32 }> <{ i32 1, i32 2 }>
//!   @b = internal global i32 2  ->  store i32 0, ptr getelementptr inbounds (<{..}>, @_MergedGlobals, 0, 1)
//!   store i32 0, ptr @b
//! ```
//!
//! # Deviations
//! GM1 (exit 2026-11-12, owner w15): a global another global's initializer,
//! an alias or a landing pad names stays unmerged, and external ones are not
//! merged: MIR has no alias to a member of a struct, and the arm64 writer
//! emits no alias. GM2 (same exit): a global with metadata stays unmerged.
//! The pin copies `!dbg`.
//!
//! # Runs
//! Once per module after the mid-end, for the targets that ask
//! ([`GlobalMerge`]): `AArch64TargetMachine::addPreSched2`-time IR preparation,
//! offset 4095, only for functions with `minsize` below `-O3`.
//!
//! # See also
//! LLVM's `GlobalMerge`
//! ([GlobalMerge.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/GlobalMerge.cpp)).

use std::collections::{BTreeSet, HashMap, HashSet};

use llrm_mir::context::{Constant, ConstantExpr, ConstantId, ConstantKind, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{GlobalKind, GlobalName, GlobalValue, GlobalVariable, Linkage, Module, Operand, UnnamedAddr};
use llrm_mir::opcode::{Attribute, Flags, Opcode};
use llrm_mir::target::GlobalMerge;
use llrm_mir::types::Type;
use llrm_mir::types::TypeId;

/// A set of globals, by position in the group.
#[derive(Clone)]
struct Bits(Vec<u64>);

impl Bits {
    fn new(len: usize) -> Bits {
        Bits(vec![0; len.div_ceil(64)])
    }

    fn set(
        &mut self,
        at: usize,
    ) {
        self.0[at / 64] |= 1 << (at % 64);
    }

    fn test(
        &self,
        at: usize,
    ) -> bool {
        self.0[at / 64] >> (at % 64) & 1 == 1
    }

    fn count(&self) -> usize {
        self.0.iter().map(|word| word.count_ones() as usize).sum()
    }

    fn union(
        &mut self,
        other: &Bits,
    ) {
        self.0.iter_mut().zip(&other.0).for_each(|(a, b)| *a |= b);
    }

    fn members(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.0.len() * 64).filter(|&at| self.test(at))
    }
}

/// A function that names a global, by index into `module.globals`.
#[derive(Clone, Copy)]
struct Site {
    function: usize,
}

/// One merged struct: its members and their offsets.
struct Plan {
    members: Vec<GlobalId>,
    /// The field index of each member in the struct.
    fields: Vec<u32>,
    types: Vec<TypeId>,
    inits: Vec<ConstantId>,
    align: u64,
}

/// `module` with its mergeable globals merged; whether anything changed.
pub fn merged(
    module: &mut Module,
    layout: &DataLayout,
    options: &GlobalMerge,
) -> Result<bool, String> {
    let named = named_elsewhere(module);
    let groups = candidates(module, layout, options, &named);
    if groups.iter().all(|(_, globals)| globals.len() < 2) {
        return Ok(false);
    }
    let wanted: HashSet<u32> = groups.iter().flat_map(|(_, globals)| globals.iter().map(|g| g.0)).collect();
    let sites = uses(module, &wanted);
    let mut plans = Vec::new();
    for (_, mut globals) in groups {
        if globals.len() > 1 {
            plans.extend(plan_group(module, layout, options, &sites, &mut globals));
        }
    }
    if plans.is_empty() {
        return Ok(false);
    }
    apply(module, plans, &sites, options)?;
    Ok(true)
}

/// Globals that something other than an instruction names, which stay: the
/// initializers (`llvm.used` among them), the aliases, the landing pads.
fn named_elsewhere(module: &Module) -> HashSet<u32> {
    let mut named = HashSet::new();
    let mut seen = HashSet::new();
    for global in &module.globals {
        if let GlobalKind::Variable(variable) = &global.kind {
            if let Some(init) = variable.initializer {
                collect(module, init, &mut seen, &mut named);
            }
        }
    }
    named.extend(module.aliases.iter().map(|alias| alias.aliasee.0));
    for global in &module.globals {
        let GlobalKind::Function(function) = &global.kind else { continue };
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            if matches!(instruction.opcode, Opcode::LandingPad { .. }) {
                for operand in &instruction.operands {
                    if let Operand::Constant(id) = operand {
                        collect(module, *id, &mut seen, &mut named);
                    }
                }
            }
        }
    }
    named
}

/// The globals constant `id` names, through aggregates and expressions.
fn collect(
    module: &Module,
    id: ConstantId,
    seen: &mut HashSet<ConstantId>,
    named: &mut HashSet<u32>,
) {
    if !seen.insert(id) {
        return;
    }
    match &module.context.get(id).kind {
        ConstantKind::Global(global) => {
            named.insert(global.0);
        }
        ConstantKind::Aggregate(members) => members.iter().for_each(|&member| collect(module, member, seen, named)),
        ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => {
            operands.iter().for_each(|&operand| collect(module, operand, seen, named))
        }
        ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => collect(module, *value, seen, named),
        ConstantKind::Expr(ConstantExpr::Binary { lhs, rhs, .. }) => {
            collect(module, *lhs, seen, named);
            collect(module, *rhs, seen, named);
        }
        _ => {}
    }
}

/// The groups of mergeable globals, in the order the pin visits them: data
/// first, then `.bss`, each by address space and section.
fn candidates(
    module: &Module,
    layout: &DataLayout,
    options: &GlobalMerge,
    named: &HashSet<u32>,
) -> Vec<((bool, u32, Option<String>), Vec<GlobalId>)> {
    let mut groups: Vec<((bool, u32, Option<String>), Vec<GlobalId>)> = Vec::new();
    let mut index: HashMap<(bool, u32, Option<String>), usize> = HashMap::new();
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Variable(variable) = &global.kind else { continue };
        let Some(init) = variable.initializer else { continue };
        let name = global.name.as_deref().unwrap_or_default().to_owned();
        // Internal and private only (GM1); the specials and what is named
        // elsewhere stay.
        if !matches!(global.linkage, Linkage::Internal | Linkage::Private)
            || name.starts_with("llvm.")
            || name.starts_with(".llvm.")
            || named.contains(&(at as u32))
            || module.global_metadata.contains_key(&(at as u32))
            || variable.externally_initialized
        {
            continue;
        }
        let section = module.sections.get(&name).cloned();
        if section.as_deref() == Some("llvm.metadata") {
            continue;
        }
        let size = layout.alloc_size(&module.context.types, variable.ty);
        if size >= options.max_offset {
            continue;
        }
        let bss = !variable.constant && section.is_none() && is_zero(module, init);
        // Constant globals are not merged (`MergeConstantGlobals` is off).
        if variable.constant {
            continue;
        }
        let key = (bss, global.address_space, section);
        let slot = *index
            .entry(key.clone())
            .or_insert_with(
                || {
                    groups.push((key, Vec::new()));
                    groups.len() - 1
                },
            );
        groups[slot].1.push(GlobalId(at as u32));
    }
    // `Globals` before `BSSGlobals`.
    groups.sort_by_key(|((bss, ..), _)| *bss);
    groups
}

/// `Constant::isNullValue`.
fn is_zero(
    module: &Module,
    id: ConstantId,
) -> bool {
    match &module.context.get(id).kind {
        ConstantKind::Zero | ConstantKind::Null => true,
        ConstantKind::Int(0) => true,
        ConstantKind::Aggregate(members) => members.iter().all(|&member| is_zero(module, member)),
        ConstantKind::Float(bits) => *bits == 0,
        _ => false,
    }
}

/// Every instruction operand that names one of `wanted`, directly or inside
/// a constant expression.
fn uses(
    module: &Module,
    wanted: &HashSet<u32>,
) -> HashMap<u32, Vec<Site>> {
    let mut sites: HashMap<u32, Vec<Site>> = HashMap::new();
    let mut named: HashMap<ConstantId, Vec<u32>> = HashMap::new();
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(function) = &global.kind else { continue };
        for (_, inst) in function.walk() {
            for value in &function.instruction(inst).operands {
                let Operand::Constant(id) = value else { continue };
                let found = named
                    .entry(*id)
                    .or_insert_with(
                        || {
                            let mut found = HashSet::new();
                            collect(module, *id, &mut HashSet::new(), &mut found);
                            let mut found: Vec<u32> = found.into_iter().filter(|g| wanted.contains(g)).collect();
                            found.sort_unstable();
                            found
                        },
                    );
                for &target in found.iter() {
                    sites.entry(target).or_default().push(Site { function: at });
                }
            }
        }
    }
    sites
}

/// `doMerge(Globals, ...)` of one group: which globals go together.
fn plan_group(
    module: &Module,
    layout: &DataLayout,
    options: &GlobalMerge,
    sites: &HashMap<u32, Vec<Site>>,
    globals: &mut Vec<GlobalId>,
) -> Vec<Plan> {
    let size = |id: GlobalId| match &module.globals[id.0 as usize].kind {
        GlobalKind::Variable(variable) => layout.alloc_size(&module.context.types, variable.ty),
        GlobalKind::Function(_) => 0,
    };
    // FIXME in the pin: better heuristics.
    globals.sort_by_key(|&id| size(id));
    let minsize = |function: usize| match &module.globals[function].kind {
        GlobalKind::Function(f) => f.attrs.iter().any(|attr| {
            matches!(
                attr,
                Attribute::Flag(name) if name == "minsize" || (options.optsize_counts && name == "optsize")
            )
        }),
        GlobalKind::Variable(_) => false,
    };

    // The sets of globals a function uses together, and how many functions
    // end in each (`UsedGlobalSet`).
    struct Set {
        globals: Bits,
        count: i64,
    }
    let mut sets = vec![Set { globals: Bits::new(globals.len()), count: 0 }];
    let mut by_function: HashMap<usize, usize> = HashMap::new();
    for (gi, id) in globals.iter().enumerate() {
        let mut expanded = vec![0usize; sets.len()];
        let mut only = 0usize;
        for site in sites.get(&id.0).map(Vec::as_slice).unwrap_or_default() {
            if options.size_only && !minsize(site.function) {
                continue;
            }
            let at = by_function.get(&site.function).copied().unwrap_or(0);
            if at == 0 {
                if only == 0 {
                    only = sets.len();
                    let mut globals = Bits::new(globals.len());
                    globals.set(gi);
                    sets.push(Set { globals, count: 1 });
                } else {
                    sets[only].count += 1;
                }
                by_function.insert(site.function, only);
                continue;
            }
            if sets[at].globals.test(gi) {
                sets[at].count += 1;
                continue;
            }
            sets[at].count -= 1;
            if expanded.get(at).copied().unwrap_or(0) != 0 {
                let to = expanded[at];
                sets[to].count += 1;
                by_function.insert(site.function, to);
                continue;
            }
            let to = sets.len();
            if expanded.len() <= at {
                expanded.resize(at + 1, 0);
            }
            expanded[at] = to;
            by_function.insert(site.function, to);
            let mut globals = sets[at].globals.clone();
            globals.set(gi);
            sets.push(Set { globals, count: 1 });
        }
    }

    // `GlobalMergeIgnoreSingleUse`: merge everything used with another global.
    let mut all = Bits::new(globals.len());
    for set in &sets {
        if set.count != 0 && set.globals.count() > 1 {
            all.union(&set.globals);
        }
    }
    merge_set(module, layout, options, globals, &all)
}

/// `doMerge(Globals, GlobalSet, ...)`: runs of the set that fit the reach,
/// each a struct.
fn merge_set(
    module: &Module,
    layout: &DataLayout,
    options: &GlobalMerge,
    globals: &[GlobalId],
    set: &Bits,
) -> Vec<Plan> {
    let types = &module.context.types;
    let int8 = types.intern(Type::Int(8));
    let members: Vec<usize> = set.members().filter(|&at| at < globals.len()).collect();
    let mut plans = Vec::new();
    let mut i = 0;
    while i < members.len() {
        let mut merged_size = 0u64;
        let mut plan = Plan { members: Vec::new(), fields: Vec::new(), types: Vec::new(), inits: Vec::new(), align: 1 };
        let mut j = i;
        while j < members.len() {
            let id = globals[members[j]];
            let GlobalKind::Variable(variable) = &module.globals[id.0 as usize].kind else { break };
            let name = module.globals[id.0 as usize].name.as_deref().unwrap_or_default().to_owned();
            // The alignment the writer would give it.
            let align = layout.global_align(
                types,
                variable.ty,
                variable.align,
                module.sections.contains_key(&name),
                true,
                true,
            );
            let padding = merged_size.next_multiple_of(align) - merged_size;
            merged_size += padding + layout.alloc_size(types, variable.ty);
            if merged_size > options.max_offset {
                break;
            }
            if padding != 0 {
                let array = types.intern(Type::Array { element: int8, count: padding });
                plan.types.push(array);
                plan.inits.push(module.context.constant(Constant { ty: array, kind: ConstantKind::Zero }));
            }
            plan.fields.push(plan.types.len() as u32);
            plan.types.push(variable.ty);
            plan.inits.push(variable.initializer.expect("a candidate is initialized"));
            plan.members.push(id);
            plan.align = plan.align.max(align);
            j += 1;
        }
        // A run of one is not merged.
        if plan.members.len() >= 2 {
            plans.push(plan);
        }
        i = j.max(i + 1);
    }
    plans
}

/// Makes the structs, points every use at them and drops the originals.
fn apply(
    module: &mut Module,
    plans: Vec<Plan>,
    sites: &HashMap<u32, Vec<Site>>,
    options: &GlobalMerge,
) -> Result<(), String> {
    let mut remove: HashSet<u32> = HashSet::new();
    // member -> (merged global, struct type, field index)
    let mut homes: HashMap<u32, (GlobalId, TypeId, u32)> = HashMap::new();
    for plan in plans {
        let first = plan.members[0];
        let (space, section) = {
            let first_global = &module.globals[first.0 as usize];
            (
                first_global.address_space,
                first_global.name.as_deref().map(str::to_owned).and_then(|name| module.sections.get(&name).cloned()),
            )
        };
        let merged_type = module.context.types.intern(Type::Struct { fields: plan.types.clone(), packed: true });
        let init =
            module.context.constant(Constant { ty: merged_type, kind: ConstantKind::Aggregate(plan.inits.clone()) });
        let name = unique_name(module, "_MergedGlobals");
        let id = GlobalId(module.globals.len() as u32);
        module
            .globals
            .push(
                GlobalValue {
                    name: GlobalName::from(name.clone()),
                    linkage: if options.mach_o { Linkage::Internal } else { Linkage::Private },
                    visibility: Default::default(),
                    unnamed_addr: UnnamedAddr::None,
                    address_space: space,
                    kind: GlobalKind::Variable(GlobalVariable {
                        ty: merged_type,
                        constant: false,
                        externally_initialized: false,
                        initializer: Some(init),
                        align: Some(plan.align),
                    }),
                },
            );
        if let Some(section) = section {
            module.sections.insert(name, section);
        }
        for (member, field) in plan.members.iter().zip(&plan.fields) {
            homes.insert(member.0, (id, merged_type, *field));
            remove.insert(member.0);
        }
    }

    // Every use becomes the constant `getelementptr inbounds (M, @M, 0, i)`,
    // inside whatever expression named the original.
    let mut memo: HashMap<ConstantId, ConstantId> = HashMap::new();
    let functions: BTreeSet<usize> =
        homes.keys().flat_map(|member| sites.get(member).into_iter().flatten().map(|site| site.function)).collect();
    let (globals, context) = (&mut module.globals, &module.context);
    for at in functions {
        let GlobalKind::Function(function) = &mut globals[at].kind else { continue };
        let mut edits = Vec::new();
        for (_, inst) in function.walk() {
            for (index, value) in function.instruction(inst).operands.iter().enumerate() {
                let Operand::Constant(old) = value else { continue };
                let new = rewrite(context, &homes, *old, &mut memo);
                if new != *old {
                    edits.push((inst, index, new));
                }
            }
        }
        for (inst, index, new) in edits {
            function.set_operand(inst, index, Operand::Constant(new));
        }
    }

    // The originals go, the rest renumbered; what is keyed by position follows.
    let kept: Vec<Option<u32>> = {
        let mut next = 0;
        (0..module.globals.len() as u32)
            .map(|at| {
                (!remove.contains(&at)).then(|| {
                    next += 1;
                    next - 1
                })
            })
            .collect()
    };
    module.retain_globals(&|id| !remove.contains(&id.0));
    module.global_metadata = std::mem::take(&mut module.global_metadata)
        .into_iter()
        .filter_map(|(at, attachments)| kept.get(at as usize).copied().flatten().map(|now| (now, attachments)))
        .collect();
    Ok(())
}

/// `base`, or `base.N` where it is taken (`Module::getOrInsert` renames).
fn unique_name(
    module: &Module,
    base: &str,
) -> String {
    let taken: HashSet<String> =
        module.globals.iter().filter_map(|global| global.name.as_deref().map(str::to_owned)).collect();
    if !taken.contains(base) {
        return base.to_owned();
    }
    (1..).map(|n| format!("{base}.{n}")).find(|name| !taken.contains(name)).expect("an unused name")
}

/// `id` with every merged global replaced by its place in the struct.
fn rewrite(
    context: &llrm_mir::context::Context,
    homes: &HashMap<u32, (GlobalId, TypeId, u32)>,
    id: ConstantId,
    memo: &mut HashMap<ConstantId, ConstantId>,
) -> ConstantId {
    if let Some(&done) = memo.get(&id) {
        return done;
    }
    let Constant { ty, kind } = context.get(id).clone();
    let new = match kind {
        ConstantKind::Global(global) => match homes.get(&global.0) {
            Some(&(merged, merged_type, field)) => {
                let base = context.constant(Constant { ty, kind: ConstantKind::Global(merged) });
                let i32_type = context.types.int(32);
                let zero = context.int(i32_type, 0);
                let index = context.int(i32_type, i128::from(field));
                context.constant(Constant {
                    ty,
                    kind: ConstantKind::Expr(ConstantExpr::GetElementPtr {
                        source: merged_type,
                        inbounds: true,
                        nusw: true,
                        nuw: false,
                        operands: vec![base, zero, index],
                    }),
                })
            }
            None => id,
        },
        ConstantKind::Aggregate(members) => {
            let rewritten: Vec<ConstantId> = members.iter().map(|&one| rewrite(context, homes, one, memo)).collect();
            if rewritten == members {
                id
            } else {
                context.constant(Constant { ty, kind: ConstantKind::Aggregate(rewritten) })
            }
        }
        ConstantKind::Expr(ConstantExpr::GetElementPtr { source, inbounds, nusw, nuw, operands }) => {
            let rewritten: Vec<ConstantId> = operands.iter().map(|&one| rewrite(context, homes, one, memo)).collect();
            if rewritten == operands {
                id
            } else {
                context.constant(Constant {
                    ty,
                    kind: ConstantKind::Expr(ConstantExpr::GetElementPtr {
                        source,
                        inbounds,
                        nusw,
                        nuw,
                        operands: rewritten,
                    }),
                })
            }
        }
        ConstantKind::Expr(ConstantExpr::Cast { op, value }) => {
            let rewritten = rewrite(context, homes, value, memo);
            if rewritten == value {
                id
            } else {
                context.constant(Constant { ty, kind: ConstantKind::Expr(ConstantExpr::Cast { op, value: rewritten }) })
            }
        }
        ConstantKind::Expr(ConstantExpr::Binary { op, lhs, rhs }) => {
            let (l, r) = (rewrite(context, homes, lhs, memo), rewrite(context, homes, rhs, memo));
            if (l, r) == (lhs, rhs) {
                id
            } else {
                context.constant(Constant { ty, kind: ConstantKind::Expr(ConstantExpr::Binary { op, lhs: l, rhs: r }) })
            }
        }
        _ => id,
    };
    memo.insert(id, new);
    new
}
