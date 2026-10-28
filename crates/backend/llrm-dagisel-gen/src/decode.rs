//! Bytes of the pin's `MatcherTable` to a tree of [`Matcher`] items.
//!
//! Reads the operands the way `SelectCodeCommon` does ([L3351][common]); an
//! opcode the pin's x86 table does not use is refused by name, so a table that
//! starts using one fails here.
//!
//! [common]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L3351

use std::collections::BTreeMap;

use llrm_dagisel::matcher::{Emit, Matcher, Opcode, Slice, Vt};

use crate::inc::Ids;

/// A decoded table: alternatives and cases are nested here and become items in
/// `flatten`.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Item(Matcher),
    Scope(Vec<Vec<Node>>),
    SwitchOpcode(Vec<(Opcode, Vec<Node>)>),
    SwitchType(Vec<(Vt, Vec<Node>)>),
}

pub struct Decoded {
    pub root: Vec<Node>,
    /// The operand lists of `Emit`s and `CompleteMatch`es, as `Slice`s point
    /// into it.
    pub lists: Vec<u16>,
}

/// `ByHwMode` types, resolved for one hardware mode (`getValueTypeForHwMode`).
pub struct HwModes(pub Vec<Vt>);

struct Decoder<'a> {
    bytes: &'a [u8],
    operand_lists: &'a [u8],
    names: BTreeMap<u8, &'a str>,
    ids: &'a Ids,
    hw: &'a HwModes,
    lists: Vec<u16>,
}

pub fn decode(
    bytes: &[u8],
    operand_lists: &[u8],
    ids: &Ids,
    hw: &HwModes,
) -> Result<Decoded, String> {
    let mut names = BTreeMap::new();
    for (name, &value) in &ids.0 {
        if name.starts_with("OPC_") {
            names.insert(value as u8, name.as_str());
        }
    }
    let mut decoder = Decoder { bytes, operand_lists, names, ids, hw, lists: Vec::new() };
    let root = decoder.sequence(0, bytes.len())?;
    Ok(Decoded { root, lists: decoder.lists })
}

fn vbr(
    first: u64,
    bytes: &[u8],
    at: &mut usize,
) -> u64 {
    let mut value = first & 127;
    let mut shift = 7;
    loop {
        let next = u64::from(bytes[*at]);
        *at += 1;
        value |= (next & 127) << shift;
        shift += 7;
        if next & 128 == 0 {
            return value;
        }
    }
}

