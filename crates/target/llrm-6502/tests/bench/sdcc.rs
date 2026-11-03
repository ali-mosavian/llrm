//! SDCC (`-mmos6502`, `-mmos65c02`) on the same kernels, run in sim65.
//!
//! The linked image (Intel hex) is wrapped in a sim65 header and run by the
//! simulator of the other columns, so the clocks are the same instrument's
//! (`ucsim_mos6502` times `lda (zp),y` at 4 and `nop` at 1). A C driver calls
//! the kernel 1 and 101 times and exits through sim65's `exit` ($FFF9) with 0
//! when the result matched `expect`; the clocks and instructions of the run
//! give the cost of a call, `(T101 - T1) / 100`. The driver's loop and the
//! handling of the result are SDCC's own, so they are taken out by running the
//! same driver against an empty `void` function called without arguments (the
//! loop, the result and an empty call), and the `jsr`/`rts` of the empty call
//! are added back (12 clocks, 2 instructions): a call is its argument set-up
//! as SDCC makes it, the body and the return, as in the other columns. Bytes =
//! the linked image with the kernel minus the image with an empty stub of the
//! same signature, plus the stub's `rts`; SDCC's library routines the kernel
//! calls (multiply, divide, `memcpy`) are in the image, as cc65's are.

use std::path::Path;
use std::process::Command;

use super::{Arg, BUF, Kernel, LZ, Row, kernel_source, run_sim, work};

/// `sdcc` and `sim65` versions, for the table's footer.
pub fn versions() -> String {
    let first = |tool: &str, arg: &str| {
        let out = Command::new(tool).arg(arg).output().ok();
        out.map(|o| {
            let text = String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr);
            text.lines().next().unwrap_or("").to_owned()
        })
        .unwrap_or_default()
    };
    let sim = std::env::var("LLRM_SIM65").unwrap_or_else(|_| "sim65".to_owned());
    let sdcc = first("sdcc", "--version");
    let version = sdcc.rsplit("TD- ").next().unwrap_or("");
    format!("sdcc {}, {}", version.split(" (").next().unwrap_or(""), first(&sim, "--version"))
}

/// The C declaration of the kernel in its source: `ret name(params)`.
pub fn prototype(
    kernel: &Kernel,
    source: &str,
) -> Result<(String, String), String> {
    // a kernel of `kernels.c` sits in its own `#ifdef`
    let text = std::fs::read_to_string(source).map_err(|e| e.to_string())?;
    let section = match text.find(&format!("#ifdef K_{}\n", kernel.name)) {
        Some(at) if kernel.hand.is_none() => &text[at..],
        _ => &text[..],
    };
    let mut from = 0;
    while let Some(found) = section[from..].find(&format!("{}(", kernel.name)) {
        let at = from + found;
        from = at + 1;
        let before = &section[..at];
        let line_start = before.rfind('\n').map_or(0, |n| n + 1);
        let ret = before[line_start..].trim();
        let after = &section[at + kernel.name.len() + 1..];
        let Some(close) = after.find(')') else { continue };
        let params = after[..close].trim();
        let rest = after[close + 1..].trim_start();
        // a definition, not a call: its return type is on the line and a body
        // follows
        if rest.starts_with('{') && !ret.is_empty() && !ret.contains('=') && !ret.contains('(') {
            return Ok((ret.trim_start_matches("static ").to_owned(), params.to_owned()));
        }
    }
    Err(format!("no definition of {} in {source}", kernel.name))
}

fn argument(arg: Arg) -> String {
    match arg {
        Arg::B(v) | Arg::W(v) => v.to_string(),
        Arg::P(symbol, off) => format!("({symbol} + {off})"),
    }
}

