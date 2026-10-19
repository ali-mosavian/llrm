//! Where an argument or a result goes, translated from LLVM's
//! `AArch64CallingConvention.td`
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64CallingConvention.td>):
//! `CC_AArch64_AAPCS` and `RetCC_AArch64_AAPCS` for Linux,
//! `CC_AArch64_DarwinPCS` and, for the variadic part of a call,
//! `CC_AArch64_DarwinPCS_VarArg`.

use crate::machine::Platform;

/// What an argument is, after legalization: the value types the convention
/// tables distinguish.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scalar {
    /// An integer or pointer promoted to 32 bits (`i1`..`i32`).
    I32,
    I64,
    F32,
    F64,
}

/// A register of the convention, by class and number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Place {
    Gpr(u32),
    Fpr(u32),
    /// A stack slot at this byte offset from the stack pointer at the call.
    Stack {
        offset: u64,
        size: u64,
    },
}

/// The argument and result registers: `x0`..`x7` and `d0`..`d7`.
pub const ARGUMENT_REGISTERS: u32 = 8;

/// One call's assignments, made argument by argument in order, as
/// `CCState::AnalyzeCallOperands` does.
#[derive(Clone, Debug)]
pub struct Assigner {
    platform: Platform,
    next_gpr: u32,
    next_fpr: u32,
    stack: u64,
}

impl Assigner {
    pub fn new(platform: Platform) -> Self {
        Self { platform, next_gpr: 0, next_fpr: 0, stack: 0 }
    }

    /// How many general-purpose and floating-point argument registers the
    /// arguments assigned so far have used.
    pub fn registers_used(&self) -> (u32, u32) {
        (self.next_gpr, self.next_fpr)
    }

    /// The bytes of stack the arguments assigned so far take.
    pub fn stack_bytes(&self) -> u64 {
        self.stack
    }

    fn stack_slot(
        &mut self,
        size: u64,
        align: u64,
    ) -> Place {
        self.stack = self.stack.next_multiple_of(align);
        let place = Place::Stack { offset: self.stack, size };
        self.stack += size;
        place
    }

    /// The place of the next argument of type `value`; `fixed` is false for the
    /// arguments a variadic callee names with `...`, which Darwin passes on the
    /// stack, each in a slot of 8 bytes.
    pub fn assign(
        &mut self,
        value: Scalar,
        fixed: bool,
    ) -> Place {
        if self.platform == Platform::Darwin && !fixed {
            return match value {
                Scalar::I32 | Scalar::I64 | Scalar::F32 | Scalar::F64 => self.stack_slot(8, 8),
            };
        }
        match value {
            Scalar::I32 | Scalar::I64 if self.next_gpr < ARGUMENT_REGISTERS => {
                self.next_gpr += 1;
                Place::Gpr(self.next_gpr - 1)
            }
            Scalar::F32 | Scalar::F64 if self.next_fpr < ARGUMENT_REGISTERS => {
                self.next_fpr += 1;
                Place::Fpr(self.next_fpr - 1)
            }
            // The registers are used up: Darwin sizes the slot as the type
            // needs, AAPCS gives every slot 8 bytes.
            Scalar::I32 | Scalar::F32 => match self.platform {
                Platform::Darwin => self.stack_slot(4, 4),
                Platform::Linux => self.stack_slot(8, 8),
            },
            Scalar::I64 | Scalar::F64 => self.stack_slot(8, 8),
        }
    }
}

impl Assigner {
    /// The places of the `members` of one array or homogeneous aggregate, which
    /// travel together (`CC_AArch64_Custom_Block`): in consecutive registers if
    /// the file has that many left, else every register of the class is marked
    /// used and the members lie contiguously on the stack, each `size` bytes,
    /// the block aligned to `align` (the Darwin PCS lets a block be aligned
    /// as its element is; AAPCS aligns it to 8 at least).
    pub fn assign_block(
        &mut self,
        member: Scalar,
        count: usize,
        size: u64,
        align: u64,
    ) -> Vec<Place> {
        let count_u32 = count as u32;
        match member {
            Scalar::I32 | Scalar::I64 if self.next_gpr + count_u32 <= ARGUMENT_REGISTERS => {
                let first = self.next_gpr;
                self.next_gpr += count_u32;
                return (first..first + count_u32).map(Place::Gpr).collect();
            }
            Scalar::F32 | Scalar::F64 if self.next_fpr + count_u32 <= ARGUMENT_REGISTERS => {
                let first = self.next_fpr;
                self.next_fpr += count_u32;
                return (first..first + count_u32).map(Place::Fpr).collect();
            }
            Scalar::I32 | Scalar::I64 => self.next_gpr = ARGUMENT_REGISTERS,
            Scalar::F32 | Scalar::F64 => self.next_fpr = ARGUMENT_REGISTERS,
        }
        let align = if self.platform == Platform::Darwin { align } else { align.max(8) };
        self.stack = self.stack.next_multiple_of(align);
        let places = (0..count as u64).map(|at| Place::Stack { offset: self.stack + at * size, size }).collect();
        self.stack += count as u64 * size;
        places
    }

    /// The register a pointer to the result of a function returning a large
    /// aggregate (`sret`) is passed in.
    pub const RESULT_POINTER: u32 = 8;
}

/// The register a result of type `value` comes back in: `RetCC_AArch64_AAPCS`
/// assigns `x0`.. and `d0`.. in order; one scalar uses the first.
pub fn result(value: Scalar) -> Place {
    match value {
        Scalar::I32 | Scalar::I64 => Place::Gpr(0),
        Scalar::F32 | Scalar::F64 => Place::Fpr(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_and_floats_take_their_own_register_files() {
        let mut assigner = Assigner::new(Platform::Linux);
        let places: Vec<Place> = [Scalar::I32, Scalar::F64, Scalar::I64, Scalar::F32]
            .into_iter()
            .map(|one| assigner.assign(one, true))
            .collect();
        assert_eq!(places, [Place::Gpr(0), Place::Fpr(0), Place::Gpr(1), Place::Fpr(1)]);
    }

    /// The ninth integer argument is on the stack: Darwin sizes the slot of an
    /// `i32` as 4 bytes, AAPCS as 8.
    #[test]
    fn a_ninth_i32_argument_is_a_stack_slot_the_platform_sizes() {
        for (platform, size) in [(Platform::Darwin, 4), (Platform::Linux, 8)] {
            let mut assigner = Assigner::new(platform);
            for _ in 0..8 {
                assigner.assign(Scalar::I64, true);
            }
            assert_eq!(assigner.assign(Scalar::I32, true), Place::Stack { offset: 0, size });
            assert_eq!(assigner.assign(Scalar::I64, true), Place::Stack { offset: 8, size: 8 });
        }
    }

    /// Darwin puts the arguments a callee names with `...` on the stack.
    #[test]
    fn darwin_passes_variadic_arguments_on_the_stack() {
        let mut assigner = Assigner::new(Platform::Darwin);
        assert_eq!(assigner.assign(Scalar::I64, true), Place::Gpr(0));
        assert_eq!(assigner.assign(Scalar::I32, false), Place::Stack { offset: 0, size: 8 });
        assert_eq!(assigner.assign(Scalar::F64, false), Place::Stack { offset: 8, size: 8 });
        let mut linux = Assigner::new(Platform::Linux);
        assert_eq!(linux.assign(Scalar::I64, true), Place::Gpr(0));
        assert_eq!(linux.assign(Scalar::I32, false), Place::Gpr(1));
    }
}
