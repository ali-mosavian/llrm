//! The QuickBASIC-family runtime's `B$` routine contracts, a port of
//! `qbopt/abi/runtime.py`: what a call into BC's runtime can do to
//! the caller, routine by routine. The Python module docstring is the full
//! account.
//!
//! `TABLE` is the text of `runtime.toml`, included at build time, not its path.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use llrm_support::hash::IndexMap;
use llrm_support::pyrepr::{self, Repr};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Reg {
    Ax,
    Bx,
    Cx,
    Dx,
    Si,
    Di,
    Bp,
    Sp,
    Ds,
    Es,
    Flags,
}

impl Reg {
    /// Declaration order, which is `tuple(Reg)`.
    pub const ALL: [Reg; 11] = [
        Reg::Ax,
        Reg::Bx,
        Reg::Cx,
        Reg::Dx,
        Reg::Si,
        Reg::Di,
        Reg::Bp,
        Reg::Sp,
        Reg::Ds,
        Reg::Es,
        Reg::Flags,
    ];

    /// The member name.
    pub fn name(self) -> &'static str {
        match self {
            Reg::Ax => "AX",
            Reg::Bx => "BX",
            Reg::Cx => "CX",
            Reg::Dx => "DX",
            Reg::Si => "SI",
            Reg::Di => "DI",
            Reg::Bp => "BP",
            Reg::Sp => "SP",
            Reg::Ds => "DS",
            Reg::Es => "ES",
            Reg::Flags => "FLAGS",
        }
    }

    /// The `StrEnum` value.
    pub fn value(self) -> &'static str {
        match self {
            Reg::Ax => "ax",
            Reg::Bx => "bx",
            Reg::Cx => "cx",
            Reg::Dx => "dx",
            Reg::Si => "si",
            Reg::Di => "di",
            Reg::Bp => "bp",
            Reg::Sp => "sp",
            Reg::Ds => "ds",
            Reg::Es => "es",
            Reg::Flags => "flags",
        }
    }

    /// `Reg(value)`.
    pub fn from_value(value: &str) -> Result<Reg, String> {
        Reg::ALL
            .into_iter()
            .find(|one| one.value() == value)
            .ok_or_else(|| format!("{} is not a valid Reg", pyrepr::string(value)))
    }
}

/// A `StrEnum` orders as its string value.
impl Ord for Reg {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value().cmp(other.value())
    }
}

impl PartialOrd for Reg {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Repr for Reg {
    fn repr(&self) -> String {
        pyrepr::str_enum("Reg", self.name(), self.value())
    }
}

pub static EVERY: LazyLock<BTreeSet<Reg>> = LazyLock::new(|| Reg::ALL.into_iter().collect());

// cmacros' own PL/M convention: cBegin saves the routine's declared list and
// cEnd restores it, so a save list is a preservation guarantee whatever the
// body and its callees do (inc/cmacros.inc, mPush/mPop around the frame).
pub static PER_CONVENTION: LazyLock<BTreeSet<Reg>> =
    LazyLock::new(|| BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Flags]));

// Ordered by how much it concedes, so "can this write something the caller can
// name" is a comparison rather than a second table.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Memory {
    None = 0,
    Arguments = 1, // only the values pushed for this call, popped before it returns
    Strings = 2,   // + any string descriptor or string data, anywhere
    // Its own data, and anything the caller handed it a pointer to. GCC's
    // ipa-modref splits a callee's effects the same way: writes at a fixed
    // address, and writes through parameter N. `ANY` says only "writes
    // memory", which is true of every routine here and tells a caller
    // nothing -- a global cannot be kept in a register across a PRINT, and
    // every program in the suite prints.
    //
    // Measured, not assumed. tools/runtime_writes.py reads a linked image:
    // over B_NBODY, B_ARRIDX and B_MATRIX, the runtime makes 287 writes to
    // a fixed address in DGROUP and not one of them names a cell in
    // BC_DATA, the segment BC puts a program's variables in. Nor does any
    // data-segment fixup in the corpus hand the runtime a pointer into it:
    // 151 are BC_CN -> BC_CN and one per program is BC_SA -> its code.
    Own = 3,
    Any = 4,
}

impl Memory {
    pub const ALL: [Memory; 5] = [
        Memory::None,
        Memory::Arguments,
        Memory::Strings,
        Memory::Own,
        Memory::Any,
    ];

    /// The member name.
    pub fn name(self) -> &'static str {
        match self {
            Memory::None => "NONE",
            Memory::Arguments => "ARGUMENTS",
            Memory::Strings => "STRINGS",
            Memory::Own => "OWN",
            Memory::Any => "ANY",
        }
    }

    /// The `IntEnum` value.
    pub fn value(self) -> i64 {
        self as i64
    }

    /// `Memory[name]`.
    pub fn from_name(name: &str) -> Result<Memory, String> {
        Memory::ALL
            .into_iter()
            .find(|one| one.name() == name)
            .ok_or_else(|| pyrepr::string(name))
    }
}

impl Repr for Memory {
    fn repr(&self) -> String {
        format!("<Memory.{}: {}>", self.name(), self.value())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Control {
    Returns,
    InlineTable, // comes back somewhere data after the call selects
    Never,
    Unknown,
}

impl Control {
    pub const ALL: [Control; 4] = [
        Control::Returns,
        Control::InlineTable,
        Control::Never,
        Control::Unknown,
    ];

    /// The member name.
    pub fn name(self) -> &'static str {
        match self {
            Control::Returns => "RETURNS",
            Control::InlineTable => "INLINE_TABLE",
            Control::Never => "NEVER",
            Control::Unknown => "UNKNOWN",
        }
    }

    /// The `StrEnum` value.
    pub fn value(self) -> &'static str {
        match self {
            Control::Returns => "returns",
            Control::InlineTable => "inline-table",
            Control::Never => "never",
            Control::Unknown => "unknown",
        }
    }

    /// `Control(value)`.
    pub fn from_value(value: &str) -> Result<Control, String> {
        Control::ALL
            .into_iter()
            .find(|one| one.value() == value)
            .ok_or_else(|| format!("{} is not a valid Control", pyrepr::string(value)))
    }
}

