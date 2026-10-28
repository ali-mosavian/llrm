//! A target's register file, from its `registers.regs`: one register a line,
//! `name bits root lane classes dwarf codeview [load]`, classes comma separated
//! and `-` for none; dwarf and codeview are the register's number in each debug
//! format, `-` where it has none; `load` is the mnemonic that loads a far
//! pointer's offset and this segment register (`les` for `es`), where there is
//! one.

/// One register: a root, or a view of one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Register {
    pub name: String,
    pub bits: u32,
    /// The full register this is a view of (itself for a root).
    pub root: String,
    /// The bit offset inside the root.
    pub lane: u32,
    pub classes: Vec<String>,
    /// Its DWARF register number, where the psABI gives it one.
    pub dwarf: Option<u16>,
    /// Its CodeView register id (`CV_HREG_e`).
    pub codeview: Option<u16>,
    /// The far-pointer load that fills this segment register.
    pub load: Option<String>,
}

impl Register {
    pub fn is(
        &self,
        class: &str,
    ) -> bool {
        self.classes.iter().any(|one| one == class)
    }
}

/// A number, or `-` for none.
fn optional(text: &str) -> Result<Option<u16>, String> {
    if text == "-" { Ok(None) } else { text.parse().map(Some).map_err(|_| text.to_owned()) }
}

/// The scratch order a `@scratch cx dx bx ax` line states: the registers a pass
/// may borrow for a moment, most preferred first (gcc's `REG_ALLOC_ORDER`).
pub fn scratch(text: &str) -> Vec<String> {
    order(text, "@scratch")
}

/// The 8-bit allocation order a `@bytes al cl dl ah ch dh bl bh` line states
/// (the GR8 class of X86RegisterInfo.td): the high bytes are in it.
pub fn bytes(text: &str) -> Vec<String> {
    order(text, "@bytes")
}

fn order(
    text: &str,
    key: &str,
) -> Vec<String> {
    text.lines()
        .filter_map(|raw| raw.split('#').next().unwrap_or("").trim().strip_prefix(key))
        .flat_map(|names| names.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
        .collect()
}

/// The registers an `@asm ax bx ..` line names: the ones inline asm may name in
/// a register constraint (`{ax}`), the 16-bit convention's.
pub fn asm_names(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|raw| raw.split('#').next().unwrap_or("").trim().strip_prefix("@asm"))
        .flat_map(|names| names.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
        .collect()
}

thread_local! {
    static PARSED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many register files this thread has parsed, for a test that a compile
/// does not parse one per block.
pub fn parsed() -> usize {
    PARSED.with(std::cell::Cell::get)
}

pub fn parse(text: &str) -> Result<Vec<Register>, String> {
    PARSED.with(|parsed| parsed.set(parsed.get() + 1));
    let mut registers = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() || line.starts_with('@') {
            continue;
        }
        let columns: Vec<&str> = line.split_whitespace().collect();
        let (name, bits, root, lane, classes, dwarf, codeview, load) = match columns[..] {
            [name, bits, root, lane, classes, dwarf, codeview] => {
                (name, bits, root, lane, classes, dwarf, codeview, None)
            }
            [name, bits, root, lane, classes, dwarf, codeview, load] => {
                (name, bits, root, lane, classes, dwarf, codeview, Some(load.to_owned()))
            }
            _ => return Err(format!("registers.regs:{}: {} columns, not 7 or 8", index + 1, columns.len())),
        };
        let number =
            |text: &str, what: &str| text.parse().map_err(|_| format!("registers.regs:{}: {what} `{text}`", index + 1));
        registers.push(Register {
            name: name.to_owned(),
            bits: number(bits, "bits")?,
            root: root.to_owned(),
            lane: number(lane, "lane")?,
            classes: if classes == "-" { Vec::new() } else { classes.split(',').map(str::to_owned).collect() },
            dwarf: optional(dwarf).map_err(|text| format!("registers.regs:{}: dwarf `{text}`", index + 1))?,
            codeview: optional(codeview).map_err(|text| format!("registers.regs:{}: codeview `{text}`", index + 1))?,
            load,
        });
    }
    Ok(registers)
}

/// How many registers hold values: the roots of class `gpr`.
pub fn allocatable(registers: &[Register]) -> usize {
    registers.iter().filter(|one| one.is("gpr") && one.root == one.name).count()
}

/// The roots of class `class`, in file order.
pub fn of_class<'a>(
    registers: &'a [Register],
    class: &str,
) -> Vec<&'a str> {
    registers.iter().filter(|one| one.is(class) && one.root == one.name).map(|one| one.name.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_roots_of_class_gpr_hold_values() {
        let text = "# a file\neax 32 eax 0 gpr,byte,base 0 17\nebp 32 ebp 0 reserved,frame 5 22\nax 16 eax 0 - 0 9\nesi 32 esi 0 gpr,base 6 23\n";
        let registers = parse(text).unwrap();
        assert_eq!(registers.len(), 4);
        assert_eq!(allocatable(&registers), 2);
        assert!(registers[0].is("byte") && registers[2].classes.is_empty());
        assert_eq!(parse("eax 32 eax").unwrap_err(), "registers.regs:1: 3 columns, not 7 or 8");
        assert_eq!((registers[0].dwarf, registers[0].codeview), (Some(0), Some(17)));
    }

    /// A register with no number in a format says so with `-`, and a word that
    /// is no number is an error, not a register with none.
    #[test]
    fn a_dash_is_no_number_and_a_word_is_an_error() {
        let none = parse("ah 8 eax 8 - - 5").unwrap();
        assert_eq!((none[0].dwarf, none[0].codeview), (None, Some(5)));
        assert_eq!(parse("ah 8 eax 8 - x 5").unwrap_err(), "registers.regs:1: dwarf `x`");
    }
}

