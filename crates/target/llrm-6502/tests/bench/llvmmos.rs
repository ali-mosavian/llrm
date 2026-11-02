//! llvm-mos (`mos-sim-clang`, the SDK in `LLVM_MOS`, default
//! `~/scratch/fetch-llvm-mos-sdk/x/llvm-mos`) on the same kernels, run in its
//! own `mos-sim --cycles`, which is cycle-exact on the instructions checked
//! (`nop` 2, `lda (zp),y` 5, `inc zp` 5, taken branch 3). The method is
//! `sdcc.rs`'s: the C driver calls the kernel 1 and 101 times and exits with
//! status 0 when the result matched; a call is `(T101 - T1) / 100` less the
//! same driver's empty call without arguments, plus its `jsr`/`rts` (12
//! clocks); bytes = the image with the kernel minus the image with an empty
//! stub, plus the stub's `rts`. llvm-mos's own libc is linked, as cc65's is.
//! `-fno-lto`: its default links the kernel and the driver as one module and
//! folds `find(9)` to a constant.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::sdcc::{driver, prototype};
use super::{Kernel, Row, kernel_source, work};

fn sdk() -> PathBuf {
    std::env::var_os("LLVM_MOS")
        .map(PathBuf::from)
        .unwrap_or_else(
            || PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("scratch/fetch-llvm-mos-sdk/x/llvm-mos"),
        )
}

/// The compiler's version line, for the table's footer.
pub fn version() -> String {
    let out = Command::new(sdk().join("bin/mos-sim-clang")).arg("--version").output().ok();
    out.and_then(|o| {
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        let line = text.lines().next()?.to_owned();
        let at = line.rfind('/').map_or(line.len(), |n| n + 1);
        Some(format!("llvm-mos {}", line[at..].trim_end_matches(')').chars().take(10).collect::<String>()))
    })
    .unwrap_or_else(|| "llvm-mos (not found)".to_owned())
}

fn tool(
    dir: &Path,
    args: &[&str],
) -> Result<(), String> {
    let out = Command::new(sdk().join("bin/mos-sim-clang"))
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("mos-sim-clang: {e}"))?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr);
        let line = said.lines().find(|l| l.contains("error")).or_else(|| said.lines().next()).unwrap_or("failed");
        return Err(format!("mos-sim-clang: {}", line.chars().take(160).collect::<String>()));
    }
    Ok(())
}

/// Runs `image`: whether it exited 0 and the clocks.
fn run(
    image: &Path,
    cmos: bool,
) -> Result<(bool, i64), String> {
    let mut sim = Command::new(sdk().join("bin/mos-sim"));
    sim.arg("--cycles");
    if cmos {
        sim.arg("--cmos");
    }
    let out = sim.arg(image).output().map_err(|e| format!("mos-sim: {e}"))?;
    let text = String::from_utf8_lossy(&out.stderr).to_string() + &String::from_utf8_lossy(&out.stdout);
    let cycles = text
        .lines()
        .find_map(|l| l.trim().strip_suffix(" cycles")?.trim().parse::<i64>().ok())
        .ok_or("mos-sim printed no cycle count")?;
    Ok((out.status.code() == Some(0), cycles))
}

/// The bytes of a `mos-sim` image: every segment but the vectors at $FFFA.
fn image_bytes(path: &Path) -> Result<i64, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let (mut at, mut total) = (0, 0);
    while at + 4 <= data.len() {
        let address = usize::from(data[at]) | usize::from(data[at + 1]) << 8;
        let length = usize::from(data[at + 2]) | usize::from(data[at + 3]) << 8;
        if address != 0xFFFA {
            total += length as i64;
        }
        at += 4 + length;
    }
    Ok(total)
}

/// The row of `kernel` built by llvm-mos at `level` (`-O2` or `-Os`), for the
/// NMOS 6502 or, with `cmos`, the 65C02.
pub fn row(
    kernel: &Kernel,
    level: &str,
    cmos: bool,
) -> Result<Row, String> {
    if !sdk().join("bin/mos-sim-clang").exists() {
        return Err("no llvm-mos SDK".into());
    }
    let tag = format!("mos{}{}", level.trim_start_matches('-'), if cmos { "m" } else { "" });
    let dir = work(kernel.name);
    let (define, source) = kernel_source(kernel);
    let (ret, params) = prototype(kernel, source.to_str().unwrap())?;
    let cpu: &[&str] = if cmos { &["-fno-lto", "-mcpu=mosw65c02"] } else { &["-fno-lto"] };
    let kernel_object = format!("{tag}-k.o");
    let mut compile = vec![level, &define, "-c", source.to_str().unwrap(), "-o", &kernel_object];
    compile.extend(cpu);
    tool(&dir, &compile)?;
    let stub_params = if params.is_empty() { "void" } else { params.as_str() };
    for (suffix, signature) in [("s", stub_params), ("n", "void")] {
        std::fs::write(dir.join(format!("{tag}-{suffix}.c")), format!("void {}({signature}) {{}}\n", kernel.name))
            .unwrap();
        let (source, object) = (format!("{tag}-{suffix}.c"), format!("{tag}-{suffix}.o"));
        let mut refs = vec![level, "-c", source.as_str(), "-o", object.as_str()];
        refs.extend(cpu);
        tool(&dir, &refs)?;
    }
    let exits = "#include <stdlib.h>\nvoid done_ok(void) { exit(0); }\nvoid done_bad(void) { exit(1); }\n";
    let build = |calls: u32, which: &str| -> Result<PathBuf, String> {
        let name = format!("{tag}-{which}{calls}");
        std::fs::write(
            dir.join(format!("{name}.c")),
            format!("{}", driver(kernel, &ret, &params, calls, which != "n", exits)),
        )
        .unwrap();
        let function = if which == "k" { kernel_object.clone() } else { format!("{tag}-{which}.o") };
        let image = format!("{name}.bin");
        let source = format!("{name}.c");
        let mut args = vec![level, source.as_str(), function.as_str(), "-o", image.as_str()];
        args.extend(cpu);
        tool(&dir, &args)?;
        Ok(dir.join(image))
    };
    let one = build(1, "k")?;
    let (matched, t1) = run(&one, cmos)?;
    if !matched {
        return Err(format!("wrong result: want {}", kernel.expect));
    }
    let (matched, t101) = run(&build(101, "k")?, cmos)?;
    if !matched {
        return Err("wrong result after 101 calls".into());
    }
    let (_, n1) = run(&build(1, "n")?, cmos)?;
    let (_, n101) = run(&build(101, "n")?, cmos)?;
    let cycles = (t101 - t1) / 100 - (n101 - n1) / 100 + 12;
    let bytes = image_bytes(&one)? - image_bytes(&build(1, "s")?)? + 1;
    Ok(Row { bytes, body: bytes, cycles, instructions: None })
}
