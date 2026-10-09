//! Zero data is uninitialised data, near and far alike: the image stores none
//! of it, and the program's start-up zeroes it, because DOS does not clear
//! memory past the image.
//!
//! DOSBox starts with zeroed RAM, so a plain run proves nothing: every program
//! here runs under `tools/dosbatch/dirty.asm`, which fills free memory with
//! 0A5h first.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_target::Target;

const PROGRAM: &str = "extern void report(long v);
static char far zfar[40000];
static char znear[300];
static char far ifar[4] = {1, 2, 3, 4};
int main(void)
{
    long far_set = 0, near_set = 0, kept = 0;
    unsigned i;
    for (i = 0; i < 40000u; i++) if (zfar[i]) far_set++;
    for (i = 0; i < 300; i++) if (znear[i]) near_set++;
    for (i = 0; i < 4; i++) kept += ifar[i];
    zfar[39999u] = 1;
    report(far_set);
    report(near_set);
    report(kept);
    return 0;
}
";

struct Lab {
    bin: PathBuf,
    root: PathBuf,
    dir: tempfile::TempDir,
}

impl Lab {
    fn new() -> Self {
        let bin = Path::new(env!("CARGO_BIN_EXE_llrm-c")).parent().unwrap().to_owned();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).to_owned();
        Self { bin, root, dir: tempfile::tempdir().unwrap() }
    }

    fn path(
        &self,
        name: &str,
    ) -> PathBuf {
        self.dir.path().join(name)
    }

    fn run(
        &self,
        program: &str,
        args: &[&str],
    ) {
        let done = Command::new(self.bin.join(program)).args(args).current_dir(self.dir.path()).output().unwrap();
        assert!(
            done.status.success(),
            "{program} {args:?}: {}{}",
            String::from_utf8_lossy(&done.stdout),
            String::from_utf8_lossy(&done.stderr)
        );
    }

    fn assemble(
        &self,
        source: &Path,
        object: &str,
        define: &[&str],
    ) {
        let out = format!("-Fo{}", self.path(object).display());
        let mut args = vec!["-q", "-c", "-Cp", "-Zg", common::assembler(), out.as_str()];
        args.extend(define);
        let source = source.to_str().unwrap();
        args.push(source);
        self.run("jwasm", &args);
    }

    /// What the assembler is told of the OS layer and C's runtime description;
    /// `nozero` adds NOZERO, a start-up that does not zero.
    fn defines(nozero: bool) -> Vec<String> {
        let target = llrm_x86_m16::M16;
        let (layer, c) = (target.os_layer().unwrap(), target.runtime("c").unwrap());
        let mut defines: Vec<String> = layer
            .defines()
            .unwrap()
            .into_iter()
            .chain(c.defines().unwrap())
            .map(|(symbol, value)| format!("-D{symbol}={value}"))
            .collect();
        defines.extend(nozero.then(|| "-DNOZERO".to_owned()));
        defines
    }

    /// `PROGRAM` at the flag, linked with the OS layer's start-up (`nozero`:
    /// one that does not zero).
    fn exe(
        &self,
        name: &str,
        flag: &str,
        nozero: bool,
    ) -> PathBuf {
        let target = llrm_x86_m16::M16;
        let (layer, c) = (target.os_layer().unwrap(), target.runtime("c").unwrap());
        let defines = Self::defines(nozero);
        let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
        self.assemble(
            &Path::new(layer.directory).join(layer.string("start").unwrap()),
            &format!("{name}-start.obj"),
            &defines,
        );
        self.assemble(
            &Path::new(c.directory).join(c.string("init_file").unwrap()),
            &format!("{name}-init.obj"),
            &defines,
        );
        std::fs::write(self.path("t.c"), PROGRAM).unwrap();
        self.run("llrm-c", &["-Os", flag, "-march=i486", "t.c", "-o", &format!("{name}.obj")]);
        let exe = self.path(&format!("{name}.exe"));
        let (start, init, program) = (format!("{name}-start.obj"), format!("{name}-init.obj"), format!("{name}.obj"));
        self.run(
            "jwlink",
            &common::jwlink(&[
                "option",
                "quiet",
                "name",
                exe.to_str().unwrap(),
                "file",
                &start,
                "file",
                &init,
                "file",
                &program,
                "file",
                "ext.obj",
                "file",
                "os.obj",
            ]),
        );
        exe
    }

    /// Each of `exes` run under DIRTY.COM, what each printed.
    fn dirty(
        &self,
        exes: &[&PathBuf],
    ) -> Vec<String> {
        let mut conf = String::from(
            "[sdl]\nautolock=false\n[dosbox]\nmemsize=16\nstartbanner=false\n[autoexec]\n@echo off\nmount c .\nc:\n",
        );
        for (at, exe) in exes.iter().enumerate() {
            conf += &format!("dirty.com {} > OUT{at}.TXT\n", exe.file_name().unwrap().to_str().unwrap());
        }
        conf += "exit\n";
        std::fs::write(self.path("dosbox.conf"), conf).unwrap();
        self.run("dosbox-x", &["-nolog", "-exit", "-conf", "dosbox.conf"]);
        (0..exes.len())
            .map(|at| {
                std::fs::read_to_string(self.path(&format!("OUT{at}.TXT")))
                    .unwrap_or_default()
                    .replace("\r\n", " ")
                    .trim()
                    .to_owned()
            })
            .collect()
    }

    fn runtime(&self) {
        let target = llrm_x86_m16::M16;
        let (layer, c) = (target.os_layer().unwrap(), target.runtime("c").unwrap());
        let defines = Self::defines(false);
        let defines: Vec<&str> = defines.iter().map(String::as_str).collect();
        self.assemble(&Path::new(c.directory).join("ext.asm"), "ext.obj", &defines);
        self.assemble(&Path::new(layer.directory).join(layer.string("implementation").unwrap()), "os.obj", &defines);
        let out = format!("-Fo{}", self.path("dirty.com").display());
        self.run("jwasm", &["-q", "-bin", &out, self.root.join("tools/dosbatch/dirty.asm").to_str().unwrap()]);
    }
}