impl Repr for Control {
    fn repr(&self) -> String {
        pyrepr::str_enum("Control", self.name(), self.value())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Contract {
    pub name: String,
    pub cleanup: Option<i64>, // bytes the callee pops off the caller's stack
    pub control: Control,
    pub enters_user_code: bool,
    pub raises_error: bool,
    pub error_handling: bool,
    pub writes: Memory,
    pub reads: Memory,
    pub clobbers: BTreeSet<Reg>,
    pub established: bool,
    pub evidence: String,
    pub documented: Option<BTreeSet<Reg>>,
    // Registers the caller has to have set before the call. None is not
    // "none" -- it is "not established", and reads as every register, the
    // same way an unestablished contract clobbers every one. The asymmetry
    // is the point: over-stating a read only keeps a value alive, while
    // under-stating one deletes the instruction that produced it. B$FILD
    // takes its long in dx:ax, nothing recorded that, and removing the
    // moves that set it up printed FADD= 918528 for 1049600.
    pub inputs: Option<BTreeSet<Reg>>,
    // Values the call's ordinary, fall-through continuation observes. Most
    // routines use exactly ``inputs``. A control-transfer boundary may also
    // inspect register state on a hidden error/resume path, however, and
    // keeping that conservative all-path set in ``inputs`` must not turn
    // caller-saved scratch values into source-program operands on the direct
    // edge. None means the ordinary continuation has the same inputs.
    pub direct_inputs: Option<BTreeSet<Reg>>,
    // `clobbers` was proven over every path the call can return along,
    // whatever code those paths reach.
    pub clobbers_reached: bool,
    // Bytes the caller pops once the call returns: C's convention, where
    // `cleanup` is 0 and the arguments are still the caller's to release.
    pub caller_cleanup: i64,
    // 386 code: a register kept under the 8086 convention keeps only its
    // 16-bit half, and FS and GS are not kept at all.
    pub i386: bool,
    // A control-transfer routine's own footprint before it invokes or resumes
    // user code. `reads`/`writes` remain the transitive effect, which is ANY;
    // handler summarization stops at the transfer and consumes these instead.
    pub direct_writes: Option<Memory>,
    pub direct_reads: Option<Memory>,
    // Its result is the flags, as `cmp result, 0` would leave them: no register holds it.
    pub flags_result: bool,
}

impl Repr for Contract {
    /// Set elements print in `Reg` order, which is sorted repr order; Python
    /// prints them in hash order, which `tools/port_diff.py` sorts away.
    fn repr(&self) -> String {
        let regs = |set: &BTreeSet<Reg>| pyrepr::frozenset(&set.iter().collect::<Vec<_>>());
        pyrepr::dataclass(
            "Contract",
            &[
                ("name", self.name.repr()),
                ("cleanup", self.cleanup.repr()),
                ("control", self.control.repr()),
                ("enters_user_code", self.enters_user_code.repr()),
                ("raises_error", self.raises_error.repr()),
                ("error_handling", self.error_handling.repr()),
                ("writes", self.writes.repr()),
                ("reads", self.reads.repr()),
                ("clobbers", regs(&self.clobbers)),
                ("established", self.established.repr()),
                ("evidence", self.evidence.repr()),
                (
                    "documented",
                    self.documented.as_ref().map_or("None".to_owned(), regs),
                ),
                (
                    "inputs",
                    self.inputs.as_ref().map_or("None".to_owned(), regs),
                ),
                (
                    "direct_inputs",
                    self.direct_inputs.as_ref().map_or("None".to_owned(), regs),
                ),
                ("clobbers_reached", self.clobbers_reached.repr()),
                ("caller_cleanup", self.caller_cleanup.repr()),
                ("i386", self.i386.repr()),
                ("direct_writes", self.direct_writes.repr()),
                ("direct_reads", self.direct_reads.repr()),
            ],
        )
    }
}

/// Whether `name`'s result is the flags: the table's `flags_result`.
pub fn flags_result(name: &str) -> bool {
    facts(name).is_some_and(|routine| routine.flags_result)
}

pub fn worst(name: &str) -> Contract {
    worst_with(name, flags_result(name))
}

fn worst_with(name: &str, flags_result: bool) -> Contract {
    Contract {
        name: name.to_owned(),
        cleanup: None,
        control: Control::Unknown,
        enters_user_code: true,
        raises_error: true,
        error_handling: false,
        writes: Memory::Any,
        reads: Memory::Any,
        clobbers: EVERY.clone(),
        established: false,
        evidence: "not established from the QuickBASIC 4.5 runtime source".to_owned(),
        documented: None,
        inputs: None,
        direct_inputs: None,
        clobbers_reached: false,
        caller_cleanup: 0,
        i386: false,
        direct_writes: None,
        flags_result,
        direct_reads: None,
    }
}

/// Inline machine code: it returns, pops nothing, may touch any memory and
/// any register, and answers in ax or dx:ax.
pub fn inline_code(name: &str) -> Contract {
    Contract {
        cleanup: Some(0),
        control: Control::Returns,
        enters_user_code: false,
        raises_error: false,
        established: true,
        evidence: "inline assembly: every register assumed clobbered, the result left in AX or DX:AX".to_owned(),
        inputs: Some(BTreeSet::new()),
        flags_result: false,
        ..worst(name)
    }
}

pub fn preserves(routine: &Contract) -> BTreeSet<Reg> {
    EVERY.difference(&routine.clobbers).copied().collect()
}

/// Registers a call may return changed. The raise and the lowering must
/// both ask this: when the raise gave B$RND0 a fresh SI that the lowering
/// thought the call kept, the value BC relied on was never put in SI.
///
/// A routine reaching user code returns from code its clobber set does not
/// describe, unless that set was proven over everything it reaches.
pub fn disturbs(routine: &Contract) -> BTreeSet<Reg> {
    if (routine.enters_user_code || routine.error_handling) && !routine.clobbers_reached {
        return EVERY.clone();
    }
    routine.clobbers.clone()
}

// The order a routine's declared inputs are listed in. `Contract.inputs` is
// a set -- which register, not which position -- and the raise and the
// lowering both have to agree on a slot per input or they would pair an
// argument with somebody else's register. Any total order does; this is
// `Reg`'s own declaration order, so a register added there cannot be left
// out of a list written somewhere else.
pub const SLOTS: [Reg; 11] = Reg::ALL;

// What a routine's contract is under one toolchain, where the routines
// differ: the table's family subtables. Keyed by the compiler's own name for
// itself, which module.py reads off COMENT 0x00 -- the family is a fact about
// the object, and it reaches this map and nothing else.
pub static VARIANTS: LazyLock<IndexMap<(&'static str, &'static str), Contract>> = LazyLock::new(|| {
    RUNTIME
        .iter()
        .flat_map(|(name, routine)| routine.families.iter().map(move |(family, one)| ((name.as_str(), *family), one.clone())))
        .collect()
});

/// A call into the program's own code, by name.
///
/// BC compiles a SUB as a PUBDEF of the same object and calls it through
/// an EXTDEF fixup, so `Module.calls` names it like a runtime routine and
/// only the PUBDEF set tells the two apart. A BASIC argument arrives as a
/// far pointer on the stack, so nothing arrives in a register -- and that
/// is the only thing established here. Everything else stays at the worst
/// case: what such a procedure clobbers is as unknown as any other.
pub fn own(name: &str) -> Contract {
    Contract {
        inputs: Some(BTreeSet::new()),
        evidence: "a PUBDEF of this same module; BC pushes a far pointer per BASIC argument"
            .to_owned(),
        ..worst(name)
    }
}

// Which routines write a runtime cell a program names by EXTDEF, where the
// runtime source settles it: the table's `writes_cells`. A call to anything
// else leaves the cell alone, unless it runs the program's own code -- which
// `enters_user_code` and an error handler say, and the caller asks.
pub static WRITERS: LazyLock<IndexMap<(&'static str, &'static str), BTreeSet<&'static str>>> = LazyLock::new(|| {
    let mut writers: IndexMap<(&'static str, &'static str), BTreeSet<&'static str>> = IndexMap::default();
    for (name, routine) in RUNTIME.iter() {
        for (family, cells) in &routine.writes_cells {
            for cell in cells {
                writers.entry((cell.as_str(), *family)).or_default().insert(name.as_str());
            }
        }
    }
    writers
});

/// Whether `name` is a runtime cell only a reference naming it reaches: one
/// `WRITERS` lists, whose address no runtime routine hands out.
pub fn named_only(name: &str, family: &str) -> bool {
    WRITERS.keys().any(|&(cell, of)| cell == name && of == family)
}

