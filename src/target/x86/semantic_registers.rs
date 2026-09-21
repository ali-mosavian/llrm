//! Register-root aliases for the direct Python machine-semantics port.
//!
//! This is the target-owned counterpart of `qbopt.model.ir:root`.  Generic
//! machine semantics carries opaque `PhysicalRegister` values and does not
//! import this x86 alias table.

use crate::codegen::machine::PhysicalRegister;

use super::X86Register;

/// The 32-bit root of an x86 general-purpose register view.
///
/// Direct port of `qbopt.model.ir:root`: direct machine semantics carries a
/// `PhysicalRegister`, so this public boundary takes and returns that same
/// value. Unknown target IDs pass through unchanged.
pub fn root(register: PhysicalRegister) -> PhysicalRegister {
    X86Register::from_physical(register)
        .map(root_register)
        .map(X86Register::physical)
        .unwrap_or(register)
}

/// The target-only alias table. Segment/x87 registers have no GPR root.
const fn root_register(register: X86Register) -> X86Register {
    match register {
        X86Register::Al | X86Register::Ah | X86Register::Ax | X86Register::Eax => X86Register::Eax,
        X86Register::Cl | X86Register::Ch | X86Register::Cx | X86Register::Ecx => X86Register::Ecx,
        X86Register::Dl | X86Register::Dh | X86Register::Dx | X86Register::Edx => X86Register::Edx,
        X86Register::Bl | X86Register::Bh | X86Register::Bx | X86Register::Ebx => X86Register::Ebx,
        X86Register::Sp | X86Register::Esp => X86Register::Esp,
        X86Register::Bp | X86Register::Ebp => X86Register::Ebp,
        X86Register::Si | X86Register::Esi => X86Register::Esi,
        X86Register::Di | X86Register::Edi => X86Register::Edi,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_every_python_root_family_and_preserves_unknown_ids() {
        for (rooted, expected) in [
            (X86Register::Al, X86Register::Eax),
            (X86Register::Ah, X86Register::Eax),
            (X86Register::Ax, X86Register::Eax),
            (X86Register::Eax, X86Register::Eax),
            (X86Register::Bl, X86Register::Ebx),
            (X86Register::Bh, X86Register::Ebx),
            (X86Register::Bx, X86Register::Ebx),
            (X86Register::Ebx, X86Register::Ebx),
            (X86Register::Cl, X86Register::Ecx),
            (X86Register::Ch, X86Register::Ecx),
            (X86Register::Cx, X86Register::Ecx),
            (X86Register::Ecx, X86Register::Ecx),
            (X86Register::Dl, X86Register::Edx),
            (X86Register::Dh, X86Register::Edx),
            (X86Register::Dx, X86Register::Edx),
            (X86Register::Edx, X86Register::Edx),
            (X86Register::Sp, X86Register::Esp),
            (X86Register::Esp, X86Register::Esp),
            (X86Register::Bp, X86Register::Ebp),
            (X86Register::Ebp, X86Register::Ebp),
            (X86Register::Si, X86Register::Esi),
            (X86Register::Esi, X86Register::Esi),
            (X86Register::Di, X86Register::Edi),
            (X86Register::Edi, X86Register::Edi),
        ] {
            assert_eq!(root(rooted.physical()), expected.physical());
        }
        assert_eq!(root(PhysicalRegister::new(99)), PhysicalRegister::new(99));
    }
}
