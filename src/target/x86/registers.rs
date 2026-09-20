//! Physical registers for the 16/32-bit x86 target.
//!
//! This module names architectural registers and their overlapping views.  It
//! deliberately does not decide which registers an ABI reserves: ABI code can
//! select a subset of [`X86RegisterClass::allocation_order`] without changing
//! the architecture's alias model.

use crate::codegen::machine::{PhysicalRegister, RegisterClass};

/// An architectural x86 register view available on an i386-class processor.
///
/// The numeric representation is stable within llrm's initial x86 target and
/// is only exposed to target-independent Machine IR through
/// [`X86Register::physical`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum X86Register {
    Al = 1,
    Cl,
    Dl,
    Bl,
    Ah,
    Ch,
    Dh,
    Bh,

    Ax,
    Cx,
    Dx,
    Bx,
    Sp,
    Bp,
    Si,
    Di,

    Eax,
    Ecx,
    Edx,
    Ebx,
    Esp,
    Ebp,
    Esi,
    Edi,

    Es,
    Cs,
    Ss,
    Ds,
    Fs,
    Gs,

    St0,
    St1,
    St2,
    St3,
    St4,
    St5,
    St6,
    St7,
}

/// A target-defined class of physical x86 register views.
///
/// `Address16` is intentionally the four registers the 16-bit ModR/M
/// addressing forms can name: `BX`, `BP`, `SI`, and `DI`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum X86RegisterClass {
    Byte = 1,
    Word,
    Dword,
    Address16,
    Segment,
    X87,
}

const BYTE_REGISTERS: [X86Register; 8] = [
    X86Register::Al,
    X86Register::Cl,
    X86Register::Dl,
    X86Register::Bl,
    X86Register::Ah,
    X86Register::Ch,
    X86Register::Dh,
    X86Register::Bh,
];

const WORD_REGISTERS: [X86Register; 8] = [
    X86Register::Ax,
    X86Register::Cx,
    X86Register::Dx,
    X86Register::Bx,
    X86Register::Sp,
    X86Register::Bp,
    X86Register::Si,
    X86Register::Di,
];

const DWORD_REGISTERS: [X86Register; 8] = [
    X86Register::Eax,
    X86Register::Ecx,
    X86Register::Edx,
    X86Register::Ebx,
    X86Register::Esp,
    X86Register::Ebp,
    X86Register::Esi,
    X86Register::Edi,
];

const ADDRESS16_REGISTERS: [X86Register; 4] = [
    X86Register::Bx,
    X86Register::Bp,
    X86Register::Si,
    X86Register::Di,
];

const SEGMENT_REGISTERS: [X86Register; 6] = [
    X86Register::Es,
    X86Register::Cs,
    X86Register::Ss,
    X86Register::Ds,
    X86Register::Fs,
    X86Register::Gs,
];

const X87_REGISTERS: [X86Register; 8] = [
    X86Register::St0,
    X86Register::St1,
    X86Register::St2,
    X86Register::St3,
    X86Register::St4,
    X86Register::St5,
    X86Register::St6,
    X86Register::St7,
];

const BYTE_ALLOCATION_ORDER: [X86Register; 8] = BYTE_REGISTERS;
const WORD_ALLOCATION_ORDER: [X86Register; 7] = [
    X86Register::Ax,
    X86Register::Cx,
    X86Register::Dx,
    X86Register::Bx,
    X86Register::Si,
    X86Register::Di,
    X86Register::Bp,
];
const DWORD_ALLOCATION_ORDER: [X86Register; 7] = [
    X86Register::Eax,
    X86Register::Ecx,
    X86Register::Edx,
    X86Register::Ebx,
    X86Register::Esi,
    X86Register::Edi,
    X86Register::Ebp,
];
const ADDRESS16_ALLOCATION_ORDER: [X86Register; 4] = [
    X86Register::Bx,
    X86Register::Si,
    X86Register::Di,
    X86Register::Bp,
];
const X87_ALLOCATION_ORDER: [X86Register; 8] = X87_REGISTERS;

const NO_REGISTERS: [X86Register; 0] = [];

