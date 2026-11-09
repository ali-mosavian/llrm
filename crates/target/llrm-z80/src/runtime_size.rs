//! The bytes of the runtime routines a module reaches: a routine is linked
//! once however many calls name it, with the routines it calls or falls into.
//! The table is `runtime/sizes.csv`, written by `runtime/sizes.py` from the
//! assembler listing of `z80_rt.s` (a size is the distance to the next global
//! symbol, as the link map gives it).

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::expand::Line;

/// One runtime routine: its bytes and the routines it calls or falls into.
struct Routine {
    bytes: u32,
    calls: Vec<&'static str>,
}

static TABLE: LazyLock<HashMap<&'static str, Routine>> = LazyLock::new(|| {
    include_str!("../runtime/sizes.csv")
        .lines()
        .map(|row| {
            let mut cells = row.split(',');
            let name = cells.next().expect("a name");
            let bytes = cells.next().and_then(|cell| cell.parse().ok()).expect("a size");
            let calls = cells.next().unwrap_or("").split(';').filter(|one| !one.is_empty()).collect();
            (name, Routine { bytes, calls })
        })
        .collect()
});

/// The symbol a `call` or `jp` of `text` names.
fn target(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("call ").or_else(|| text.strip_prefix("jp "))?;
    rest.rsplit(',').next().map(str::trim)
}

/// `__mulsi3`'s clocks, the call instruction through the return, measured in ucsim by `runtime/clocks.py`: the
/// routine shifts the multiplier right once per pass until it is spent and adds on each set bit, so the multiplicand
/// does not matter. A product by 100 (7 bits, 3 set) is 1094.
const MULSI3_BASE: u32 = 167;
const MULSI3_PASS: u32 = 120;
const MULSI3_ADD: u32 = 29;

/// The clocks of `call __mulsi3` whose multiplier is `multiplier`; the loop runs once for a zero multiplier.
pub fn mulsi3_clocks(multiplier: u32) -> u32 {
    MULSI3_BASE + MULSI3_PASS * (u32::BITS - multiplier.leading_zeros()).max(1) + MULSI3_ADD * multiplier.count_ones()
}

/// The multiplier assumed for a `call __mulsi3` whose multiplier is not a constant in sight: a typical 16-bit value.
const TYPICAL_MULTIPLIER: u32 = 0xA5A5;

/// The count assumed for a long shift routine whose count is not a constant in sight.
const TYPICAL_SHIFT: u32 = 8;

/// The long shift routines' clocks, the call instruction through the return, linear in the count (`runtime/clocks.py`).
const ASHLSI3_BASE: u32 = 59;
const ASHLSI3_PER: u32 = 40;
const LSHRSI3_BASE: u32 = 59;
const LSHRSI3_PER: u32 = 45;
const ASHRSI3_BASE: u32 = 59;
const ASHRSI3_PER: u32 = 45;

/// The long divide and remainder routines' clocks, the call instruction through the return (`runtime/clocks.py`):
/// (routine, base, per step). The speed family does a step per quotient bit, `base + per * max(0, na - nb)` with `na`
/// and `nb` the bit lengths of dividend and divisor (a divide by 10 of a 24-bit value is 6.6k T); the size family
/// does 32 steps whatever the operands, one constant. The step cost depends on the quotient bits, so the model is
/// within 13% (speed) and 8% (size) of the measured clocks, not exact.
const DIVIDES: [(&str, u32, u32); 12] = [
    ("__udivsi3", 1091, 322),
    ("__umodsi3", 1087, 322),
    ("__divsi3", 1153, 329),
    ("__modsi3", 1134, 329),
    ("__udivremsi4", 1159, 322),
    ("__divremsi4", 1289, 334),
    ("__s_udivsi3", 6126, 0),
    ("__s_umodsi3", 6130, 0),
    ("__s_divsi3", 6341, 0),
    ("__s_modsi3", 6329, 0),
    ("__s_udivremsi4", 6292, 0),
    ("__s_divremsi4", 6466, 0),
];

/// The dividend and divisor bit lengths assumed for a long divide whose operand is not a constant in sight.
const TYPICAL_DIVIDEND_BITS: u32 = 24;
const TYPICAL_DIVISOR_BITS: u32 = 8;

