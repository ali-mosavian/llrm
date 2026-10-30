import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import gen

CLANG = os.environ.get("LLRM_PIN_CLANG", os.path.expanduser("~/scratch/llvm-build/bin/clang"))


def test_the_same_seed_and_shape_give_the_same_text_and_other_seeds_other_text():
    for shape in gen.SHAPES:
        assert gen.emit(3, shape) == gen.emit(3, shape)
        assert len({gen.emit(seed, shape) for seed in range(20)}) > 10, shape


def test_every_program_is_valid_c_with_a_g_and_a_callee():
    for shape in gen.SHAPES:
        for seed in range(25):
            text = gen.emit(seed, shape)
            assert " g(int a, int b)" in text and "int c0(int, int);" in text, text
            if os.path.exists(CLANG):
                done = subprocess.run([CLANG, "-fsyntax-only", "-x", "c", "-"], input=text, text=True, capture_output=True)
                assert done.returncode == 0, f"{shape} {seed}: {done.stderr}\n{text}"


def test_an_unknown_shape_is_refused_by_name():
    try:
        gen.emit(0, "nope")
    except ValueError as why:
        assert "nope" in str(why)
    else:
        raise AssertionError("accepted")
