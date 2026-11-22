//! `SLPVectorizer`'s lane reuse and build-vector gathers: the lanes of a
//! bundle that repeat, packed once and read again through a shuffle mask
//! (`ReuseShuffleIndices`), and the vector a gather builds from its scalars.
//!
//! # How it works
//! `tryToFindDuplicates` (l.9997) lists the distinct lanes of a bundle and the
//! lane each position reads; `EstimatePackPlusShuffleVsInserts` (l.10069)
//! keeps the bundle packed when the distinct lanes' inserts and the reuse
//! shuffle cost no more than inserting every lane. A gather is priced and
//! built by `processBuildVector` (l.22606): `TryPackScalars` (l.22861) puts
//! each distinct non-constant scalar at its first lane, a value in every lane
//! becomes lane 0 and a broadcast, `getBuildVectorCost` (l.14851) and
//! `getGatherCost` (l.21526) price the inserts, `finalize` (l.15683) the
//! shuffle of the reuse mask over the packed lanes.
//!
//! DEVIATE SLP5 (owner arm64, exit 2026-11-24): no undef lane, no
//! `isGatherShuffledEntry` (a gather read from another entry's vector), no
//! alternate-opcode bundles, loads that scatter or stride; a bundle the pin
//! keeps unpacked with its repeats (`UseOriginal`) is a gather here; a count
//! of distinct lanes that is not a power of two is not packed.
//!
//! # Runs
//! Under `slptree`, for each bundle that repeats a lane and each gather.
//!
//! # Used by
//! `slptree`.
//!
//! # Example
//! A bundle whose lanes repeat is packed once with its distinct lanes and read
//! back through a shuffle mask.
//!
//! ```text
//! bundle lanes: (%a, %b, %a, %b)
//! =>
//! %v = <2 x T> (%a, %b)
//! %r = shufflevector %v, poison, <0, 1, 0, 1>
//! ```

use llrm_mir::module::Operand;
use llrm_mir::tti::{CostKind, LaneOp, ShuffleKind, Tti, Ty, VectorInstrContext, VectorTy};

/// `PoisonMaskElem`.
pub(crate) const POISON: i32 = -1;

/// Whether an operand is a constant (`isConstant`).
pub(crate) fn constant_of(operand: Operand) -> bool {
    matches!(operand, Operand::Constant(_))
}

/// `tryToFindDuplicates`, first half: the distinct lanes (constants are
/// always distinct) and, for each lane, its place among them. Empty reuse:
/// no lane repeats.
pub(crate) fn unique_lanes(lanes: &[Operand]) -> (Vec<Operand>, Vec<i32>) {
    let mut unique: Vec<Operand> = Vec::with_capacity(lanes.len());
    let mut reuse = Vec::with_capacity(lanes.len());
    for lane in lanes {
        let found = if constant_of(*lane) { None } else { unique.iter().position(|one| one == lane) };
        match found {
            Some(at) => reuse.push(at as i32),
            None => {
                reuse.push(unique.len() as i32);
                unique.push(*lane);
            }
        }
    }
    if unique.len() == lanes.len() {
        reuse.clear();
    }
    (unique, reuse)
}

/// `ShuffleVectorInst::isIdentityMask` of a mask the width of its source.
pub(crate) fn identity(mask: &[i32]) -> bool {
    mask.iter().enumerate().all(|(at, &lane)| lane == POISON || lane as usize == at)
}

/// `TryPackScalars` with `IsRootPoison`: the scalars with each distinct
/// non-constant one at its first lane (a value in every lane at lane 0), the
/// others poison (`None`), and the mask that reads them back.
pub(crate) fn pack_scalars(scalars: &[Option<Operand>]) -> (Vec<Option<Operand>>, Vec<i32>) {
    let mut packed: Vec<Option<Operand>> = scalars.to_vec();
    let mut mask = vec![POISON; scalars.len()];
    let present: Vec<Operand> = scalars.iter().flatten().copied().collect();
    let splat = !present.is_empty()
        && present.iter().all(|lane| *lane == present[0])
        && (scalars.len() > 2 || scalars.first() == scalars.last());
    let mut non_constants = 0;
    let mut single = 0;
    let mut first: Vec<(Operand, usize)> = Vec::new();
    for (at, lane) in scalars.iter().enumerate() {
        let Some(lane) = lane else { continue };
        if constant_of(*lane) {
            mask[at] = at as i32;
            continue;
        }
        non_constants += 1;
        single = at;
        packed[at] = None;
        if splat {
            packed[0] = Some(*lane);
            mask[at] = 0;
        } else {
            let home = match first.iter().find(|(seen, _)| seen == lane) {
                Some((_, home)) => *home,
                None => {
                    first.push((*lane, at));
                    at
                }
            };
            packed[home] = Some(*lane);
            mask[at] = home as i32;
        }
    }
    if non_constants == 1 {
        if splat {
            mask.fill(POISON);
            packed.swap(0, single);
        }
        mask[single] = single as i32;
    }
    (packed, mask)
}

