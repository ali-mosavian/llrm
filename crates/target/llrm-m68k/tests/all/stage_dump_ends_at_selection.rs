//! `LLRM_MIR_STAGES` stopped at the shared pipeline's last stage, but the m68k
//! mid-end (LSR with the m68k TTI, exit-test and multiply narrowing) edits the
//! IR after it: the dump showed a pointer walk where selection read the
//! end-biased index, and compiling the dumped IR again ran LSR twice and gave a
//! third shape (a day of COPY-OS chasing a form the dump did not show).

use llrm_m68k::driver::{Level, TARGET_MID_END_STAGE, assembly_dumping, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@cells = global [256 x i16] zeroinitializer, align 2
define void @clear(i16 %n) {
  br label %l
l:
  %i = phi i32 [ 0, %0 ], [ %j, %l ]
  %p = getelementptr inbounds nuw [2 x i8], ptr @cells, i32 %i
  store i16 7, ptr %p, align 2
  %j = add nuw nsw i32 %i, 1
  %c = icmp eq i32 %j, 256
  br i1 %c, label %x, label %l
x:
  ret void
}";

#[test]
fn the_dump_ends_with_the_ir_after_the_targets_own_mid_end() {
    let dir = std::env::temp_dir().join(format!("llrm-stage-dump-{}", std::process::id()));
    assembly_dumping(IR, cpu_of("68000").unwrap(), Level::O2, Some(&dir)).expect("compiles");
    let last = dir.join(TARGET_MID_END_STAGE);
    let text = std::fs::read_to_string(&last).unwrap_or_else(|error| panic!("{}: {error}", last.display()));
    assert!(text.contains("define") && text.contains("@clear"), "{text}");
    let mut stages: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(".ll"))
        .collect();
    stages.sort();
    assert_eq!(stages.last().map(String::as_str), Some(TARGET_MID_END_STAGE), "{stages:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
