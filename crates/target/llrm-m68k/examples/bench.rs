//! `bench [SET]`: each kernel in `bench/<SET>` (`kernels`, or `library`)
//! compiled by this compiler and by the pin's `llc` (-O2 and -O2 with
//! `optsize`), run under `qemu-m68k` with the same `_start` and data, and its
//! size and executed instruction count printed as a Markdown table. Needs the
//! pin's m68k build (`$LLVM_M68K`, required) and
//! `qemu-m68k`.

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_m68k::compile::{Item, Options, items, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::kernels::{Arg, data, library_data, library_table, program_with_data, source_in, table};
use llrm_m68k::object::{assemble, executable, write_executable};

/// Executed instructions counted before a run is called runaway.
const CAP: usize = 2_000_000;

/// The exit status and the instructions executed inside `k`. The run is
/// bounded: `timeout 10`, and the trace is counted from a pipe, never stored;
/// past [`CAP`] instructions the emulator is killed and the status is -1.
fn run(exe: &Path) -> (i32, usize) {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    let mut child = Command::new("timeout")
        .args(["10", "qemu-m68k", "-cpu", "m68000", "-one-insn-per-tb", "-d", "exec", "-D", "/dev/stderr"])
        .arg(exe)
        .stderr(Stdio::piped())
        .spawn()
        .expect("timeout runs qemu-m68k");
    let mut seen = 0;
    let mut inside = 0;
    for line in BufReader::new(child.stderr.take().unwrap()).lines().map_while(Result::ok) {
        seen += 1;
        inside += usize::from(line.ends_with(" k"));
        if seen > CAP {
            child.kill().ok();
            child.wait().ok();
            return (-1, inside);
        }
    }
    (child.wait().ok().and_then(|status| status.code()).unwrap_or(-1), inside)
}

/// The `.text` of an object, from `llvm-objdump -s`.
fn text_of(
    llvm: &Path,
    object: &Path,
) -> Vec<u8> {
    let out = Command::new(llvm.join("llvm-objdump")).args(["-s", "-j", ".text"]).arg(object).output().unwrap();
    let mut bytes = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines().skip_while(|line| !line.starts_with("Contents of")).skip(1)
    {
        let hex = line.trim_start().split("  ").next().unwrap_or("");
        for group in hex.split(' ').skip(1) {
            bytes.extend((0..group.len()).step_by(2).map(|at| u8::from_str_radix(&group[at..at + 2], 16).unwrap()));
        }
    }
    bytes
}

/// The `.text` of `object`, which must hold leaf code: no relocation in it.
fn leaf_text(
    llvm: &Path,
    object: &Path,
    leaf: bool,
) -> Vec<u8> {
    if !leaf {
        return text_of(llvm, object);
    }
    let relocations = Command::new(llvm.join("llvm-objdump")).arg("-r").arg(object).output().unwrap();
    let listing = String::from_utf8_lossy(&relocations.stdout).to_string();
    let in_text = listing.split("RELOCATION RECORDS FOR").find(|part| part.trim_start().starts_with("[.text]"));
    assert!(
        !in_text.is_some_and(|part| part.contains("R_68K")),
        "{} needs relocations: the bench kernels are leaf code",
        object.display()
    );
    text_of(llvm, object)
}

fn llc(
    llvm: &Path,
    ir: &Path,
    dir: &Path,
    optsize: bool,
    leaf: bool,
) -> Vec<u8> {
    let source = std::fs::read_to_string(ir).unwrap();
    let source = if optsize { source.replace(") {", ") optsize {") } else { source };
    let input = dir.join(if optsize { "os.ll" } else { "o2.ll" });
    std::fs::write(&input, source).unwrap();
    let object = input.with_extension("o");
    let status = Command::new(llvm.join("llc"))
        .args(["-mtriple=m68k-linux-gnu", "-mcpu=M68000", "-relocation-model=static", "-O2", "-filetype=obj"])
        .arg(&input)
        .arg("-o")
        .arg(&object)
        .status()
        .unwrap();
    assert!(status.success(), "llc failed on {}", ir.display());
    leaf_text(llvm, &object, leaf)
}

/// The `.text` of the C kernel compiled by m68k gcc, unpacked without root into
/// `$GCC_M68K`; `None` when it is unset or not there.
fn gcc(
    llvm: &Path,
    source: &Path,
    dir: &Path,
    opt: &str,
    leaf: bool,
) -> Option<Vec<u8>> {
    let root = PathBuf::from(std::env::var("GCC_M68K").ok()?);
    let driver = root.join("usr/bin/m68k-linux-gnu-gcc");
    if !driver.exists() {
        return None;
    }
    let object = dir.join(format!("gcc{opt}.o"));
    let status = Command::new(&driver)
        .env("LD_LIBRARY_PATH", root.join("usr/lib/x86_64-linux-gnu"))
        .arg(format!("-B{}/usr/lib/gcc-cross/m68k-linux-gnu/15/", root.display()))
        .arg(format!("-B{}/usr/m68k-linux-gnu/bin/", root.display()))
        .args(["-ffreestanding", "-nostdlib", "-fno-asynchronous-unwind-tables", "-mcpu=68000", opt, "-c"])
        .arg(source)
        .arg("-o")
        .arg(&object)
        .status()
        .ok()?;
    status.success().then(|| leaf_text(llvm, &object, leaf))
}

fn main() {
    let llvm = PathBuf::from(std::env::var("LLVM_M68K").expect("LLVM_M68K: the pin's m68k build, its bin directory"));
    let dir = std::env::temp_dir().join(format!("llrm-m68k-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let set = std::env::args().nth(1).unwrap_or_else(|| "kernels".to_owned());
    let kernel_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("bench").join(&set);
    let (rows, block): (Vec<(&str, Vec<Arg>, i32, bool)>, Vec<u8>) = if set == "library" {
        (library_table(), library_data())
    } else {
        (table().into_iter().map(|(name, args, expected)| (name, args, expected, true)).collect(), data())
    };
    // the routine `fnptr` is given a pointer to, compiled by this compiler for
    // every variant
    let helper = (set == "library").then(|| {
        let module = llrm_mir::parse::module(&source_in(&set, "h")).unwrap();
        items(&module, "h", Cpu::M68000).unwrap()
    });
    // the size of the routine alone: the runtime it calls is assembled beside
    // it to resolve the symbols, and taken off
    let measure = |code: &[Item], runtime: &[Item]| -> Result<usize, String> {
        let mut all = code.to_vec();
        all.extend(runtime.iter().cloned());
        Ok(assemble(&all, 0)?.code.len() - assemble(runtime, 0).map_or(0, |one| one.code.len()))
    };
    println!(
        "| kernel | raw B | ours B | ours -Os B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | raw insns | ours insns | ours -Os insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for (name, args, expected, leaf) in rows {
        let ir = kernel_dir.join(format!("{name}.ll"));
        // a routine that is not a leaf calls the runtime of `rt.ll`, compiled
        // with it as one module so the local labels of the two do not meet
        let mut text = source_in(&set, name);
        let mut runtime_names: Vec<&str> = Vec::new();
        if !leaf {
            text.push_str(source_in(&set, "rt").split_once('\n').map_or("", |(_, rest)| rest));
            runtime_names = vec!["__mulsi3", "__udivsi3", "__umodsi3", "__divsi3", "__modsi3"];
        }
        let module = llrm_mir::parse::module(&text).unwrap();
        let runtime: Vec<Item> =
            runtime_names.iter().flat_map(|runtime_name| items(&module, runtime_name, Cpu::M68000).unwrap()).collect();
        let mut cells = Vec::new();
        let mut verdicts = Vec::new();
        let mut counts = Vec::new();
        let variants: Vec<Result<(Vec<Item>, usize), String>> = vec![
            // the raw IR, as the overlay compiles it without the LSR pass `llc`
            // runs
            items_with(&module, "k", Cpu::M68000, Options { mid_end: false, ..Options::default() }).and_then(|code| {
                let size = measure(&code, &runtime)?;
                Ok((code, size))
            }),
            items(&module, "k", Cpu::M68000).and_then(|code| {
                let size = measure(&code, &runtime)?;
                Ok((code, size))
            }),
            // size wanted over clocks, as `-Os` or an `optsize` function
            items_with(&module, "k", Cpu::M68000, Options { size: true, ..Options::default() }).and_then(|code| {
                let size = measure(&code, &runtime)?;
                Ok((code, size))
            }),
            Ok({
                let bytes = llc(&llvm, &ir, &dir, false, leaf);
                let size = bytes.len();
                (vec![Item::Label("k".to_owned()), Item::Bytes(bytes)], size)
            }),
            Ok({
                let bytes = llc(&llvm, &ir, &dir, true, leaf);
                let size = bytes.len();
                (vec![Item::Label("k".to_owned()), Item::Bytes(bytes)], size)
            }),
        ];
        let mut variants = variants;
        for opt in ["-Os", "-O2"] {
            variants.push(match gcc(&llvm, &kernel_dir.join(format!("{name}.c")), &dir, opt, leaf) {
                Some(bytes) => {
                    let size = bytes.len();
                    Ok((vec![Item::Label("k".to_owned()), Item::Bytes(bytes)], size))
                }
                None => Err("no gcc".to_owned()),
            });
        }
        for (index, variant) in variants.into_iter().enumerate() {
            match variant {
                Ok((code, size)) => {
                    cells.push(size.to_string());
                    if !leaf {
                        counts.push("-".to_owned());
                        // our own code runs against the runtime of `rt.ll`; the
                        // references call libgcc, which is not linked here
                        if index < 3 {
                            let mut code = code;
                            code.extend(helper.clone().unwrap_or_default());
                            code.extend(runtime.clone());
                            let exe = dir.join(format!("{name}.elf"));
                            write_executable(&exe, &executable(&program_with_data(&args, code, &block)).unwrap())
                                .unwrap();
                            let (status, _) = run(&exe);
                            verdicts.push(if status == expected { "ok".to_owned() } else { format!("got {status}") });
                        } else {
                            verdicts.push("ok".to_owned());
                        }
                        continue;
                    }
                    let mut code = code;
                    code.extend(helper.clone().unwrap_or_default());
                    let exe = dir.join(format!("{name}.elf"));
                    write_executable(&exe, &executable(&program_with_data(&args, code, &block)).unwrap()).unwrap();
                    let (status, executed) = run(&exe);
                    counts.push(executed.to_string());
                    verdicts.push(if status == expected { "ok".to_owned() } else { format!("got {status}") });
                }
                Err(error) => {
                    cells.push("-".to_owned());
                    counts.push("-".to_owned());
                    verdicts.push(format!("fails: {error}"));
                }
            }
        }
        let verdict =
            if verdicts.iter().all(|one| one == "ok") { format!("{expected} all") } else { verdicts.join(" / ") };
        println!("| {name} | {} | {} |", cells.join(" | "), counts.join(" | ") + " | " + &verdict);
    }
    std::fs::remove_dir_all(&dir).ok();
}
