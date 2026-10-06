//! Real-mode DOS as a machine: the built-in description (`dos.toml`), the
//! BASIC runtime's variant of it and the processors it prices. The type and its
//! parser are `llrm-target`'s.

use std::sync::LazyLock;

pub use llrm_target::machine::*;

pub const DOS: &str = include_str!("machines/dos.toml");

/// The built-in description, which nothing can change.
pub static BUILT_IN: LazyLock<Machine> = LazyLock::new(|| Machine::parse(DOS, &CPUS).expect("the built-in DOS description parses"));

/// The built-in description as a BASIC runtime runs it: compiled code only
/// ever runs on the program's stack, which is in the data group.
pub static BASIC: LazyLock<Machine> =
    LazyLock::new(|| Machine { segments: Segments { stack_is_data: true, ..BUILT_IN.segments.clone() }, ..BUILT_IN.clone() });

/// The processors a description may name. `llrm-core` checks that the
/// backend prices exactly these.
pub const CPUS: [&str; 8] = ["386", "486", "P5", "P6", "K5", "K6", "K7", "Core"];

#[cfg(test)]
mod tests {
    use super::*;

    /// Only VGA was foreign, so a PEEK or POKE of the BIOS data area or the
    /// ROM counted as reaching every global.
    #[test]
    fn test_the_bios_and_dos_areas_below_program_data_and_the_rom_above_are_foreign() {
        let dos = Machine::parse(DOS, &CPUS).unwrap();
        let every = (0, 0xFFFF);
        assert_eq!(dos.foreign_span((0, 0), (0x46C, 0x46C), 4), Some((0x46C, 0x470)));
        assert_eq!(dos.foreign_span((0x40, 0x40), (0x17, 0x17), 1), Some((0x417, 0x418)));
        assert_eq!(dos.foreign_span((0x6F, 0x6F), (0xF, 0xF), 1), Some((0x6FF, 0x700)));
        assert_eq!(dos.foreign_span((0x70, 0x70), (0, 0), 1), None);
        assert_eq!(dos.foreign_span((0xF000, 0xF000), every, 1), Some((0xF0000, 0x100000)));
        assert_eq!(dos.foreign_span((0xFFFF, 0xFFFF), every, 1), Some((0xFFFF0, 0x10FFF0)));
        assert_eq!(dos.foreign_span((0xE000, 0xE000), (0, 0), 1), None);
    }

    #[test]
    fn test_vga_selectors_are_foreign_only_in_real_mode() {
        let dos = Machine::parse(DOS, &CPUS).unwrap();
        let every = (0, 0xFFFF);
        assert_eq!(dos.foreign_span((0xA000, 0xAF8C), every, 1), Some((0xA0000, 0xAF8C0 + 0x1_0000)));
        assert_eq!(dos.foreign_span((0xA000, 0xB801), every, 1), None);
        assert_eq!(dos.foreign_span((0x9FFF, 0xA000), every, 1), None);
        assert_eq!(dos.foreign_span((0x9FFF, 0x9FFF), (0x10, 0x11), 2), Some((0xA0000, 0xA0003)));
        assert_eq!(dos.foreign_span((0xB800, 0xB800), every, 1), Some((0xB8000, 0xC8000)));
        assert_eq!(dos.foreign_span((0xB801, 0xB801), every, 1), None);
        let protected = Machine { addressing: Addressing::Protected, ..dos };
        assert_eq!(protected.foreign_span((0xA000, 0xA000), every, 1), None);
    }

    #[test]
    fn test_a_vga_register_touches_no_memory_a_dma_port_may_and_an_unlisted_port_may_touch_any() {
        let dos = Machine::parse(DOS, &CPUS).unwrap();
        assert_eq!(dos.port_memory((0x3C4, 0x3C5)), PortMemory::None);
        assert_eq!(dos.port_memory((0x3C0, 0x3DF)), PortMemory::None);
        assert_eq!(dos.port_memory((0x0B, 0x0B)), PortMemory::Dma);
        assert_eq!(dos.port_memory((0x3C9, 0x3F0)), PortMemory::Any);
        assert_eq!(dos.port_memory((0x3F0, 0x3F8)), PortMemory::Dma);
        assert_eq!(dos.port_memory((0x300, 0x300)), PortMemory::Any);
    }
    /// A word read at an odd offset may be the one at FFFFh, which faults on
    /// a 286 or later; an aligned one, or a byte, never traps in real mode.
    #[test]
    fn test_only_an_access_that_may_cross_offset_ffff_traps() {
        let dos = Machine::parse(DOS, &CPUS).unwrap();
        assert!(!dos.access_may_trap(1, 1) && !dos.access_may_trap(2, 2) && !dos.access_may_trap(4, 4) && !dos.access_may_trap(2, 8));
        assert!(dos.access_may_trap(2, 1) && dos.access_may_trap(4, 2) && dos.access_may_trap(10, 8));
        let wrapping = Machine { segment_end_faults: false, ..dos.clone() };
        assert!(!wrapping.access_may_trap(4, 1));
        let protected = Machine { addressing: Addressing::Protected, ..dos };
        assert!(protected.access_may_trap(1, 1));
    }
}
