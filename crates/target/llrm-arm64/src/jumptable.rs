//! Jump tables: `JumpTableDest32` expanded to the instructions that read an
//! entry, the table's contents as label differences, and the choice of the
//! entry width (`AArch64CompressJumpTables`).

use llrm_object::table::Entry;

use crate::frame::Lowered;
use crate::mc::inst::{AddSub, Addr, Extend, Gpr, Inst, MemOperand, Memory, Shift, Target, Width};
use crate::mfunc::{MFunc, Op};

/// A jump table of a function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Table {
    /// The data symbol the code addresses (`G$`: a private name).
    pub symbol: String,
    /// The label the entries are relative to: a block, or a label inside the
    /// block of the jump.
    pub base: String,
    /// The label of each entry's block.
    pub entries: Vec<String>,
    pub entry: Entry,
}

impl Table {
    /// Bytes per entry.
    pub fn width(&self) -> usize {
        usize::from(self.entry.width)
    }
}

impl Table {
    /// The table as the assembler reads it: in the constants section, one
    /// directive per entry.
    pub fn assembly(
        &self,
        platform: crate::machine::Platform,
    ) -> String {
        use crate::machine::Platform;
        let mut out = match platform {
            Platform::Darwin => "\t.section\t__TEXT,__const\n".to_owned(),
            Platform::Linux => "\t.section\t.rodata,\"a\",@progbits\n".to_owned(),
        };
        if self.entry.width == 4 {
            out.push_str("\t.p2align\t2\n");
        }
        out.push_str(&format!("{}:\n", self.symbol));
        for entry in &self.entries {
            let line = match (self.entry.width, self.entry.shift) {
                (1, shift) => format!(".byte\t({entry}-{})>>{shift}", self.base),
                (2, shift) => format!(".short\t({entry}-{})>>{shift}", self.base),
                _ => format!(".long\t{entry}-{}", self.base),
            };
            out.push_str(&format!("\t{line}\n"));
        }
        out
    }
}

/// The label inside the block of the jump that the 4-byte entries are relative
/// to (`Ltmp`).
pub fn base_label(
    function: &str,
    id: u32,
) -> String {
    format!(".L{function}.jtb{id}")
}

pub fn symbol(
    function: &str,
    id: u32,
) -> String {
    format!("G$JTI.{function}.{id}")
}

/// `LowerJumpTableDest` for the 4-byte entries: `dst` is the entry's address
/// from the table `table` and the entry `index`, `scratch` the offset read.
pub fn dest32(
    function: &str,
    id: u32,
    dst: Gpr,
    scratch: Gpr,
    table: Gpr,
    index: Gpr,
) -> Vec<Inst> {
    // The entry is read first: `dst` may then take the register of `table` or
    // `index`, which are dead once it is loaded. The pin keeps `adr` first and
    // marks both results early-clobber.
    vec![
        Inst::Load {
            mem: Memory::Int { width: Width::X, bytes: 4, sign: true, reg: scratch },
            addr: Addr::Register { base: table, index, extend: Extend::Uxtx, shifted: true },
            mmo: MemOperand::default(),
        },
        Inst::Label(base_label(function, id)),
        Inst::Adr { page: false, rd: dst, target: Target::Label(base_label(function, id)), got: false },
        Inst::AddSubShifted {
            op: AddSub { subtract: false, set_flags: false },
            width: Width::X,
            rd: dst,
            rn: dst,
            rm: scratch,
            shift: Shift::Lsl,
            amount: 0,
        },
        Inst::BranchReg { link: false, rn: dst },
    ]
}

/// The tables the jumps of `function` read, by table number.
pub fn tables_of(function: &MFunc) -> Vec<Table> {
    let mut out: Vec<(u32, Table)> = Vec::new();
    for op in function.blocks.iter().flat_map(|block| block.ops.iter()) {
        let Op::JumpTable { id, targets, .. } = op else { continue };
        if out.iter().any(|(one, _)| one == id) {
            continue;
        }
        out.push((
            *id,
            Table {
                symbol: symbol(&function.name, *id),
                base: base_label(&function.name, *id),
                entries: targets.clone(),
                entry: Entry { width: 4, shift: 0, signed: true },
            },
        ));
    }
    out.sort_by_key(|(id, _)| *id);
    out.into_iter().map(|(_, table)| table).collect()
}

/// The `Label` the jump of `table` is preceded by, in `blocks`.
fn position(
    blocks: &[Lowered],
    base: &str,
) -> Option<(usize, usize)> {
    blocks.iter().enumerate().find_map(|(b, block)| {
        block.insts.iter().position(|inst| matches!(inst, Inst::Label(name) if name == base)).map(|i| (b, i))
    })
}

