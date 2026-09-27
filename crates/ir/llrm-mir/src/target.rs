//! What analyses may ask of the target, which MIR does not state: the
//! driver names one for its program (`program::Program::target`), as
//! LLVM's `TargetMachine` gives its analyses `TargetTransformInfo`.

/// Where the target keeps no program data, as linear addresses: old
/// `abi::machine::Machine::foreign_span`. A real-mode target has some (its
/// video memory and ROM); any other none.
pub trait Machine {
    /// The linear bytes that `width`-byte accesses at `selectors` and
    /// `offsets` (unsigned words) reach, where foreign memory holds them all.
    fn foreign_span(&self, selectors: (i64, i64), offsets: (i64, i64), width: i64) -> Option<(i64, i64)>;

    /// What each operation costs on this target, for profitability.
    fn costs(&self) -> OperationCosts;

    /// How many integer values fit in registers at once.
    fn registers(&self) -> i64;

    /// Of `registers`, how many survive a call.
    fn call_registers(&self) -> i64;
}

/// Machine-neutral costs a MIR profitability decision may compare.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OperationCosts {
    pub add: i64,
    pub multiply: i64,
    pub divide: i64,
    pub shift: i64,
    pub address: i64,
    pub load: i64,
    pub store: i64,
    pub memory_update: i64,
    pub branch: i64,
    pub prefix: i64,
    pub r#move: i64,
    pub call: i64,
    pub return_: i64,
    pub float_add: i64,
    pub float_multiply: i64,
    pub float_divide: i64,
    pub float_load: i64,
    pub float_store: i64,
    pub extend: i64,
    pub fill: i64,
    pub fill_cell: i64,
}

impl Default for OperationCosts {
    fn default() -> Self {
        Self {
            add: 1,
            multiply: 1,
            divide: 1,
            shift: 1,
            address: 1,
            load: 1,
            store: 1,
            memory_update: 1,
            branch: 1,
            prefix: 0,
            r#move: 1,
            call: 1,
            return_: 1,
            float_add: 1,
            float_multiply: 1,
            float_divide: 1,
            float_load: 1,
            float_store: 1,
            extend: 1,
            fill: 1,
            fill_cell: 1,
        }
    }
}


/// A target that states nothing: no foreign memory, unit prices, and no
/// registers, which leaves pressure unpriced.
pub struct Neutral;

impl Machine for Neutral {
    fn foreign_span(&self, _: (i64, i64), _: (i64, i64), _: i64) -> Option<(i64, i64)> {
        None
    }

    fn costs(&self) -> OperationCosts {
        OperationCosts::default()
    }

    fn registers(&self) -> i64 {
        0
    }

    fn call_registers(&self) -> i64 {
        0
    }
}