const AL_ALIASES: [X86Register; 2] = [X86Register::Ax, X86Register::Eax];
const CL_ALIASES: [X86Register; 2] = [X86Register::Cx, X86Register::Ecx];
const DL_ALIASES: [X86Register; 2] = [X86Register::Dx, X86Register::Edx];
const BL_ALIASES: [X86Register; 2] = [X86Register::Bx, X86Register::Ebx];
const AH_ALIASES: [X86Register; 2] = [X86Register::Ax, X86Register::Eax];
const CH_ALIASES: [X86Register; 2] = [X86Register::Cx, X86Register::Ecx];
const DH_ALIASES: [X86Register; 2] = [X86Register::Dx, X86Register::Edx];
const BH_ALIASES: [X86Register; 2] = [X86Register::Bx, X86Register::Ebx];
const AX_ALIASES: [X86Register; 3] = [X86Register::Al, X86Register::Ah, X86Register::Eax];
const CX_ALIASES: [X86Register; 3] = [X86Register::Cl, X86Register::Ch, X86Register::Ecx];
const DX_ALIASES: [X86Register; 3] = [X86Register::Dl, X86Register::Dh, X86Register::Edx];
const BX_ALIASES: [X86Register; 3] = [X86Register::Bl, X86Register::Bh, X86Register::Ebx];
const SP_ALIASES: [X86Register; 1] = [X86Register::Esp];
const BP_ALIASES: [X86Register; 1] = [X86Register::Ebp];
const SI_ALIASES: [X86Register; 1] = [X86Register::Esi];
const DI_ALIASES: [X86Register; 1] = [X86Register::Edi];
const EAX_ALIASES: [X86Register; 3] = [X86Register::Al, X86Register::Ah, X86Register::Ax];
const ECX_ALIASES: [X86Register; 3] = [X86Register::Cl, X86Register::Ch, X86Register::Cx];
const EDX_ALIASES: [X86Register; 3] = [X86Register::Dl, X86Register::Dh, X86Register::Dx];
const EBX_ALIASES: [X86Register; 3] = [X86Register::Bl, X86Register::Bh, X86Register::Bx];
const ESP_ALIASES: [X86Register; 1] = [X86Register::Sp];
const EBP_ALIASES: [X86Register; 1] = [X86Register::Bp];
const ESI_ALIASES: [X86Register; 1] = [X86Register::Si];
const EDI_ALIASES: [X86Register; 1] = [X86Register::Di];

const EAX_FAMILY: [X86Register; 4] = [
    X86Register::Al,
    X86Register::Ah,
    X86Register::Ax,
    X86Register::Eax,
];
const ECX_FAMILY: [X86Register; 4] = [
    X86Register::Cl,
    X86Register::Ch,
    X86Register::Cx,
    X86Register::Ecx,
];
const EDX_FAMILY: [X86Register; 4] = [
    X86Register::Dl,
    X86Register::Dh,
    X86Register::Dx,
    X86Register::Edx,
];
const EBX_FAMILY: [X86Register; 4] = [
    X86Register::Bl,
    X86Register::Bh,
    X86Register::Bx,
    X86Register::Ebx,
];
const ESP_FAMILY: [X86Register; 2] = [X86Register::Sp, X86Register::Esp];
const EBP_FAMILY: [X86Register; 2] = [X86Register::Bp, X86Register::Ebp];
const ESI_FAMILY: [X86Register; 2] = [X86Register::Si, X86Register::Esi];
const EDI_FAMILY: [X86Register; 2] = [X86Register::Di, X86Register::Edi];
const ES_FAMILY: [X86Register; 1] = [X86Register::Es];
const CS_FAMILY: [X86Register; 1] = [X86Register::Cs];
const SS_FAMILY: [X86Register; 1] = [X86Register::Ss];
const DS_FAMILY: [X86Register; 1] = [X86Register::Ds];
const FS_FAMILY: [X86Register; 1] = [X86Register::Fs];
const GS_FAMILY: [X86Register; 1] = [X86Register::Gs];
const ST0_FAMILY: [X86Register; 1] = [X86Register::St0];
const ST1_FAMILY: [X86Register; 1] = [X86Register::St1];
const ST2_FAMILY: [X86Register; 1] = [X86Register::St2];
const ST3_FAMILY: [X86Register; 1] = [X86Register::St3];
const ST4_FAMILY: [X86Register; 1] = [X86Register::St4];
const ST5_FAMILY: [X86Register; 1] = [X86Register::St5];
const ST6_FAMILY: [X86Register; 1] = [X86Register::St6];
const ST7_FAMILY: [X86Register; 1] = [X86Register::St7];

