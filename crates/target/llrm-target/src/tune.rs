//! A CPU's prices, from the family's `machine/tune/<family>/` files: one file
//! a CPU, named by its gcc `-march`. A target's `timings.times` reads a row
//! from here where its cell is `=` (`Timings::parse_tuned`).

/// x86's CPUs: `-march` name and file text.
pub const X86: &[(&str, &str)] = &[
    ("i386", include_str!("../../../../machine/tune/x86/i386.toml")),
    ("i486", include_str!("../../../../machine/tune/x86/i486.toml")),
    ("pentium", include_str!("../../../../machine/tune/x86/pentium.toml")),
    ("pentiumpro", include_str!("../../../../machine/tune/x86/pentiumpro.toml")),
    ("k5", include_str!("../../../../machine/tune/x86/k5.toml")),
    ("k6", include_str!("../../../../machine/tune/x86/k6.toml")),
    ("athlon", include_str!("../../../../machine/tune/x86/athlon.toml")),
    ("core2", include_str!("../../../../machine/tune/x86/core2.toml")),
    ("x86-64", include_str!("../../../../machine/tune/x86/x86-64.toml")),
];