impl Decoder<'_> {
    fn byte(
        &self,
        at: &mut usize,
    ) -> Result<u8, String> {
        let b = *self.bytes.get(*at).ok_or("the table ends inside an opcode")?;
        *at += 1;
        Ok(b)
    }

    fn vbr(
        &self,
        at: &mut usize,
    ) -> Result<u64, String> {
        let first = u64::from(self.byte(at)?);
        Ok(if first & 128 != 0 { vbr(first, self.bytes, at) } else { first })
    }

    fn signed_vbr(
        &self,
        at: &mut usize,
    ) -> Result<i64, String> {
        let mut value = 0i64;
        let mut shift = 0u32;
        loop {
            let next = u64::from(self.byte(at)?);
            value |= ((next & 127) << shift) as i64;
            shift += 7;
            if next & 128 == 0 {
                if shift < 64 && next & 0x40 != 0 {
                    value |= (u64::MAX << shift) as i64;
                }
                return Ok(value);
            }
        }
    }

    fn vt(
        &self,
        at: &mut usize,
    ) -> Result<Vt, String> {
        Ok(Vt(self.vbr(at)? as u16))
    }

    fn hw_vt(
        &self,
        at: &mut usize,
    ) -> Result<Vt, String> {
        let index = usize::from(self.byte(at)?);
        self.hw.0.get(index).copied().ok_or_else(|| format!("no type for hardware mode index {index}"))
    }

    fn u16(
        &self,
        at: &mut usize,
    ) -> Result<u16, String> {
        let low = self.byte(at)?;
        Ok(u16::from(low) | u16::from(self.byte(at)?) << 8)
    }

    fn inline_list(
        &mut self,
        at: &mut usize,
        count: usize,
    ) -> Result<Slice, String> {
        let start = self.lists.len() as u32;
        for _ in 0..count {
            let v = self.vbr(at)?;
            self.lists.push(v as u16);
        }
        Ok(Slice { start, len: count as u8 })
    }

    /// The operands of an `EmitNode`/`MorphNodeTo`: a count, then an index into
    /// `OperandLists`.
    fn operand_list(
        &mut self,
        at: &mut usize,
    ) -> Result<Slice, String> {
        let count = usize::from(self.byte(at)?);
        let start = self.lists.len() as u32;
        if count != 0 {
            let mut index = self.vbr(at)? as usize;
            for _ in 0..count {
                let first = u64::from(self.operand_lists[index]);
                index += 1;
                let value = if first & 128 != 0 { vbr(first, self.operand_lists, &mut index) } else { first };
                self.lists.push(value as u16);
            }
        }
        Ok(Slice { start, len: count as u8 })
    }

    /// `OPC_EmitNode*` and `OPC_MorphNodeTo*`: the long forms spell flags and
    /// result count, the compressed ones put the count (and sometimes the
    /// flags) in the opcode.
    fn emit(
        &mut self,
        name: &str,
        at: &mut usize,
    ) -> Result<Emit, String> {
        let opcode = self.u16(at)?;
        let rest =
            name.strip_prefix("OPC_MorphNodeTo").or_else(|| name.strip_prefix("OPC_EmitNode")).ok_or("not an emit")?;
        let (flags, num_vts) = if rest.is_empty() {
            let flags = self.byte(at)?;
            (flags, usize::from(self.byte(at)?))
        } else {
            let count = usize::from(digit(rest)?);
            let flags = match &rest[1..] {
                "" => self.byte(at)?,
                "None" => 0,
                "Chain" => 1,
                "GlueInput" => 2,
                "GlueOutput" => 4,
                _ => return Err(format!("{name} is not in the pin's x86 table")),
            };
            (flags, count)
        };
        if num_vts > 2 {
            return Err(format!("{name}: {num_vts} result types"));
        }
        let mut vts = [Vt(0); 2];
        for slot in vts.iter_mut().take(num_vts) {
            *slot = self.vt(at)?;
        }
        let operands = self.operand_list(at)?;
        Ok(Emit { opcode, flags, vts, num_vts: num_vts as u8, operands })
    }

    /// `MVT::<name>`, for the opcodes that imply their type.
    fn fixed_vt(
        &self,
        name: &str,
    ) -> Result<Vt, String> {
        Ok(Vt(self.ids.get(&format!("MVT::{name}"))? as u16))
    }

    fn sequence(
        &mut self,
        start: usize,
        end: usize,
    ) -> Result<Vec<Node>, String> {
        let mut at = start;
        let mut out = Vec::new();
        while at < end {
            let opcode = self.byte(&mut at)?;
            let name =
                *self.names.get(&opcode).ok_or_else(|| format!("byte {opcode} at {} is not an opcode", at - 1))?;
            let item = match name {
                "OPC_Scope" => {
                    let mut alternatives = Vec::new();
                    loop {
                        let skip = self.vbr(&mut at)? as usize;
                        if skip == 0 {
                            break;
                        }
                        alternatives.push(self.sequence(at, at + skip)?);
                        at += skip;
                    }
                    out.push(Node::Scope(alternatives));
                    continue;
                }
                "OPC_SwitchOpcode" | "OPC_SwitchType" => {
                    let by_opcode = name == "OPC_SwitchOpcode";
                    let (mut opcodes, mut types) = (Vec::new(), Vec::new());
                    loop {
                        let size = self.vbr(&mut at)? as usize;
                        if size == 0 {
                            break;
                        }
                        if by_opcode {
                            let case = self.u16(&mut at)?;
                            opcodes.push((case, self.sequence(at, at + size)?));
                        } else {
                            let case = self.vt(&mut at)?;
                            types.push((case, self.sequence(at, at + size)?));
                        }
                        at += size;
                    }
                    out.push(if by_opcode { Node::SwitchOpcode(opcodes) } else { Node::SwitchType(types) });
                    continue;
                }
                "OPC_RecordNode" => Matcher::RecordNode,
                "OPC_RecordMemRef" => Matcher::RecordMemRef,
                "OPC_CaptureGlueInput" => Matcher::CaptureGlueInput,
                "OPC_CaptureDeactivationSymbol" => Matcher::CaptureDeactivationSymbol,
                "OPC_MoveParent" => Matcher::MoveParent,
                "OPC_CheckImmAllOnesV" => Matcher::CheckImmAllOnesV,
                "OPC_CheckImmAllZerosV" => Matcher::CheckImmAllZerosV,
                "OPC_CheckUndef" => Matcher::CheckUndef,
                "OPC_CheckFoldableChainNode" => Matcher::CheckFoldableChainNode,
                "OPC_MoveChild" => Matcher::MoveChild(self.byte(&mut at)?),
                "OPC_MoveSibling" => Matcher::MoveSibling(self.byte(&mut at)?),
                "OPC_CheckSame" => Matcher::CheckSame(self.byte(&mut at)?),
                "OPC_CheckPatternPredicate" => Matcher::CheckPatternPredicate(u16::from(self.byte(&mut at)?)),
                "OPC_CheckPatternPredicateTwoByte" => Matcher::CheckPatternPredicate(self.u16(&mut at)?),
                "OPC_CheckPredicate" => Matcher::CheckPredicate(u16::from(self.byte(&mut at)?)),
                "OPC_CheckComplexPat" => {
                    let pattern = self.byte(&mut at)?;
                    Matcher::CheckComplexPat { pattern, record: self.byte(&mut at)? }
                }
                "OPC_CheckOpcode" => Matcher::CheckOpcode(self.u16(&mut at)?),
                "OPC_CheckType" => Matcher::CheckType(self.vt(&mut at)?),
                "OPC_CheckTypeI32" => Matcher::CheckType(self.fixed_vt("i32")?),
                "OPC_CheckTypeI64" => Matcher::CheckType(self.fixed_vt("i64")?),
                "OPC_CheckTypeByHwMode" => Matcher::CheckType(self.hw_vt(&mut at)?),
                "OPC_CheckTypeRes" => {
                    let res = self.byte(&mut at)?;
                    Matcher::CheckTypeRes { res, vt: self.vt(&mut at)? }
                }
                "OPC_CheckTypeResByHwMode" => {
                    let res = self.byte(&mut at)?;
                    Matcher::CheckTypeRes { res, vt: self.hw_vt(&mut at)? }
                }
                "OPC_CheckTypeByHwMode0" => Matcher::CheckType(self.hw.0[0]),
                "OPC_CheckInteger" => Matcher::CheckInteger(self.signed_vbr(&mut at)?),
                "OPC_CheckCondCode" => Matcher::CheckCondCode(self.byte(&mut at)?),
                "OPC_CheckChild2CondCode" => Matcher::CheckChild2CondCode(self.byte(&mut at)?),
                "OPC_CheckValueType" => Matcher::CheckValueType(self.vt(&mut at)?),
                "OPC_CheckAndImm" => Matcher::CheckAndImm(self.vbr(&mut at)?),
                "OPC_CheckOrImm" => Matcher::CheckOrImm(self.vbr(&mut at)?),
                "OPC_EmitInteger" => {
                    let vt = self.vt(&mut at)?;
                    Matcher::EmitInteger { vt, value: self.signed_vbr(&mut at)? }
                }
                "OPC_EmitIntegerI8" | "OPC_EmitIntegerI16" | "OPC_EmitIntegerI32" | "OPC_EmitIntegerI64" => {
                    let vt = self.fixed_vt(&name["OPC_EmitInteger".len()..].to_lowercase())?;
                    Matcher::EmitInteger { vt, value: self.signed_vbr(&mut at)? }
                }
                "OPC_EmitIntegerByHwMode" => {
                    let vt = self.hw_vt(&mut at)?;
                    Matcher::EmitInteger { vt, value: self.signed_vbr(&mut at)? }
                }
                "OPC_EmitIntegerByHwMode0" => {
                    Matcher::EmitInteger { vt: self.hw.0[0], value: self.signed_vbr(&mut at)? }
                }
                "OPC_EmitRegister" => {
                    let vt = self.vt(&mut at)?;
                    Matcher::EmitRegister { vt, register: u16::from(self.byte(&mut at)?) }
                }
                "OPC_EmitRegisterI32" => {
                    Matcher::EmitRegister { vt: self.fixed_vt("i32")?, register: u16::from(self.byte(&mut at)?) }
                }
                "OPC_EmitRegisterI64" => {
                    Matcher::EmitRegister { vt: self.fixed_vt("i64")?, register: u16::from(self.byte(&mut at)?) }
                }
                "OPC_EmitRegisterByHwMode" => {
                    let vt = self.hw_vt(&mut at)?;
                    Matcher::EmitRegister { vt, register: u16::from(self.byte(&mut at)?) }
                }
                "OPC_EmitConvertToTarget" => Matcher::EmitConvertToTarget(self.byte(&mut at)?),
                "OPC_EmitMergeInputChains1_0" | "OPC_EmitMergeInputChains1_1" | "OPC_EmitMergeInputChains1_2" => {
                    let record = u16::from(name.as_bytes()[name.len() - 1] - b'0');
                    let start = self.lists.len() as u32;
                    self.lists.push(record);
                    Matcher::EmitMergeInputChains(Slice { start, len: 1 })
                }
                "OPC_EmitMergeInputChains" => {
                    let count = usize::from(self.byte(&mut at)?);
                    let start = self.lists.len() as u32;
                    for _ in 0..count {
                        let record = u16::from(self.byte(&mut at)?);
                        self.lists.push(record);
                    }
                    Matcher::EmitMergeInputChains(Slice { start, len: count as u8 })
                }
                "OPC_EmitCopyToReg" => {
                    let record = self.byte(&mut at)?;
                    Matcher::EmitCopyToReg { record, register: u16::from(self.byte(&mut at)?) }
                }
                "OPC_EmitNodeXForm" => {
                    let xform = self.byte(&mut at)?;
                    Matcher::EmitNodeXForm { xform, record: self.byte(&mut at)? }
                }
                "OPC_CompleteMatch" => {
                    let count = usize::from(self.byte(&mut at)?);
                    let list = self.inline_list(&mut at, count)?;
                    Matcher::CompleteMatch(list)
                }
                _ if name.starts_with("OPC_RecordChild") => Matcher::RecordChild(digit(name)?),
                _ if name.starts_with("OPC_MoveChild") => Matcher::MoveChild(digit(name)?),
                _ if name.starts_with("OPC_MoveSibling") => Matcher::MoveSibling(digit(name)?),
                _ if name.starts_with("OPC_CheckChild") && name.ends_with("Same") => {
                    Matcher::CheckChildSame { child: digit(name)?, record: self.byte(&mut at)? }
                }
                _ if name.starts_with("OPC_CheckChild") && name.ends_with("Integer") => {
                    Matcher::CheckChildInteger { child: digit(name)?, value: self.signed_vbr(&mut at)? }
                }
                _ if name.starts_with("OPC_CheckChild") && name.contains("Type") => {
                    let child = digit(name)?;
                    let vt = if name.ends_with("TypeI32") {
                        self.fixed_vt("i32")?
                    } else if name.ends_with("TypeI64") {
                        self.fixed_vt("i64")?
                    } else if name.ends_with("ByHwMode0") {
                        self.hw.0[0]
                    } else if name.ends_with("ByHwMode") {
                        self.hw_vt(&mut at)?
                    } else {
                        self.vt(&mut at)?
                    };
                    Matcher::CheckChildType { child, vt }
                }
                _ if name.starts_with("OPC_CheckPatternPredicate") => {
                    Matcher::CheckPatternPredicate(u16::from(digit(name)?))
                }
                _ if name.starts_with("OPC_CheckPredicate") && !name.contains("With") => {
                    Matcher::CheckPredicate(u16::from(digit(name)?))
                }
                _ if name.starts_with("OPC_CheckComplexPat") => {
                    Matcher::CheckComplexPat { pattern: digit(name)?, record: self.byte(&mut at)? }
                }
                _ if name.starts_with("OPC_EmitConvertToTarget") => Matcher::EmitConvertToTarget(digit(name)?),
                "OPC_EmitCopyToRegTwoByte" => {
                    let record = self.byte(&mut at)?;
                    let low = u16::from(self.byte(&mut at)?);
                    Matcher::EmitCopyToReg { record, register: low | u16::from(self.byte(&mut at)?) << 8 }
                }
                _ if name.starts_with("OPC_EmitCopyToReg") => {
                    Matcher::EmitCopyToReg { record: digit(name)?, register: u16::from(self.byte(&mut at)?) }
                }
                _ if name.starts_with("OPC_EmitNode") && name != "OPC_EmitNodeXForm"
                    || name.starts_with("OPC_MorphNodeTo") =>
                {
                    let emit = self.emit(name, &mut at)?;
                    if name.starts_with("OPC_Morph") { Matcher::MorphNodeTo(emit) } else { Matcher::EmitNode(emit) }
                }
                _ => return Err(format!("{name} (byte {} of the table) is not in the pin's x86 table", at - 1)),
            };
            out.push(Node::Item(item));
        }
        if at != end {
            return Err(format!("a sequence overran its end ({at} > {end})"));
        }
        Ok(out)
    }
}

