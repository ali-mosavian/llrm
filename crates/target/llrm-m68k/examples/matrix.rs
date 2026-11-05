//! `matrix [SET]`: the kernels of `bench/<SET>` (`kernels`, or `library`) built
//! by this compiler twice (`ours`: `llrm-cc` over the C source, as a user runs
//! it; `ours-ir`: the backend alone on the kernel's IR), m68k gcc (`$GCC_M68K`)
//! and the pin's clang (`$LLRM_PIN_CLANG` into `$LLVM_M68K/llc`) at O2 and Os
//! for the 68000, 68020, 68040 and 68060, run under `qemu-m68k` and priced as
//! `cycles` does. Prints the competitor-matrix rows: `target prog level
//! compiler code_bytes clocks instrs note`, tab separated. An empty field means
//! the instrument has no number; `note` says why.

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_m68k::compile::{Item, Options, items, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::isa::Cpu as Model;
use llrm_m68k::kernels::{Arg, data, library_data, library_table, program_with_data, source_in, table};
use llrm_m68k::object::{assemble, executable, write_executable};

#[allow(dead_code)]
#[path = "cycles.rs"]
mod cycles;

/// The `.text` of an object that holds leaf code: no relocation in it.
fn leaf_text(
    objdump: &Path,
    object: &Path,
) -> Result<Vec<u8>, String> {
    let relocations = Command::new(objdump).arg("-r").arg(object).output().map_err(|e| e.to_string())?;
    let listing = String::from_utf8_lossy(&relocations.stdout).to_string();
    let in_text = listing.split("RELOCATION RECORDS FOR").find(|part| part.trim_start().starts_with("[.text]"));
    if in_text.is_some_and(|part| part.contains("R_68K")) {
        return Err("calls a runtime routine (relocation in .text)".to_owned());
    }
    let out = Command::new(objdump).args(["-s", "-j", ".text"]).arg(object).output().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines().skip_while(|line| !line.starts_with("Contents of")).skip(1)
    {
        let hex = line.trim_start().split("  ").next().unwrap_or("");
        for group in hex.split(' ').skip(1) {
            bytes.extend((0..group.len()).step_by(2).map(|at| u8::from_str_radix(&group[at..at + 2], 16).unwrap()));
        }
    }
    Ok(bytes)
}

/// The kernel's C source built by m68k gcc into an object's text.
fn gcc_object(
    root: &Path,
    cpu: &str,
    level: &str,
    source: &Path,
    object: &Path,
) -> Result<(), String> {
    let status = Command::new(root.join("usr/bin/m68k-linux-gnu-gcc"))
        .env("LD_LIBRARY_PATH", root.join("usr/lib/x86_64-linux-gnu"))
        .arg(format!("-B{}/usr/lib/gcc-cross/m68k-linux-gnu/15/", root.display()))
        .arg(format!("-B{}/usr/m68k-linux-gnu/bin/", root.display()))
        .args(["-ffreestanding", "-nostdlib", "-fno-asynchronous-unwind-tables"])
        .arg(format!("-mcpu={cpu}"))
        .arg(format!("-{level}"))
        .arg("-c")
        .arg(source)
        .arg("-o")
        .arg(object)
        .status()
        .map_err(|e| e.to_string())?;
    status.success().then_some(()).ok_or_else(|| "gcc failed".to_owned())
}

/// This compiler as a user runs it: `llrm-cc` over the C source (clang front
/// end, the mid-end, the m68k backend) to assembly, assembled by gas. The pin's
/// `llvm-mc` does not parse our text (`subq.l`), and gas lengthens a plain
/// forward `bcc` to 4 bytes, so branches become gas's relaxing `jcc`.
fn ours_object(
    llrm_cc: &Path,
    root: &Path,
    cpu: &str,
    level: &str,
    source: &Path,
    object: &Path,
) -> Result<(), String> {
    let asm = object.with_extension("s");
    let status = Command::new(llrm_cc)
        .env("LLRM_CC_TARGET", format!("m68k-{cpu}"))
        .arg(format!("-{level}"))
        .arg("-S")
        .arg(source)
        .arg("-o")
        .arg(&asm)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("llrm-cc failed".to_owned());
    }
    let text = std::fs::read_to_string(&asm).map_err(|e| e.to_string())?;
    std::fs::write(&asm, format!(".text\n{}", relaxing_branches(&text))).map_err(|e| e.to_string())?;
    let status = Command::new(root.join("usr/bin/m68k-linux-gnu-as"))
        .env("LD_LIBRARY_PATH", root.join("usr/lib/x86_64-linux-gnu"))
        .arg(format!("-m{cpu}"))
        .arg("-o")
        .arg(object)
        .arg(&asm)
        .status()
        .map_err(|e| e.to_string())?;
    status.success().then_some(()).ok_or_else(|| "as failed".to_owned())
}