/// The driver: the data of `driver()` in C, and the call loop.
pub fn driver(
    kernel: &Kernel,
    ret: &str,
    params: &str,
    calls: u32,
    with_args: bool,
    exits: &str,
) -> String {
    let mut c = String::new();
    let params = if params.is_empty() || !with_args { "void" } else { params };
    c.push_str(&format!("extern {ret} {}({params});\n", kernel.name));
    c.push_str(&format!("unsigned char buf[16] = {{{}}};\n", BUF.map(|b| b.to_string()).join(",")));
    c.push_str("unsigned char str[] = \"hello world\";\nunsigned char tab[8] = {2,5,8,11,14,17,20,23};\n");
    c.push_str(&format!("unsigned char lz[] = {{{LZ}}};\nunsigned char lzout[640];\nunsigned char dst[64];\n"));
    for (name, text) in [
        ("mix", "Hello, World 42!"),
        ("mix2", "hELLO, wORLD 42?"),
        ("num", "  -4829xyz"),
        ("pat", "o w"),
        ("set", "lo"),
    ] {
        c.push_str(&format!("unsigned char {name}[] = \"{text}\";\n"));
    }
    c.push_str(exits);
    // every datum stays linked whether or not the kernel (or its stub) reads it
    // (llvm-mos collects unused sections): one read of each, once, before the
    // loop
    c.push_str(
        "volatile unsigned char keep_sink;\nvoid keep(void) { keep_sink = buf[0] + str[0] + tab[0] + lz[0] + lzout[0] + dst[0] + mix[0] + mix2[0] + num[0] + pat[0] + set[0]; }\n",
    );
    let wide = kernel.wide_result || kernel.hand.as_ref().is_some_and(|h| h.rel.is_some());
    let result = if wide { "unsigned" } else { "unsigned char" };
    let args: Vec<String> = kernel.args.iter().filter(|_| with_args).map(|&a| argument(a)).collect();
    let call = format!("{}({})", kernel.name, args.join(", "));
    let value = match kernel.hand.as_ref().and_then(|h| h.rel) {
        Some(from) => format!("(unsigned)({call}) - (unsigned){from}"),
        None => call,
    };
    let want = if wide { kernel.expect } else { kernel.expect & 0xFF };
    c.push_str(&format!(
        "void main(void) {{\n  unsigned char i;\n  {result} r = 0;\n  keep();\n  for (i = 0; i < {calls}; i++) r = ({result})({value});\n  if (r == {want}U) done_ok(); else done_bad();\n}}\n"
    ));
    c
}

/// sim65's `exit` ($FFF9) with 0 when the result matched, 1 when not.
const SDCC_EXITS: &str = "void done_ok(void) { __asm lda #0 \n jmp 0xFFF9 __endasm; }\nvoid done_bad(void) { __asm lda #1 \n jmp 0xFFF9 __endasm; }\n";

/// The data bytes of an Intel hex file: the image the simulator loads.
fn image_bytes(path: &Path) -> Result<i64, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut total = 0;
    for line in text.lines() {
        let line = line.trim().trim_start_matches(':');
        if line.len() >= 8 && &line[6..8] == "00" {
            total += i64::from_str_radix(&line[0..2], 16).map_err(|e| e.to_string())?;
        }
    }
    Ok(total)
}

/// The image of `ihx` as a sim65 program: header, load address, reset address
/// (the program's reset vector), and the bytes from the lowest address up.
fn sim65_program(
    ihx: &Path,
    cmos: bool,
    out: &Path,
) -> Result<(), String> {
    let text = std::fs::read_to_string(ihx).map_err(|e| e.to_string())?;
    let mut image = std::collections::BTreeMap::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches(':');
        if line.len() < 10 || &line[6..8] != "00" {
            continue;
        }
        let byte = |at: usize| u8::from_str_radix(&line[at..at + 2], 16).map_err(|e| e.to_string());
        let (count, address) = (byte(0)? as usize, (usize::from(byte(2)?) << 8) | usize::from(byte(4)?));
        for n in 0..count {
            image.insert(address + n, byte(8 + 2 * n)?);
        }
    }
    let word = |at: usize| {
        u16::from(image.get(&at).copied().unwrap_or(0)) | u16::from(image.get(&(at + 1)).copied().unwrap_or(0)) << 8
    };
    let reset = word(0xFFFC);
    // sim65 keeps $FFF1 and up for its hooks: the vectors are not loaded
    let body: Vec<(usize, u8)> = image.into_iter().filter(|(at, _)| *at < 0xFFF1).collect();
    let (low, high) = (body.first().ok_or("empty image")?.0, body.last().unwrap().0);
    let mut file = b"sim65".to_vec();
    file.extend([2, u8::from(cmos), 0, low as u8, (low >> 8) as u8, reset as u8, (reset >> 8) as u8]);
    let mut bytes = vec![0u8; high - low + 1];
    for (at, value) in body {
        bytes[at - low] = value;
    }
    file.extend(bytes);
    std::fs::write(out, file).map_err(|e| e.to_string())
}