impl X86RegisterClass {
    /// Every target register class in stable numeric order.
    pub const ALL: [Self; 6] = [
        Self::Byte,
        Self::Word,
        Self::Dword,
        Self::Address16,
        Self::Segment,
        Self::X87,
    ];

    /// The opaque target-independent Machine IR class identifier.
    pub const fn machine_class(self) -> RegisterClass {
        RegisterClass::new(self as u32)
    }

    /// Recovers a target class from its Machine IR identifier.
    pub fn from_machine_class(class: RegisterClass) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.machine_class() == class)
    }

    /// All architectural views that can satisfy this class.
    pub const fn members(self) -> &'static [X86Register] {
        match self {
            Self::Byte => &BYTE_REGISTERS,
            Self::Word => &WORD_REGISTERS,
            Self::Dword => &DWORD_REGISTERS,
            Self::Address16 => &ADDRESS16_REGISTERS,
            Self::Segment => &SEGMENT_REGISTERS,
            Self::X87 => &X87_REGISTERS,
        }
    }

    /// A deterministic, conservative allocation preference.
    ///
    /// Stack-pointer views do not appear in the general-purpose orders.  The
    /// segment class has no generic order because assigning a segment register
    /// is ABI- and instruction-specific; an x87 allocator may use the stack
    /// register order directly.
    pub const fn allocation_order(self) -> &'static [X86Register] {
        match self {
            Self::Byte => &BYTE_ALLOCATION_ORDER,
            Self::Word => &WORD_ALLOCATION_ORDER,
            Self::Dword => &DWORD_ALLOCATION_ORDER,
            Self::Address16 => &ADDRESS16_ALLOCATION_ORDER,
            Self::Segment => &NO_REGISTERS,
            Self::X87 => &X87_ALLOCATION_ORDER,
        }
    }
}

impl X86Register {
    /// Every architectural register view in stable numeric order.
    pub const ALL: [Self; 38] = [
        Self::Al,
        Self::Cl,
        Self::Dl,
        Self::Bl,
        Self::Ah,
        Self::Ch,
        Self::Dh,
        Self::Bh,
        Self::Ax,
        Self::Cx,
        Self::Dx,
        Self::Bx,
        Self::Sp,
        Self::Bp,
        Self::Si,
        Self::Di,
        Self::Eax,
        Self::Ecx,
        Self::Edx,
        Self::Ebx,
        Self::Esp,
        Self::Ebp,
        Self::Esi,
        Self::Edi,
        Self::Es,
        Self::Cs,
        Self::Ss,
        Self::Ds,
        Self::Fs,
        Self::Gs,
        Self::St0,
        Self::St1,
        Self::St2,
        Self::St3,
        Self::St4,
        Self::St5,
        Self::St6,
        Self::St7,
    ];

    /// The opaque target-independent Machine IR register identifier.
    pub const fn physical(self) -> PhysicalRegister {
        PhysicalRegister::new(self as u32)
    }

