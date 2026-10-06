//! The target-generic layer: what a target description is, owned by no ISA.
//! A target crate supplies the data; the passes read the type.

pub mod addressing;
pub mod layout;
pub mod machine;
pub mod object;
pub mod opcosts;
pub mod registers;
pub mod runtime;
pub mod timings;

use std::rc::Rc;

use llrm_mir::target::{AddressForm, OperationCosts};
use machine::Machine;

/// What a CPU's profile states that a target prices its operations from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuPrices {
    /// Clocks per kind of instruction form.
    pub costs: Vec<(String, i64)>,
    /// What an operand-size prefix costs.
    pub prefix: i64,
    /// What an address-size prefix costs besides.
    pub address_stall: i64,
    /// Registers an allocator may hold values in, and those a call keeps.
    pub registers: i64,
    pub call_registers: i64,
    /// The indexed addresses the target states, native form first.
    pub address_forms: Vec<AddressForm>,
    /// What each operation costs: the target's mapping applied to `costs`.
    pub operations: OperationCosts,
    /// The target's address spaces by role.
    pub spaces: llrm_mir::spaces::Spaces,
}

/// A cost model that is only what a target describes: its registers, address forms and
/// operation prices.
pub fn described(prices: &CpuPrices) -> Rc<dyn llrm_mir::target::Machine> {
    Rc::new(Described { registers: prices.registers, call_registers: prices.call_registers, address_forms: prices.address_forms.clone(), operations: prices.operations.clone(), spaces: prices.spaces })
}

struct Described {
    registers: i64,
    call_registers: i64,
    address_forms: Vec<AddressForm>,
    operations: OperationCosts,
    spaces: llrm_mir::spaces::Spaces,
}

impl llrm_mir::target::Machine for Described {
    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        self.spaces
    }

    /// Memory without segments is linear: no selector and offset reach foreign memory.
    fn foreign_span(&self, _: (i64, i64), _: (i64, i64), _: i64) -> Option<(i64, i64)> {
        None
    }

    fn costs(&self) -> OperationCosts {
        self.operations.clone()
    }

    fn registers(&self) -> i64 {
        self.registers
    }

    fn call_registers(&self) -> i64 {
        self.call_registers
    }

    fn two_address(&self) -> bool {
        true
    }

    fn address_forms(&self) -> Vec<AddressForm> {
        self.address_forms.clone()
    }
}

/// A target's cost model, built from a CPU's prices: what the passes ask of it.
pub type CostModel = fn(&CpuPrices) -> Rc<dyn llrm_mir::target::Machine>;

/// A target, as the driver picks one by name: what a frontend of it starts
/// from. The passes' view of a target grows here as the backend stops naming
/// one (docs/targets.md).
pub trait Target {
    /// The name `--target` takes.
    fn name(&self) -> &'static str;

    /// The platform description a frontend of this target defaults to.
    fn machine(&self) -> Machine;

    /// The processors this target prices, as a platform description names them.
    fn cpus(&self) -> &'static [&'static str];

    /// The data layout programs are built under and the address spaces a
    /// frontend's addresses are.
    fn layout(&self) -> layout::Layout;

    /// The bytes an argument of `width` takes on the stack, and the least a
    /// stack cell holds.
    fn stack_slot_bytes(&self) -> i64;

    /// The register a frame's cells are addressed through.
    fn frame_register(&self) -> iced_x86::Register;

    /// Where the first argument lies from the frame register: past the saved
    /// frame register and the return address, which a far call makes longer.
    fn first_argument_offset(&self, far: bool) -> i64;

    /// The registers a result of `width` bytes leaves in, low part first.
    fn results(&self, width: u32) -> Vec<iced_x86::Register>;

    /// The register that holds the stack's top.
    fn stack_pointer(&self) -> iced_x86::Register;

    /// The registers a callee keeps for its caller that an allocator may hold
    /// a value in: each by its full register and by the one a prologue pushes.
    fn callee_saved(&self) -> Vec<(iced_x86::Register, iced_x86::Register)>;

    /// One CPU's timings (a column of the target's `timings.times`), if the target prices it.
    fn cpu_table(&self, name: &str) -> Option<timings::CpuTable>;

    /// The target's register file, in `registers.regs`'s format.
    fn registers_text(&self) -> String;

    /// The target's instruction forms: the family's and its own, in `x86.instr`'s format.
    fn forms_text(&self) -> String;

    /// The operand size an instruction has without a size prefix, in bytes.
    fn operand_bytes(&self) -> i64;

    /// The CPU a compile is priced for where none is asked.
    fn default_cpu(&self) -> &'static str;

    /// The operations priced by `price` (the CPU's clocks of a form), as the target's
    /// `opcosts.txt` makes them of forms; `prefix` is the CPU's operand-size prefix cost.
    fn operation_costs(&self, price: &dyn Fn(&str) -> i64, prefix: i64) -> OperationCosts;

    /// The registers an allocator may hold values in.
    fn register_capacity(&self) -> i64;

    /// The indexed addresses a memory access may use, priced by `costs` (its
    /// operand-size prefix in `prefix`) and `address_stall`: native form first.
    fn address_forms(&self, costs: &OperationCosts, address_stall: i64) -> Vec<AddressForm>;

    /// How this target's passes are given the prices of a CPU.
    fn cost_model(&self) -> CostModel;

    /// The calling conventions a program for this target may name, by the names `calling.toml`
    /// gives them; the first is the one a language's own functions use.
    fn conventions(&self) -> &'static [&'static str];

    /// The machine's physical addresses by name (the text screen's video memory): one fact, which a
    /// target's address space turns into the pointer it has.
    fn physical_addresses(&self) -> Vec<(String, u64)> {
        Vec::new()
    }

    /// The OS layer a language's runtime is built on here, if the target has one for it.
    fn runtime(&self, _language: &str) -> Option<runtime::Description> {
        None
    }

    /// The object file and listing this target writes.
    fn object(&self) -> object::ObjectFormat;

    /// What a frame is built of.
    fn frame_registers(&self) -> FrameRegisters {
        FrameRegisters { pointer: self.frame_register(), stack: self.stack_pointer(), saved: self.callee_saved(), slot: self.stack_slot_bytes() }
    }
}

/// The registers a procedure's frame is made of. LIR names the frame register
/// BP and the stack pointer SP whatever the target; a listing and an object
/// spell them as the target has them.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FrameRegisters {
    pub pointer: iced_x86::Register,
    pub stack: iced_x86::Register,
    /// Each callee-saved register by its full register and the one pushed.
    pub saved: Vec<(iced_x86::Register, iced_x86::Register)>,
    /// The bytes the stack is kept a multiple of.
    pub slot: i64,
}

impl FrameRegisters {
    /// `register` as this target spells it: the frame register and stack pointer
    /// LIR calls BP and SP.
    pub fn spelled(&self, register: iced_x86::Register) -> iced_x86::Register {
        match register {
            iced_x86::Register::BP => self.pointer,
            iced_x86::Register::SP => self.stack,
            other => other,
        }
    }
}

/// The I/O ports of a PC: a `[[ports]]`-only description that a platform appends to
/// its own text.
pub const PC_PORTS: &str = include_str!("machines/pc-ports.toml");
