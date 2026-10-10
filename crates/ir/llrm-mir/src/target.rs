//! What analyses may ask of the target, which MIR does not state: the
//! driver names one for its program (`program::Program::target`), as
//! LLVM's `TargetMachine` gives its analyses `TargetTransformInfo`.

use std::collections::BTreeSet;

use crate::spaces::Spaces;

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

    /// What each operation costs in code bytes, for a decision made for
    /// size; by default the same prices.
    fn size_costs(&self) -> OperationCosts {
        self.costs()
    }

    /// How many integer values fit in registers at once.
    fn registers(&self) -> i64;

    /// Of `registers`, how many survive a call.
    fn call_registers(&self) -> i64;

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
    /// `rep movs` as a copy sets it up (ES, the two addresses, the count) and
    /// what each cell costs it; `direction` is `std` and `cld` around a
    /// backward one.
    pub copy: i64,
    pub copy_cell: i64,
    pub direction: i64,
    /// How many operations a completely unrolled loop may total before the work
    /// it saves is boosted (`peelsize::Limits`); 0: the target states none
    /// and the pass's own is used.
    pub unroll_budget: i64,
}

impl OperationCosts {
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
            float_store: 1,
            float_release: 0,
            extend: 1,
            set: 1,
            fill: 1,
            fill_cell: 1,
            copy: 1,
            copy_cell: 1,
            direction: 1,
            unroll_budget: 0,
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
