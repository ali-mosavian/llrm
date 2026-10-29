//! A platform row (`machines/<name>.toml`): where the program loads and where the imaginary
//! registers live in zero page.

pub const C64: &str = include_str!("machines/c64.toml");
/// The ld65 memory layout of a C64 PRG.
pub const C64_LD65: &str = include_str!("machines/c64.cfg");
pub const SIM65: &str = include_str!("machines/sim65.toml");

/// Every platform row; the stubs are marked `status = "stub"` in their file.
pub const ROWS: [&str; 6] = [
    C64,
    SIM65,
    include_str!("machines/apple2.toml"),
    include_str!("machines/atari8.toml"),
    include_str!("machines/nes.toml"),
    include_str!("machines/bbc.toml"),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Platform {
    pub name: String,
    pub load_address: u16,
    /// The zero-page address of `rc0`.
    pub imag_base: u8,
}

impl Platform {
    pub fn parse(text: &str) -> Result<Platform, String> {
        let doc: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
        let table = doc.get("platform").and_then(|t| t.as_table()).ok_or("no [platform]")?;
        let number = |key: &str| {
            table.get(key).and_then(|v| v.as_integer()).ok_or_else(|| format!("platform.{key} is not a number"))
        };
        Ok(Platform {
            name: table.get("name").and_then(|v| v.as_str()).ok_or("platform.name is not a string")?.to_owned(),
            load_address: u16::try_from(number("load_address")?).map_err(|_| "load_address is not 16 bits")?,
            imag_base: u8::try_from(number("imag_base")?).map_err(|_| "imag_base is not 8 bits")?,
        })
    }
}

/// What the C64 PRG starts with: the load address, `10 SYS 2061`, and `start`, which calls `_main`,
/// shows its result in the top-left screen cell and stops.
pub fn c64_startup() -> String {
    ".segment \"LOADADDR\"\n.word $0801\n\
     .segment \"BASIC\"\n.word next\n.word 10\n.byte $9E, \"2061\", 0\nnext:\n.word 0\n\
     .segment \"CODE\"\n.export start\nstart:\n  jsr _main\n  sta $0400\nhalt:\n  jmp halt\n"
        .to_owned()
}
