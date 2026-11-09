//! A call's stack arguments are the words pushed for that call alone.

use std::process::Command;

/// Two soft-float multiplies and two conversions: the `ILP` schedule opened
/// `__fixdfsi`'s one-word frame inside `__muldf3`'s six, so `__muldf3` ran with
/// seven words pushed and its arguments at the wrong addresses (tests/run/c
/// `object_extent_loop` reported 33564 for 82).
#[test]
fn a_call_has_only_its_own_stack_arguments_pushed() {
    let dir = std::env::temp_dir().join("llrm-call-frames-z80");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\ntarget triple = \"z80-unknown-none\"\n\
         @x = global double 1.5\n@z = global double 5.5\n\
         define i16 @main() {\n\
           %1 = load volatile double, ptr @x\n  %2 = fmul double %1, 4.0\n  %3 = fptosi double %2 to i32\n\
           %4 = load volatile double, ptr @z\n  %5 = fmul double %4, 16.0\n  %6 = fptosi double %5 to i32\n\
           call void @report(i32 %3)\n  call void @report(i32 %6)\n  ret i16 0\n}\n\
         declare void @report(i32)\n",
    )
    .unwrap();
    let out = dir.join("g.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    let text = std::fs::read_to_string(out).unwrap();
    let mut pushed = 0i32;
    // A large area is one sum, `ld r, #-n` / `add r, sp` / `ld sp, r` (the
    // frame's own comes before the first label).
    let (mut in_body, mut amount) = (false, 0i32);
    for line in text.lines().map(str::trim) {
        in_body |= line.ends_with(':') && line.starts_with('L');
        if let Some(n) = line.strip_prefix("ld hl, #").or_else(|| line.strip_prefix("ld iy, #")) {
            amount = n.parse().unwrap_or(0);
        } else if in_body && (line == "ld sp, hl" || line == "ld sp, iy") {
            pushed -= amount / 2;
        }
        if line == "push ix" || line == "pop ix" {
            continue;
        }
        if line.starts_with("push ") {
            pushed += 1;
        } else if line.starts_with("pop ") {
            pushed -= 1;
        } else if let Some(callee) = line.strip_prefix("call ") {
            let words = match callee {
                "__muldf3" => 6,
                "__fixdfsi" => 1,
                _ => 0,
            };
            assert_eq!(pushed, words, "{callee} runs with {pushed} words pushed:\n{text}");
        }
    }
}

/// Nine stack words were reserved by one sum and stored through `ix` (`ld bc, #112;
/// ld -2(ix), c; ld -1(ix), b`: 9 B / 31 T a word) because only areas up to
/// 16 B were pushed. Each is `ld bc, #k; push bc`, the last argument first.
#[test]
fn a_stack_area_over_sixteen_bytes_is_pushed_last_argument_first() {
    let dir = std::env::temp_dir().join("llrm-call-frames-z80-wide");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    let args: Vec<String> = (101..=112).map(|k| format!("i16 {k}")).collect();
    std::fs::write(
        &source,
        format!(
            "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\ntarget triple = \"z80-unknown-none\"\n\
             declare void @f({0})\ndefine void @g() {{\n  call void @f({1})\n  ret void\n}}\n",
            vec!["i16"; 12].join(", "),
            args.join(", ")
        ),
    )
    .unwrap();
    let out = dir.join("g.s");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-O2", "-o", out.to_str().unwrap(), source.to_str().unwrap()])
        .output()
        .expect("the driver runs");
    assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
    let text = std::fs::read_to_string(out).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let stores = lines.iter().filter(|line| line.starts_with("ld ") && line.contains("(ix)")).count();
    assert_eq!(stores, 0, "a stack argument is stored through ix:\n{text}");
    let mut pushed = Vec::new();
    for pair in lines.windows(2) {
        if pair[1] == "push bc" {
            pushed.push(pair[0].strip_prefix("ld bc, #").expect("a constant loaded for the push").to_owned());
        }
    }
    let want: Vec<String> = (104..=112).rev().map(|k| k.to_string()).collect();
    assert_eq!(pushed, want, "{text}");
}

