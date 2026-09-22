//! Port of `qbopt/abi/ports.py`: what an `in` or `out` can reach, by the
//! device its port names. The Python module docstring is the full account.

/// The VGA DAC: PEL mask, read index, write index, data. The palette is mapped
/// at no address.
pub const SILENT: [(i64, i64); 1] = [(0x3C6, 0x3C9)];

pub fn silent(port: i64) -> bool {
    SILENT
        .iter()
        .any(|&(low, high)| low <= port && port <= high)
}
