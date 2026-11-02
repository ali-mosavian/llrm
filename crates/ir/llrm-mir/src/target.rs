//! What analyses may ask of the target, which MIR does not state: the
//! driver names one for its program (`program::Program::target`), as
//! LLVM's `TargetMachine` gives its analyses `TargetTransformInfo`.

use std::collections::BTreeSet;

use crate::spaces::Spaces;

/// The tuning limits of a target: one default (the pin's value, which every
/// target without an entry takes) and the overrides a target description
/// states. The names are the pin's `TargetLoweringBase` fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tuning {
    /// `MaxStoresPerMemset`, and at size `MaxStoresPerMemsetOptSize`.
    pub max_stores_per_memset: usize,
    pub max_stores_per_memset_opt_size: usize,
    /// `MaxStoresPerMemcpy` and `...OptSize`: the load and store pairs of a
    /// constant copy.
    pub max_stores_per_memcpy: usize,
    pub max_stores_per_memcpy_opt_size: usize,
    /// `MaxStoresPerMemmove` and `...OptSize`.
    pub max_stores_per_memmove: usize,
    pub max_stores_per_memmove_opt_size: usize,
    /// Clocks a byte of code is worth, in thousandths, where a decision is
    /// made for size; 0: bytes alone decide.
    pub size_milliclocks_per_byte: i64,
    /// What a byte is worth in thousandths of a clock at speed; 0: bytes and
    /// clocks are not traded.
    pub speed_milliclocks_per_byte: i64,
    /// A multiply by a constant at speed is the chain of fewest clocks the
    /// CPU's table gives, with no byte term (gcc's `synth_mult` at
    /// -O1..-O3); false: the chain where `speed_milliclocks_per_byte` pays
    /// for its bytes.
    pub multiply_by_clocks: bool,
    /// `UnrollingPreferences::Threshold` and `MaxPercentThresholdBoost` of the
    /// full-unroll pass; none: the pass's own, by optimization level.
    pub unroll_threshold: Option<u32>,
    pub unroll_max_percent_threshold_boost: Option<u32>,
    /// The highest optimization level the two entries above hold at; above it
    /// the pass's own. None: every level.
    pub unroll_override_max_level: Option<u32>,
    /// `UnrollingPreferences::PartialThreshold`: what a partly unrolled loop
    /// (the runtime unroll, the unroll-and-jam outer count) may total; none:
    /// the pass's own, 150.
    pub unroll_partial_threshold: Option<u32>,
    /// `-unswitch-threshold` of the non-trivial loop unswitch (-O3): the cost
    /// (loop size times copies) a candidate must stay under; none: 50.
    pub unswitch_threshold: Option<i64>,
    /// `InlineParams::DefaultThreshold`, the inliner's bar for a caller not
    /// optimized for size; none: 225, or 250 at -O3.
    pub inline_threshold: Option<i64>,
    /// Two compares joined by `and`/`or` stay two branches at every level, as
    /// the pin does at optsize; false: the pin's cost rule decides.
    pub split_merged_conditions: bool,
    /// The inliner's D11 bound (a callee that stays must not grow the code)
    /// also holds a caller optimized for speed; false: the pin's threshold
    /// alone decides there.
    pub inline_size_guard_at_speed: bool,
    /// A constant offset of an address is a displacement the access folds;
    /// SimplifyCFG's sinking of two such accesses behind a phi of the offsets
    /// is refused (it costs the displacement). false: the pin's.
    pub keep_constant_displacements: bool,
}

impl Tuning {
    /// X86ISelLowering.cpp: 16/8, 8/4, 8/4 stores; no clock price for a byte.
    pub const PIN: Tuning = Tuning {
        max_stores_per_memset: 16,
        max_stores_per_memset_opt_size: 8,
        max_stores_per_memcpy: 8,
        max_stores_per_memcpy_opt_size: 4,
        max_stores_per_memmove: 8,
        max_stores_per_memmove_opt_size: 4,
        size_milliclocks_per_byte: 0,
        speed_milliclocks_per_byte: 0,
        multiply_by_clocks: false,
        unroll_threshold: None,
        unroll_max_percent_threshold_boost: None,
        unroll_override_max_level: None,
        unroll_partial_threshold: None,
        unswitch_threshold: None,
        inline_threshold: None,
        split_merged_conditions: false,
        inline_size_guard_at_speed: false,
        keep_constant_displacements: false,
    };
}