/// Each of `family`'s named-only cells, and the routines that write it.
pub fn writers(family: &str) -> impl Iterator<Item = (&'static str, impl Iterator<Item = &'static str>)> + '_ {
    WRITERS.iter().filter(move |((_, of), _)| *of == family).map(|((cell, _), routines)| (*cell, routines.iter().copied()))
}

/// The named-only cells runtime routine `routine` writes: those whose
/// `WRITERS` list it. None when it may run the program's own code, which
/// writes anything.
pub fn named_writes(routine: &str, family: &str) -> Option<Vec<&'static str>> {
    if ENTERS_USER_CODE.contains(routine) {
        return None;
    }
    Some(
        WRITERS
            .iter()
            .filter(|((_, of), writers)| *of == family && writers.contains(routine))
            .map(|((cell, _), _)| *cell)
            .collect(),
    )
}

/// One contract per call site, chosen once for the whole module.
///
/// A side map rather than a field on the body: a contract is a fact about
/// the machine, and the raise and the lowering both need the same answer
/// for the same call. Passing it keeps them from looking one up
/// separately and disagreeing -- which is what a per-family contract
/// makes possible, since B$ENRA reads bx under one runtime and not
/// another.
///
/// `family` is `module.Family`'s value, as a string so this stays below
/// module.py. It selects a variant where one is established and changes
/// nothing otherwise -- a caller with no family in hand gets the
/// conservative contract, which is the honest answer for an object whose
/// toolchain nobody read.
///
/// Python's defaults are `family=""` and `defined=frozenset()`.
pub fn per_call(
    calls: &IndexMap<i64, String>,
    family: &str,
    defined: &BTreeSet<String>,
) -> IndexMap<i64, Contract> {
    let variants: &IndexMap<(&str, &str), Contract> = &VARIANTS;
    calls
        .iter()
        .map(|(&at, name)| {
            (
                at,
                if defined.contains(name) {
                    own(name)
                } else {
                    variants
                        .get(&(name.as_str(), family))
                        .cloned()
                        .unwrap_or_else(|| contract(Some(name)))
                },
            )
        })
        .collect()
}

/// Whether this routine's inputs are established at all.
///
/// `inputs` is None where nothing is known and an empty set where the
/// routine is known to read no register. The two are not the same fact
/// and reading them as one is how a call to a routine whose code is not
/// in the tree came out with no requirements at all -- which says it
/// reads nothing, the one thing known to be false about it.
pub fn established_inputs(routine: &Contract) -> bool {
    routine.inputs.is_some()
}

/// Which registers this routine reads its arguments in, in slot order.
///
/// Empty where nothing is established: a routine whose code is not in the
/// tree declares no inputs, and the conservative read set the raise builds
/// for it is a liveness dependency rather than an argument list.
pub fn slots(routine: &Contract) -> Vec<Reg> {
    let Some(inputs) = routine.inputs.as_ref().filter(|inputs| !inputs.is_empty()) else {
        return Vec::new();
    };
    SLOTS
        .into_iter()
        .filter(|one| inputs.contains(one))
        .collect()
}

/// Register values observed by the syntactic continuation of a call.
///
/// ``inputs`` remains the conservative all-path contract used by analyses
/// that must account for hidden runtime transfers. MIR and lowering describe
/// the explicit CFG edge and therefore use this narrower set where one has
/// been established.
pub fn direct_slots(routine: &Contract) -> Vec<Reg> {
    let chosen = if routine.direct_inputs.is_none() {
        &routine.inputs
    } else {
        &routine.direct_inputs
    };
    let Some(chosen) = chosen.as_ref().filter(|chosen| !chosen.is_empty()) else {
        return Vec::new();
    };
    SLOTS
        .into_iter()
        .filter(|one| chosen.contains(one))
        .collect()
}

pub fn writes_caller_memory(routine: &Contract) -> bool {
    routine.writes >= Memory::Strings
}

/// Whether a pass keeping caller values in registers must refuse a body
/// holding this call outright, rather than reasoning about what it touches.
pub fn barrier(routine: &Contract) -> bool {
    routine.enters_user_code || routine.error_handling || !routine.established
}

/// Whether the module has an ON ERROR handler, so a raised error can reach user code.
pub fn handles_errors<'a>(routines: impl IntoIterator<Item = &'a Contract>) -> bool {
    routines.into_iter().any(|routine| routine.error_handling)
}

/// Stack argument bytes whose bits are values rather than caller pointers:
/// an established cleanup of a routine the table marks `by_value`.
pub fn numeric_stack_arguments(name: &str) -> Option<i64> {
    if !facts(name).is_some_and(|routine| routine.by_value) {
        return None;
    }
    let routine = contract(Some(name));
    if routine.established { routine.cleanup } else { None }
}

// Where the rows live. Data, not code: every field is a claim about the
// runtime, and a claim wants an audit trail more than it wants a Python
// literal. `tools/runtime_writes.py` regenerates the measured columns from
// a linked image, so re-measuring is a diff against this file rather than a
// rewrite of one.
pub const TABLE: &str = include_str!("runtime.toml");

/// The runtime families a table may name, by the compiler's own name for itself.
pub const FAMILIES: [&str; 3] = ["qb45", "pds71", "vbdos"];

/// Everything the table says about one routine.
#[derive(Clone, Debug, Default)]
pub struct Routine {
    /// Its row, where the table states contract fields for it.
    pub contract: Option<Contract>,
    /// Each family's own row.
    pub families: IndexMap<&'static str, Contract>,
    pub flags_result: bool,
    pub calls_program: bool,
    pub never_returns: bool,
    pub error_funnel: Option<BTreeSet<Reg>>,
    pub by_value: bool,
    pub writes_cells: IndexMap<&'static str, Vec<String>>,
}

const CONTRACT_FIELDS: [&str; 19] = [
    "from", "cleanup", "control", "enters_user_code", "raises_error", "error_handling", "writes", "reads", "clobbers",
    "established", "evidence", "documented", "inputs", "direct_inputs", "clobbers_reached", "caller_cleanup", "i386",
    "direct_writes", "direct_reads",
];

fn family(key: &str) -> Option<&'static str> {
    FAMILIES.into_iter().find(|one| *one == key)
}

fn boolean(value: &toml::Value, at: &str) -> Result<bool, String> {
    value.as_bool().ok_or_else(|| format!("{at} is not a bool"))
}

fn integer(value: &toml::Value, at: &str) -> Result<i64, String> {
    value.as_integer().ok_or_else(|| format!("{at} is not an int"))
}

fn string<'a>(value: &'a toml::Value, at: &str) -> Result<&'a str, String> {
    value.as_str().ok_or_else(|| format!("{at} is not a str"))
}

fn strings(value: &toml::Value, at: &str) -> Result<Vec<String>, String> {
    value
        .as_array()
        .ok_or_else(|| format!("{at} is not a list"))?
        .iter()
        .map(|one| string(one, at).map(str::to_owned))
        .collect()
}

fn regs(value: &toml::Value, at: &str) -> Result<BTreeSet<Reg>, String> {
    strings(value, at)?.iter().map(|one| Reg::from_value(one)).collect()
}