fn digit(name: &str) -> Result<u8, String> {
    name.bytes().find(u8::is_ascii_digit).map(|d| d - b'0').ok_or_else(|| format!("{name} has no digit"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flatten::flatten;

    fn ids() -> Ids {
        Ids::parse(
            "OPC_Scope 0\nOPC_RecordNode 1\nOPC_CheckOpcode 2\nOPC_CheckPredicateWithOperands 3\nOPC_CompleteMatch 4\nOPC_MoveParent 5\nMVT::i32 7\n",
        )
        .unwrap()
    }

    fn hw() -> HwModes {
        HwModes(vec![Vt(7), Vt(7)])
    }

    #[test]
    fn a_byte_that_is_no_opcode_is_refused() {
        let err = decode(&[200], &[], &ids(), &hw()).err().unwrap();
        assert!(err.contains("not an opcode"), "{err}");
    }

    #[test]
    fn an_opcode_the_x86_table_does_not_use_is_refused_by_name() {
        let err = decode(&[3], &[], &ids(), &hw()).err().unwrap();
        assert!(err.contains("OPC_CheckPredicateWithOperands"), "{err}");
    }

    #[test]
    fn a_scope_decodes_to_alternatives_and_flattens_to_item_skips() {
        // Scope: [RecordNode] | [MoveParent, MoveParent]
        let bytes = [0, 1, 1, 2, 5, 5, 0];
        let decoded = decode(&bytes, &[], &ids(), &hw()).unwrap();
        let [Node::Scope(alternatives)] = decoded.root.as_slice() else { panic!("{:?}", decoded.root) };
        assert_eq!(alternatives.len(), 2);
        let flat = flatten(&decoded.root, &decoded.lists);
        assert_eq!(
            flat.items,
            [
                Matcher::Scope,
                Matcher::Alt(1),
                Matcher::RecordNode,
                Matcher::Alt(2),
                Matcher::MoveParent,
                Matcher::MoveParent,
                Matcher::Alt(0)
            ]
        );
    }

    #[test]
    fn a_sequence_that_runs_past_its_alternative_is_refused() {
        // The alternative says 1 byte but `CheckOpcode` needs 2 more.
        let err = decode(&[0, 1, 2, 0], &[], &ids(), &hw()).err().unwrap();
        assert!(err.contains("table ends") || err.contains("overran"), "{err}");
    }

    /// `OPC_EmitCopyToRegTwoByte` (an AArch64 register past 255, `WZR`, `SP`) was refused as not in the
    /// x86 table, so the AArch64 table could not be decoded at all.
    #[test]
    fn a_copy_to_a_register_past_255_decodes_both_bytes() {
        let ids = Ids::parse("OPC_EmitCopyToRegTwoByte 6\nOPC_CompleteMatch 4\nMVT::i32 7\n").unwrap();
        let decoded = decode(&[6, 2, 0x34, 0x12], &[], &ids, &hw()).unwrap();
        assert_eq!(decoded.root, [Node::Item(Matcher::EmitCopyToReg { record: 2, register: 0x1234 })]);
    }

    /// The AArch64 table opens a pattern with `OPC_CaptureDeactivationSymbol`.
    #[test]
    fn a_deactivation_symbol_capture_decodes() {
        let ids = Ids::parse("OPC_CaptureDeactivationSymbol 12\nMVT::i32 7\n").unwrap();
        let decoded = decode(&[12], &[], &ids, &hw()).unwrap();
        assert_eq!(decoded.root, [Node::Item(Matcher::CaptureDeactivationSymbol)]);
    }

    #[test]
    fn a_type_check_on_a_result_decodes() {
        // WebAssembly's table checks result 1 of a node: the decode refused it
        // as "not in the pin's x86 table".
        let ids = Ids::parse("OPC_CheckTypeRes 0\nMVT::i32 7\n").unwrap();
        let decoded = decode(&[0, 1, 7], &[], &ids, &hw()).unwrap();
        let flat = flatten(&decoded.root, &decoded.lists);
        assert_eq!(flat.items, [Matcher::CheckTypeRes { res: 1, vt: Vt(7) }]);
    }
}
