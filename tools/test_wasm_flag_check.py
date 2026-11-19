import wasm_flag_check as w

# What llrm-cc ran for a wasm32 -Os cell (logged through a clang wrapper, uc/new dist 2026-11-09).
OURS = ("-target wasm32-unknown-unknown -ffreestanding -Os -Ibench -Iruntime/shared/posix -S -emit-llvm "
        "-Xclang -disable-llvm-passes -Xclang -disable-O0-optnone -o /tmp/x.ll prog.c")
PIN = ["--target=wasm32", "-ffreestanding", "-Os", "-c", "prog.c", "-Ibench", "-o", "p.o"]


def test_the_real_command_lines_agree():
    assert w.mismatch(OURS.split(), PIN) == ""


def test_a_flag_on_one_side_only_is_refused_not_measured():
    """arm64 shipped 24/72 cells off by 4..16 B because only the externals had -fomit-frame-pointer."""
    assert "fomit-frame-pointer" in w.mismatch(OURS.split(), PIN + ["-fomit-frame-pointer"])
    assert "fomit-frame-pointer" in w.mismatch((OURS + " -fomit-frame-pointer").split(), PIN)


def test_a_missing_ffreestanding_is_refused():
    assert "ffreestanding" in w.mismatch(OURS.split(), [f for f in PIN if f != "-ffreestanding"])


def test_a_different_level_or_triple_is_refused():
    assert w.mismatch(OURS.split(), ["--target=wasm32", "-ffreestanding", "-O2"])
    assert "triple" in w.mismatch(OURS.split(), ["--target=wasm64", "-ffreestanding", "-Os"])