/// The clocks a `call` adds beyond the call instruction itself: the routine's body, where the runtime has a model
/// for it. `before` are the instruction texts that precede the call in its function, nearest last. `__mulsi3`
/// takes the multiplier from the last write of `bc` before the call when that is `ld bc, #n` (the high word is
/// zero for a constant under 65536, the common case); the long shifts take the count likewise from `a`; any
/// other write, or none in sight, leaves a typical operand.
pub fn routine_clocks(
    text: &str,
    before: &[&str],
) -> u32 {
    const CALL: u32 = 17;
    let Some(routine) = text.strip_prefix("call ").map(str::trim) else { return 0 };
    // the nearest instruction that touches the register decides: a constant only if it loads one, else unknown
    let immediate = |register: &str| {
        let prefix = format!("ld {register}, #");
        let (halves, implicit): (&[&str], &[&str]) = if register == "bc" {
            (&["bc", "b", "c"], &["exx", "ldir", "lddr", "ldi", "ldd", "cpir", "cpdr", "cpi", "cpd", "djnz"])
        } else {
            (&["a"], &["rrca", "rlca", "rra", "rla", "cpl", "daa", "neg", "exx"])
        };
        before
            .iter()
            .rev()
            .take(10)
            .take_while(|one| !one.starts_with("call ") && !one.starts_with("jp ") && !one.starts_with("jr "))
            .find(|one| {
                one.starts_with(prefix.as_str())
                    || {
                        let (mnemonic, operands) = one.split_once(' ').unwrap_or((one, ""));
                        let written = operands.split(',').next().unwrap_or("").trim();
                        implicit.contains(&mnemonic) || (mnemonic != "push" && halves.contains(&written))
                    }
            })
            .and_then(|one| one.strip_prefix(prefix.as_str())?.trim().parse::<u32>().ok())
    };
    let counted = |base: u32, per: u32| base + per * immediate("a").unwrap_or(TYPICAL_SHIFT) - CALL;
    match routine {
        "__mulsi3" => mulsi3_clocks(immediate("bc").unwrap_or(TYPICAL_MULTIPLIER)) - CALL,
        "__ashlsi3" => counted(ASHLSI3_BASE, ASHLSI3_PER),
        "__lshrsi3" => counted(LSHRSI3_BASE, LSHRSI3_PER),
        "__ashrsi3" => counted(ASHRSI3_BASE, ASHRSI3_PER),
        _ => DIVIDES.iter().find(|(name, ..)| *name == routine).map_or(0, |&(_, base, per)| {
            let divisor_bits = immediate("bc").map_or(TYPICAL_DIVISOR_BITS, |divisor| u32::BITS - divisor.leading_zeros());
            base + per * TYPICAL_DIVIDEND_BITS.saturating_sub(divisor_bits) - CALL
        }),
    }
}

/// The bytes of routine `name` alone, if the runtime has it.
pub fn own_bytes(name: &str) -> Option<u32> {
    TABLE.get(name).map(|routine| routine.bytes)
}

/// The runtime routines the printed `lines` call or jump to.
pub fn referenced(lines: &[Line]) -> HashSet<String> {
    lines
        .iter()
        .filter_map(|line| match line {
            Line::Inst(text) => target(text).filter(|name| TABLE.contains_key(*name)).map(str::to_owned),
            _ => None,
        })
        .collect()
}