/// Where the target keeps no program data, as linear addresses: old
/// `abi::machine::Machine::foreign_span`. A real-mode target has some (its
/// video memory and ROM); any other none.
pub trait Machine {
    /// The linear bytes that `width`-byte accesses at `selectors` and
    /// `offsets` (unsigned words) reach, where foreign memory holds them all.
    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)>;

    /// The address spaces by role: the numbers the target's description gives
    /// them.
    fn spaces(&self) -> Spaces;

    /// What each operation costs on this target, for profitability.
    fn costs(&self) -> OperationCosts;

    /// LLVM's per-instruction cost model of this target, for the passes that
    /// translate LLVM's (InlineCost, the SCEV expander, loop strength
    /// reduction); none: they price by the generic rules.
    fn tti(&self) -> Option<std::rc::Rc<dyn crate::tti::Tti>> {
        None
    }

    /// The library functions the target's runtime has and the widths of `int`
    /// and `size_t` (`TargetLibraryInfo`'s `initialize` of the triple); none:
    /// the pin's defaults for an unknown OS, `int` as wide as a pointer.
    fn library(&self) -> Option<std::rc::Rc<dyn crate::libfunc::Library>> {
        None
    }

    /// The target's `TargetLowering` facts, for the passes that translate
    /// code reading them; none: the base class's answers.
    fn lowering(&self) -> Option<std::rc::Rc<dyn crate::lowering::Lowering>> {
        None
    }

    /// Whether a loop that stores one word or dword to every cell of an object
    /// is a `memset.pattern` fill at -Os, which isel makes `rep stosw` or
    /// `rep stosd` (DEVIATE: the pin leaves it a loop).
    fn fills_by_pattern(&self) -> bool {
        false
    }

    /// The target's tuning limits: the pin's values, with this target's
    /// overrides. A target with no entry here takes `Tuning::PIN`.
    fn tuning(&self) -> Tuning {
        Tuning::PIN
    }

    /// Clocks a byte of code is worth, in thousandths, where a decision is
    /// made for size: the smaller form is kept while the clocks it costs are
    /// under this per byte saved. None: bytes alone decide.
    fn size_milliclocks_per_byte(&self) -> i64 {
        self.tuning().size_milliclocks_per_byte
    }

    /// What each operation costs in code bytes, for a decision made for
    /// size; by default the same prices.
    fn size_costs(&self) -> OperationCosts {
        self.costs()
    }

    /// Whether a division by a constant stays a divide instruction on
    /// `bits`-bit values, `size` saying the build is for size
    /// (`isIntDivCheap`, read the other way): the remainder is then the
    /// divide's own, not `x - (x / c) * c`. By default no: a division is a
    /// multiply.
    fn keeps_division(
        &self,
        bits: u32,
        size: bool,
    ) -> bool {
        let _ = (bits, size);
        false
    }

    /// How many integer values fit in registers at once.
    fn registers(&self) -> i64;

    /// Of `registers`, how many survive a call.
    fn call_registers(&self) -> i64;

    /// How many floating values fit in registers at once: the depth of a
    /// register stack, or a file of their own. None: floating values share
    /// `registers`, or the target does not say.
    fn float_registers(&self) -> i64 {
        0
    }

    /// Registers an access through a pointer wider than its offset takes
    /// for its selector, besides those its address names.
    fn far_access_registers(&self) -> i64 {
        0
    }

    /// Segment registers a far pointer's selector can be held in, besides
    /// `registers`: none where the target has none.
    fn segment_registers(&self) -> i64 {
        0
    }

    /// Whether an arithmetic result is made in the register of its first
    /// operand (x86: `sub dst, src`), so a first operand that stays live is
    /// copied before the result is made.
    fn two_address(&self) -> bool {
        false
    }

    /// Whether the target has several registers a comparison's result can
    /// be kept in (LLVM's `hasMultipleConditionRegisters`), so that one made
    /// in a block and read by a branch in another is held there. Not where
    /// the result is the flags, one register that anything arithmetic
    /// overwrites: x86.
    fn multiple_condition_registers(&self) -> bool {
        false
    }

    /// Of `registers`, how many an address's pointer and index may be held
    /// in, where only some can: none where any can.
    fn address_registers(&self) -> i64 {
        0
    }

    /// Of `registers`, how many survive a call to `callee`, named where the
    /// call is direct: its own contract may keep more than any call does.
    fn kept_across(
        &self,
        _callee: Option<&str>,
    ) -> i64 {
        self.call_registers()
    }

    /// The convention a function nothing outside the program reaches takes,
    /// where the target states one.
    fn private_convention(&self) -> Option<PrivateConvention> {
        None
    }

    /// The convention a function in `convention` takes when its callers'
    /// cleanup of the stack arguments moves to the function: C's own,
    /// `fastcc`, by default; a target whose description states more says which.
    fn callee_pop(
        &self,
        convention: u32,
    ) -> Option<u32> {
        (convention == 0).then_some(crate::opcode::FAST)
    }

    /// The bytes of stack the `arguments` take under `convention`, where the
    /// target knows; else every argument is on it.
    fn stack_argument_bytes(
        &self,
        _convention: u32,
        _arguments: &[Argument],
    ) -> Option<i64> {
        None
    }

    /// What multiplying by the constant `factor`, above one, costs: a
    /// multiply, or the shifts and adds the target makes it of.
    fn multiply_by(
        &self,
        _factor: i64,
    ) -> i64 {
        self.costs().multiply
    }

    /// The indexed addresses a memory access may use, native form first.
    fn address_forms(&self) -> Vec<AddressForm>;

    /// Whether a `width`-byte load at an address a multiple of `align` may
    /// trap wherever it points. By default any may: only one known
    /// dereferenceable runs where the program would not have run it.
    fn load_may_trap(
        &self,
        _width: u64,
        _align: u64,
    ) -> bool {
        true
    }

    /// Where `Intrinsic::Window` makes a huge pointer far: the far pointers'
    /// space, and the bytes from the one it makes that no displacement
    /// carries out of. None where the target has no such window.
    fn huge_window(&self) -> Option<(u32, i64)> {
        None
    }

    /// Whether an I/O access to a port in the inclusive range `ports` may
    /// read or write memory. By default any may.
    fn port_touches_memory(
        &self,
        _ports: (i64, i64),
    ) -> bool {
        true
    }
}

