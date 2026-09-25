//! Low-level `NtParse` state-table byte encoding (MASM `prsnt.asm` / `prsutil.asm`).
//!
//! Independent of grammar / `parser_tables`; used by a future `buildprs` host tool.

pub const ND_ACCEPT: u8 = 0;
pub const ND_REJECT: u8 = 1;
pub const ND_MARK: u8 = 2;
pub const ND_EMIT: u8 = 3;
pub const ND_BRANCH: u8 = 4;

pub const DEFAULT_ENCODE1BYTE: u8 = 240;

/// Branch operand decoded from a state-table node (`prsnt.asm:PsOneByteOperand`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchTarget {
    /// Byte `255`: ε-success / accept without consuming a token.
    Accept,
    /// Single-byte relative branch: absolute index in the state buffer.
    Relative(usize),
    /// Two-byte absolute offset into the state buffer.
    Absolute(usize),
}

impl BranchTarget {
    pub fn position(self) -> Option<usize> {
        match self {
            BranchTarget::Accept => None,
            BranchTarget::Relative(p) | BranchTarget::Absolute(p) => Some(p),
        }
    }
}

/// One decoded entry from a state-table byte stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateEntry {
    Accept,
    Reject,
    Mark(u8),
    Emit(u16),
    Branch(BranchTarget),
    Node { node_id: u16, branch: BranchTarget },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodeConfig {
    pub encode1byte: u8,
}

impl Default for EncodeConfig {
    fn default() -> Self {
        Self {
            encode1byte: DEFAULT_ENCODE1BYTE,
        }
    }
}

impl EncodeConfig {
    pub fn new(encode1byte: u8) -> Self {
        Self { encode1byte }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    TruncatedNodeId,
    InvalidAbsoluteOperand { offset: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    UnexpectedEof,
}

/// Append a node id using 1- or 2-byte form (`prsnt.asm:NotDirective`).
pub fn encode_node_id(
    buf: &mut Vec<u8>,
    node_id: u16,
    config: &EncodeConfig,
) -> Result<(), EncodeError> {
    let threshold = config.encode1byte as u16;
    if node_id < threshold {
        buf.push(node_id as u8);
        Ok(())
    } else {
        let temp = node_id as u32 + 255 * config.encode1byte as u32;
        if temp > u16::MAX as u32 {
            return Err(EncodeError::TruncatedNodeId);
        }
        buf.push((temp >> 8) as u8);
        buf.push((temp & 0xFF) as u8);
        Ok(())
    }
}

/// Read a node id and advance `pc`.
pub fn decode_node_id(
    state: &[u8],
    pc: &mut usize,
    config: &EncodeConfig,
) -> Result<u16, DecodeError> {
    let first = read_byte(state, pc)?;
    if first < config.encode1byte {
        Ok(first as u16)
    } else {
        let second = read_byte(state, pc)?;
        let raw = (first as u16) << 8 | second as u16;
        Ok(raw - 255 * config.encode1byte as u16)
    }
}

/// Append a branch operand (`prsnt.asm:PsOneByteNodeId` / `PsOneByteOperand`).
pub fn encode_branch_operand(
    buf: &mut Vec<u8>,
    cursor_after_operand: usize,
    target: BranchTarget,
    config: &EncodeConfig,
) -> Result<(), EncodeError> {
    match target {
        BranchTarget::Accept => {
            buf.push(255);
            Ok(())
        }
        BranchTarget::Absolute(offset) => encode_absolute_operand(buf, offset, config),
        BranchTarget::Relative(pos) => {
            if let Some(byte) = relative_operand_byte(cursor_after_operand, pos, config) {
                buf.push(byte);
                Ok(())
            } else {
                encode_absolute_operand(buf, pos, config)
            }
        }
    }
}

/// Read a branch operand and advance `pc`.
pub fn decode_branch_operand(
    state: &[u8],
    pc: &mut usize,
    config: &EncodeConfig,
) -> Result<BranchTarget, DecodeError> {
    let first = read_byte(state, pc)?;
    if first == 255 {
        return Ok(BranchTarget::Accept);
    }
    let cursor_after = *pc;
    if first < config.encode1byte {
        let half = config.encode1byte / 2;
        let target = if first <= half {
            cursor_after + first as usize
        } else {
            cursor_after + first as usize - config.encode1byte as usize
        };
        Ok(BranchTarget::Relative(target))
    } else {
        let second = read_byte(state, pc)?;
        let raw = (first as u16) << 8 | second as u16;
        let offset = raw as u32 - 256 * config.encode1byte as u32;
        Ok(BranchTarget::Absolute(offset as usize))
    }
}

fn encode_absolute_operand(
    buf: &mut Vec<u8>,
    offset: usize,
    config: &EncodeConfig,
) -> Result<(), EncodeError> {
    let temp = offset as u32 + 256 * config.encode1byte as u32;
    if temp > u16::MAX as u32 {
        return Err(EncodeError::InvalidAbsoluteOperand { offset });
    }
    let high = (temp >> 8) as u8;
    let low = (temp & 0xFF) as u8;
    if high < config.encode1byte {
        return Err(EncodeError::InvalidAbsoluteOperand { offset });
    }
    if high == 255 {
        return Err(EncodeError::InvalidAbsoluteOperand { offset });
    }
    buf.push(high);
    buf.push(low);
    Ok(())
}

fn relative_operand_byte(
    cursor_after_operand: usize,
    target: usize,
    config: &EncodeConfig,
) -> Option<u8> {
    let rel = target as i64 - cursor_after_operand as i64;
    let half = config.encode1byte as i64 / 2;
    let threshold = config.encode1byte as i64;
    if (0..=half).contains(&rel) {
        Some(rel as u8)
    } else if rel < 0 {
        let encoded = rel + threshold;
        if encoded > half && encoded < threshold {
            Some(encoded as u8)
        } else {
            None
        }
    } else {
        None
    }
}

fn read_byte(state: &[u8], pc: &mut usize) -> Result<u8, DecodeError> {
    state
        .get(*pc)
        .copied()
        .ok_or(DecodeError::UnexpectedEof)
        .map(|b| {
            *pc += 1;
            b
        })
}

/// Incrementally build a state-table byte stream.
#[derive(Debug, Clone, Default)]
pub struct StateEncoder {
    buf: Vec<u8>,
    config: EncodeConfig,
}

impl StateEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: EncodeConfig) -> Self {
        Self {
            buf: Vec::new(),
            config,
        }
    }