/// The Rust source of a target's `REGISTER_INFO`: its register file as an
/// `llrm_lir::registers::Info`, from the text of its `registers.regs`. `id` is
/// the type of a register's id (`iced_x86::Register`), whose variants the
/// descriptions' names are; `classes` the class names
/// `llrm_lir::registers::class` has a bit for, a class not among them is
/// refused.
pub fn source(
    text: &str,
    id: &str,
    classes: &[&str],
    effects: &str,
) -> Result<String, String> {
    source_with(text, id, id, classes, effects)
}

/// `source`, for a target whose register constants (`X0`, `SP`) live in its
/// own module `names` and not on the id type `id`: the target numbers its file.
pub fn source_with(
    text: &str,
    id: &str,
    names: &str,
    classes: &[&str],
    effects: &str,
) -> Result<String, String> {
    let registers = parse(text)?;
    let mut widths: Vec<u32> = registers.iter().map(|one| one.bits).collect();
    widths.sort_unstable();
    widths.dedup();
    if widths.len() > WIDTH_COLUMNS {
        return Err(format!(
            "registers.regs: {} register widths, the table has room for {WIDTH_COLUMNS}",
            widths.len()
        ));
    }
    let mut code = String::from(
        "pub static REGISTER_INFO: llrm_lir::registers::Info = {\n    let mut table: [Option<llrm_lir::registers::Entry>; 256] = [None; 256];\n",
    );
    for one in &registers {
        let mut mask = Vec::new();
        for class in &one.classes {
            if !classes.contains(&class.as_str()) {
                return Err(format!(
                    "registers.regs: {} has the class `{class}`, which llrm_lir::registers has no bit for",
                    one.name
                ));
            }
            mask.push(format!("llrm_lir::registers::class::{}", class.to_uppercase()));
        }
        let mask = if mask.is_empty() { "0".to_owned() } else { mask.join(" | ") };
        code.push_str(&format!(
            "    table[{names}::{}.index()] = Some(llrm_lir::registers::Entry {{ id: {names}::{}, name: {:?}, bits: {}, root: {names}::{}, lane: {}, classes: {mask} }});\n",
            one.name.to_uppercase(),
            one.name.to_uppercase(),
            one.name,
            one.bits,
            one.root.to_uppercase(),
            one.lane
        ));
    }
    // The roles: the root the description gives the class `frame`, and `stack`.
    let role = |class: &str| -> Result<String, String> {
        let roots: Vec<&Register> = registers.iter().filter(|one| one.is(class) && one.root == one.name).collect();
        match roots[..] {
            [one] => Ok(format!("{names}::{}", one.name.to_uppercase())),
            _ => {
                Err(format!("registers.regs: {} registers are the root with the class `{class}`, not one", roots.len()))
            }
        }
    };
    let loads: Vec<String> = registers
        .iter()
        .filter_map(|one| one.load.as_ref().map(|load| format!("({names}::{}, {load:?})", one.name.to_uppercase())))
        .collect();
    let scratch: Vec<String> = scratch(text).iter().map(|name| format!("{names}::{}", name.to_uppercase())).collect();
    let bytes: Vec<String> = bytes(text).iter().map(|name| format!("{names}::{}", name.to_uppercase())).collect();
    let optional = |class: &str| -> Result<String, String> {
        let roots: Vec<&Register> = registers.iter().filter(|one| one.is(class)).collect();
        match roots[..] {
            [] => Ok("None".to_owned()),
            [one] => Ok(format!("Some({names}::{})", one.name.to_uppercase())),
            _ => Err(format!("registers.regs: {} registers have the class `{class}`, not one", roots.len())),
        }
    };
    let columns: Vec<String> = (0..WIDTH_COLUMNS).map(|at| widths.get(at).copied().unwrap_or(0).to_string()).collect();
    // Each root at each width: the first register by iced's number.
    code.push_str(&format!(
        "    let loads: &'static [({id}, &'static str)] = &[{}];\n    let scratch: &'static [{id}] = &[{}];\n    let bytes: &'static [{id}] = &[{}];\n    let widths: [u32; {WIDTH_COLUMNS}] = [{}];\n    let mut views: [[Option<{id}>; {WIDTH_COLUMNS}]; 256] = [[None; {WIDTH_COLUMNS}]; 256];\n    let mut at = 0;\n    while at < 256 {{\n        if let Some(entry) = table[at] {{\n            let mut column = 0;\n            while column < {WIDTH_COLUMNS} {{\n                let root = entry.root.index();\n                if widths[column] == entry.bits && views[root][column].is_none() {{\n                    views[root][column] = Some(entry.id);\n                }}\n                column += 1;\n            }}\n        }}\n        at += 1;\n    }}\n    llrm_lir::registers::Info {{ table, frame: {}, stack: {}, loads, scratch, bytes, data_segment: {}, stack_segment: {}, code_segment: {}, far_segment: {}, widths, effects: {effects}, cache: llrm_lir::registers::Cache::new(), views }}\n}};\n",
        loads.join(", "),
        scratch.join(", "),
        bytes.join(", "),
        columns.join(", "),
        role("frame")?,
        role("stack")?,
        optional("data_segment")?,
        optional("stack_segment")?,
        optional("code_segment")?,
        optional("far_segment")?
    ));
    Ok(code)
}

