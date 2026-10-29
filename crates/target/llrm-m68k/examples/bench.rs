//! `bench [DIR]`: each kernel in `bench/kernels` compiled by this compiler and
//! by the pin's `llc` (-O2 and -O2 with `optsize`), run under `qemu-m68k` with
//! the same `_start` and data, and its size and executed instruction count
//! printed as a Markdown table. Needs the pin's m68k build (`$LLVM_M68K`,
//! default `~/scratch/llvm-build-m68k/bin`) and `qemu-m68k`.

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_m68k::compile::{Item, Options, items, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::kernels::{program, source, table};
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
) -> Vec<u8> {
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
    leaf_text(llvm, &object)
}

/// The `.text` of the C kernel compiled by m68k gcc, unpacked without root into
/// `$GCC_M68K` (default `~/scratch/gcc-m68k/root`); `None` when it is not
/// there.
fn gcc(
    llvm: &Path,
    source: &Path,
    dir: &Path,
    opt: &str,
) -> Option<Vec<u8>> {
    let root = PathBuf::from(
        std::env::var("GCC_M68K").unwrap_or(format!("{}/scratch/gcc-m68k/root", std::env::var("HOME").ok()?)),
    );
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
    status.success().then(|| leaf_text(llvm, &object))
}

fn main() {
    let home = std::env::var("HOME").unwrap();
    let llvm = PathBuf::from(std::env::var("LLVM_M68K").unwrap_or(format!("{home}/scratch/llvm-build-m68k/bin")));
    let dir = std::env::temp_dir().join(format!("llrm-m68k-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let kernel_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/kernels");
    println!(
        "| kernel | raw B | ours B | llc -O2 B | llc -Os B | gcc -Os B | gcc -O2 B | raw insns | ours insns | llc -O2 | llc -Os | gcc -Os | gcc -O2 | result |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for (name, args, expected) in table() {
        let ir = kernel_dir.join(format!("{name}.ll"));
        let module = llrm_mir::parse::module(&source(name)).unwrap();
        let mut cells = Vec::new();
        let mut verdicts = Vec::new();
        let mut counts = Vec::new();
        let variants: Vec<Result<(Vec<Item>, usize), String>> = vec![
            // the raw IR, as the overlay compiles it without the LSR pass `llc`
            // runs
            items_with(&module, "k", Cpu::M68000, Options { mid_end: false, ..Options::default() }).map(|code| {
                let size = assemble(&code, 0).map(|one| one.code.len()).unwrap_or(0);
                (code, size)
            }),
            items(&module, "k", Cpu::M68000).map(|code| {
                let size = assemble(&code, 0).map(|one| one.code.len()).unwrap_or(0);
                (code, size)
            }),
            Ok({
                let bytes = llc(&llvm, &ir, &dir, false);
                let size = bytes.len();
                (vec![Item::Label("k".to_owned()), Item::Bytes(bytes)], size)
            }),
            Ok({
                let bytes = llc(&llvm, &ir, &dir, true);
                let size = bytes.len();
                (vec![Item::Label("k".to_owned()), Item::Bytes(bytes)], size)
            }),
        ];
        let mut variants = variants;
        for opt in ["-Os", "-O2"] {
            variants.push(match gcc(&llvm, &kernel_dir.join(format!("{name}.c")), &dir, opt) {
                Some(bytes) => {
                    let size = bytes.len();
                    Ok((vec![Item::Label("k".to_owned()), Item::Bytes(bytes)], size))
                }
                None => Err("no gcc".to_owned()),
            });
        }
        for variant in variants {
            match variant {
                Ok((code, size)) => {
                    let exe = dir.join(format!("{name}.elf"));
                    write_executable(&exe, &executable(&program(&args, code)).unwrap()).unwrap();
                    let (status, executed) = run(&exe);
                    cells.push(size.to_string());
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
