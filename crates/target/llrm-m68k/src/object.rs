//! A static m68k Linux executable: ELF32 big-endian `ET_EXEC`, one `PT_LOAD`
//! segment holding the code, with a symbol table so `llvm-objdump` names it.
//!
//! The functions are encoded by [`crate::encode`] and linked here: labels and
//! function names get their addresses, then every fixup (absolute 16/32 and
//! pc-relative 8/16/32, see `encode.rs`) is resolved in place. An undefined
//! symbol is an error.
//!
//! DEVIATE: spike-local, beside no object-writer trait. Reason:
//! the object refactor (`ObjectTarget`, `FixupKind`, `AsmBackend`) is not on
//! this branch; the fixup kinds above map onto it. Exit: the refactor landing.

use std::collections::HashMap;

use crate::compile::Item;
use crate::encode::{Encoded, encode};
use crate::mc::{Inst, Operand};
use crate::reg;

/// Where the segment is loaded.
const BASE: u32 = 0x80000000;
const EHDR: u32 = 52;
const PHDR: u32 = 32;
const EM_68K: u16 = 4;

fn inst(
    opcode: &'static str,
    operands: Vec<Operand>,
) -> Item {
    Item::Inst(Inst { opcode, operands })
}

/// `_start`: push `args` (last first), call `callee`, pop them, and exit with
/// the result as the status (`exit` is Linux m68k syscall 1: `d0` the number,
/// `d1` the status, `trap #0`).
pub fn start(
    callee: &str,
    args: &[i64],
) -> Vec<Item> {
    let mut out = vec![Item::Label("_start".to_owned())];
    for arg in args.iter().rev() {
        out.push(inst("MOV32ei", vec![Operand::Reg(reg::SP), Operand::Imm(*arg)]));
    }
    out.push(inst("CALLb", vec![Operand::Symbol(callee.to_owned(), 0)]));
    if !args.is_empty() {
        out.push(inst(
            "ADD32ai",
            vec![Operand::Reg(reg::SP), Operand::Reg(reg::SP), Operand::Imm(4 * args.len() as i64)],
        ));
    }
    out.push(inst("MOV32rr", vec![Operand::Reg(reg::D1), Operand::Reg(reg::D0)]));
    out.push(inst("MOVQ", vec![Operand::Reg(reg::D0), Operand::Imm(1)]));
    out.push(inst("TRAP", vec![Operand::Imm(0)]));
    out
}

fn put(
    out: &mut Vec<u8>,
    value: u64,
    bytes: usize,
) {
    out.extend_from_slice(&value.to_be_bytes()[8 - bytes..]);
}

/// Writes `value` big-endian into `size` bytes at `at`, or says it does not
/// fit.
fn patch(
    code: &mut [u8],
    at: usize,
    size: u8,
    value: i64,
    signed: bool,
) -> Result<(), String> {
    let bits = 8 * u32::from(size);
    let fits = if signed {
        value >= -(1i64 << (bits - 1)) && value < (1i64 << (bits - 1))
    } else {
        value >= -(1i64 << (bits - 1)) && value < (1i64 << bits)
    };
    if !fits {
        return Err(format!("{value} does not fit {size} bytes at {at}"));
    }
    code[at..at + size as usize].copy_from_slice(&(value as u64).to_be_bytes()[8 - size as usize..]);
    Ok(())
}

/// Writes `bytes` as an executable file (`qemu-m68k` exits 1, silently, on a
/// file without the execute bit).
pub fn write_executable(
    path: &std::path::Path,
    bytes: &[u8],
) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, bytes)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

/// The code of `items` placed at `base`, every fixup resolved, and the
/// address of each label.
pub fn assemble(
    items: &[Item],
    base: u32,
) -> Result<(Vec<u8>, Vec<(String, u32)>), String> {
    let mut code = Vec::new();
    let mut symbols: HashMap<&str, u32> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    let mut pending: Vec<(usize, Encoded)> = Vec::new();
    for item in items {
        match item {
            Item::Label(name) => {
                if symbols.insert(name, base + code.len() as u32).is_some() {
                    return Err(format!("{name} is defined twice"));
                }
                order.push(name);
            }
            Item::Inst(one) => {
                let encoded = encode(one)?;
                pending.push((code.len(), encoded.clone()));
                code.extend_from_slice(&encoded.bytes);
            }
        }
    }
    for (start, encoded) in &pending {
        for fixup in &encoded.fixups {
            let target =
                *symbols.get(fixup.symbol.as_str()).ok_or_else(|| format!("undefined symbol {}", fixup.symbol))?;
            let at = start + fixup.offset;
            let value = i64::from(target) + fixup.addend - if fixup.pc_rel { i64::from(base) + at as i64 } else { 0 };
            patch(&mut code, at, fixup.size, value, fixup.pc_rel)?;
        }
    }
    Ok((code, order.into_iter().map(|name| (name.to_owned(), symbols[name])).collect()))
}