/// The mask of a gather read through `reuse` (the entry's
/// `ReuseShuffleIndices`) over `packed`'s mask (`finalize`'s `ExtMask`).
pub(crate) fn composed(
    packed: &[i32],
    reuse: &[i32],
) -> Vec<i32> {
    if reuse.is_empty() {
        return packed.to_vec();
    }
    reuse.iter().map(|&lane| if lane == POISON { POISON } else { packed[lane as usize] }).collect()
}

/// The scalars a gather of `lanes` (distinct, `reuse` over them) packs into a
/// vector of `width`, and the mask that reads the vector built from them.
pub(crate) fn gather_plan(
    lanes: &[Operand],
    reuse: &[i32],
    width: usize,
) -> (Vec<Option<Operand>>, Vec<i32>) {
    let mut padded: Vec<Option<Operand>> = lanes.iter().map(|lane| Some(*lane)).collect();
    padded.resize(width, None);
    let (packed, mask) = pack_scalars(&padded);
    (packed, composed(&mask, reuse))
}

fn valid(cost: llrm_mir::tti::Cost) -> Option<i64> {
    cost.valid.then_some(cost.value)
}

/// The inserts of `scalars` into a poison vector (`getGatherCost`: constants
/// and poison lanes are free).
fn inserts_cost(
    tti: &dyn Tti,
    kind: CostKind,
    vector: VectorTy,
    scalars: impl Iterator<Item = (usize, bool)>,
) -> Option<i64> {
    let lanes: Vec<u32> = scalars.filter(|(_, insert)| *insert).map(|(index, _)| index as u32).collect();
    if lanes.is_empty() {
        return Some(0);
    }
    valid(tti.demanded_insert_overhead(vector, &lanes, kind)?)
}

/// The cost of a single-source shuffle by `mask` of `vector` (identity is
/// free, `ShuffleCostBuilder::createShuffleVector`).
pub(crate) fn shuffle_cost(
    tti: &dyn Tti,
    kind: CostKind,
    vector: VectorTy,
    mask: &[i32],
) -> Option<i64> {
    if mask.is_empty() || (mask.len() == vector.count as usize && identity(mask)) {
        return Some(0);
    }
    valid(tti.shuffle_cost(ShuffleKind::PermuteSingleSrc, vector, mask, kind)?)
}

/// `getBuildVectorCost` of the packed scalars, and the shuffle that reads
/// them: the cost of a gather entry.
pub(crate) fn gather_cost(
    tti: &dyn Tti,
    kind: CostKind,
    lane: Ty,
    lanes: &[Operand],
    reuse: &[i32],
) -> Option<i64> {
    let width = if reuse.is_empty() { lanes.len() } else { reuse.len() };
    let vector = VectorTy { lane, count: width as u32 };
    if lanes.iter().all(|lane| constant_of(*lane)) {
        return Some(0);
    }
    let (packed, mask) = gather_plan(lanes, reuse, width);
    let present: Vec<(usize, Operand)> =
        packed.iter().enumerate().filter_map(|(at, lane)| lane.map(|lane| (at, lane))).collect();
    let mut total = 0;
    let same = present.iter().all(|(_, lane)| *lane == present[0].1);
    if same && !constant_of(present[0].1) {
        // `isSplat`: one insert, and the broadcast where the value repeats.
        let (at, _) = present[0];
        // `ShuffleCostEstimator::gather`'s splat: one `getVectorInstrCost`
        // insert into poison, free at lane 0 (not the bulk
        // `getScalarizationOverhead` the other gathers pay).
        let insert =
            tti.vector_instr_cost(LaneOp::Insert, vector, Some(at as u32), at == 0, VectorInstrContext::None, kind)?;
        total += valid(insert)?;
        if present.len() > 1 {
            total += shuffle_cost(tti, kind, vector, &vec![0; width])?;
        }
    } else {
        total += inserts_cost(tti, kind, vector, present.iter().map(|(at, lane)| (*at, !constant_of(*lane))))?;
    }
    Some(total + shuffle_cost(tti, kind, vector, &mask)?)
}