/// Runs `ihx` in sim65: whether the driver exited 0 (the result matched), the
/// clocks and the instructions executed.
fn run(
    ihx: &Path,
    cmos: bool,
) -> Result<(bool, i64, Option<i64>), String> {
    let program = ihx.with_extension("bin");
    sim65_program(ihx, cmos, &program)?;
    let (code, cycles, instructions) = run_sim(&program)?;
    Ok((code == 0, cycles, instructions))
}

fn tool(
    dir: &Path,
    args: &[&str],
) -> Result<(), String> {
    let out = Command::new("sdcc").current_dir(dir).args(args).output().map_err(|e| format!("sdcc: {e}"))?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr);
        let line = said.lines().find(|l| l.contains("error")).or_else(|| said.lines().next()).unwrap_or("failed");
        return Err(format!("sdcc: {}", line.chars().take(160).collect::<String>()));
    }
    Ok(())
}

/// The row of `kernel` built by SDCC for the NMOS 6502 or, with `cmos`, the
/// 65C02.
pub fn row(
    kernel: &Kernel,
    cmos: bool,
) -> Result<Row, String> {
    let tag = if cmos { "sdccm" } else { "sdcc" };
    let dir = work(kernel.name);
    let (define, source) = kernel_source(kernel);
    let (ret, params) = prototype(kernel, source.to_str().unwrap())?;
    let target = if cmos { "-mmos65c02" } else { "-mmos6502" };
    let kernel_object = format!("{tag}-k.rel");
    tool(&dir, &[target, &define, "-c", source.to_str().unwrap(), "-o", &kernel_object])?;
    // two stubs, each one `rts`: the kernel's own signature (for the bytes) and
    // none (for the loop: no argument set-up is made for it)
    let stub_params = if params.is_empty() { "void" } else { params.as_str() };
    for (suffix, signature) in [("s", stub_params), ("n", "void")] {
        std::fs::write(dir.join(format!("{tag}-{suffix}.c")), format!("void {}({signature}) {{}}\n", kernel.name))
            .unwrap();
        tool(&dir, &[target, "-c", &format!("{tag}-{suffix}.c"), "-o", &format!("{tag}-{suffix}.rel")])?;
    }
    // `k`: the kernel, `s`: its stub, `n`: the empty call without arguments
    let build = |calls: u32, which: &str| -> Result<std::path::PathBuf, String> {
        let name = format!("{tag}-{which}{calls}");
        std::fs::write(dir.join(format!("{name}.c")), driver(kernel, &ret, &params, calls, which != "n", SDCC_EXITS))
            .unwrap();
        tool(&dir, &[target, "-c", &format!("{name}.c"), "-o", &format!("{name}-d.rel")])?;
        let ihx = format!("{name}.ihx");
        let function = if which == "k" { kernel_object.clone() } else { format!("{tag}-{which}.rel") };
        tool(&dir, &[target, &format!("{name}-d.rel"), &function, "-o", &ihx])?;
        Ok(dir.join(ihx))
    };
    // the kernel must answer right, once and 101 times
    let one = build(1, "k")?;
    let (matched, t1, i1) = run(&one, cmos)?;
    if !matched {
        return Err(format!("wrong result: want {}", kernel.expect));
    }
    let (matched, t101, i101) = run(&build(101, "k")?, cmos)?;
    if !matched {
        return Err("wrong result after 101 calls".into());
    }
    let (_, n1, j1) = run(&build(1, "n")?, cmos)?;
    let (_, n101, j101) = run(&build(101, "n")?, cmos)?;
    let cycles = (t101 - t1) / 100 - (n101 - n1) / 100 + 12;
    let instructions =
        i1.zip(i101).zip(j1.zip(j101)).map(|((i1, i101), (j1, j101))| (i101 - i1) / 100 - (j101 - j1) / 100 + 2);
    let bytes = image_bytes(&one)? - image_bytes(&build(1, "s")?)? + 1;
    Ok(Row { bytes, body: bytes, cycles, instructions })
}