/// The id constants of a target that numbers its own file: `pub const X0: Reg
/// = Reg::from_raw(1);`, one per register, numbered in the file's order from 1
/// (0 is no register).
pub fn constants(
    text: &str,
    id: &str,
) -> Result<String, String> {
    let registers = parse(text)?;
    if registers.len() > 255 {
        return Err(format!("registers.regs: {} registers, ids are one byte and 0 is none", registers.len()));
    }
    let mut code = String::new();
    for (at, one) in registers.iter().enumerate() {
        code.push_str(&format!("pub const {}: {id} = {id}::from_raw({});\n", one.name.to_uppercase(), at + 1));
    }
    Ok(code)
}

/// The most register widths a description may state.
const WIDTH_COLUMNS: usize = 8;

#[cfg(test)]
mod source_tests {
    use super::source;

    const CLASSES: [&str; 4] = ["gpr", "int", "frame", "stack"];

    /// A class the table has no bit for would be dropped without a word: the
    /// generated mask would miss it and a query for it answer no.
    #[test]
    fn a_class_without_a_bit_is_refused_where_the_table_is_made() {
        let text =
            "ebp 32 ebp 0 frame - 3\nesp 32 esp 0 stack - 4\neax 32 eax 0 gpr,int - 1\nal 8 eax 0 int,byte - 2\n";
        let error = source(text, "Reg", &CLASSES, "rows").err().expect("refused");
        assert!(error.contains("al has the class `byte`"), "{error}");
        let made = source(
            "ebp 32 ebp 0 frame - 3\nesp 32 esp 0 stack - 4\neax 32 eax 0 gpr,int - 1\n",
            "Reg",
            &CLASSES,
            "rows",
        )
        .expect("made");
        assert!(made.contains("Reg::EAX.index()"), "{made}");
        assert!(made.contains("llrm_lir::registers::class::GPR | llrm_lir::registers::class::INT"), "{made}");
    }

    /// A target whose constants are not on the id type (AArch64's `X0` is no
    /// iced register) names them in its own module; the tables still hold ids.
    #[test]
    fn a_target_names_its_constants_in_its_own_module() {
        let text = "fp 64 fp 0 frame - 29\nsp 64 sp 0 stack - 31\n";
        let made = super::source_with(text, "Reg", "regs", &CLASSES, "rows").expect("made");
        assert!(made.contains("regs::FP.index()"), "{made}");
        assert!(made.contains("&'static [Reg] ="), "{made}");
        assert!(!made.contains("Reg::FP"), "{made}");
    }

