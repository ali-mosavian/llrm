//! The Atari, Amiga and Mac images: headers, entry bytes, relocations. The
//! code is `_start` calling `add(40, 2)` and exiting; no emulator runs it
//! here (the Hatari test needs an EmuTOS image and is ignored).

use llrm_m68k::compile::{Item, items};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::{amiga, atari, mac};

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";
const ADD: &str = "define i32 @add(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}";

fn program(start: impl Fn(&str, &[i64]) -> Vec<Item>) -> Vec<Item> {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ADD}")).expect("parses");
    let mut all = start("add", &[40, 2]);
    all.extend(items(&module, "add", Cpu::M68000).expect("compiles"));
    all
}

fn long(
    bytes: &[u8],
    at: usize,
) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `_start` is 32 bytes (two pushes, `jsr add`, pop, `Pterm`); `add` follows
/// at 32. The `jsr` address is at text offset 14, so it is relocated.
#[test]
fn an_atari_prg_has_its_header_text_and_relocation() {
    let prg = atari::program(&program(atari::start)).unwrap();
    assert_eq!(hex(&prg[..28]), format!("601a{:08x}{}0000", 42, "00".repeat(20)));
    assert_eq!(hex(&prg[28..36]), "2f3c000000022f3c");
    assert_eq!(hex(&prg[28 + 24..28 + 32]), "3f003f3c004c4e41");
    assert_eq!(hex(&prg[28 + 42..]), "0000000e00");
}

#[test]
fn an_amiga_load_file_has_its_hunks_and_relocation() {
    let file = amiga::load_file(&program(amiga::start)).unwrap();
    let words: Vec<u32> = file.chunks(4).map(|word| u32::from_be_bytes(word.try_into().unwrap())).collect();
    // the header: no libraries, one hunk, hunks 0 to 0, its size (36 bytes: 9
    // longs, `_start` ends in `rts`), then the code
    assert_eq!(words[..9], [0x3F3, 0, 1, 0, 0, 9, 0x3E9, 9, 0x2f3c0000]);
    // after the 9 code longs: relocate offset 14 in hunk 0, end
    assert_eq!(words[8 + 9..], [0x3EC, 1, 0, 14, 0, 0x3F2]);
}

fn word(
    bytes: &[u8],
    at: usize,
) -> usize {
    usize::from(u16::from_be_bytes(bytes[at..at + 2].try_into().unwrap()))
}

/// A resource fork: header, data (`CODE 0`, `CODE 1`), map. The call is
/// `jsr (d16,%pc)`: from the extension word at 14 to `add` at 24.
#[test]
fn a_mac_resource_fork_holds_code_0_and_code_1() {
    let fork = mac::resource_file(&program(mac::start)).unwrap();
    let (data, map) = (long(&fork, 0) as usize, long(&fork, 4) as usize);
    assert_eq!((data, long(&fork, 8) as usize, long(&fork, 12) as usize), (256, 70, fork.len() - map));
    assert_eq!(map, data + 70);
    // the type list: one type, `CODE`, two resources, references at 10
    let types = map + word(&fork, map + 24);
    assert_eq!(
        (word(&fork, types), &fork[types + 2..types + 6], word(&fork, types + 6), word(&fork, types + 8)),
        (0, &b"CODE"[..], 1, 10)
    );
    // the references: ids 0 and 1, data offsets 0 and 28
    let first = types + 10;
    let refs: Vec<(usize, usize)> = (0..2)
        .map(|at| (word(&fork, first + 12 * at), long(&fork, first + 12 * at + 4) as usize & 0x00FF_FFFF))
        .collect();
    assert_eq!(refs, [(0, 0), (1, 28)]);
    // CODE 0: 40, 0, 8, 32; one entry at offset 4 loading segment 1
    assert_eq!(hex(&fork[data + 4..data + 28]), "00000028000000000000000800000020000 43f3c0001a9f0".replace(' ', ""));
    // CODE 1: its header words, then `_start`
    let code1 = data + 28;
    assert_eq!(long(&fork, code1) as usize, 38);
    assert_eq!(hex(&fork[code1 + 4..code1 + 8]), "00000001");
    assert_eq!(hex(&fork[code1 + 8 + 12..code1 + 8 + 16]), "4eba000a");
}

/// vamos (amitools) runs the hunk file; the process's return code is `d0`.
#[test]
#[ignore = "needs vamos: uv tool install amitools --with \"machine68k<0.4\" (0.4.1 breaks vamos 0.8.1)"]
fn an_amiga_load_file_returns_its_sum_under_vamos() {
    let vamos = which("vamos").expect("vamos is on PATH");
    let path = std::env::temp_dir().join(format!("llrm-m68k-{}.hunk", std::process::id()));
    std::fs::write(&path, amiga::load_file(&program(amiga::start)).unwrap()).unwrap();
    let out = std::process::Command::new(vamos).arg(&path).output().unwrap();
    std::fs::remove_file(&path).ok();
    // 40 + 2
    assert_eq!(out.status.code(), Some(42));
}

fn which(name: &str) -> Result<std::path::PathBuf, ()> {
    std::env::var_os("PATH")
        .and_then(|paths| std::env::split_paths(&paths).map(|dir| dir.join(name)).find(|path| path.exists()))
        .ok_or(())
}

/// Hatari with EmuTOS runs the PRG, which ends in `Pterm(42)`. The image is
/// `$EMUTOS_IMG` (EmuTOS 1.4 `etos256us.img`, from emutos.sourceforge.io; not
/// in the repo).
#[test]
#[ignore = "needs hatari and an EmuTOS image in $EMUTOS_IMG"]
fn an_atari_prg_ends_with_the_sum_under_hatari() {
    let image = std::env::var("EMUTOS_IMG").expect("EMUTOS_IMG names an EmuTOS image");
    let dir = std::env::temp_dir().join(format!("llrm-m68k-hatari-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ADD.PRG"), atari::program(&program(atari::start)).unwrap()).unwrap();
    let out = std::process::Command::new("hatari")
        .current_dir(&dir)
        .env("SDL_VIDEODRIVER", "dummy")
        .env("SDL_AUDIODRIVER", "dummy")
        .args(["--tos", &image, "--machine", "st", "--fast-boot", "true", "--run-vbls", "900"])
        .args(["--log-level", "debug", "--trace", "gemdos", "ADD.PRG"])
        .output()
        .unwrap();
    std::fs::remove_dir_all(&dir).ok();
    let log = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(log.contains("Pterm(42)"), "no Pterm(42) in the GEMDOS trace");
}