/// What advancing a pair pointer in a carrying space costs, given the price
/// of its instructions: the offset widened, the displacement added, the
/// carry copied and shifted down and up by the stride, added to the
/// selector, and the offset and selector copied out.
pub const fn carry_cost(
    extend: i64,
    add: i64,
    shift: i64,
    r#move: i64,
) -> i64 {
    extend + 2 * add + 2 * shift + 3 * r#move
}

/// What advancing a pair pointer in a carrying space by a constant costs:
/// the offset's sum, its carry spread to a mask, the mask cut to the
/// selector's stride, and added to the selector.
pub const fn step_cost(add: i64) -> i64 {
    4 * add
}

/// Machine-neutral costs a MIR profitability decision may compare.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OperationCosts {
    pub add: i64,
    pub multiply: i64,
    pub divide: i64,
    /// The same on a dword where the target's own word is smaller
    /// (`imul`/`idiv` r32 in real mode, before the operand-size prefix);
    /// equal to `multiply`/`divide` elsewhere.
    pub multiply_long: i64,
    pub divide_long: i64,
    /// Percent by which a constant divide's multiply-high sequence must cost
    /// more than the divide for the divide to stay (`isIntDivCheap`,
    /// priced); 0 where the target states none.
    pub divide_margin: i64,
    pub shift: i64,
    pub address: i64,
    /// An address advanced in a space whose displacement carries into its
    /// selector (a huge pointer's): the offset's sum, its carry, and the
    /// selector stepped by it.
    pub carry: i64,
    /// The same advance by a constant (`step_cost`).
    pub carry_step: i64,
    pub load: i64,
    pub store: i64,
    pub memory_update: i64,
    pub branch: i64,
    pub prefix: i64,
    pub r#move: i64,
    pub call: i64,
    pub return_: i64,
    /// What one argument word costs around a call: pushed by the caller, read
    /// by the callee.
    pub argument: i64,
    /// What a caller pays to take `n` words of arguments off the stack: the
    /// cheaper of `n` pops (`pop` each) and one `adjust` of the stack
    /// pointer.
    pub pop: i64,
    pub adjust: i64,
    /// What a return popping its callee's arguments costs more than a plain
    /// one.
    pub return_pops: i64,
    pub float_add: i64,
    pub float_multiply: i64,
    pub float_divide: i64,
    pub float_load: i64,
    /// What a floating load costs more than the operation that reads it where
    /// the target folds it into that operation's memory form (x87 `fmul
    /// m32`): the load moved out of a loop saves this much each trip, not
    /// `float_load`.
    pub float_load_folded: i64,
    /// Whether a price counts a load its one reader takes from memory at
    /// `float_load_folded`, as the pass that moves loads asks (`hoist`); the
    /// description never states it, and the passes that price a body
    /// by its loads count each as an instruction.
    pub folds_loads: bool,
    pub float_store: i64,
    /// What dropping a floating value from the register stack costs once its
    /// last reader is past (x87 `fstp st(0)`): one held across a loop is
    /// released after it.
    pub float_release: i64,
    pub extend: i64,
    /// A comparison held as a value (`setcc`), before it is extended.
    pub set: i64,
    pub fill: i64,
    pub fill_cell: i64,
    /// The code bytes of the routine a fill through a huge pointer is laid
    /// down as, in a table priced by bytes; 0 where there is none.
    pub huge_fill: i64,
    /// The same for a copy or move through huge pointers: the smaller of the
    /// upward and downward routines.
    pub huge_copy: i64,
    /// `rep movs` as a copy sets it up (ES, the two addresses, the count) and
    /// what each cell costs it; `direction` is `std` and `cld` around a
    /// backward one.
    pub copy: i64,
    pub copy_cell: i64,
    /// What a copy of a length known only at run time adds to `copy`: the
    /// count in dwords and the odd bytes after it, a second `rep movs`.
    pub copy_variable: i64,
    pub direction: i64,
    /// How many operations a completely unrolled loop may total before the work
    /// it saves is boosted (`peelsize::Limits`); 0: the target states none
    /// and the pass's own is used.
    pub unroll_budget: i64,
    /// gcc's by-pieces ratios for a constant memset (`SET_RATIO` for a
    /// nonzero byte, `CLEAR_RATIO` for zero; the `_size` ones when tuned for
    /// size): stores while their number is under it, else a fill. 0: the
    /// target states none for the CPU.
    pub pieces_set: i64,
    pub pieces_clear: i64,
    pub pieces_set_size: i64,
    pub pieces_clear_size: i64,
    /// The fewest case values for which a switch is a jump table (gcc's
    /// `case_values_threshold`); 0: the target has no table jump.
    pub jump_table: i64,
    /// Bytes of one line of the CPU's data cache (its first level); 0: the
    /// target states none (no cache, or not known).
    pub cache_line: i64,
}