/// QCport stored 27,264 bytes of zeros in far data, and Nib's huge.nib 84,000.
/// Where the start-up zeroes the far uninitialised data they are not stored;
/// the zeros are there all the same, near and far, on memory DOS left dirty.
/// The same program on a start-up that does not zero reads the dirt,
/// which shows the run can see it.
#[test]
fn test_zero_far_data_is_not_stored_and_is_zero_on_dirty_memory() {
    let lab = Lab::new();
    lab.runtime();
    let bss = lab.exe("bss", "-mfar-bss", false);
    let unzeroed = lab.exe("raw", "-mfar-bss", true);
    let stored = lab.exe("stored", "-mno-far-bss", false);
    let printed = lab.dirty(&[&bss, &unzeroed, &stored]);
    // far bytes set, near bytes set, the initialised far array's sum.
    assert_eq!(printed[0], "0 0 10", "zeroed: {printed:?}");
    assert_eq!(printed[1], "40000 300 10", "a start-up that does not zero reads the 0A5h: {printed:?}");
    assert_eq!(printed[2], "0 0 10", "stored zeros: {printed:?}");
    let size = |exe: &PathBuf| std::fs::metadata(exe).unwrap().len();
    assert!(size(&bss) + 39_000 < size(&stored), "{} against {}", size(&bss), size(&stored));
}

/// Where the program's start-up does not zero (Borland's, Open Watcom's), far
/// zero data stays stored: the listing says so, and only `-mfar-bss` moves it.
#[test]
fn test_far_zero_data_moves_only_where_the_startup_zeroes_it() {
    let lab = Lab::new();
    std::fs::write(lab.path("t.c"), PROGRAM).unwrap();
    let listing = |flag: &str| {
        lab.run("llrm-c", &["-Os", flag, "t.c", "-S", "-o", "t.s"]);
        std::fs::read_to_string(lab.path("t.s")).unwrap()
    };
    let far = listing("-mfar-bss");
    assert!(
        far.contains("'FAR_BSS'") && far.contains("db 40000 dup (?)") && !far.contains("000h,000h,000h,000h"),
        "{far}"
    );
    let plain = listing("-mno-far-bss");
    assert!(!plain.contains("FAR_BSS") && plain.contains("000h,000h,000h,000h"));
}

/// Nib's start-up zeroes the far uninitialised data too: a huge array of zeros
/// is not stored (84 KB of the EXE of bench/huge) and reads as zeros on dirty
/// memory.
#[test]
fn test_a_nib_huge_array_of_zeros_is_not_stored_and_reads_zero_on_dirty_memory() {
    let lab = Lab::new();
    lab.runtime();
    std::fs::write(
        lab.path("zeros.nib"),
        "huge var z: i16[200, 201] = [[0] * 201] * 200

fn main() -> i16:
    let mut t: i32 = 0
    for r in 0..200:
        for c in 0..201:
            t += i32(z[r, c])
    print(t)
    return 0
",
    )
    .unwrap();
    let exe = lab.path("ZEROS.EXE");
    let done = Command::new(lab.root.join("tools/nib-build.sh"))
        .arg(lab.path("zeros.nib"))
        .arg(&exe)
        .arg("-Os")
        .env("TOOLCHAIN", &lab.bin)
        .env("LLRM_BIN", &lab.bin)
        .output()
        .unwrap();
    assert!(
        done.status.success(),
        "{}{}",
        String::from_utf8_lossy(&done.stdout),
        String::from_utf8_lossy(&done.stderr)
    );
    assert!(std::fs::metadata(&exe).unwrap().len() < 10_000, "the 80,400 zero bytes were stored");
    assert_eq!(lab.dirty(&[&exe]), ["0"]);
}
