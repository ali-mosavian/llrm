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

pub fn parse(text: &str) -> Result<Vec<Register>, String> {
    let mut registers = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
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
            "    table[{id}::{} as usize] = Some(llrm_lir::registers::Entry {{ id: {id}::{}, name: {:?}, bits: {}, root: {id}::{}, lane: {}, classes: {mask} }});\n",
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
            [one] => Ok(format!("{id}::{}", one.name.to_uppercase())),
            _ => {
                Err(format!("registers.regs: {} registers are the root with the class `{class}`, not one", roots.len()))
            }
        }
    };
    let loads: Vec<String> = registers
        .iter()
        .filter_map(|one| one.load.as_ref().map(|load| format!("({id}::{}, {load:?})", one.name.to_uppercase())))
        .collect();
    let optional = |class: &str| -> Result<String, String> {
        let roots: Vec<&Register> = registers.iter().filter(|one| one.is(class)).collect();
        match roots[..] {
            [] => Ok("None".to_owned()),
            [one] => Ok(format!("Some({id}::{})", one.name.to_uppercase())),
            _ => Err(format!("registers.regs: {} registers have the class `{class}`, not one", roots.len())),
        }
    };
    let columns: Vec<String> = (0..WIDTH_COLUMNS).map(|at| widths.get(at).copied().unwrap_or(0).to_string()).collect();
    // Each root at each width: the first register by iced's number.
    code.push_str(&format!(
        "    let loads: &'static [({id}, &'static str)] = &[{}];\n    let widths: [u32; {WIDTH_COLUMNS}] = [{}];\n    let mut views: [[Option<{id}>; {WIDTH_COLUMNS}]; 256] = [[None; {WIDTH_COLUMNS}]; 256];\n    let mut at = 0;\n    while at < 256 {{\n        if let Some(entry) = table[at] {{\n            let mut column = 0;\n            while column < {WIDTH_COLUMNS} {{\n                let root = entry.root as usize;\n                if widths[column] == entry.bits && views[root][column].is_none() {{\n                    views[root][column] = Some(entry.id);\n                }}\n                column += 1;\n            }}\n        }}\n        at += 1;\n    }}\n    llrm_lir::registers::Info {{ table, frame: {}, stack: {}, loads, data_segment: {}, stack_segment: {}, code_segment: {}, far_segment: {}, widths, views }}\n}};\n",
        loads.join(", "),
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
        let error = source(text, "Reg", &CLASSES).err().expect("refused");
        assert!(error.contains("al has the class `byte`"), "{error}");
        let made =
            source("ebp 32 ebp 0 frame - 3\nesp 32 esp 0 stack - 4\neax 32 eax 0 gpr,int - 1\n", "Reg", &CLASSES)
                .expect("made");
        assert!(made.contains("Reg::EAX as usize"), "{made}");
        assert!(made.contains("llrm_lir::registers::class::GPR | llrm_lir::registers::class::INT"), "{made}");
    }

    /// Two frame registers (or none) are no role: the table is not made.
    #[test]
    fn a_role_needs_exactly_one_root() {
        let one = "ebp 32 ebp 0 frame,stack - 1\n";
        assert!(source(one, "Reg", &["frame", "stack"]).is_ok());
        let two = "ebp 32 ebp 0 frame,stack - 1\nesi 32 esi 0 frame - 2\n";
        let error = source(two, "Reg", &["frame", "stack"]).err().expect("refused");
        assert!(error.contains("2 registers are the root with the class `frame`"), "{error}");
        let none = "eax 32 eax 0 - - 1\n";
        assert!(source(none, "Reg", &["frame", "stack"]).err().expect("refused").contains("0 registers"));
    }

    /// Two registers meaning the same segment is no role: the table is not
    /// made.
    #[test]
    fn a_segment_role_is_one_register_or_none() {
        let base = "ebp 32 ebp 0 frame - 1\nesp 32 esp 0 stack - 2\n";
        let classes = ["frame", "stack", "data_segment"];
        assert!(
            source(&format!("{base}ds 16 ds 0 data_segment - 3\n"), "Reg", &classes)
                .unwrap()
                .contains("data_segment: Some(Reg::DS)")
        );
        assert!(source(base, "Reg", &classes).unwrap().contains("data_segment: None"));
        let two = format!("{base}ds 16 ds 0 data_segment - 3\nes 16 es 0 data_segment - 4\n");
        assert!(
            source(&two, "Reg", &classes).err().expect("refused").contains("2 registers have the class `data_segment`")
        );
    }

    /// A far load names the segment register it fills; a row without one has
    /// none.
    #[test]
    fn a_load_is_the_eighth_column() {
        let text = "ebp 32 ebp 0 frame - 1\nesp 32 esp 0 stack - 2\nes 16 es 0 - - 3 les\nds 16 ds 0 - - 4\n";
        let made = source(text, "Reg", &["frame", "stack"]).unwrap();
        assert!(made.contains("(Reg::ES, \"les\")") && !made.contains("(Reg::DS"), "{made}");
        assert_eq!(
            crate::registers::parse("a 8 a 0 - - 1 x y").unwrap_err(),
            "registers.regs:1: 9 columns, not 7 or 8"
        );
    }
}
