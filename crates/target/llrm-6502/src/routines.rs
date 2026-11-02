//! `machines/routines.toml`: what a call of a runtime routine costs in clocks,
//! for pricing the same function with the copy a call and a loop.

pub const ROUTINES: &str = include_str!("machines/routines.toml");

/// The prices of a copy as a call, and the worth of a byte against them.
pub struct CopyPrices {
    /// What a call of `memcpy` adds to the `jsr`, for a copy of unknown length.
    pub call_clocks: u64,
    /// The clocks a byte of code is worth, at speed and at size.
    pub byte_clocks: (u64, u64),
}

pub fn copy_prices() -> CopyPrices {
    let doc: toml::Table = ROUTINES.parse().expect("routines.toml is well formed");
    let table = doc.get("copy").and_then(|t| t.as_table()).expect("a [copy] table");
    let number = |key: &str| {
        table
            .get(key)
            .and_then(|v| v.as_integer())
            .and_then(|v| u64::try_from(v).ok())
            .unwrap_or_else(|| panic!("copy.{key} is not a count"))
    };
    CopyPrices {
        call_clocks: number("entry_clocks") + number("typical_bytes") * number("clocks_per_byte"),
        byte_clocks: (number("byte_clocks_speed"), number("byte_clocks_size")),
    }
}