/// The offset of each block's label, and of the labels inside the blocks.
pub fn offsets(blocks: &[Lowered]) -> std::collections::HashMap<String, i64> {
    let mut out = std::collections::HashMap::new();
    let mut at = 0i64;
    for block in blocks {
        out.insert(block.label.clone(), at);
        for inst in &block.insts {
            match inst {
                Inst::Label(name) => {
                    out.insert(name.clone(), at);
                }
                _ if inst.is_directive() => {}
                _ => at += 4,
            }
        }
    }
    out
}

/// `AArch64CompressJumpTables`: the narrowest entries that reach every block
/// from the lowest one, which becomes the base. The three instructions that
/// read an entry are as many for every width, so the choice moves no code.
pub fn compress(
    blocks: &mut [Lowered],
    tables: &mut [Table],
) -> Result<(), String> {
    let at = offsets(blocks);
    for table in tables {
        let place = |name: &str| at.get(name).copied().ok_or_else(|| format!("a jump table names the unplaced {name}"));
        let mut lowest: Option<(&String, i64)> = None;
        let mut highest = i64::MIN;
        for name in &table.entries {
            let offset = place(name)?;
            highest = highest.max(offset);
            if lowest.is_none_or(|(_, low)| offset < low) {
                lowest = Some((name, offset));
            }
        }
        let Some((low_name, low)) = lowest else { continue };
        let span = (highest - low) >> 2;
        let (bytes, shift) = match span {
            0..=0xFF => (1u8, 2u8),
            0x100..=0xFFFF => (2, 2),
            _ => continue,
        };
        let (b, i) = position(blocks, &table.base).ok_or("a jump table with no jump")?;
        let insts = &mut blocks[b].insts;
        let Inst::Adr { target, .. } = &mut insts[i + 1] else {
            return Err("a jump table whose jump is not the expansion".into());
        };
        *target = Target::Label(low_name.clone());
        let Inst::Load { mem: Memory::Int { width, bytes: size, sign, .. }, addr, .. } = &mut insts[i - 1] else {
            return Err("a jump table whose jump is not the expansion".into());
        };
        (*width, *size, *sign) = (Width::W, bytes, false);
        if let Addr::Register { shifted, .. } = addr {
            *shifted = bytes == 2;
        }
        let Inst::AddSubShifted { amount, .. } = &mut insts[i + 2] else {
            return Err("a jump table whose jump is not the expansion".into());
        };
        *amount = shift;
        insts.remove(i);
        blocks[b].locs.remove(i);
        table.base = low_name.clone();
        table.entry = Entry { width: bytes, shift, signed: false };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nop() -> Inst {
        Inst::AddSubImm {
            op: AddSub { subtract: false, set_flags: false },
            width: Width::X,
            rd: Gpr(1),
            rn: Gpr(1),
            imm12: 0,
            shift12: false,
        }
    }

    /// A jump through a table of two blocks, the second `words` words after the
    /// first.
    fn function(words: usize) -> (Vec<Lowered>, Vec<Table>) {
        let jump = dest32("f", 0, Gpr(10), Gpr(11), Gpr(9), Gpr(8));
        let blocks = vec![
            Lowered { label: "entry".into(), locs: vec![None; jump.len()], insts: jump },
            Lowered { label: "a".into(), locs: vec![None; words], insts: vec![nop(); words] },
            Lowered { label: "b".into(), locs: vec![None], insts: vec![nop()] },
        ];
        let table = Table {
            symbol: symbol("f", 0),
            base: base_label("f", 0),
            entries: vec!["b".into(), "a".into()],
            entry: Entry { width: 4, shift: 0, signed: true },
        };
        (blocks, vec![table])
    }

    /// Every table was four bytes an entry whatever the blocks' distance, so a
    /// ten-case switch cost 40 bytes where the pin's is 10.
    #[test]
    fn blocks_within_255_words_make_a_byte_table_based_at_the_lowest() {
        let (mut blocks, mut tables) = function(255);
        compress(&mut blocks, &mut tables).unwrap();
        assert_eq!(tables[0].entry, Entry { width: 1, shift: 2, signed: false });
        assert_eq!(tables[0].base, "a");
        assert!(!blocks[0].insts.iter().any(|inst| matches!(inst, Inst::Label(_))), "the 4-byte base is gone");
    }

    #[test]
    fn blocks_beyond_255_words_make_a_halfword_table() {
        let (mut blocks, mut tables) = function(256);
        compress(&mut blocks, &mut tables).unwrap();
        assert_eq!(tables[0].entry, Entry { width: 2, shift: 2, signed: false });
    }

    #[test]
    fn blocks_beyond_65535_words_keep_the_word_table() {
        let (mut blocks, mut tables) = function(65_536);
        compress(&mut blocks, &mut tables).unwrap();
        assert_eq!(tables[0].entry, Entry { width: 4, shift: 0, signed: true });
        assert_eq!(tables[0].base, base_label("f", 0));
    }
}
