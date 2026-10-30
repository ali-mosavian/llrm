//! The bench kernels: small leaf functions in `bench/kernels`, each with its
//! arguments and the result worked out by hand, and the program that runs one.
//!
//! Probe of the target's sizes: the datalayout in each kernel is the pin's
//! M68k one (`p:32:16:32`, `i32:16:32`), so `int`, `long`, pointers and
//! `size_t` are 32 bits.

/// The pin's M68k datalayout, as clang prints it for `m68k-linux-gnu`.
pub const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

use crate::compile::Item;
use crate::mc::Operand;
use crate::object::{assemble, item};
use crate::reg;

pub enum Arg {
    Imm(i64),
    /// A function, by name.
    Func(&'static str),
    /// An offset into the data block.
    Data(i64),
}
use Arg::{Data, Func, Imm};

/// `(kernel, arguments, the result worked out by hand)`.
pub fn table() -> Vec<(&'static str, Vec<Arg>, i32)> {
    vec![
        ("sum", vec![Imm(10)], 45),
        ("popcount", vec![Imm(0xF0F0)], 8),
        ("max", vec![Imm(7), Imm(42)], 42),
        ("absdiff", vec![Imm(10), Imm(52)], 42),
        ("clamp", vec![Imm(100), Imm(0), Imm(42)], 42),
        ("fib", vec![Imm(10)], 89),
        ("gcd", vec![Imm(126), Imm(84)], 42),
        ("arrsum", vec![Data(32), Imm(16)], 80),
        ("arrmax", vec![Data(32), Imm(16)], 9),
        ("countgt", vec![Data(32), Imm(16), Imm(4)], 9),
        ("strlen", vec![Data(0)], 13),
        ("copy", vec![Data(16), Data(0), Imm(13)], 33),
    ]
}

/// `(routine, arguments, the result worked out by hand, leaf)`: the second
/// set, library-style routines written after the first set was tuned
/// (`bench/library`). A routine that is not a leaf calls the runtime, which
/// the bench does not link: only its size is compared.
pub fn library_table() -> Vec<(&'static str, Vec<Arg>, i32, bool)> {
    vec![
        ("memcpy", vec![Data(16), Data(0), Imm(13)], 33, true),
        ("memset", vec![Data(16), Imm(7), Imm(16)], 14, true),
        ("strcmp", vec![Data(0), Data(32)], 32, true),
        ("hex", vec![Imm(0xBEEF), Data(240)], 204, true),
        ("crc16", vec![Data(224), Imm(9)], 152, true),
        ("bsearch", vec![Data(48), Imm(16), Imm(21)], 10, true),
        ("partition", vec![Data(112), Imm(16)], 19, true),
        ("muldiv", vec![Imm(1000), Imm(7)], 236, false),
        ("structcopy", vec![Data(200), Data(176)], 40, false),
        ("fnptr", vec![Func("h"), Imm(5)], 36, true),
    ]
}

/// The data block of the library set: strings, two arrays, a struct, a
/// checksum input and a buffer, at the offsets [`library_table`] names.
pub fn library_data() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut put = |at: usize, data: &[u8]| {
        if bytes.len() < at + data.len() {
            bytes.resize(at + data.len(), 0);
        }
        bytes[at..at + data.len()].copy_from_slice(data);
    };
    put(0, b"hello, world!\0");
    put(32, b"hello, worlD!\0");
    let sorted: Vec<u8> = (0..16u32).flat_map(|n| (2 * n + 1).to_be_bytes()).collect();
    put(48, &sorted);
    let unsorted: Vec<u8> =
        [3u32, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3].iter().flat_map(|n| n.to_be_bytes()).collect();
    put(112, &unsorted);
    let record: Vec<u8> = (1..=5u32).flat_map(|n| n.to_be_bytes()).chain([37]).collect();
    put(176, &record);
    put(224, b"123456789");
    put(255, &[0]);
    bytes
}

/// "hello, world!", room to copy it to, then sixteen longs.
pub fn data() -> Vec<u8> {
    let mut bytes = b"hello, world!\0\0\0".to_vec();
    bytes.extend([0; 16]);
    for long in [3u32, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3] {
        bytes.extend(long.to_be_bytes());
    }
    bytes
}

fn start(args: &[Arg]) -> Vec<Item> {
    let mut out = vec![Item::Label("_start".to_owned())];
    for arg in args.iter().rev() {
        let value = match arg {
            Imm(value) => Operand::Imm(*value),
            Func(name) => Operand::Symbol((*name).to_owned(), 0),
            Data(offset) => Operand::Symbol(".data".to_owned(), *offset),
        };
        out.push(item("MOV32ei", vec![Operand::Reg(reg::SP), value]));
    }
    out.push(item("CALLb", vec![Operand::Symbol("k".to_owned(), 0)]));
    out.push(item("ADD32ai", vec![Operand::Reg(reg::SP), Operand::Reg(reg::SP), Operand::Imm(4 * args.len() as i64)]));
    out.push(item("MOV32rr", vec![Operand::Reg(reg::D1), Operand::Reg(reg::D0)]));
    out.push(item("MOVQ", vec![Operand::Reg(reg::D0), Operand::Imm(1)]));
    out.push(item("TRAP", vec![Operand::Imm(0)]));
    out
}

/// `_start`, the kernel, padding to a long, and the data.
pub fn program(
    args: &[Arg],
    kernel: Vec<Item>,
) -> Vec<Item> {
    program_with_data(args, kernel, &data())
}

/// [`program`] over another data block.
pub fn program_with_data(
    args: &[Arg],
    kernel: Vec<Item>,
    data: &[u8],
) -> Vec<Item> {
    let mut all = start(args);
    all.extend(kernel);
    let size = assemble(&all, 0).map(|one| one.code.len()).unwrap_or(0);
    all.push(Item::Bytes(vec![0; (4 - size % 4) % 4]));
    all.push(Item::Label(".data".to_owned()));
    all.push(Item::Bytes(data.to_vec()));
    all
}

/// The IR of a kernel.
pub fn source(name: &str) -> String {
    source_in("kernels", name)
}

/// The IR of a routine of the set in `bench/<set>`.
pub fn source_in(
    set: &str,
    name: &str,
) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("bench").join(set).join(format!("{name}.ll"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}