/// `name`'s entry, or else the entry of a `PREFIX*` table it falls under.
fn lookup<'a>(routines: &'a IndexMap<String, Routine>, name: &str) -> Option<&'a Routine> {
    routines.get(name).or_else(|| {
        routines
            .iter()
            .find(|(key, _)| key.strip_suffix('*').is_some_and(|stem| name.starts_with(stem)))
            .map(|(_, routine)| routine)
    })
}

/// One row: `worst()` with the fields it states, or with `from`, the row it
/// names with them. A row named by `from` is `NAME` or `NAME.family`.
fn row(name: &str, table: &toml::Table, base: bool, tables: &toml::Table, routines: &IndexMap<String, Routine>, depth: usize) -> Result<Contract, String> {
    let at = |key: &str| format!("{name}.{key}");
    let mut one = match table.get("from") {
        None => worst_with(name, lookup(routines, name).is_some_and(|routine| routine.flags_result)),
        Some(from) => {
            let from = string(from, &at("from"))?;
            if depth > FAMILIES.len() + 8 {
                return Err(format!("{} does not end", at("from")));
            }
            let (source, of) = match from.rsplit_once('.') {
                Some((source, of)) if family(of).is_some() => (source, Some(of)),
                _ => (from, None),
            };
            let named = tables.get(source).and_then(toml::Value::as_table);
            let named = match of {
                Some(of) => named.and_then(|table| table.get(of)).and_then(toml::Value::as_table),
                None => named,
            };
            let named = named.ok_or_else(|| format!("{} names no row {from}", at("from")))?;
            Contract { name: name.to_owned(), ..row(source, named, of.is_none(), tables, routines, depth + 1)? }
        }
    };
    for (key, value) in table {
        let at = at(key);
        match key.as_str() {
            "from" => {}
            "cleanup" => one.cleanup = Some(integer(value, &at)?).filter(|bytes| *bytes >= 0),
            "control" => one.control = Control::from_value(string(value, &at)?)?,
            "enters_user_code" => one.enters_user_code = boolean(value, &at)?,
            "raises_error" => one.raises_error = boolean(value, &at)?,
            "error_handling" => one.error_handling = boolean(value, &at)?,
            "writes" => one.writes = Memory::from_name(string(value, &at)?)?,
            "reads" => one.reads = Memory::from_name(string(value, &at)?)?,
            "clobbers" => one.clobbers = regs(value, &at)?,
            "established" => one.established = boolean(value, &at)?,
            "evidence" => {
                let own = string(value, &at)?.trim();
                one.evidence = if table.contains_key("from") { format!("{own} {}", one.evidence) } else { own.to_owned() };
            }
            "documented" => one.documented = Some(regs(value, &at)?),
            "inputs" => one.inputs = Some(regs(value, &at)?),
            "direct_inputs" => one.direct_inputs = Some(regs(value, &at)?),
            "clobbers_reached" => one.clobbers_reached = boolean(value, &at)?,
            "caller_cleanup" => one.caller_cleanup = integer(value, &at)?,
            "i386" => one.i386 = boolean(value, &at)?,
            "direct_writes" => one.direct_writes = Some(Memory::from_name(string(value, &at)?)?),
            "direct_reads" => one.direct_reads = Some(Memory::from_name(string(value, &at)?)?),
            _ if base => {}
            _ => return Err(format!("{at} is not a contract field")),
        }
    }
    Ok(one)
}

/// Every routine the table describes, by name, in table order.
///
/// A table's contract fields make the routine's row, and its family
/// subtables that family's own; `row` says how each is filled. Anything else
/// in it is a fact about the routine, and a key that is neither is refused.
pub fn load(text: &str) -> Result<IndexMap<String, Routine>, String> {
    let tables: toml::Table = text.parse().map_err(|error: toml::de::Error| error.to_string())?;
    let mut routines: IndexMap<String, Routine> = IndexMap::default();
    for (name, table) in &tables {
        let table = table.as_table().ok_or_else(|| format!("{} is not a table", pyrepr::string(name)))?;
        let mut routine = Routine::default();
        for (key, value) in table {
            let at = format!("{name}.{key}");
            match key.as_str() {
                "flags_result" => routine.flags_result = boolean(value, &at)?,
                "calls_program" => routine.calls_program = boolean(value, &at)?,
                "never_returns" => routine.never_returns = boolean(value, &at)?,
                "error_funnel" => routine.error_funnel = Some(regs(value, &at)?),
                "by_value" => routine.by_value = boolean(value, &at)?,
                "writes_cells" => {
                    for (of, cells) in value.as_table().ok_or_else(|| format!("{at} is not a table"))? {
                        let of = family(of).ok_or_else(|| format!("{at}.{of} is not a family"))?;
                        routine.writes_cells.insert(of, strings(cells, &at)?);
                    }
                }
                key if family(key).is_some() || CONTRACT_FIELDS.contains(&key) => {}
                _ => return Err(format!("{at} is not a field")),
            }
        }
        routines.insert(name.clone(), routine);
    }
    for (name, table) in &tables {
        let table = table.as_table().expect("checked above");
        let contract = if table.keys().any(|key| CONTRACT_FIELDS.contains(&key.as_str())) {
            Some(row(name, table, true, &tables, &routines, 0)?)
        } else {
            None
        };
        let mut families = IndexMap::default();
        for of in FAMILIES {
            if let Some(one) = table.get(of) {
                let one = one.as_table().ok_or_else(|| format!("{name}.{of} is not a table"))?;
                families.insert(of, row(name, one, false, &tables, &routines, 0)?);
            }
        }
        let routine = &mut routines[name.as_str()];
        routine.contract = contract;
        routine.families = families;
    }
    Ok(routines)
}

pub static RUNTIME: LazyLock<IndexMap<String, Routine>> = LazyLock::new(|| load(TABLE).expect("runtime.toml loads"));

/// What the table says about `name`, directly or through a `PREFIX*` table.
fn facts(name: &str) -> Option<&'static Routine> {
    lookup(&RUNTIME, name)
}

/// One entry per runtime name the table gives a row.
pub static CONTRACTS: LazyLock<IndexMap<String, Contract>> = LazyLock::new(|| {
    RUNTIME.iter().filter_map(|(name, routine)| Some((name.clone(), routine.contract.clone()?))).collect()
});

/// The routines that read a table laid inline after their own call site: the
/// rows whose control says so.
pub static INLINE_TABLE: LazyLock<BTreeSet<&'static str>> = LazyLock::new(|| {
    CONTRACTS.iter().filter(|(_, routine)| routine.control == Control::InlineTable).map(|(name, _)| name.as_str()).collect()
});

/// Routines that hand control back to the program: the table's `calls_program`.
pub static ENTERS_USER_CODE: LazyLock<BTreeSet<&'static str>> =
    LazyLock::new(|| RUNTIME.iter().filter(|(_, routine)| routine.calls_program).map(|(name, _)| name.as_str()).collect());

/// Whether a runtime entry never comes back to its caller: an established
/// NEVER row, or a routine the table says `never_returns`.
pub fn never_returns(name: &str) -> bool {
    let folded = name.to_uppercase();
    let known = CONTRACTS.get(&folded);
    known.is_some_and(|known| known.established && known.control == Control::Never)
        || facts(&folded).is_some_and(|routine| routine.never_returns)
}

