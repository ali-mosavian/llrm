//! The target-generic layer: what a target description is, owned by no ISA.
//! A target crate supplies the data; the passes read the type.

pub mod layout;
pub mod machine;

use machine::Machine;

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

    /// What a frame is built of.
    fn frame_registers(&self) -> FrameRegisters {
        FrameRegisters { pointer: self.frame_register(), stack: self.stack_pointer(), saved: self.callee_saved() }
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