    /// Recovers an x86 view from its Machine IR register identifier.
    pub fn from_physical(register: PhysicalRegister) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.physical() == register)
    }

    /// Architectural views which directly overlap this view, excluding self.
    ///
    /// Direct overlap is deliberately not transitive: `AL` and `AH` occupy
    /// disjoint bytes, though each overlaps `AX`.  Use [`Self::alias_family`]
    /// when an allocator needs the transitive register family instead.
    pub const fn aliases(self) -> &'static [Self] {
        match self {
            Self::Al => &AL_ALIASES,
            Self::Cl => &CL_ALIASES,
            Self::Dl => &DL_ALIASES,
            Self::Bl => &BL_ALIASES,
            Self::Ah => &AH_ALIASES,
            Self::Ch => &CH_ALIASES,
            Self::Dh => &DH_ALIASES,
            Self::Bh => &BH_ALIASES,
            Self::Ax => &AX_ALIASES,
            Self::Cx => &CX_ALIASES,
            Self::Dx => &DX_ALIASES,
            Self::Bx => &BX_ALIASES,
            Self::Sp => &SP_ALIASES,
            Self::Bp => &BP_ALIASES,
            Self::Si => &SI_ALIASES,
            Self::Di => &DI_ALIASES,
            Self::Eax => &EAX_ALIASES,
            Self::Ecx => &ECX_ALIASES,
            Self::Edx => &EDX_ALIASES,
            Self::Ebx => &EBX_ALIASES,
            Self::Esp => &ESP_ALIASES,
            Self::Ebp => &EBP_ALIASES,
            Self::Esi => &ESI_ALIASES,
            Self::Edi => &EDI_ALIASES,
            Self::Es
            | Self::Cs
            | Self::Ss
            | Self::Ds
            | Self::Fs
            | Self::Gs
            | Self::St0
            | Self::St1
            | Self::St2
            | Self::St3
            | Self::St4
            | Self::St5
            | Self::St6
            | Self::St7 => &NO_REGISTERS,
        }
    }

    /// The complete architectural register family containing this view.
    ///
    /// This relation is reflexive, symmetric, and transitive.  It is useful
    /// to an allocator that cannot represent independent low/high-byte lanes.
    pub const fn alias_family(self) -> &'static [Self] {
        match self {
            Self::Al | Self::Ah | Self::Ax | Self::Eax => &EAX_FAMILY,
            Self::Cl | Self::Ch | Self::Cx | Self::Ecx => &ECX_FAMILY,
            Self::Dl | Self::Dh | Self::Dx | Self::Edx => &EDX_FAMILY,
            Self::Bl | Self::Bh | Self::Bx | Self::Ebx => &EBX_FAMILY,
            Self::Sp | Self::Esp => &ESP_FAMILY,
            Self::Bp | Self::Ebp => &EBP_FAMILY,
            Self::Si | Self::Esi => &ESI_FAMILY,
            Self::Di | Self::Edi => &EDI_FAMILY,
            Self::Es => &ES_FAMILY,
            Self::Cs => &CS_FAMILY,
            Self::Ss => &SS_FAMILY,
            Self::Ds => &DS_FAMILY,
            Self::Fs => &FS_FAMILY,
            Self::Gs => &GS_FAMILY,
            Self::St0 => &ST0_FAMILY,
            Self::St1 => &ST1_FAMILY,
            Self::St2 => &ST2_FAMILY,
            Self::St3 => &ST3_FAMILY,
            Self::St4 => &ST4_FAMILY,
            Self::St5 => &ST5_FAMILY,
            Self::St6 => &ST6_FAMILY,
            Self::St7 => &ST7_FAMILY,
        }
    }

    /// Whether two views share at least one architectural bit.
    pub fn overlaps(self, other: Self) -> bool {
        match (self.root(), other.root()) {
            (Some(left), Some(right)) if left == right => self.lanes() & other.lanes() != 0,
            _ => false,
        }
    }

    /// The immediate wider architectural view, if one exists.
    pub const fn super_register(self) -> Option<Self> {
        match self {
            Self::Al | Self::Ah => Some(Self::Ax),
            Self::Cl | Self::Ch => Some(Self::Cx),
            Self::Dl | Self::Dh => Some(Self::Dx),
            Self::Bl | Self::Bh => Some(Self::Bx),
            Self::Ax => Some(Self::Eax),
            Self::Cx => Some(Self::Ecx),
            Self::Dx => Some(Self::Edx),
            Self::Bx => Some(Self::Ebx),
            Self::Sp => Some(Self::Esp),
            Self::Bp => Some(Self::Ebp),
            Self::Si => Some(Self::Esi),
            Self::Di => Some(Self::Edi),
            Self::Eax
            | Self::Ecx
            | Self::Edx
            | Self::Ebx
            | Self::Esp
            | Self::Ebp
            | Self::Esi
            | Self::Edi
            | Self::Es
            | Self::Cs
            | Self::Ss
            | Self::Ds
            | Self::Fs
            | Self::Gs
            | Self::St0
            | Self::St1
            | Self::St2
            | Self::St3
            | Self::St4
            | Self::St5
            | Self::St6
            | Self::St7 => None,
        }
    }

    /// The immediate narrower architectural views, in deterministic order.
    pub const fn sub_registers(self) -> &'static [Self] {
        match self {
            Self::Ax => &[Self::Al, Self::Ah],
            Self::Cx => &[Self::Cl, Self::Ch],
            Self::Dx => &[Self::Dl, Self::Dh],
            Self::Bx => &[Self::Bl, Self::Bh],
            Self::Eax => &[Self::Ax],
            Self::Ecx => &[Self::Cx],
            Self::Edx => &[Self::Dx],
            Self::Ebx => &[Self::Bx],
            Self::Esp => &[Self::Sp],
            Self::Ebp => &[Self::Bp],
            Self::Esi => &[Self::Si],
            Self::Edi => &[Self::Di],
            Self::Al
            | Self::Cl
            | Self::Dl
            | Self::Bl
            | Self::Ah
            | Self::Ch
            | Self::Dh
            | Self::Bh
            | Self::Sp
            | Self::Bp
            | Self::Si
            | Self::Di
            | Self::Es
            | Self::Cs
            | Self::Ss
            | Self::Ds
            | Self::Fs
            | Self::Gs
            | Self::St0
            | Self::St1
            | Self::St2
            | Self::St3
            | Self::St4
            | Self::St5
            | Self::St6
            | Self::St7 => &NO_REGISTERS,
        }
    }

    const fn root(self) -> Option<Self> {
        match self {
            Self::Al | Self::Ah | Self::Ax | Self::Eax => Some(Self::Eax),
            Self::Cl | Self::Ch | Self::Cx | Self::Ecx => Some(Self::Ecx),
            Self::Dl | Self::Dh | Self::Dx | Self::Edx => Some(Self::Edx),
            Self::Bl | Self::Bh | Self::Bx | Self::Ebx => Some(Self::Ebx),
            Self::Sp | Self::Esp => Some(Self::Esp),
            Self::Bp | Self::Ebp => Some(Self::Ebp),
            Self::Si | Self::Esi => Some(Self::Esi),
            Self::Di | Self::Edi => Some(Self::Edi),
            Self::Es
            | Self::Cs
            | Self::Ss
            | Self::Ds
            | Self::Fs
            | Self::Gs
            | Self::St0
            | Self::St1
            | Self::St2
            | Self::St3
            | Self::St4
            | Self::St5
            | Self::St6
            | Self::St7 => Some(self),
        }
    }

    const fn lanes(self) -> u8 {
        match self {
            Self::Al | Self::Cl | Self::Dl | Self::Bl => 0b0001,
            Self::Ah | Self::Ch | Self::Dh | Self::Bh => 0b0010,
            Self::Ax
            | Self::Cx
            | Self::Dx
            | Self::Bx
            | Self::Sp
            | Self::Bp
            | Self::Si
            | Self::Di => 0b0011,
            Self::Eax
            | Self::Ecx
            | Self::Edx
            | Self::Ebx
            | Self::Esp
            | Self::Ebp
            | Self::Esi
            | Self::Edi
            | Self::Es
            | Self::Cs
            | Self::Ss
            | Self::Ds
            | Self::Fs
            | Self::Gs
            | Self::St0
            | Self::St1
            | Self::St2
            | Self::St3
            | Self::St4
            | Self::St5
            | Self::St6
            | Self::St7 => u8::MAX,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{X86Register, X86RegisterClass};

    #[test]
    fn direct_aliases_are_symmetric() {
        for register in all_registers() {
            for alias in register.aliases() {
                assert!(
                    alias.aliases().contains(&register),
                    "{alias:?} does not directly alias {register:?}"
                );
            }
        }
    }

    #[test]
    fn alias_families_are_transitive() {
        for register in all_registers() {
            let family = register.alias_family();
            assert!(family.contains(&register));
            for member in family {
                assert_eq!(member.alias_family(), family);
            }
        }
        assert!(!X86Register::Al.overlaps(X86Register::Ah));
        assert!(X86Register::Al.overlaps(X86Register::Ax));
        assert!(X86Register::Ah.overlaps(X86Register::Eax));
    }

    #[test]
    fn address16_class_has_exact_modrm_members() {
        assert_eq!(
            X86RegisterClass::Address16.members(),
            [
                X86Register::Bx,
                X86Register::Bp,
                X86Register::Si,
                X86Register::Di,
            ]
        );
    }

    #[test]
    fn allocation_order_is_deterministic() {
        assert_eq!(
            X86RegisterClass::Dword.allocation_order(),
            [
                X86Register::Eax,
                X86Register::Ecx,
                X86Register::Edx,
                X86Register::Ebx,
                X86Register::Esi,
                X86Register::Edi,
                X86Register::Ebp,
            ]
        );
        assert_eq!(
            X86RegisterClass::Dword.allocation_order(),
            X86RegisterClass::Dword.allocation_order()
        );
    }

    fn all_registers() -> impl Iterator<Item = X86Register> {
        [
            X86RegisterClass::Byte.members(),
            X86RegisterClass::Word.members(),
            X86RegisterClass::Dword.members(),
            X86RegisterClass::Segment.members(),
            X86RegisterClass::X87.members(),
        ]
        .into_iter()
        .flatten()
        .copied()
    }
}
