//! Immutable x87 target-ranking costs.
//!
//! These values are a direct Rust spelling of Python's `_I386_COSTS` x87
//! entries in `qbopt/backend/cpu.py` and the x87 columns in
//! `qbopt/cycles/timings.py`. They are target tuning data, not a generic cost
//! model or a statement about source-language semantics.

/// One supported x86 tuning profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum X86Cpu {
    I386,
    I486,
    P5,
    P6,
    K5,
    K6,
    K7,
    Core,
}

/// The target-ranking cost of one x87 instruction form.
///
/// Memory arithmetic is distinct from its register-stack counterpart because
/// the Python allocator prices the folded load explicitly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct X86FloatCosts {
    pub load: u32,
    pub exchange: u32,
    pub store: u32,
    pub convert_store: u32,
    pub add: u32,
    pub add_memory: u32,
    pub multiply: u32,
    pub multiply_memory: u32,
    pub divide: u32,
    pub divide_memory: u32,
    pub control_load: u32,
    pub control_store: u32,
}

impl X86Cpu {
    /// Returns this profile's immutable x87 ranking costs.
    ///
    /// `I386` comes from `qbopt/backend/cpu.py::_I386_COSTS`; the remaining
    /// profiles directly transpose `qbopt/cycles/timings.py` lines 180-217.
    pub const fn float_costs(self) -> X86FloatCosts {
        match self {
            Self::I386 => X86FloatCosts {
                load: 8,
                exchange: 18,
                store: 8,
                convert_store: 35,
                add: 23,
                add_memory: 31,
                multiply: 27,
                multiply_memory: 35,
                divide: 88,
                divide_memory: 96,
                control_load: 8,
                control_store: 8,
            },
            Self::I486 => X86FloatCosts {
                load: 8,
                exchange: 4,
                store: 8,
                convert_store: 35,
                add: 8,
                add_memory: 16,
                multiply: 16,
                multiply_memory: 24,
                divide: 73,
                divide_memory: 81,
                control_load: 8,
                control_store: 8,
            },
            Self::P5 => X86FloatCosts {
                load: 2,
                exchange: 1,
                store: 4,
                convert_store: 7,
                add: 3,
                add_memory: 5,
                multiply: 3,
                multiply_memory: 5,
                divide: 39,
                divide_memory: 41,
                control_load: 2,
                control_store: 4,
            },
            Self::P6 => X86FloatCosts {
                load: 2,
                exchange: 0,
                store: 4,
                convert_store: 7,
                add: 3,
                add_memory: 5,
                multiply: 5,
                multiply_memory: 7,
                divide: 56,
                divide_memory: 58,
                control_load: 2,
                control_store: 4,
            },
            Self::K5 => X86FloatCosts {
                load: 6,
                exchange: 2,
                store: 6,
                convert_store: 7,
                add: 5,
                add_memory: 7,
                multiply: 8,
                multiply_memory: 10,
                divide: 56,
                divide_memory: 62,
                control_load: 6,
                control_store: 6,
            },
            Self::K6 => X86FloatCosts {
                load: 6,
                exchange: 2,
                store: 4,
                convert_store: 6,
                add: 2,
                add_memory: 8,
                multiply: 2,
                multiply_memory: 8,
                divide: 56,
                divide_memory: 62,
                control_load: 6,
                control_store: 4,
            },
            Self::K7 => X86FloatCosts {
                load: 4,
                exchange: 2,
                store: 6,
                convert_store: 12,
                add: 4,
                add_memory: 8,
                multiply: 4,
                multiply_memory: 8,
                divide: 24,
                divide_memory: 28,
                control_load: 4,
                control_store: 6,
            },
            Self::Core => X86FloatCosts {
                load: 6,
                exchange: 0,
                store: 6,
                convert_store: 12,
                add: 3,
                add_memory: 9,
                multiply: 5,
                multiply_memory: 11,
                divide: 24,
                divide_memory: 30,
                control_load: 6,
                control_store: 6,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{X86Cpu, X86FloatCosts};

    #[test]
    fn float_costs_match_the_python_oracle_for_every_profile_and_form() {
        for (cpu, expected) in [
            (
                X86Cpu::I386,
                X86FloatCosts {
                    load: 8,
                    exchange: 18,
                    store: 8,
                    convert_store: 35,
                    add: 23,
                    add_memory: 31,
                    multiply: 27,
                    multiply_memory: 35,
                    divide: 88,
                    divide_memory: 96,
                    control_load: 8,
                    control_store: 8,
                },
            ),
            (
                X86Cpu::I486,
                X86FloatCosts {
                    load: 8,
                    exchange: 4,
                    store: 8,
                    convert_store: 35,
                    add: 8,
                    add_memory: 16,
                    multiply: 16,
                    multiply_memory: 24,
                    divide: 73,
                    divide_memory: 81,
                    control_load: 8,
                    control_store: 8,
                },
            ),
            (
                X86Cpu::P5,
                X86FloatCosts {
                    load: 2,
                    exchange: 1,
                    store: 4,
                    convert_store: 7,
                    add: 3,
                    add_memory: 5,
                    multiply: 3,
                    multiply_memory: 5,
                    divide: 39,
                    divide_memory: 41,
                    control_load: 2,
                    control_store: 4,
                },
            ),
            (
                X86Cpu::P6,
                X86FloatCosts {
                    load: 2,
                    exchange: 0,
                    store: 4,
                    convert_store: 7,
                    add: 3,
                    add_memory: 5,
                    multiply: 5,
                    multiply_memory: 7,
                    divide: 56,
                    divide_memory: 58,
                    control_load: 2,
                    control_store: 4,
                },
            ),
            (
                X86Cpu::K5,
                X86FloatCosts {
                    load: 6,
                    exchange: 2,
                    store: 6,
                    convert_store: 7,
                    add: 5,
                    add_memory: 7,
                    multiply: 8,
                    multiply_memory: 10,
                    divide: 56,
                    divide_memory: 62,
                    control_load: 6,
                    control_store: 6,
                },
            ),
            (
                X86Cpu::K6,
                X86FloatCosts {
                    load: 6,
                    exchange: 2,
                    store: 4,
                    convert_store: 6,
                    add: 2,
                    add_memory: 8,
                    multiply: 2,
                    multiply_memory: 8,
                    divide: 56,
                    divide_memory: 62,
                    control_load: 6,
                    control_store: 4,
                },
            ),
            (
                X86Cpu::K7,
                X86FloatCosts {
                    load: 4,
                    exchange: 2,
                    store: 6,
                    convert_store: 12,
                    add: 4,
                    add_memory: 8,
                    multiply: 4,
                    multiply_memory: 8,
                    divide: 24,
                    divide_memory: 28,
                    control_load: 4,
                    control_store: 6,
                },
            ),
            (
                X86Cpu::Core,
                X86FloatCosts {
                    load: 6,
                    exchange: 0,
                    store: 6,
                    convert_store: 12,
                    add: 3,
                    add_memory: 9,
                    multiply: 5,
                    multiply_memory: 11,
                    divide: 24,
                    divide_memory: 30,
                    control_load: 6,
                    control_store: 6,
                },
            ),
        ] {
            assert_eq!(cpu.float_costs(), expected, "{cpu:?}");
        }
    }
}