impl OperationCosts {
    /// The ratio under which a constant memset of `zero` bytes, or of any other
    /// byte, is stores; 0 where none is stated. `size`: tuned for size.
    pub fn pieces(
        &self,
        size: bool,
        zero: bool,
    ) -> i64 {
        match (size, zero) {
            (true, true) => self.pieces_clear_size,
            (true, false) => self.pieces_set_size,
            (false, true) => self.pieces_clear,
            (false, false) => self.pieces_set,
        }
    }

    /// What a caller pays to take `words` words of arguments off the stack.
    pub fn cleanup(
        &self,
        words: i64,
    ) -> i64 {
        (words * self.pop).min(self.adjust)
    }

    /// `price` for an operation on `width`-byte values where the code's own
    /// operand size is `operand` bytes: a word or dword that is not that
    /// size runs under the operand-size prefix (a dword in real mode, a word in
    /// flat code).
    pub fn sized(
        &self,
        price: i64,
        width: i64,
        operand: i64,
    ) -> i64 {
        price + if width != operand && width > 1 { self.prefix } else { 0 }
    }
}

/// What one `lea` of `width`-byte values costs, where the target has an
/// address form for it. A `word` one is a plain address (a base and an
/// index, unscaled, no prefix); any other takes a form that scales, which
/// costs its address-size prefix, and runs under the operand-size prefix a
/// dword does.
pub fn three_operand(
    costs: &OperationCosts,
    forms: &[AddressForm],
    width: i64,
    operand: i64,
    scale: i64,
    word: bool,
) -> Option<i64> {
    if word {
        return forms.iter().any(|form| !form.secondary && form.index_width == 2).then_some(costs.address);
    }
    let form = forms.iter().find(|form| form.index_width == 4 && form.scales.contains(&scale))?;
    Some(costs.sized(costs.address, width, operand) + form.use_cost)
}