/// What a bundle's lanes are, for `EstimatePackPlusShuffleVsInserts`.
pub(crate) struct Packing<'a> {
    pub kind: CostKind,
    pub lanes: &'a [Operand],
    pub unique: &'a [Operand],
    pub reuse: &'a [i32],
    /// The lanes are instructions of one opcode (`S`), and whether a load.
    pub state: Option<bool>,
    pub gather_only: bool,
    /// The user is the root (store) entry.
    pub root_operand: bool,
    pub require_scheduling: bool,
    /// An operand of the lane's operation is not a constant, for each column.
    pub columns_with_repeats: usize,
    pub lane: Ty,
    pub half_unique_ok: bool,
}

/// `getNumberOfParts`: the registers the vector takes.
fn parts(
    tti: &dyn Tti,
    vector: VectorTy,
) -> u32 {
    let bits = lane_bits(vector.lane) * vector.count;
    let registers = bits.div_ceil(tti.vector_register_bits().max(1));
    if registers >= vector.count || vector.count % registers != 0 { 1 } else { registers }
}

fn lane_bits(lane: Ty) -> u32 {
    match lane {
        Ty::Int(bits) => u32::from(bits),
        Ty::Float => 32,
        Ty::Double => 64,
        _ => 64,
    }
}

/// `getFullVectorNumberOfElements`.
fn full_vector_elements(
    tti: &dyn Tti,
    lane: Ty,
    count: u32,
) -> u32 {
    let registers = parts(tti, VectorTy { lane, count });
    if registers == 0 || registers >= count {
        count.next_power_of_two()
    } else {
        count.div_ceil(registers).next_power_of_two() * registers
    }
}

/// `EstimatePackPlusShuffleVsInserts`: whether to pack the lanes with a reuse
/// shuffle, and whether to keep the originals instead.
pub(crate) fn estimate_pack(
    tti: &dyn Tti,
    p: &Packing,
) -> Option<(bool, bool)> {
    let (n, u) = (p.lanes.len() as u32, p.unique.len() as u32);
    let non_constant = p.unique.iter().filter(|lane| !constant_of(**lane)).count();
    if non_constant == 1 && (u == 1 || p.unique.iter().all(|lane| !constant_of(*lane))) {
        return Some((false, false));
    }
    if p.gather_only && n > 4 && n.is_power_of_two() && !u.is_power_of_two() && non_constant * 2 < u as usize {
        return Some((false, false));
    }
    let is_load = p.state == Some(true);
    if p.root_operand && !p.gather_only {
        return Some((true, if is_load { identity(p.reuse) } else { !p.require_scheduling }));
    }
    let vector = VectorTy { lane: p.lane, count: n };
    let uniques = VectorTy { lane: p.lane, count: u };
    let (registers, unique_registers) = (parts(tti, vector), parts(tti, uniques));
    if !p.require_scheduling && n / u == 1 && (registers <= 1 || unique_registers >= registers) {
        return Some((true, true));
    }
    let mask = if u > n / 2 { &[][..] } else { p.reuse };
    let reuses = valid(tti.shuffle_cost(ShuffleKind::PermuteSingleSrc, vector, mask, p.kind)?)?;
    if p.state.is_some() && !p.gather_only {
        let one_duplicate = !is_load && u + 1 == n;
        let mostly_unique = u * 2 > n;
        let half_unique = u * 2 == n && p.half_unique_ok;
        let threshold = i64::from(registers) * if n > 4 { 1 } else { 2 };
        if one_duplicate || ((mostly_unique || half_unique) && reuses > threshold) {
            return Some((true, true));
        }
    }
    let skip = !p.gather_only && !p.require_scheduling;
    let (inserts, uniques_cost) = if skip {
        (0, reuses)
    } else {
        let demanded = (0..n as usize)
            .filter(|&at| p.reuse.get(at).is_some_and(|&lane| lane != POISON && !constant_of(p.unique[lane as usize])));
        let inserts = inserts_cost(tti, p.kind, vector, demanded.map(|at| (at, true)))?;
        let uniques = inserts_cost(
            tti,
            p.kind,
            uniques,
            p.unique.iter().enumerate().map(|(at, lane)| (at, !constant_of(*lane))),
        )?;
        (inserts, uniques + reuses)
    };
    if uniques_cost <= inserts {
        if p.state.is_some() && !is_load {
            let min_vf = (tti.min_vector_register_bits() / lane_bits(p.lane)).max(2);
            let width = |count: u32| full_vector_elements(tti, p.lane, count).max(min_vf);
            let extra_reshuffle = !p.gather_only && p.columns_with_repeats > 1;
            return Some((true, width(u) >= width(n) && !extra_reshuffle));
        }
        return Some((true, false));
    }
    if uniques_cost - inserts < 4 {
        return Some((p.state.is_some(), false));
    }
    let keep = !p.gather_only && !p.require_scheduling;
    Some((keep, keep))
}