/// Plain `bcc label` lines as gas's relaxing `jcc label`, which picks the short
/// form when it reaches, as the pin's MC and our assembler do.
fn relaxing_branches(text: &str) -> String {
    const CONDITIONS: [&str; 16] =
        ["ra", "hi", "ls", "cc", "cs", "ne", "eq", "vc", "vs", "pl", "mi", "ge", "lt", "gt", "le", "hs"];
    text.lines()
        .map(|line| {
            let mut words = line.split_whitespace();
            let (Some(op), Some(target), None) = (words.next(), words.next(), words.next()) else {
                return line.to_owned();
            };
            match op.strip_prefix('b') {
                Some(cond) if CONDITIONS.contains(&cond) && !target.starts_with('#') => format!("\tj{cond}\t{target}"),
                _ => line.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The pin's clang (front end and `-O` pipeline, to IR) into the pin's `llc`:
/// the pin's clang is built for x86 only, and the distro's clang 20 miscompiles
/// the kernels (a `move` between a `cmp` and its branch).
fn clang_object(
    clang: &Path,
    llc: &Path,
    cpu: &str,
    level: &str,
    source: &Path,
    object: &Path,
) -> Result<(), String> {
    let ir = object.with_extension("ll");
    let status = Command::new(clang)
        .args(["--target=m68k-linux-gnu", "-ffreestanding", "-fno-asynchronous-unwind-tables", "-fno-pic"])
        .arg(format!("-mcpu={cpu}"))
        .arg(format!("-{level}"))
        .args(["-S", "-emit-llvm"])
        .arg(source)
        .arg("-o")
        .arg(&ir)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("clang failed".to_owned());
    }
    let status = Command::new(llc)
        .args(["-mtriple=m68k-linux-gnu", "-relocation-model=static", "-O2", "-filetype=obj"])
        .arg(format!("-mcpu=M{cpu}"))
        .arg(&ir)
        .arg("-o")
        .arg(object)
        .status()
        .map_err(|e| e.to_string())?;
    status.success().then_some(()).ok_or_else(|| "llc failed".to_owned())
}

fn main() {
    let gcc_root = PathBuf::from(std::env::var("GCC_M68K").expect("GCC_M68K: the unpacked m68k binutils root"));
    let clang = PathBuf::from(std::env::var("LLRM_PIN_CLANG").expect("LLRM_PIN_CLANG: the pin's clang"));
    let llrm_cc =
        PathBuf::from(std::env::var("LLRM_CC").expect("LLRM_CC: the llrm-cc binary, llrm-m68k-llc beside it"));
    let llc = PathBuf::from(std::env::var("LLVM_M68K").expect("LLVM_M68K: the pin's m68k build, its bin directory"))
        .join("llc");
    let objdump = gcc_root.join("usr/bin/m68k-linux-gnu-objdump");
    let dir = std::env::temp_dir().join(format!("llrm-m68k-matrix-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let set = std::env::args().nth(1).unwrap_or_else(|| "kernels".to_owned());
    let kernel_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("bench").join(&set);
    let (rows, block): (Vec<(&str, Vec<Arg>, i32, bool)>, Vec<u8>) = if set == "library" {
        (library_table(), library_data())
    } else {
        (table().into_iter().map(|(name, args, expected)| (name, args, expected, true)).collect(), data())
    };
    let cpus = [
        ("68000", Cpu::M68000, Model::M68000),
        ("68020", Cpu::M68020, Model::M68020),
        ("68040", Cpu::M68040, Model::M68040),
        ("68060", Cpu::M68060, Model::M68060),
    ];
    // `MATRIX_CPUS=68040,68060` keeps the named columns
    let only = std::env::var("MATRIX_CPUS").ok();
    for (cpu_name, cpu, model) in cpus {
        if only.as_ref().is_some_and(|only| !only.split(',').any(|one| one == cpu_name)) {
            continue;
        }
        // `fnptr` is handed a pointer to `h`, compiled by this compiler for
        // every column
        let helper = (set == "library").then(|| {
            let module = llrm_mir::parse::module(&source_in(&set, "h")).unwrap();
            items(&module, "h", cpu).unwrap()
        });
        for (name, args, expected, leaf) in &rows {
            let mut text = source_in(&set, name);
            let mut runtime_names: Vec<&str> = Vec::new();
            if !leaf {
                text.push_str(source_in(&set, "rt").split_once('\n').map_or("", |(_, rest)| rest));
                runtime_names = vec!["__mulsi3", "__udivsi3", "__umodsi3", "__divsi3", "__modsi3"];
            }
            let module = llrm_mir::parse::module(&text).unwrap();
            let runtime: Vec<Item> = runtime_names.iter().flat_map(|one| items(&module, one, cpu).unwrap()).collect();
            for level in ["O2", "Os"] {
                let row =
                    |compiler: &str, size: Option<usize>, clocks: Option<u64>, insns: Option<usize>, note: &str| {
                        let cell = |value: Option<String>| value.unwrap_or_default();
                        println!(
                            "m68k-{cpu_name}\t{name}\t{level}\t{compiler}\t{}\t{}\t{}\t{note}",
                            cell(size.map(|n| n.to_string())),
                            cell(clocks.map(|n| n.to_string())),
                            cell(insns.map(|n| n.to_string())),
                        );
                    };
                // clocks and instructions inside `k` of one variant's code, and
                // the program's result
                let measure = |code: Vec<Item>, id: &str| -> (Option<u64>, Option<usize>, String) {
                    let mut code = code;
                    code.extend(helper.clone().unwrap_or_default());
                    code.extend(runtime.clone());
                    let exe = dir.join(format!("{name}_{cpu_name}_{level}_{id}.elf"));
                    write_executable(&exe, &executable(&program_with_data(args, code, &block)).unwrap()).unwrap();
                    let status = Command::new("timeout")
                        .args(["10", "qemu-m68k", "-cpu", &format!("m{cpu_name}")])
                        .arg(&exe)
                        .status()
                        .ok()
                        .and_then(|s| s.code());
                    if status != Some(*expected) {
                        return (None, None, format!("wrong result: {status:?}, want {expected}"));
                    }
                    let total = cycles::price(&gcc_root, &exe, &[model], false);
                    if total.insns == 0 {
                        return (None, None, "priced no instruction".to_owned());
                    }
                    let note = match (total.partial, total.unknown) {
                        (_, unknown) if unknown > 0 => format!("{unknown} executed instructions unpriced"),
                        (true, _) => "data-dependent term at its bound".to_owned(),
                        _ => String::new(),
                    };
                    (Some(total.clocks[0]), Some(total.insns), note)
                };

                let options = Options { size: level == "Os", ..Options::default() };
                match items_with(&module, "k", cpu, options) {
                    Ok(code) => {
                        let size = {
                            let mut all = code.clone();
                            all.extend(runtime.iter().cloned());
                            assemble(&all, 0).unwrap().code.len()
                                - assemble(&runtime, 0).map_or(0, |one| one.code.len())
                        };
                        if *leaf {
                            let (clocks, insns, note) = measure(code, "ours-ir");
                            row("ours-ir", Some(size), clocks, insns, &note);
                        } else {
                            row("ours-ir", Some(size), None, None, "calls the runtime: bytes only");
                        }
                    }
                    Err(error) => row("ours-ir", None, None, None, &format!("compile failed: {error}")),
                }

                for compiler in ["ours", "gcc", "clang"] {
                    if !leaf {
                        row(compiler, None, None, None, "n/a: calls the runtime, not linked (ours-ir: bytes only)");
                        continue;
                    }
                    let source = kernel_dir.join(format!("{name}.c"));
                    let object = dir.join(format!("{name}_{cpu_name}_{level}_{compiler}.o"));
                    let built = if compiler == "ours" {
                        ours_object(&llrm_cc, &gcc_root, cpu_name, level_flag(level), &source, &object)
                    } else if compiler == "gcc" {
                        gcc_object(&gcc_root, cpu_name, level_flag(level), &source, &object)
                    } else {
                        clang_object(&clang, &llc, cpu_name, level_flag(level), &source, &object)
                    };
                    match built.and_then(|()| leaf_text(&objdump, &object)) {
                        Ok(bytes) => {
                            let size = bytes.len();
                            let (clocks, insns, note) =
                                measure(vec![Item::Label("k".to_owned()), Item::Bytes(bytes)], compiler);
                            row(compiler, Some(size), clocks, insns, &note);
                        }
                        Err(error) => row(compiler, None, None, None, &format!("n/a: {error}")),
                    }
                }
            }
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// The `-O` flag of a level.
fn level_flag(level: &str) -> &'static str {
    if level == "Os" { "Os" } else { "O2" }
}
