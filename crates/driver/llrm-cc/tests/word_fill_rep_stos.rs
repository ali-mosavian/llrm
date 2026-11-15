//! fill_nest -O2 m32 filled 64 words a row by a loop (`mov word ptr
//! [ecx+edx*2], 4660; inc; cmp; jb`, 5 clk a word, 27,771 clk against gcc's
//! 24,667): the pattern fill formed was expanded to that loop before isel. m32
//! now lets isel make it `rep stosw`.

use std::path::Path;
use std::process::Command;

const SOURCE: &str = "short t[64][64];
void f(void)
{
    short y, x;
    for (y = 0; y < 64; ++y)
        for (x = 0; x < 64; ++x) t[y][x] = 4660;
}
";

#[test]
fn a_word_fill_nest_at_m32_speed_is_rep_stosw() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("wordfill-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("w.c"), SOURCE).unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_32-dos")
        // -fhosted: the default -ffreestanding turns the loop idioms off.
        .args(["-O2", "-w", "-S", "-march=i486", "-fhosted", "-o"])
        .arg(dir.join("w.s"))
        .arg(dir.join("w.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let text = std::fs::read_to_string(dir.join("w.s")).unwrap();
    assert!(text.contains("rep stosw") || text.contains("rep stosd"), "a store loop, no rep stos:\n{text}");
    assert!(!text.contains("4660\n    inc"), "{text}");
}