/// The executable, entered at `_start`, which `items` must define. Function
/// labels (those without a leading `.`) become symbols.
pub fn executable(items: &[Item]) -> Result<Vec<u8>, String> {
    let text_at = BASE + EHDR + PHDR;
    let (code, labels) = assemble(items, text_at)?;
    let symbols: HashMap<&str, u32> = labels.iter().map(|(name, at)| (name.as_str(), *at)).collect();
    let order: Vec<&str> = labels.iter().map(|(name, _)| name.as_str()).collect();
    let entry = *symbols.get("_start").ok_or("no _start")?;

    // .strtab and .symtab: a null symbol, then each function
    let mut strtab = vec![0u8];
    let mut symtab = vec![0u8; 16];
    let functions: Vec<&str> = order.iter().copied().filter(|name| !name.starts_with('.')).collect();
    for (at, name) in functions.iter().enumerate() {
        let end = functions.get(at + 1).map_or(text_at + code.len() as u32, |next| symbols[next]);
        put(&mut symtab, strtab.len() as u64, 4);
        strtab.extend_from_slice(name.as_bytes());
        strtab.push(0);
        put(&mut symtab, u64::from(symbols[name]), 4);
        put(&mut symtab, u64::from(end - symbols[name]), 4);
        symtab.push(0x12); // GLOBAL FUNC
        symtab.push(0);
        put(&mut symtab, 1, 2); // .text
    }
    let shstrtab = b"\0.text\0.symtab\0.strtab\0.shstrtab\0".to_vec();
    let (text_name, symtab_name, strtab_name, shstrtab_name) = (1u32, 7, 15, 23);

    let text_off = EHDR + PHDR;
    let symtab_off = text_off + code.len() as u32;
    let strtab_off = symtab_off + symtab.len() as u32;
    let shstrtab_off = strtab_off + strtab.len() as u32;
    let shdrs_off = (shstrtab_off + shstrtab.len() as u32 + 3) & !3;

    let mut out = Vec::new();
    out.extend_from_slice(&[0x7f, b'E', b'L', b'F', 1, 2, 1, 0]);
    out.extend_from_slice(&[0; 8]);
    put(&mut out, 2, 2); // ET_EXEC
    put(&mut out, u64::from(EM_68K), 2);
    put(&mut out, 1, 4);
    put(&mut out, u64::from(entry), 4);
    put(&mut out, u64::from(EHDR), 4); // phoff
    put(&mut out, u64::from(shdrs_off), 4);
    put(&mut out, 0, 4); // flags
    put(&mut out, u64::from(EHDR), 2);
    put(&mut out, u64::from(PHDR), 2);
    put(&mut out, 1, 2);
    put(&mut out, 40, 2); // shentsize
    put(&mut out, 5, 2);
    put(&mut out, 4, 2); // shstrndx
    // PT_LOAD, R+X, the file from its start
    let filesz = text_off + code.len() as u32;
    for word in [1u32, 0, BASE, BASE, filesz, filesz, 5, 0x1000] {
        put(&mut out, u64::from(word), 4);
    }
    out.extend_from_slice(&code);
    out.extend_from_slice(&symtab);
    out.extend_from_slice(&strtab);
    out.extend_from_slice(&shstrtab);
    out.resize(shdrs_off as usize, 0);
    let section = |out: &mut Vec<u8>,
                   name: u32,
                   kind: u32,
                   flags: u32,
                   addr: u32,
                   off: u32,
                   size: u32,
                   link: u32,
                   info: u32,
                   align: u32,
                   entsize: u32| {
        for word in [name, kind, flags, addr, off, size, link, info, align, entsize] {
            put(out, u64::from(word), 4);
        }
    };
    section(&mut out, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    section(&mut out, text_name, 1, 6, text_at, text_off, code.len() as u32, 0, 0, 2, 0);
    section(&mut out, symtab_name, 2, 0, 0, symtab_off, symtab.len() as u32, 3, 1, 4, 16);
    section(&mut out, strtab_name, 3, 0, 0, strtab_off, strtab.len() as u32, 0, 0, 1, 0);
    section(&mut out, shstrtab_name, 3, 0, 0, shstrtab_off, shstrtab.len() as u32, 0, 0, 1, 0);
    Ok(out)
}
