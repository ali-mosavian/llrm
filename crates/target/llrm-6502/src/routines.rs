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

/// The compact variant of the runtime routine `name`, called at size, if it has one.
pub fn compact_variant(name: &str) -> Option<&'static str> {
    static TABLE: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    TABLE
        .get_or_init(|| {
            let doc: toml::Table = ROUTINES.parse().expect("routines.toml is well formed");
            doc.get("compact")
                .and_then(|t| t.as_table())
                .expect("a [compact] table")
                .iter()
                .map(|(fast, compact)| (fast.clone(), compact.as_str().expect("a routine name").to_owned()))
                .collect()
        })
        .iter()
        .find(|(fast, _)| fast == name)
        .map(|(_, compact)| compact.as_str())
}

/// The aligned variant of the compact divide `name`, called where the module's divides pay for it.
pub fn aligned_variant(name: &str) -> Option<&'static str> {
    static TABLE: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    TABLE
        .get_or_init(|| {
            let doc: toml::Table = ROUTINES.parse().expect("routines.toml is well formed");
            doc.get("aligned")
                .and_then(|t| t.as_table())
                .expect("an [aligned] table")
                .iter()
                .map(|(compact, aligned)| (compact.clone(), aligned.as_str().expect("a routine name").to_owned()))
                .collect()
        })
        .iter()
        .find(|(compact, _)| compact == name)
        .map(|(_, aligned)| aligned.as_str())
}

/// What an aligned divide call saves over a compact one, in clocks (`[divide]`).
pub fn aligned_divide_saving() -> u64 {
    let doc: toml::Table = ROUTINES.parse().expect("routines.toml is well formed");
    let table = doc.get("divide").and_then(|t| t.as_table()).expect("a [divide] table");
    let number = |key: &str| {
        table.get(key).and_then(|v| v.as_integer()).and_then(|v| u64::try_from(v).ok()).unwrap_or_else(|| panic!("divide.{key} is not a count"))
    };
    number("compact_clocks").saturating_sub(number("aligned_clocks"))
}

thread_local! {
    static ALIGNED_DIVIDES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the compact divides are the aligned ones for this compile; set by the driver.
pub fn set_aligned_divides(on: bool) {
    ALIGNED_DIVIDES.with(|f| f.set(on));
}

pub fn aligned_divides() -> bool {
    ALIGNED_DIVIDES.with(|f| f.get())
}
