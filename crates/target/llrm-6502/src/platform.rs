//! A platform row (`machines/<name>.toml`): where the program loads and where
//! the imaginary registers live in zero page.

pub const C64: &str = include_str!("machines/c64.toml");
/// The ld65 memory layout of a C64 PRG.
pub const C64_LD65: &str = include_str!("machines/c64.cfg");
/// The ld65 memory layout of an Apple II DOS 3.3 binary.
pub const APPLE2_LD65: &str = include_str!("machines/apple2.cfg");
/// The ld65 memory layout of an NES NROM image.
pub const NES_LD65: &str = include_str!("machines/nes.cfg");
/// The ld65 memory layout of an Atari 8-bit XEX file.
pub const ATARI8_LD65: &str = include_str!("machines/atari8.cfg");
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
    /// The zero-page bytes from `imag_base` the program may use as registers:
    /// `rc0`..`rc15` and scratch cells above them (`rc16`.., at most 26
    /// bytes). 16 where a row does not say.
    pub zero_page: u8,
    /// Where the soft stack (`rs0`) starts; it grows down.
    pub soft_stack: u16,
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
            zero_page: match table.get("zero_page") {
                None => 16,
                Some(_) => u8::try_from(number("zero_page")?).map_err(|_| "zero_page is not 8 bits")?,
            },
            soft_stack: u16::try_from(number("soft_stack")?).map_err(|_| "soft_stack is not 16 bits")?,
        })
    }
}

/// What the C64 PRG starts with: the load address, `10 SYS 2061`, and `start`,
/// which sets the soft stack pointer, calls `_main`, shows its result in the
/// top-left screen cell and stops.
pub fn c64_startup(platform: &Platform) -> String {
    format!(
        ".segment \"LOADADDR\"\n.word $0801\n\
         .segment \"BASIC\"\n.word next\n.word 10\n.byte $9E, \"2061\", 0\nnext:\n.word 0\n\
         .segment \"CODE\"\n.export start\nstart:\n  lda #${:02X}\n  sta {sp_low}\n  lda #${:02X}\n  sta {sp_high}\n  jsr _main\n  sta $0400\nhalt:\n  jmp halt\n",
        platform.soft_stack & 0xFF,
        platform.soft_stack >> 8,
        sp_low = crate::target::imaginary::name(0),
        sp_high = crate::target::imaginary::name(1)
    )
}

/// What an Apple II binary starts with: the DOS 3.3 header (load address and
/// length), then `start`, which sets the soft stack pointer, calls `_main` and
/// shows its result in the top-left cell of text page 1 (`$0400`, the same cell
/// as on the C64).
pub fn apple2_startup(platform: &Platform) -> String {
    format!(
        ".import __MAIN_START__, __MAIN_LAST__\n.segment \"HEADER\"\n.word __MAIN_START__\n.word __MAIN_LAST__ - __MAIN_START__\n\
         .segment \"CODE\"\n.export start\nstart:\n  lda #${:02X}\n  sta {sp_low}\n  lda #${:02X}\n  sta {sp_high}\n  jsr _main\n  sta $0400\nhalt:\n  jmp halt\n",
        platform.soft_stack & 0xFF,
        platform.soft_stack >> 8,
        sp_low = crate::target::imaginary::name(0),
        sp_high = crate::target::imaginary::name(1)
    )
}

/// What an NES image starts with: the iNES header (2 x 16K PRG, 1 x 8K CHR,
/// mapper 0), then `start`, the reset vector, which sets the soft stack
/// pointer, calls `_main` and keeps its result in `$0000`. `nmi` and `irq` are
/// `rti`.
pub fn nes_startup(platform: &Platform) -> String {
    format!(
        ".segment \"HEADER\"\n.byte \"NES\", $1A, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0\n\
         .segment \"CODE\"\n.export start\nstart:\n  sei\n  cld\n  ldx #$FF\n  txs\n  lda #${:02X}\n  sta {sp_low}\n  lda #${:02X}\n  sta {sp_high}\n  jsr _main\n  sta $00\nhalt:\n  jmp halt\n\
         nmi:\n  rti\nirq:\n  rti\n\
         .segment \"VECTORS\"\n.word nmi, start, irq\n.segment \"CODE\"\n",
        platform.soft_stack & 0xFF,
        platform.soft_stack >> 8,
        sp_low = crate::target::imaginary::name(0),
        sp_high = crate::target::imaginary::name(1)
    )
}

/// What an Atari XEX file starts with: `$FFFF`, the load chunk header (start
/// and inclusive end), then `start`, which sets the soft stack pointer, calls
/// `_main` and keeps its result in `$0600`; the RUNAD chunk (`$02E0`) at the
/// end points at `start`.
pub fn atari8_startup(platform: &Platform) -> String {
    format!(
        ".import __MAIN_START__, __MAIN_LAST__\n.segment \"HEADER\"\n.word $FFFF\n\
         .segment \"MAINHDR\"\n.word __MAIN_START__\n.word __MAIN_LAST__ - 1\n\
         .segment \"CODE\"\n.export start, halt\nstart:\n  lda #${:02X}\n  sta {sp_low}\n  lda #${:02X}\n  sta {sp_high}\n  jsr _main\n  sta $0600\nhalt:\n  jmp halt\n\
         .segment \"TRAILER\"\n.word $02E0, $02E1, start\n.segment \"CODE\"\n",
        platform.soft_stack & 0xFF,
        platform.soft_stack >> 8,
        sp_low = crate::target::imaginary::name(0),
        sp_high = crate::target::imaginary::name(1)
    )
}