/// The registers each error funnel reads: the table's `error_funnel`.
pub static ERROR_FUNNEL_INPUTS: LazyLock<IndexMap<&'static str, BTreeSet<Reg>>> = LazyLock::new(|| {
    RUNTIME.iter().filter_map(|(name, routine)| Some((name.as_str(), routine.error_funnel.clone()?))).collect()
});

/// What this call can do. A name with no entry -- a user SUB or FUNCTION, a
/// runtime routine nothing in the corpus has called yet, an indirect call with
/// no name at all -- gets the worst case rather than an absence.
pub fn contract(name: Option<&str>) -> Contract {
    let Some(name) = name else {
        return worst("");
    };
    CONTRACTS.get(name).cloned().unwrap_or_else(|| worst(name))
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_runtime.py`.
    //!
    //! Skipped, needing `wholeseg`:
    //! `test_registered_handler_does_not_land_in_a_phi_edge`,
    //! `test_handler_discovery_accepts_lowered_push_only_with_matching_relocations`,
    //! `test_addrm_event_adapter_emits_without_fallback`.
    //! `test_event_stub_requires_exact_relocation` monkeypatches `omf.fixups`
    //! to give the field a displacement of 1; here the FIXUPP record itself
    //! says so instead.

    use super::*;

    /// `{0: name}`.
    fn at_zero(name: &str) -> IndexMap<i64, String> {
        IndexMap::from_iter([(0, name.to_owned())])
    }

    /// `runtime.per_call({0: name}, family)[0]`.
    fn one(name: &str, family: &str) -> Contract {
        per_call(&at_zero(name), family, &BTreeSet::new())[&0].clone()
    }

    fn gp() -> BTreeSet<Reg> {
        BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])
    }

    fn names() -> Vec<String> {
        let mut names: Vec<String> = CONTRACTS.keys().cloned().collect();
        names.sort();
        names
    }

    // qbopt.legacy.calls COMPARE, MULTIPLY, DIVIDE, REMAINDER.
    const ABSORBED: [&str; 4] = ["B$CPI4", "B$MUI4", "B$DVI4", "B$RMI4"];

    const X87: [&str; 6] = ["B$FCMP", "B$FILD", "B$FIL2", "B$FIST", "B$FIS2", "B$FUST"];

    #[test]
    fn enums_repr_like_python() {
        assert_eq!(Reg::Ax.repr(), "<Reg.AX: 'ax'>");
        assert_eq!(Memory::Own.repr(), "<Memory.OWN: 3>");
        assert_eq!(
            Control::InlineTable.repr(),
            "<Control.INLINE_TABLE: 'inline-table'>"
        );
    }

    /// Python's `_contracts()`, one `name repr` line each, frozensets sorted.
    #[test]
    fn the_contract_table_matches_python() {
        let mut got: Vec<String> = CONTRACTS
            .iter()
            .map(|(name, one)| format!("{name} {}", one.repr()))
            .collect();
        got.sort();
        let want: Vec<&str> = include_str!("../../../../tests/fixtures/abi/runtime-contracts.txt")
            .lines()
            .collect();
        assert_eq!(got, want);
        let order: Vec<&str> = include_str!("../../../../tests/fixtures/abi/runtime-contract-order.txt")
            .lines()
            .collect();
        assert_eq!(CONTRACTS.keys().collect::<Vec<_>>(), order);
    }

    #[test]
    fn the_variants_match_python() {
        let mut got: Vec<String> = VARIANTS
            .iter()
            .map(|((name, family), one)| format!("{name} {family} {}", one.repr()))
            .collect();
        got.sort();
        let want: Vec<&str> = include_str!("../../../../tests/fixtures/abi/runtime-variants.txt")
            .lines()
            .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn test_unknown_name_is_the_worst_case() {
        let unknown = contract(Some("B$NOSUCHTHING"));
        assert_eq!(unknown, worst("B$NOSUCHTHING"));
        assert!(!unknown.established);
        assert!(barrier(&unknown));
    }

    #[test]
    fn test_an_unnamed_call_is_the_worst_case() {
        assert_eq!(contract(None).clobbers, *EVERY);
    }

    #[test]
    fn test_evk1_alias_keeps_event_effects() {
        for family in ["pds71", "vbdos"] {
            let alias = one("B$EVK1", family);
            let original = contract(Some("B$EVCK"));
            assert_eq!(
                Contract {
                    name: original.name.clone(),
                    evidence: original.evidence.clone(),
                    ..alias.clone()
                },
                original
            );
            assert!(barrier(&alias));
            assert!(alias.enters_user_code);
            assert_eq!(alias.writes, Memory::Any);
        }
    }

    #[test]
    fn test_evk1_alias_requires_an_established_family() {
        for family in ["", "qb45", "unknown"] {
            assert_eq!(one("B$EVK1", family), worst("B$EVK1"));
        }
    }

    #[test]
    fn test_evk1_user_definition_overrides_runtime_alias() {
        let defined = BTreeSet::from(["B$EVK1".to_owned()]);
        assert_eq!(
            per_call(&at_zero("B$EVK1"), "pds71", &defined)[&0],
            own("B$EVK1")
        );
    }

    #[test]
    fn test_timer_interfaces_bound_inputs_without_claiming_preservation() {
        for family in ["pds71", "vbdos"] {
            for name in ["B$ONTA", "B$ETT0", "B$ETT1", "B$ETT2"] {
                let routine = one(name, family);
                assert_eq!(routine.inputs, Some(gp()));
                assert_eq!(
                    Contract {
                        inputs: None,
                        evidence: worst(name).evidence,
                        ..routine
                    },
                    worst(name)
                );
                assert_eq!(one(name, ""), worst(name));
            }
        }
    }

    #[test]
    fn test_runtime_return_interface_keeps_unknown_effects() {
        for family in ["pds71", "vbdos"] {
            let routine = one("B$RETA", family);
            assert_eq!(routine.inputs, Some(gp()));
            assert_eq!(
                Contract {
                    inputs: None,
                    evidence: worst("B$RETA").evidence,
                    ..routine
                },
                worst("B$RETA")
            );
        }
    }

    /// B$FCMD's source is a stub, and it was the worst case in every family:
    /// COMMAND$ refused nbody. Each shipped library's code returns, pops
    /// nothing, reads no register and answers in AX; VBDOS's B$ERAS, B$ASSN,
    /// B$LDFS and B$PER8 are their QB 4.5 code, and were the worst case there
    /// too (arrudt, nestud, byref2 refused).
    #[test]
    fn test_read_runtime_routines_are_established_in_every_family() {
        for family in ["qb45", "pds71", "vbdos"] {
            for (name, cleanup) in [("B$FCMD", 0), ("B$ERAS", 2), ("B$ASSN", 12), ("B$LDFS", 6), ("B$PER8", 8)] {
                let routine = one(name, family);
                assert!(routine.established, "{name} {family}");
                assert_eq!((routine.control, routine.enters_user_code, routine.cleanup), (Control::Returns, false, Some(cleanup)), "{name} {family}");
                assert_eq!(direct_slots(&routine), Vec::new(), "{name} {family}");
            }
        }
    }

    #[test]
    fn test_the_worst_case_concedes_nothing() {
        let blank = worst("");
        assert_eq!(blank.writes, Memory::Any);
        assert_eq!(blank.reads, Memory::Any);
        assert_eq!(blank.clobbers, *EVERY);
        assert_eq!(blank.control, Control::Unknown);
        assert_eq!(blank.cleanup, None);
        assert!(blank.enters_user_code);
        assert!(writes_caller_memory(&blank));
        assert_eq!(preserves(&blank), BTreeSet::new());
    }

    #[test]
    fn test_no_contract_claims_more_than_the_worst_case() {
        for name in names() {
            let routine = &CONTRACTS[&name];
            let blank = worst(&routine.name);
            assert!(routine.writes <= blank.writes);
            assert!(routine.reads <= blank.reads);
            assert!(routine.clobbers.is_subset(&blank.clobbers));
            assert!(routine.enters_user_code <= blank.enters_user_code);
        }
    }

    #[test]
    fn test_an_unestablished_routine_is_left_at_the_worst_case() {
        for name in names()
            .into_iter()
            .filter(|name| !CONTRACTS[name].established)
        {
            let routine = &CONTRACTS[&name];
            assert_eq!(
                *routine,
                Contract {
                    evidence: routine.evidence.clone(),
                    ..worst(&name)
                }
            );
        }
    }

    #[test]
    fn test_every_claim_below_the_worst_case_cites_something() {
        for name in names()
            .into_iter()
            .filter(|name| CONTRACTS[name] != worst(name))
        {
            let evidence = &CONTRACTS[&name].evidence;
            assert!(
                [".asm", ".inc", ".py", "AGENTS.md"]
                    .iter()
                    .any(|cited| evidence.contains(cited))
            );
        }
    }

    #[test]
    fn test_preserved_and_clobbered_partition_the_registers() {
        for name in names() {
            let routine = &CONTRACTS[&name];
            assert_eq!(
                preserves(routine)
                    .union(&routine.clobbers)
                    .copied()
                    .collect::<BTreeSet<_>>(),
                *EVERY
            );
            assert!(preserves(routine).is_disjoint(&routine.clobbers));
        }
    }

    #[test]
    fn test_a_documented_clobber_set_is_recorded_only_where_it_differs() {
        for name in names()
            .into_iter()
            .filter(|name| CONTRACTS[name].documented.is_some())
        {
            let routine = &CONTRACTS[&name];
            let mut clobbers = routine.clobbers.clone();
            clobbers.remove(&Reg::Flags);
            assert_ne!(routine.documented, Some(clobbers));
        }
    }

    #[test]
    fn test_the_table_is_keyed_by_its_own_names() {
        for name in names() {
            assert_eq!(CONTRACTS[&CONTRACTS[&name].name], CONTRACTS[&name]);
        }
    }

    #[test]
    fn test_anything_touching_user_code_is_refused() {
        for name in ["B$EVCK", "B$OEGA", "B$RESN", "B$FERR"] {
            assert!(barrier(&CONTRACTS[name]));
        }
    }

    #[test]
    fn test_the_dispatchers_can_do_anything() {
        for name in ["B$EVCK", "B$OEGA", "B$RESN"] {
            let dispatcher = &CONTRACTS[name];
            assert!(dispatcher.enters_user_code);
            assert_eq!(dispatcher.writes, Memory::Any);
            assert_eq!(dispatcher.reads, Memory::Any);
            assert_eq!(dispatcher.clobbers, *EVERY);
        }
    }

    #[test]
    fn test_resume_separates_its_direct_footprint_from_resumed_user_code() {
        let resume = &CONTRACTS["B$RESN"];
        assert!(resume.reads == Memory::Any && resume.writes == Memory::Any);
        assert_eq!(resume.direct_reads, Some(Memory::Own));
        assert_eq!(resume.direct_writes, Some(Memory::Strings));
    }

    #[test]
    fn test_the_err_function_refuses_the_body_without_claiming_to_dispatch() {
        let err = &CONTRACTS["B$FERR"];
        assert!(!err.enters_user_code);
        assert!(err.error_handling);
        assert!(barrier(err));
    }

    #[test]
    fn test_the_routines_that_do_not_come_back_say_so() {
        for name in ["B$CENP", "B$CEND", "B$RESN"] {
            assert_eq!(CONTRACTS[name].control, Control::Never);
        }
    }

    #[test]
    fn test_cleanup_agrees_with_the_arity_calls_py_measured() {
        for name in ABSORBED {
            assert_eq!(CONTRACTS[name].cleanup, Some(8));
        }
    }

    #[test]
    fn test_the_absorbed_four_touch_no_caller_memory() {
        for name in ABSORBED {
            let absorbed = &CONTRACTS[name];
            assert!(!writes_caller_memory(absorbed));
            assert!(absorbed.reads <= Memory::Arguments);
            assert_eq!(absorbed.control, Control::Returns);
            assert!(!absorbed.enters_user_code);
            assert!(absorbed.clobbers.is_subset(&BTreeSet::from([
                Reg::Ax,
                Reg::Bx,
                Reg::Cx,
                Reg::Dx,
                Reg::Flags
            ])));
        }
    }

    #[test]
    fn test_compare_clobbers_only_the_flags_it_returns_in() {
        let compare = &CONTRACTS["B$CPI4"];
        assert_eq!(compare.clobbers, BTreeSet::from([Reg::Flags]));
        assert_eq!(
            compare.documented,
            Some(BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx]))
        );
    }

    #[test]
    fn test_the_x87_helpers_keep_the_index_registers() {
        for name in X87 {
            let kept = preserves(&CONTRACTS[name]);
            assert!(BTreeSet::from([Reg::Si, Reg::Di, Reg::Bx, Reg::Cx]).is_subset(&kept));
        }
    }

    #[test]
    fn test_the_x87_helpers_touch_no_caller_memory() {
        for name in X87 {
            assert!(!writes_caller_memory(&CONTRACTS[name]));
            assert!(!barrier(&CONTRACTS[name]));
        }
    }

    #[test]
    fn test_the_two_forms_differ_only_by_the_sign_extension() {
        let long_form = &CONTRACTS["B$FILD"];
        let int_form = &CONTRACTS["B$FIL2"];
        assert!(!long_form.clobbers.contains(&Reg::Dx));
        assert!(int_form.clobbers.contains(&Reg::Dx));
        let mut without = int_form.clobbers.clone();
        without.remove(&Reg::Dx);
        assert_eq!(without, long_form.clobbers);
    }

    #[test]
    fn test_the_result_registers_are_the_clobbered_ones() {
        assert!(CONTRACTS["B$FIST"].clobbers.contains(&Reg::Ax));
        assert!(CONTRACTS["B$FIST"].clobbers.contains(&Reg::Dx));
        for name in ["B$FIS2", "B$FUST"] {
            assert!(CONTRACTS[name].clobbers.contains(&Reg::Ax));
            assert!(
                !CONTRACTS[name].clobbers.contains(&Reg::Dx),
                "{name} returns one word"
            );
        }
    }

    #[test]
    fn test_the_string_routines_write_caller_memory() {
        for name in ["B$SASS", "B$STDL", "B$SCAT", "B$STI2", "B$LTRM"] {
            assert!(writes_caller_memory(&CONTRACTS[name]));
        }
    }

    /// Unestablished, RESUME and RESUME label read every register, and the
    /// raise refused /V objects over a dead high word of EAX.
    #[test]
    fn test_resume_reads_only_its_label() {
        for family in ["qb45", "pds71", "vbdos"] {
            assert_eq!(one("B$RES0", family).inputs, Some(BTreeSet::new()));
            assert_eq!(one("B$RESA", family).inputs, Some(BTreeSet::from([Reg::Ax])));
        }
    }

    /// VBDOS's SPACE$, CHR$, ASC and LEFT$ were worst case, conceding si, di
    /// and ds, which their code saves.
    #[test]
    fn test_vbdos_string_functions_keep_si_di_and_ds() {
        for (name, cleanup) in [("B$SPAC", 2), ("B$FCHR", 2), ("B$FASC", 2), ("B$LEFT", 4)] {
            let got = one(name, "vbdos");
            assert!(got.established, "{name}");
            assert_eq!(got.cleanup, Some(cleanup), "{name}");
            assert_eq!(got.inputs, Some(BTreeSet::new()), "{name}");
            assert_eq!(
                got.clobbers,
                BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Es, Reg::Flags]),
                "{name}"
            );
        }
    }

    /// VBDOS's LTRIM$, concatenation and assignment were said to keep es,
    /// which B$RefString leaves as the string's segment.
    #[test]
    fn test_vbdos_far_strings_clobber_es() {
        for name in ["B$LTRM", "B$SCAT", "B$SASS"] {
            assert!(one(name, "vbdos").clobbers.contains(&Reg::Es), "{name}");
            assert!(!one(name, "qb45").clobbers.contains(&Reg::Es), "{name}");
        }
    }

    /// Unestablished, SPACE$, CHR$, ASC and LEFT$ conceded si and di,
    /// which callers keep live across them. VBDOS's are variants.
    #[test]
    fn test_string_functions_keep_si_and_di() {
        for name in ["B$SPAC", "B$FCHR", "B$FASC", "B$LEFT"] {
            for family in ["qb45", "pds71"] {
                let got = one(name, family);
                assert!(got.established, "{name} {family}");
                assert!(!got.clobbers.contains(&Reg::Si), "{name} {family}");
                assert!(!got.clobbers.contains(&Reg::Di), "{name} {family}");
            }
        }
    }

    #[test]
    fn test_the_routines_that_touch_no_caller_memory() {
        for name in ABSORBED.into_iter().chain(["B$DSEG", "B$FERR"]) {
            assert!(!writes_caller_memory(&CONTRACTS[name]));
        }
    }

    #[test]
    fn test_a_contract_says_which_registers_it_reads() {
        assert_eq!(
            contract(Some("B$FILD")).inputs,
            Some(BTreeSet::from([Reg::Ax, Reg::Dx]))
        );
        assert_eq!(
            contract(Some("B$FIL2")).inputs,
            Some(BTreeSet::from([Reg::Ax]))
        );
        assert_eq!(contract(Some("B$FIST")).inputs, Some(BTreeSet::new()));
        assert!(!contract(Some("B$NOSUCH")).established);
    }

    #[test]
    fn test_on_goto_takes_its_branch_index_in_bx() {
        assert_eq!(
            contract(Some("B$OGTA")).inputs,
            Some(BTreeSet::from([Reg::Bx]))
        );
    }

    #[test]
    fn test_every_established_contract_says_what_it_reads() {
        let mut unknown: Vec<&str> = CONTRACTS
            .iter()
            .filter(|(_, one)| one.established && one.inputs.is_none())
            .map(|(name, _)| name.as_str())
            .collect();
        unknown.sort();
        assert_eq!(
            unknown,
            ["B$ENRA", "B$EXSA"],
            "unestablished inputs: {unknown:?}"
        );
        assert_eq!(
            worst("anything").inputs,
            None,
            "a name with no entry reads everything"
        );
    }

    #[test]
    fn test_a_routine_that_enters_user_code_still_writes_anything() {
        for name in ["B$CENP", "B$EVCK", "B$OEGA", "B$RESN"] {
            let contract = contract(Some(name));
            assert!(
                contract.enters_user_code,
                "{name} is listed here but does not enter user code"
            );
            assert_eq!(
                contract.writes,
                Memory::Any,
                "{name} was narrowed and must not be"
            );
        }
        assert_eq!(
            contract(Some("B$NOTAROUTINE")).writes,
            Memory::Any,
            "an unestablished contract must concede everything"
        );
    }

    #[test]
    fn test_the_measured_routines_are_narrowed() {
        let narrowed: Vec<&String> = CONTRACTS
            .iter()
            .filter(|(_, one)| one.writes == Memory::Own)
            .map(|(name, _)| name)
            .collect();
        assert!(
            narrowed.len() >= 15,
            "only {} routines carry the measurement",
            narrowed.len()
        );
        for name in narrowed {
            assert!(
                !CONTRACTS[name].enters_user_code,
                "{name} enters user code and is narrowed"
            );
        }
    }

    #[test]
    fn test_every_row_in_the_table_is_loaded() {
        let rows: toml::Table = TABLE.parse().unwrap();
        for (name, one) in CONTRACTS.iter() {
            let row = rows[name].as_table().unwrap();
            assert_eq!(one.writes.name(), row["writes"].as_str().unwrap());
            assert_eq!(one.reads.name(), row["reads"].as_str().unwrap());
            assert_eq!(
                one.clobbers.iter().map(|r| r.value()).collect::<BTreeSet<_>>(),
                row["clobbers"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect::<BTreeSet<_>>()
            );
            assert_eq!(one.established, row["established"].as_bool().unwrap());
            assert_eq!(one.evidence.trim(), row["evidence"].as_str().unwrap().trim());
        }
    }

    /// A misspelt field was silently ignored, leaving the worst case where
    /// the row meant to narrow it.
    #[test]
    fn test_the_loader_refuses_a_malformed_row() {
        for text in [
            "[\"B$X\"]\nclobers = [\"ax\"]\n",
            "[\"B$X\"]\nclobbers = [\"zz\"]\n",
            "[\"B$X\".qb46]\ncleanup = 0\n",
            "[\"B$X\".qb45]\nby_value = true\n",
            "[\"B$X\"]\nfrom = \"B$X\"\n",
            "[\"B$X\"]\nfrom = \"B$Y.vbdos\"\n",
        ] {
            assert!(load(text).is_err(), "{text}");
        }
    }

    /// A family's row is worst() with its own fields, not the routine's row
    /// with them, and only that family's calls get it.
    #[test]
    fn test_a_family_row_applies_over_the_worst_case() {
        let text = "[\"B$X\"]\ncleanup = 2\nestablished = true\ninputs = []\n\n[\"B$X\".vbdos]\ninputs = [\"bx\"]\n\n[\"B$X\".pds71]\nfrom = \"B$X\"\ncleanup = 4\n";
        let routines = load(text).unwrap();
        let routine = &routines["B$X"];
        assert_eq!(routine.families["vbdos"], Contract { inputs: Some(BTreeSet::from([Reg::Bx])), ..worst("B$X") });
        assert_eq!(routine.families["pds71"], Contract { cleanup: Some(4), ..routine.contract.clone().unwrap() });
        assert!(!routine.families.contains_key("qb45"));
    }

    #[test]
    fn test_an_unestablished_row_concedes_everything() {
        for (name, one) in CONTRACTS.iter() {
            if one.established {
                continue;
            }
            assert_eq!(
                one.writes,
                Memory::Any,
                "{name} is not established and narrows its writes"
            );
            assert_eq!(
                one.reads,
                Memory::Any,
                "{name} is not established and narrows its reads"
            );
        }
    }

    #[test]
    fn test_the_semicolon_long_print_reads_no_register() {
        let one = contract(Some("B$PSI4"));
        assert!(one.established, "B$PSI4 is not in the table");
        assert_eq!(
            one.inputs,
            Some(BTreeSet::new()),
            "it reads {:?}",
            one.inputs
        );
        assert_eq!(one.cleanup, Some(4), "it pops {:?}", one.cleanup);
        assert_eq!(one.cleanup, contract(Some("B$PEI4")).cleanup);
        assert_eq!(one.clobbers, contract(Some("B$PSI2")).clobbers);
        assert_eq!(one.writes, contract(Some("B$PSI2")).writes);
        assert_eq!(one.reads, contract(Some("B$PSI2")).reads);
    }

    // tests/test_environ_contract.py

    #[test]
    fn test_vbdos_environ_keeps_general_inputs_and_unknown_effects() {
        let rule = one("B$FEVS", "vbdos");
        assert_eq!(rule.inputs, Some(gp()));
        assert_eq!(rule.cleanup, None);
        assert_eq!(rule.clobbers, *EVERY);
        assert!(rule.reads == Memory::Any && rule.writes == Memory::Any);
        assert_eq!(rule.control, Control::Unknown);
        assert!(rule.raises_error && barrier(&rule));
    }

    #[test]
    fn test_environ_evidence_does_not_claim_other_runtime_versions() {
        for family in ["qb45", "pds71", ""] {
            assert_eq!(one("B$FEVS", family).inputs, None);
        }
    }

    // tests/test_trig_contract.py

    #[test]
    fn test_vbdos_trig_retains_conservative_register_and_memory_effects() {
        for name in ["B$SIN4", "B$SIN8", "B$COS4", "B$COS8"] {
            let routine = one(name, "vbdos");
            assert_eq!(routine.inputs, Some(gp()));
            assert_eq!(routine.cleanup, Some(0));
            assert_eq!(routine.clobbers, *EVERY);
            assert_eq!(routine.reads, Memory::Any);
            assert_eq!(routine.writes, Memory::Any);
            assert!(routine.raises_error);
        }
    }

    // tests/test_rounding_contracts.py

    #[test]
    fn test_vbdos_string_conversion_keeps_unproved_effects() {
        for symbol in ["B$STR4", "B$STR8"] {
            let rule = one(symbol, "vbdos");
            assert_eq!(rule.inputs, Some(gp()));
            assert_eq!(rule.cleanup, None);
            assert_eq!(rule.clobbers, *EVERY);
            assert!(rule.reads == Memory::Any && rule.writes == Memory::Any);
            assert!(rule.control == Control::Unknown && rule.raises_error);
            assert_eq!(one(symbol, "qb45").inputs, None);
        }
    }

    #[test]
    fn test_vbdos_rounding_keeps_unknown_effects() {
        for symbol in ["B$INT4", "B$INT8"] {
            let rule = one(symbol, "vbdos");
            assert_eq!(rule.cleanup, Some(0));
            assert_eq!(rule.inputs, Some(gp()));
            assert_eq!(rule.clobbers, *EVERY);
            assert!(rule.reads == Memory::Any && rule.writes == Memory::Any);
            assert!(rule.control == Control::Unknown && rule.raises_error);
            assert_eq!(one(symbol, "qb45").inputs, None);
        }
    }

    // tests/test_file_contracts.py

    #[test]
    fn test_peos_register_interface_does_not_claim_fixed_stack_cleanup() {
        let contract = one("B$PEOS", "vbdos");
        assert_eq!(contract.inputs, Some(gp()));
        assert_eq!(contract.cleanup, None);
        assert_eq!(contract.clobbers, *EVERY);
        assert_eq!(contract.control, Control::Unknown);
        assert_eq!(contract.writes, Memory::Any);
    }

    #[test]
    fn test_vbdos_file_setup_retains_unknown_effects() {
        for (name, cleanup) in [
            ("B$FREF", Some(0)),
            ("B$OPEN", Some(8)),
            ("B$DSKI", Some(2)),
            ("B$FEOF", Some(2)),
            ("B$CLOS", None),
            ("B$FLEN", Some(2)),
            ("B$FMID", Some(6)),
            ("B$SCMP", Some(4)),
            ("B$SCPF", Some(2)),
            ("B$LNIN", Some(10)),
            ("B$ERS1", Some(2)),
            ("B$RTRM", Some(2)),
            ("B$RGHT", Some(4)),
            ("B$FMKI", Some(2)),
            ("B$FMKL", Some(4)),
            ("B$FCVI", Some(2)),
            ("B$FCVS", Some(2)),
            ("B$CSCN", None),
            ("B$WIDT", Some(4)),
            ("B$SLEP", Some(4)),
            ("B$TIMR", Some(0)),
            ("B$FRI2", Some(2)),
            ("B$STI4", Some(4)),
        ] {
            let contract = one(name, "vbdos");
            assert_eq!(contract.cleanup, cleanup, "{name}");
            assert_eq!(contract.inputs, Some(gp()));
            assert_eq!(contract.clobbers, *EVERY);
            assert_eq!(contract.reads, Memory::Any);
            assert_eq!(contract.writes, Memory::Any);
            assert_eq!(contract.control, Control::Unknown);
            assert!(contract.raises_error);
        }
    }

    #[test]
    fn test_qb45_string_assignment_and_double_print_have_fixed_cleanup() {
        for (name, cleanup) in [("B$ASSN", 12), ("B$PER8", 8)] {
            assert_eq!(one(name, "qb45").cleanup, Some(cleanup));
        }
    }

    // tests/test_entry_contract.py

    #[test]
    fn test_vbdos_output_channel_keeps_unknown_effects() {
        let contract = one("B$CHOU", "vbdos");
        assert_eq!(contract.inputs, Some(gp()));
        assert_eq!(contract.cleanup, Some(2));
        assert_eq!(contract.clobbers, *EVERY);
        assert_eq!(contract.reads, Memory::Any);
        assert_eq!(contract.writes, Memory::Any);
        assert_eq!(contract.control, Control::Unknown);
        assert!(contract.raises_error);
    }

    #[test]
    fn test_vbdos_statement_exit_has_stack_neutral_interface() {
        let contract = one("B$EXTS", "vbdos");
        assert_eq!(contract.inputs, Some(gp()));
        assert_eq!(contract.direct_inputs, Some(BTreeSet::new()));
        assert_eq!(contract.cleanup, Some(0));
        assert_eq!(contract.clobbers, *EVERY);
        assert_eq!(contract.reads, Memory::Any);
        assert_eq!(contract.writes, Memory::Any);
        assert_eq!(contract.control, Control::Unknown);
        assert!(contract.raises_error);
    }

    #[test]
    fn test_vbdos_float_print_interfaces() {
        for (name, cleanup) in [
            ("B$PCR4", 4),
            ("B$PSR4", 4),
            ("B$PCR8", 8),
            ("B$PSR8", 8),
        ] {
            let contract = one(name, "vbdos");
            assert_eq!(contract.cleanup, Some(cleanup));
            assert_eq!(contract.inputs, Some(gp()));
            assert_eq!(contract.clobbers, *EVERY);
            assert_eq!(contract.reads, Memory::Any);
            assert_eq!(contract.writes, Memory::Any);
            assert_eq!(contract.control, Control::Unknown);
            assert!(contract.raises_error);
        }
    }
}
