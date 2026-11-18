//! The runtime's scratch cells (`__sa`, `__st`..) were BSS: three-byte
//! addresses in every routine that works in them (`__mulsi3` 83 B where 65 B
//! do, `__rt_udivmod32` 144 B where 107 B do). A platform row that has the
//! bytes gives them to zero page.

use llrm_6502::platform::{Platform, ROWS};

#[test]
fn the_rows_with_room_give_the_runtime_its_scratch_and_the_full_ones_do_not() {
    for row in ROWS {
        let platform = Platform::parse(row).expect("a row");
        let room = platform.name.starts_with("sim65") || platform.name == "atari8";
        assert_eq!(platform.scratch_zero_page >= 15, room, "{}", platform.name);
    }
}

/// The scratch sits after the registers: it must end inside the zero page.
#[test]
fn the_scratch_ends_inside_zero_page() {
    for row in ROWS {
        let platform = Platform::parse(row).expect("a row");
        if platform.scratch_zero_page > 0 {
            let end = u32::from(platform.imag_base) + u32::from(platform.zero_page) + 15;
            assert!(end <= 0x100, "{}: ends at {end:#x}", platform.name);
        }
    }
}

/// The registers, the runtime's scratch and the static cells share zero page:
/// together they end by $FF.
#[test]
fn the_registers_the_scratch_and_the_static_cells_end_inside_zero_page() {
    for row in ROWS {
        let platform = Platform::parse(row).expect("a row");
        if platform.static_zero_page > 0 {
            let end = u32::from(platform.imag_base)
                + u32::from(platform.zero_page)
                + u32::from(platform.scratch_zero_page)
                + u32::from(platform.static_zero_page);
            assert!(end <= 0x100, "{}: ends at {end:#x}", platform.name);
            assert!(platform.scratch_zero_page > 0, "{}: static cells follow the scratch", platform.name);
        }
    }
}