impl Default for OperationCosts {
    fn default() -> Self {
        Self {
            add: 1,
            multiply: 1,
            divide: 1,
            multiply_long: 1,
            divide_long: 1,
            divide_margin: 0,
            shift: 1,
            address: 1,
            carry: 1,
            carry_step: 1,
            load: 1,
            store: 1,
            memory_update: 1,
            branch: 1,
            prefix: 0,
            r#move: 1,
            call: 1,
            return_: 1,
            argument: 2,
            pop: 1,
            adjust: 1,
            return_pops: 0,
            float_add: 1,
            float_multiply: 1,
            float_divide: 1,
            float_load: 1,
            float_load_folded: 1,
            folds_loads: false,
            float_store: 1,
            float_release: 0,
            extend: 1,
            set: 1,
            fill: 1,
            fill_cell: 1,
            huge_fill: 0,
            huge_copy: 0,
            copy: 1,
            copy_cell: 1,
            copy_variable: 0,
            direction: 1,
            unroll_budget: 0,
            pieces_set: 0,
            pieces_clear: 0,
            pieces_set_size: 0,
            pieces_clear_size: 0,
            jump_table: 0,
            cache_line: 0,
        }
    }
}

/// One legal indexed-address family and its costs above the native form.
///
/// Machine-neutral: MIR may know an address can use a four-byte index with
/// scales 1/2/4/8, never how x86 spells it.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AddressForm {
    pub index_width: i64,
    pub scales: BTreeSet<i64>,
    pub extra_bytes: i64,
    pub use_cost: i64,
    pub extension_cost: i64,
    pub secondary: bool,
    // How many distinct bases one index can pair with at once; None is any.
    pub partners: Option<i64>,
    // The registers an address takes as its base and as its index, where an
    // address is one of each: a register is of one class or the other. None is
    // any.
    pub bases: Option<i64>,
    pub indices: Option<i64>,
    // Compatibility name for `secondary`; both views stay identical.
    pub fallback: Option<bool>,
}

impl AddressForm {
    /// The dataclass constructor and `__post_init__`.
    pub fn new(
        index_width: i64,
        scales: BTreeSet<i64>,
        extra_bytes: i64,
        use_cost: i64,
        extension_cost: i64,
        secondary: bool,
        fallback: Option<bool>,
    ) -> Result<Self, String> {
        if fallback.is_some() && secondary && fallback != Some(secondary) {
            return Err("an address form cannot disagree about whether it is secondary".to_owned());
        }
        let selected = fallback.unwrap_or(secondary);
        Ok(Self {
            index_width,
            scales,
            extra_bytes,
            use_cost,
            extension_cost,
            secondary: selected,
            partners: None,
            bases: None,
            indices: None,
            fallback: Some(selected),
        })
    }

    /// How many registers can each hold an address alone: the one that
    /// pairs and its partners. None is any.
    pub fn address_registers(&self) -> Option<i64> {
        self.partners.map(|partners| partners + 1)
    }

    /// The classes of an address's two registers, where the form has them.
    pub fn register_classes(&self) -> Option<(i64, i64)> {
        self.bases.zip(self.indices)
    }