/// The bytes of `names` and every routine they reach, each once.
pub fn reached(names: &HashSet<String>) -> u32 {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut work: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut bytes = 0;
    while let Some(name) = work.pop() {
        let Some((key, routine)) = TABLE.get_key_value(name) else { continue };
        if seen.insert(key) {
            bytes += routine.bytes;
            work.extend(routine.calls.iter().copied());
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The price counted `call __mulsi3` as the 17 T call alone: x * 3 is 465 T (ucsim), 448 of them the routine.
    #[test]
    fn a_long_multiply_call_prices_the_routine_by_its_constant_multiplier() {
        assert_eq!(routine_clocks("call __mulsi3", &["push hl", "ld bc, #3"]), 448);
        assert_eq!(routine_clocks("call __mulsi3", &["ld bc, #100", "ld l, c"]), 1094 - 17);
        assert_eq!(routine_clocks("call __mulsi3", &["ld bc, #3", "call _f", "ld hl, #0"]), mulsi3_clocks(TYPICAL_MULTIPLIER) - 17);
        assert_eq!(routine_clocks("call _f", &["ld bc, #3"]), 0);
        // idioms -O1 sum(): `ld bc, #0` is the high word pushed first, bc is then loaded from a variable: the
        // multiplier is unknown and was priced as 0 (167 T) instead of a typical one
        let variable = ["ld bc, #0", "push bc", "ld c, -4(ix)", "ld b, -3(ix)"];
        assert_eq!(routine_clocks("call __mulsi3", &variable), mulsi3_clocks(TYPICAL_MULTIPLIER) - 17);
        assert_eq!(routine_clocks("call __ashlsi3", &["ld a, #5", "xor a, c"]), 59 + 40 * TYPICAL_SHIFT - 17);
        // a long shifted by 5: 59 + 40 * 5 = 259 T for the call (ucsim), 242 of them the routine
        assert_eq!(routine_clocks("call __ashlsi3", &["ld a, #5"]), 59 + 40 * 5 - 17);
        assert_eq!(routine_clocks("call __lshrsi3", &["ld a, #5"]), 59 + 45 * 5 - 17);
    }

    /// The price counted a long divide call as the 17 T call alone: a 32-bit divide by a variable is 6.3k T (ucsim), which
    /// hid what a function that divides in a loop costs (div_constants_long: 33M T measured, 3M priced), so a change
    /// of that function's multiplies looked 42% slower when it is 3.6% of the program.
    #[test]
    fn a_long_divide_call_prices_the_routine_by_the_divisor_bits() {
        assert_eq!(routine_clocks("call __udivsi3", &["ld bc, #10"]), 1091 + 322 * (24 - 4) - 17);
        assert_eq!(routine_clocks("call __modsi3", &["ld bc, #1000", "ld a, #3"]), 1134 + 329 * (24 - 10) - 17);
        assert_eq!(routine_clocks("call __udivsi3", &["ld c, -4(ix)"]), 1091 + 322 * (24 - 8) - 17);
        // a divisor wider than the dividend takes the base alone
        assert_eq!(routine_clocks("call __udivsi3", &["ld bc, #65535", "ld a, #1"]), 1091 + 322 * (24 - 16) - 17);
        assert_eq!(routine_clocks("call __s_divsi3", &["ld bc, #10"]), 6341 - 17);
        assert_eq!(routine_clocks("call __divremsi4", &["ld bc, #10"]), 1289 + 334 * 20 - 17);
    }

    /// Clocks measured in ucsim (runtime/clocks.py): 0 -> 287, 1 -> 316, 100 -> 1094, 0xFFFF -> 2551, 0xFFFFFFFF -> 4935.
    #[test]
    fn the_long_multiply_clocks_follow_the_multiplier() {
        assert_eq!(
            [0, 1, 100, 0xFFFF, 0xFFFF_FFFF].map(mulsi3_clocks),
            [287, 316, 1094, 2551, 4935]
        );
    }

    /// `runtime/clocks.py` measures the routine again and compares the constants above.
    #[test]
    fn the_clock_model_is_the_measured_runtime() {
        let done = std::process::Command::new("python3")
            .args([concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/clocks.py"), "--check"])
            .output();
        if let Ok(done) = done {
            let stderr = String::from_utf8_lossy(&done.stderr).into_owned();
            // no sdasz80 or ucsim on the machine: the check cannot run
            if !stderr.contains("sdasz80") && !stderr.contains("ucsim") {
                assert!(done.status.success(), "{stderr}");
            }
        }
    }

    /// `__mulsi3` assembles to 57 B (sdasz80 listing of `runtime/z80_rt.s`).
    #[test]
    fn the_long_multiply_routine_is_57_bytes() {
        assert_eq!(own_bytes("__mulsi3"), Some(57));
    }

    /// `runtime/sizes.csv` is what `runtime/sizes.py` writes from `z80_rt.s`.
    #[test]
    fn the_size_table_is_the_assembled_runtime() {
        let done = std::process::Command::new("python3")
            .args([concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/sizes.py"), "--check"])
            .output();
        if let Ok(done) = done {
            let stderr = String::from_utf8_lossy(&done.stderr).into_owned();
            // no sdasz80 on the machine: the check cannot run
            if !stderr.contains("sdasz80") {
                assert!(done.status.success(), "{stderr}");
            }
        }
    }
}