    pub fn config(&self) -> &EncodeConfig {
        &self.config
    }

    pub fn bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn accept(&mut self) {
        self.buf.push(ND_ACCEPT);
    }

    pub fn reject(&mut self) {
        self.buf.push(ND_REJECT);
    }

    pub fn mark(&mut self, operand: u8) {
        self.buf.push(ND_MARK);
        self.buf.push(operand);
    }

    pub fn emit(&mut self, opcode: u16) {
        self.buf.push(ND_EMIT);
        self.buf.extend_from_slice(&opcode.to_le_bytes());
    }

    pub fn branch(&mut self, target: BranchTarget) -> Result<(), EncodeError> {
        self.node(ND_BRANCH as u16, target)
    }

    pub fn node(&mut self, node_id: u16, target: BranchTarget) -> Result<(), EncodeError> {
        encode_node_id(&mut self.buf, node_id, &self.config)?;
        let cursor_after = self.buf.len() + 1;
        encode_branch_operand(&mut self.buf, cursor_after, target, &self.config)
    }

    /// Write a node id and reserve one byte for a branch operand to patch later.
    pub fn emit_node_deferred_branch(
        &mut self,
        node_id: u16,
    ) -> Result<(usize, usize), EncodeError> {
        encode_node_id(&mut self.buf, node_id, &self.config)?;
        let operand_pos = self.buf.len();
        self.buf.push(0);
        Ok((operand_pos, operand_pos + 1))
    }

    /// Patch a previously reserved branch operand in place.
    pub fn patch_branch_operand(
        &mut self,
        operand_pos: usize,
        cursor_after_operand: usize,
        target: BranchTarget,
    ) -> Result<(), EncodeError> {
        let mut encoded = Vec::new();
        encode_branch_operand(&mut encoded, cursor_after_operand, target, &self.config)?;
        if encoded.len() == 1 {
            self.buf[operand_pos] = encoded[0];
            Ok(())
        } else {
            self.buf
                .splice(operand_pos..operand_pos + 1, encoded.into_iter());
            Ok(())
        }
    }
}