    /// Whether this form is cheap enough to try before a frame spill.
    pub fn before_spill(
        &self,
        costs: &OperationCosts,
    ) -> bool {
        let direct = self.extension_cost + self.use_cost <= costs.load;
        let amortized = self.extension_cost <= costs.r#move
            && self.extension_cost + self.use_cost <= costs.shift + costs.address + costs.r#move + costs.store;
        !self.secondary || direct || amortized
    }
}

/// What an argument is, for where a convention puts it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Argument {
    pub bytes: i64,
    pub integer: bool,
    pub pointer: bool,
    pub floating: bool,
    /// The front end passes it in memory whatever it is (a struct by value).
    pub memory: bool,
}

/// The convention the target gives a private function, and those a function
/// must have for it to take it: a marked function (Pascal's, an interrupt's)
/// keeps the protocol its marker names.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PrivateConvention {
    pub to: u32,
    pub from: Vec<u32>,
}

/// A target that states nothing: no foreign memory, unit prices, and no
/// registers, which leaves pressure unpriced.
pub struct Neutral;

impl Machine for Neutral {
    fn spaces(&self) -> Spaces {
        Spaces::FLAT
    }

    fn foreign_span(
        &self,
        _: (i64, i64),
        _: (i64, i64),
        _: i64,
    ) -> Option<(i64, i64)> {
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

    /// An address adds one index, unscaled and free.
    fn address_forms(&self) -> Vec<AddressForm> {
        vec![AddressForm::new(2, BTreeSet::from([1]), 0, 0, 0, false, None).expect("no fallback to disagree")]
    }
}

#[cfg(test)]
mod operand_size_tests {
    use super::*;

    fn form(
        index_width: i64,
        secondary: bool,
        use_cost: i64,
    ) -> AddressForm {
        AddressForm::new(index_width, BTreeSet::from([1, 2, 4, 8]), 0, use_cost, 0, secondary, None).expect("a form")
    }

    /// The operand-size prefix is for the size that is not the code's own: a
    /// dword in real mode, a word when flat.
    #[test]
    fn the_prefix_is_for_the_size_that_is_not_the_codes_own() {
        let costs = OperationCosts { prefix: 1, ..OperationCosts::default() };
        assert_eq!(
            (
                costs.sized(5, 4, 2),
                costs.sized(5, 2, 2),
                costs.sized(5, 4, 4),
                costs.sized(5, 2, 4),
                costs.sized(5, 1, 4)
            ),
            (6, 5, 5, 6, 5)
        );
    }

    /// A flat target's dword address has no prefix and its price is the
    /// `lea`'s; it had none, because only the prefixed (secondary) form was
    /// looked for. A word address needs a form of index width 2.
    #[test]
    fn a_native_dword_address_is_priced_and_a_word_one_needs_a_word_form() {
        let costs = OperationCosts { address: 2, prefix: 1, ..OperationCosts::default() };
        let flat = [form(4, false, 0)];
        assert_eq!(three_operand(&costs, &flat, 4, 4, 2, false), Some(2));
        assert_eq!(three_operand(&costs, &flat, 2, 4, 1, true), None);
        let real = [form(2, false, 0), form(4, true, 1)];
        assert_eq!(three_operand(&costs, &real, 4, 2, 2, false), Some(2 + 1 + 1));
        assert_eq!(three_operand(&costs, &real, 2, 2, 1, true), Some(2));
    }
}

#[cfg(test)]
mod tests {
    use super::Tuning;

    /// X86ISelLowering.cpp:2936-2941 and the clock-free default: a target with
    /// no entry in its description takes exactly these.
    #[test]
    fn the_default_tuning_is_the_pins() {
        let pin = Tuning::PIN;
        assert_eq!((pin.max_stores_per_memset, pin.max_stores_per_memset_opt_size), (16, 8));
        assert_eq!((pin.max_stores_per_memcpy, pin.max_stores_per_memcpy_opt_size), (8, 4));
        assert_eq!((pin.max_stores_per_memmove, pin.max_stores_per_memmove_opt_size), (8, 4));
        assert_eq!(pin.size_milliclocks_per_byte, 0);
        assert_eq!(
            (pin.unroll_threshold, pin.unroll_max_percent_threshold_boost, pin.unroll_partial_threshold),
            (None, None, None)
        );
        assert_eq!(pin.inline_threshold, None);
    }
}
