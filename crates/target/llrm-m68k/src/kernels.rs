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
    /// An offset into the data block.
    Data(i64),
}
use Arg::{Data, Imm};

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
    let mut all = start(args);
    all.extend(kernel);
    let size = assemble(&all, 0).map(|one| one.code.len()).unwrap_or(0);
    all.push(Item::Bytes(vec![0; (4 - size % 4) % 4]));
    all.push(Item::Label(".data".to_owned()));
    all.push(Item::Bytes(data()));
    all
}

/// The IR of a kernel.
pub fn source(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/kernels").join(format!("{name}.ll"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}