/// Walk a state-table byte stream entry by entry.
#[derive(Debug, Clone)]
pub struct StateDecoder<'a> {
    state: &'a [u8],
    pc: usize,
    config: EncodeConfig,
}

impl<'a> StateDecoder<'a> {
    pub fn new(state: &'a [u8]) -> Self {
        Self {
            state,
            pc: 0,
            config: EncodeConfig::default(),
        }
    }

    pub fn with_config(state: &'a [u8], config: EncodeConfig) -> Self {
        Self {
            state,
            pc: 0,
            config,
        }
    }

    pub fn with_config_at(state: &'a [u8], config: EncodeConfig, pc: usize) -> Self {
        Self { state, pc, config }
    }

    pub fn pc(&self) -> usize {
        self.pc
    }

    pub fn remaining(&self) -> &'a [u8] {
        &self.state[self.pc..]
    }

    pub fn next(&mut self) -> Result<Option<StateEntry>, DecodeError> {
        if self.pc >= self.state.len() {
            return Ok(None);
        }
        let directive = read_byte(self.state, &mut self.pc)?;
        if directive <= ND_EMIT {
            return Ok(Some(match directive {
                ND_ACCEPT => StateEntry::Accept,
                ND_REJECT => StateEntry::Reject,
                ND_MARK => StateEntry::Mark(read_byte(self.state, &mut self.pc)?),
                ND_EMIT => {
                    let lo = read_byte(self.state, &mut self.pc)?;
                    let hi = read_byte(self.state, &mut self.pc)?;
                    StateEntry::Emit(u16::from_le_bytes([lo, hi]))
                }
                _ => unreachable!("directive <= ND_EMIT covers 0..=3"),
            }));
        }

        self.pc -= 1;
        let node_id = decode_node_id(self.state, &mut self.pc, &self.config)?;
        let branch = decode_branch_operand(self.state, &mut self.pc, &self.config)?;
        if node_id == ND_BRANCH as u16 {
            Ok(Some(StateEntry::Branch(branch)))
        } else {
            Ok(Some(StateEntry::Node { node_id, branch }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(entries: &[StateEntry]) {
        let mut enc = StateEncoder::new();
        for entry in entries {
            match *entry {
                StateEntry::Accept => enc.accept(),
                StateEntry::Reject => enc.reject(),
                StateEntry::Mark(n) => enc.mark(n),
                StateEntry::Emit(w) => enc.emit(w),
                StateEntry::Branch(t) => enc.branch(t).expect("branch encode"),
                StateEntry::Node { node_id, branch } => {
                    enc.node(node_id, branch).expect("node encode");
                }
            }
        }
        let bytes = enc.into_bytes();
        let mut dec = StateDecoder::new(&bytes);
        for expected in entries {
            let got = dec.next().expect("decode").expect("entry");
            assert_eq!(got, *expected, "bytes={bytes:?}");
        }
        assert!(dec.next().expect("decode").is_none());
    }

    #[test]
    fn directives_accept_reject_mark_emit_le() {
        roundtrip(&[
            StateEntry::Accept,
            StateEntry::Reject,
            StateEntry::Mark(6),
            StateEntry::Emit(0x3412),
        ]);
        let mut enc = StateEncoder::new();
        enc.emit(0x7856);
        assert_eq!(&enc.bytes()[1..3], &[0x56, 0x78]);
    }

    #[test]
    fn node_id_one_byte_and_two_byte() {
        let mut enc = StateEncoder::new();
        let config = *enc.config();
        encode_node_id(&mut enc.buf, 42, &config).unwrap();
        encode_node_id(&mut enc.buf, 500, &config).unwrap();
        assert_eq!(&enc.bytes()[0..1], &[42]);
        assert_eq!(&enc.bytes()[1..3], &[241, 4]); // 500 + 255*240 = 61700 = 0xF104

        let mut pc = 0;
        assert_eq!(decode_node_id(enc.bytes(), &mut pc, &config).unwrap(), 42);
        assert_eq!(decode_node_id(enc.bytes(), &mut pc, &config).unwrap(), 500);
    }

    #[test]
    fn branch_operand_accept() {
        let mut enc = StateEncoder::new();
        enc.node(10, BranchTarget::Accept).unwrap();
        assert_eq!(enc.bytes(), &[10, 255]);

        let mut dec = StateDecoder::new(enc.bytes());
        assert_eq!(
            dec.next().unwrap(),
            Some(StateEntry::Node {
                node_id: 10,
                branch: BranchTarget::Accept,
            })
        );
    }

    #[test]
    fn branch_operand_relative_forward_and_back() {
        let mut enc = StateEncoder::new();
        enc.accept(); // 0: accept
        enc.reject(); // 1: reject
        let forward_target = 4;
        enc.node(5, BranchTarget::Relative(forward_target)).unwrap(); // 2..=3: node + rel byte
        let back_target = 0;
        enc.node(6, BranchTarget::Relative(back_target)).unwrap();

        let bytes = enc.bytes();
        // [0]=accept, [1]=reject, [2]=5, [3]=rel, [4]=6, [5]=rel
        assert_eq!(bytes[2], 5);
        // node at 2: cursor_after_operand = 4, forward to 4 => rel=0
        assert_eq!(bytes[3], 0);

        // node at 4: cursor_after=6, back to 0 => rel=0-6+240=234
        assert_eq!(bytes[5], 234);

        let mut dec = StateDecoder::new(bytes);
        assert_eq!(dec.next().unwrap(), Some(StateEntry::Accept));
        assert_eq!(dec.next().unwrap(), Some(StateEntry::Reject));
        assert_eq!(
            dec.next().unwrap(),
            Some(StateEntry::Node {
                node_id: 5,
                branch: BranchTarget::Relative(4),
            })
        );
        assert_eq!(
            dec.next().unwrap(),
            Some(StateEntry::Node {
                node_id: 6,
                branch: BranchTarget::Relative(0),
            })
        );
    }

    #[test]
    fn branch_operand_absolute() {
        let mut enc = StateEncoder::new();
        enc.node(7, BranchTarget::Absolute(1000)).unwrap();
        assert_eq!(&enc.bytes()[0..1], &[7]);
        assert_eq!(&enc.bytes()[1..3], &[243, 232]); // 1000 + 256*240 = 62440 = 0xF3E8

        let mut dec = StateDecoder::new(enc.bytes());
        assert_eq!(
            dec.next().unwrap(),
            Some(StateEntry::Node {
                node_id: 7,
                branch: BranchTarget::Absolute(1000),
            })
        );
    }

    #[test]
    fn unconditional_branch_entry() {
        roundtrip(&[StateEntry::Branch(BranchTarget::Relative(3))]);
        roundtrip(&[StateEntry::Branch(BranchTarget::Absolute(512))]);
    }

    #[test]
    fn configurable_encode1byte() {
        let config = EncodeConfig::new(200);
        let mut enc = StateEncoder::with_config(config);
        encode_node_id(&mut enc.buf, 250, &config).unwrap();
        assert_eq!(&enc.bytes(), &[200, 50]);

        let mut pc = 0;
        assert_eq!(decode_node_id(enc.bytes(), &mut pc, &config).unwrap(), 250);
    }

    #[test]
    fn decode_branch_helpers_match_encoder() {
        let config = EncodeConfig::default();
        let mut buf = Vec::new();

        let cursor = buf.len() + 1;
        encode_branch_operand(&mut buf, cursor, BranchTarget::Accept, &config).unwrap();

        let cursor = buf.len() + 1;
        encode_branch_operand(&mut buf, cursor, BranchTarget::Relative(5), &config).unwrap();

        let cursor = buf.len() + 1;
        encode_branch_operand(&mut buf, cursor, BranchTarget::Relative(3), &config).unwrap();

        let cursor = buf.len() + 1;
        encode_branch_operand(&mut buf, cursor, BranchTarget::Absolute(300), &config).unwrap();

        let mut pc = 0;
        assert_eq!(
            decode_branch_operand(&buf, &mut pc, &config).unwrap(),
            BranchTarget::Accept
        );
        assert_eq!(
            decode_branch_operand(&buf, &mut pc, &config).unwrap(),
            BranchTarget::Relative(5)
        );
        assert_eq!(
            decode_branch_operand(&buf, &mut pc, &config).unwrap(),
            BranchTarget::Relative(3)
        );
        assert_eq!(
            decode_branch_operand(&buf, &mut pc, &config).unwrap(),
            BranchTarget::Absolute(300)
        );
    }
}
