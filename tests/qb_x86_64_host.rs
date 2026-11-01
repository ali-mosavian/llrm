//! A QB program compiled for x86-64 (`llrm-qb -m64`, ELF64), linked with the
//! host runtime on the headless device and run natively: the object, its module
//! header, the runtime's start-up and the screen it leaves.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Compiles `source` for x86-64, links it with the runtime, runs it; the text
/// its screen shows.
fn run(
    name: &str,
    source: &str,
) -> String {
    let dir: PathBuf = std::env::temp_dir().join(format!("{name}_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let bas = dir.join(format!("{name}.bas"));
    fs::write(&bas, source).unwrap();
    let object = dir.join(format!("{name}.o"));
    let compiled = Command::new(env!("CARGO_BIN_EXE_llrm-qb"))
        .args(["-m64", "-O2", "-fobject-format=elf"])
        .arg(&bas)
        .arg("-o")
        .arg(&object)
        .output()
        .expect("llrm-qb runs");
    assert!(compiled.status.success(), "llrm-qb: {}", String::from_utf8_lossy(&compiled.stderr));

    let qb = root().join("runtime/qb");
    let mut sources: Vec<PathBuf> = fs::read_dir(&qb)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "c"))
        .collect();
    for extra in [
        "posix/startup.c",
        "posix/block.c",
        "posix/ad.c",
        "posix/mathops.c",
        "posix/paintq.c",
        "posix/module.c",
        "posix/program.c",
        "posix/real.c",
        "dev/headless/headless.c",
        "dev/headless/main.c",
        "dev/host/video.c",
        "dev/host/vgadata.c",
        "dev/host/tick.c",
        "../shared/posix/os.c",
    ] {
        sources.push(qb.join(extra));
    }
    let program = dir.join(name);
    let mut link = Command::new("cc");
    link.args(["-std=gnu11", "-w", "-g", "-pthread", "-no-pie", "-D__cdecl="]);
    for include in ["posix", ".", "dev", "dev/headless", "dev/host", "../shared/posix"] {
        link.arg("-I").arg(qb.join(include));
    }
    let linked = link.args(&sources).arg(&object).arg("-lm").arg("-o").arg(&program).output().expect("cc runs");
    assert!(linked.status.success(), "link: {}", String::from_utf8_lossy(&linked.stderr));

    let done = Command::new(&program).output().expect("the program runs");
    let screen = String::from_utf8_lossy(&done.stdout).into_owned();
    assert!(done.status.success(), "the program failed:\n{screen}{}", String::from_utf8_lossy(&done.stderr));
    fs::remove_dir_all(&dir).ok();
    screen
}

/// `-m64` wrote an OMF-style object that no ELF linker takes (no module header
/// symbol, `$` in the runtime's names): the program could not be linked at all.
#[test]
#[cfg(target_arch = "x86_64")]
fn print_hello_runs_natively_on_x86_64() {
    assert_eq!(run("hello64", "PRINT \"HELLO\"\nEND\n").trim_end(), "HELLO");
}

/// DATESERIAL called B$DTSR, which no flat runtime defined: a program using it
/// did not link. A month or day out of range carries (month 14 of 1992 is
/// February 1993, day 0 of March the 29th of February), a year of 0 to 99 is
/// 19xx.
#[test]
#[cfg(target_arch = "x86_64")]
fn dateserial_counts_days_from_1899_12_30_and_carries_on_x86_64() {
    let screen = run(
        "dateserial64",
        "' dialect: vbdos\nPRINT DATESERIAL(1992, 2, 29); DATESERIAL(1992, 14, 1); DATESERIAL(1992, 3, 0); DATESERIAL(95, 1, 1); DATESERIAL(1900, 1, 1)\nEND\n",
    );
    assert_eq!(screen.split_whitespace().collect::<Vec<_>>(), ["33663", "34001", "33663", "34700", "2"]);
}

/// A CURRENCY printed, read, turned to text and back did not link: B$PECY,
/// B$PSCY, B$STCY, B$RDCY, B$FMKC and B$FCVC had no flat definition. The
/// extremes are exact (a double holds 16 digits, not their 19), the fifth place
/// rounds half away from zero, and USING shows the digits.
#[test]
#[cfg(target_arch = "x86_64")]
fn currency_is_exact_to_four_places_on_x86_64() {
    let screen = run(
        "currency64",
        "' dialect: pds71\nDIM c AS CURRENCY, d AS CURRENCY, e AS CURRENCY, f AS CURRENCY\nREAD c, d, e\nPRINT c; d; e\nf = CVC(MKC$(c))\nPRINT STR$(f); \"|\"; STR$(d); \"|\"; LTRIM$(STR$(0@)); \"|\"; STR$(-.05@)\nPRINT USING \"#.####\"; e\nWRITE e, d\nDATA 922337203685477.5807, -922337203685477.5808, 1.00005\nEND\n",
    );
    let words: Vec<&str> = screen.split_whitespace().collect();
    assert_eq!(
        words,
        [
            "922337203685477.5807",
            "-922337203685477.5808",
            "1.0001",
            "922337203685477.5807|-922337203685477.5808|0|-.05",
            "1.0001",
            "1.0001,-922337203685477.5808"
        ]
    );
}

/// B$SCLR was defined by no flat runtime, x86-64's included: a program with
/// CLEAR did not link. CLEAR zeroes numbers, strings and arrays.
#[test]
#[cfg(target_arch = "x86_64")]
fn clear_zeroes_the_module_variables_on_x86_64() {
    let screen = run(
        "clear64",
        "' dialect: qb45\nDIM a(3) AS INTEGER\nx% = 5: l& = 70000: s$ = \"hello\": a(2) = 9\nCLEAR\nPRINT STR$(x%); STR$(l&); s$; \"|\"; STR$(a(2))\nEND\n",
    );
    assert_eq!(screen.trim_end(), " 0 0| 0");
}