/// The stores of a call's stack arguments came out of the scheduler in any
/// order (`10, 8, 6, 4, 0, 2`), a reload between them reused the pair just
/// stored, and the call kept its reserve: six `push bc` to open the area, then
/// a store through `ix` per word (`__udivdi3` of two loaded `i64`).
/// The stores are chained top word first, so every one is a push.
#[test]
fn stack_arguments_that_the_scheduler_would_reorder_are_pushed() {
    let dir = std::env::temp_dir().join("llrm-call-frames-z80-order");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("g.ll");
    std::fs::write(
        &source,
        "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\ntarget triple = \"z80-unknown-none\"\n\
         @x = global i64 0\n@y = global i64 0\n\
         define i64 @g() {\n\
           %a = load volatile i64, ptr @x\n  %b = load volatile i64, ptr @y\n  %q = udiv i64 %a, %b\n\
           %c = load volatile i64, ptr @x\n  %r = add i64 %q, %c\n  %d = udiv i64 %r, %b\n  ret i64 %d\n}\n",
    )
    .unwrap();
    for level in ["-O2", "-Os"] {
        let out = dir.join("g.s");
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
            .args(["-S", level, "-o", out.to_str().unwrap(), source.to_str().unwrap()])
            .output()
            .expect("the driver runs");
        assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
        let text = std::fs::read_to_string(out).unwrap();
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        assert!(
            lines.windows(2).all(|pair| pair != ["push bc", "push bc"]),
            "{level}: a reserved area (push bc, push bc, ...) is stored into instead of pushed:\n{text}"
        );
    }
}

/// Fourteen volatile loads passed in source order: the last argument is pushed
/// first, so every value is live in reverse and spills; each stack word was
/// loaded, spilled, reloaded and pushed (17 B a word). Stored in place from
/// the spill (6 B a word) the call is smaller and faster; a call with few
/// spilled words keeps its pushes, which are 1 B where a store is 6.
#[test]
fn spilled_stack_words_are_stored_in_place_and_held_ones_are_pushed() {
    let call = |args: usize| {
        let calls = vec!["i16"; args].join(", ");
        let loads: String = (0..args)
            .map(|i| {
                format!(
                    "  %p{i} = getelementptr [{args} x i16], ptr @v, i16 0, i16 {i}\n  %a{i} = load volatile i16, ptr %p{i}\n"
                )
            })
            .collect();
        let values = (0..args).map(|i| format!("i16 %a{i}")).collect::<Vec<_>>().join(", ");
        format!(
            "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\ntarget triple = \"z80-unknown-none\"\n\
             @v = global [{args} x i16] zeroinitializer\ndeclare void @f({calls})\n\
             define void @g() {{\n{loads}  call void @f({values})\n  ret void\n}}\n"
        )
    };
    let dir = std::env::temp_dir().join("llrm-call-frames-z80-spilled");
    std::fs::create_dir_all(&dir).unwrap();
    let pushes = |args: usize, level: &str| {
        let source = dir.join(format!("g{args}.ll"));
        std::fs::write(&source, call(args)).unwrap();
        let out = dir.join(format!("g{args}.s"));
        let done = Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
            .args(["-S", level, "-o", out.to_str().unwrap(), source.to_str().unwrap()])
            .output()
            .expect("the driver runs");
        assert_eq!(done.status.code(), Some(0), "{}", String::from_utf8_lossy(&done.stderr));
        let text = std::fs::read_to_string(out).unwrap();
        text.lines().filter(|line| line.trim() == "push bc").count()
    };
    for level in ["-O2", "-Os"] {
        assert_eq!(pushes(14, level), 0, "{level}: spilled stack words are reloaded to be pushed");
        assert!(pushes(10, level) >= 4, "{level}: a call with few spilled words lost its pushes");
    }
}
