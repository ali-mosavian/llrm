//! Runtime decoder for recovered QBasic `NtParse` state-table bytes.

const ND_ACCEPT: u8 = 0;
const ND_REJECT: u8 = 1;
const ND_MARK: u8 = 2;
const ND_EMIT: u8 = 3;
const ND_BRANCH: u8 = 4;

/// Branch operand decoded from a state-table node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BranchTarget {
    Accept,
    Relative(usize),
    Absolute(usize),
}

/// One decoded entry from a state-table byte stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StateEntry {
    Accept,
    Reject,
    Mark(u8),
    Emit(u16),
    Branch(BranchTarget),
    Node { node_id: u16, branch: BranchTarget },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EncodeConfig {
    encode1byte: u8,
}

impl EncodeConfig {
    pub(super) fn new(encode1byte: u8) -> Self {
        Self { encode1byte }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DecodeError {
    UnexpectedEof,
}

/// Walk a state-table byte stream entry by entry.
#[derive(Debug, Clone)]
pub(super) struct StateDecoder<'a> {
    state: &'a [u8],
    pc: usize,
    config: EncodeConfig,
}

impl<'a> StateDecoder<'a> {
    pub(super) fn with_config_at(state: &'a [u8], config: EncodeConfig, pc: usize) -> Self {
        Self { state, pc, config }
    }

    pub(super) fn next(&mut self) -> Result<Option<StateEntry>, DecodeError> {
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
        if node_id == u16::from(ND_BRANCH) {
            Ok(Some(StateEntry::Branch(branch)))
        } else {
            Ok(Some(StateEntry::Node { node_id, branch }))
        }
    }
}

fn decode_node_id(state: &[u8], pc: &mut usize, config: &EncodeConfig) -> Result<u16, DecodeError> {
    let first = read_byte(state, pc)?;
    if first < config.encode1byte {
        Ok(u16::from(first))
    } else {
        let second = read_byte(state, pc)?;
        let raw = u16::from(first) << 8 | u16::from(second);
        Ok(raw - 255 * u16::from(config.encode1byte))
    }
}

fn decode_branch_operand(
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
            cursor_after + usize::from(first)
        } else {
            cursor_after + usize::from(first) - usize::from(config.encode1byte)
        };
        Ok(BranchTarget::Relative(target))
    } else {
        let second = read_byte(state, pc)?;
        let raw = u16::from(first) << 8 | u16::from(second);
        let offset = u32::from(raw) - 256 * u32::from(config.encode1byte);
        Ok(BranchTarget::Absolute(offset as usize))
    }
}

fn read_byte(state: &[u8], pc: &mut usize) -> Result<u8, DecodeError> {
    state
        .get(*pc)
        .copied()
        .ok_or(DecodeError::UnexpectedEof)
        .map(|byte| {
            *pc += 1;
            byte
        })
}
