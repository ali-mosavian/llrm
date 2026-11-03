//! `WebAssemblyLowerBrUnless`: `BR_UNLESS` becomes `BR_IF` over the inverted
//! comparison that defines its condition, or over an `EQZ` of the condition.
//!
//! # See also
//! `WebAssemblyLowerBrUnless.cpp`, pin d1106deb71cc.

use crate::dagisel::generated::ids::opcode as op;
use crate::mir::{Function, Inst, Operand, ValType};

/// The comparison that is true when `opcode` is false.
fn inverse(opcode: u16) -> Option<u16> {
    Some(match opcode {
        op::EQ_I32 => op::NE_I32,
        op::NE_I32 => op::EQ_I32,
        op::GT_S_I32 => op::LE_S_I32,
        op::GE_S_I32 => op::LT_S_I32,
        op::LT_S_I32 => op::GE_S_I32,
        op::LE_S_I32 => op::GT_S_I32,
        op::GT_U_I32 => op::LE_U_I32,
        op::GE_U_I32 => op::LT_U_I32,
        op::LT_U_I32 => op::GE_U_I32,
        op::LE_U_I32 => op::GT_U_I32,
        op::EQ_I64 => op::NE_I64,
        op::NE_I64 => op::EQ_I64,
        op::GT_S_I64 => op::LE_S_I64,
        op::GE_S_I64 => op::LT_S_I64,
        op::LT_S_I64 => op::GE_S_I64,
        op::LE_S_I64 => op::GT_S_I64,
        op::GT_U_I64 => op::LE_U_I64,
        op::GE_U_I64 => op::LT_U_I64,
        op::LT_U_I64 => op::GE_U_I64,
        op::LE_U_I64 => op::GT_U_I64,
        op::EQ_F32 => op::NE_F32,
        op::NE_F32 => op::EQ_F32,
        op::EQ_F64 => op::NE_F64,
        op::NE_F64 => op::EQ_F64,
        _ => return None,
    })
}

/// Whether anything changed.
pub fn lower_br_unless(f: &mut Function) -> bool {
    let mut changed = false;
    for b in 0..f.blocks.len() {
        let mut i = 0;
        while i < f.blocks[b].len() {
            if f.blocks[b][i].opcode != op::BR_UNLESS {
                i += 1;
                continue;
            }
            changed = true;
            let [dst, Operand::Use(mut cond)] = f.blocks[b][i].operands[..] else {
                unreachable!("BR_UNLESS dst, cond")
            };
            let mut inverted = false;
            if f.is_stackified(cond) {
                // A stackified register is defined by the instruction just
                // above.
                let at = i - 1;
                let def = &mut f.blocks[b][at];
                debug_assert!(def.defs().eq([cond]));
                if let Some(opposite) = inverse(def.opcode) {
                    def.opcode = opposite;
                    inverted = true;
                } else if def.opcode == op::EQZ_I32 {
                    let Operand::Use(source) = def.operands[1] else { unreachable!("EQZ_I32 dst, src") };
                    cond = source;
                    f.blocks[b].remove(at);
                    i -= 1;
                    inverted = true;
                }
            }
            if !inverted {
                let tmp = f.new_vreg(ValType::I32);
                f.blocks[b].insert(i, Inst::new(op::EQZ_I32, &[Operand::Def(tmp), Operand::Use(cond)]));
                i += 1;
                f.stackify(tmp);
                cond = tmp;
            }
            f.blocks[b][i] = Inst::new(op::BR_IF, &[dst, Operand::Use(cond)]);
            i += 1;
        }
    }
    changed
}