    /// A target that numbers its own file gets one constant per register, in
    /// the file's order, and none for more than a byte holds.
    #[test]
    fn a_target_numbers_its_registers_from_one() {
        let made = super::constants("fp 64 fp 0 frame - 29\nsp 64 sp 0 stack - 31\n", "Reg").expect("made");
        assert_eq!(made, "pub const FP: Reg = Reg::from_raw(1);\npub const SP: Reg = Reg::from_raw(2);\n");
        let many: String = (0..256).map(|at| format!("r{at} 64 r{at} 0 gpr - -\n")).collect();
        assert!(super::constants(&many, "Reg").is_err());
    }

    /// Two frame registers (or none) are no role: the table is not made.
    #[test]
    fn a_role_needs_exactly_one_root() {
        let one = "ebp 32 ebp 0 frame,stack - 1\n";
        assert!(source(one, "Reg", &["frame", "stack"], "rows").is_ok());
        let two = "ebp 32 ebp 0 frame,stack - 1\nesi 32 esi 0 frame - 2\n";
        let error = source(two, "Reg", &["frame", "stack"], "rows").err().expect("refused");
        assert!(error.contains("2 registers are the root with the class `frame`"), "{error}");
        let none = "eax 32 eax 0 - - 1\n";
        assert!(source(none, "Reg", &["frame", "stack"], "rows").err().expect("refused").contains("0 registers"));
    }

    /// Two registers meaning the same segment is no role: the table is not
    /// made.
    #[test]
    fn a_segment_role_is_one_register_or_none() {
        let base = "ebp 32 ebp 0 frame - 1\nesp 32 esp 0 stack - 2\n";
        let classes = ["frame", "stack", "data_segment"];
        assert!(
            source(&format!("{base}ds 16 ds 0 data_segment - 3\n"), "Reg", &classes, "rows")
                .unwrap()
                .contains("data_segment: Some(Reg::DS)")
        );
        assert!(source(base, "Reg", &classes, "rows").unwrap().contains("data_segment: None"));
        let two = format!("{base}ds 16 ds 0 data_segment - 3\nes 16 es 0 data_segment - 4\n");
        assert!(
            source(&two, "Reg", &classes, "rows")
                .err()
                .expect("refused")
                .contains("2 registers have the class `data_segment`")
        );
    }

    /// A far load names the segment register it fills; a row without one has
    /// none.
    #[test]
    fn a_load_is_the_eighth_column() {
        let text = "ebp 32 ebp 0 frame - 1\nesp 32 esp 0 stack - 2\nes 16 es 0 - - 3 les\nds 16 ds 0 - - 4\n";
        let made = source(text, "Reg", &["frame", "stack"], "rows").unwrap();
        assert!(made.contains("(Reg::ES, \"les\")") && !made.contains("(Reg::DS"), "{made}");
        assert_eq!(
            crate::registers::parse("a 8 a 0 - - 1 x y").unwrap_err(),
            "registers.regs:1: 9 columns, not 7 or 8"
        );
    }

    /// A `@scratch` line is an order, not a register row.
    #[test]
    fn a_scratch_line_is_an_order_not_a_register() {
        let text = "@scratch cx ax\nebp 32 ebp 0 frame - 1\nesp 32 esp 0 stack - 2\n";
        assert_eq!(crate::registers::scratch(text), ["cx", "ax"]);
        assert!(crate::registers::bytes(text).is_empty());
        assert_eq!(crate::registers::parse(text).unwrap().len(), 2);
        assert!(source(text, "Reg", &["frame", "stack"], "rows").unwrap().contains("&[Reg::CX, Reg::AX]"));
    }

    /// The GR8 order with the high bytes is the description's `@bytes` line;
    /// without one the register file has no byte order.
    #[test]
    fn a_bytes_line_is_the_byte_order() {
        let text = "@bytes al ah\nebp 32 ebp 0 frame - 1\nesp 32 esp 0 stack - 2\n";
        assert_eq!(crate::registers::bytes(text), ["al", "ah"]);
        assert!(
            source(text, "Reg", &["frame", "stack"], "rows")
                .unwrap()
                .contains("bytes: &'static [Reg] = &[Reg::AL, Reg::AH]")
        );
    }
}
