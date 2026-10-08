"""The kernel stays a call and the rest inlines: why the table no longer builds with -fno-inline-functions."""
import shutil
import subprocess
from pathlib import Path

import pytest

import wrap

SOURCE = """extern void report(long value);
static int helper(int x) { return x * 3 + 1; }
long bench_k(int n)
{
    int i; long s = 0;
    for (i = 0; i < n; ++i) s += helper(i);
    return s;
}

int main(void)
{
    report(bench_k(100));
    return 0;
}
"""


def test_main_calls_the_kernel_through_a_volatile_pointer():
    text = wrap.wrapped("k", SOURCE)
    assert "long (*volatile bench_k_via)(int n) = bench_k;" in text
    assert "report((*bench_k_via)(100));" in text
    assert text.index("bench_k_via)(int n) = bench_k") < text.index("int main")


@pytest.mark.skipif(shutil.which("gcc") is None, reason="needs gcc")
def test_the_helper_is_inlined_and_the_kernel_stays_a_call(tmp_path):
    """With -fno-inline-functions on every compiler the table measured no inlining at -O2 or -O3; with the flag off and
    the kernel unwrapped, gcc and clang inline the kernel into main and the harness never enters it."""
    (tmp_path / "k.c").write_text(wrap.wrapped("k", SOURCE))
    out = subprocess.run(["gcc", "-m32", "-O2", "-S", "-o", "-", str(tmp_path / "k.c")], capture_output=True, text=True, check=True).stdout
    calls = [line.split()[-1] for line in out.splitlines() if line.strip().startswith("call")]
    assert not any("helper" in call for call in calls), out
    assert any(call.startswith("*") or call == "eax" for call in calls) or "*%e" in out, out
    unwrapped = subprocess.run(["gcc", "-m32", "-O2", "-S", "-o", "-", "-x", "c", "-"], input=SOURCE, capture_output=True, text=True, check=True).stdout
    assert "call\tbench_k" not in unwrapped, "the premise: gcc inlines the unwrapped kernel into main"


def test_the_pointer_has_the_kernels_own_parameters():
    """`long (*)()` for a kernel of `unsigned short` is an error in clang (lru, floats, sieve)."""
    assert "long (*volatile bench_k_via)(unsigned short n) = bench_k;" in wrap.wrapped("k", SOURCE.replace("bench_k(int n)", "bench_k(unsigned short n)"))
