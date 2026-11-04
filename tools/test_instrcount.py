"""An unchanged binary counts the same twice: the old driver sorted and printed
its timings, which moved the instruction count by ~15 per repetition (m64
round 5), so a 30-instruction regression could not be told from noise."""
import os
import shutil
import subprocess

import pytest

HERE = os.path.dirname(__file__)
SRC = "int bench_main_(void){volatile long s=0;for(long i=0;i<2000;i++)s+=i*i;return (int)s;}\n"


def build(tmp, extra=""):
    c = tmp / "k.c"
    c.write_text(SRC.replace("return", extra + "return"))
    exe = tmp / "k"
    subprocess.run(["gcc", "-O1", "-static", "-o", str(exe), str(c), os.path.join(HERE, "instrcount_driver.c")], check=True)
    return str(exe)


def count(exe):
    out = subprocess.run([os.path.join(HERE, "instrcount.sh"), exe], capture_output=True, text=True, check=True)
    return int(out.stdout)


needs = pytest.mark.skipif(not (shutil.which("perf") and shutil.which("gcc") and shutil.which("setarch")), reason="perf, gcc and setarch")


@needs
def test_an_unchanged_binary_counts_the_same_twice(tmp_path):
    exe = build(tmp_path)
    try:
        first = count(exe)
    except subprocess.CalledProcessError:
        pytest.skip("perf not permitted")
    assert first > 2000 and count(exe) == first and count(exe) == first


@needs
def test_thirty_more_instructions_show(tmp_path):
    try:
        base = count(build(tmp_path))
    except subprocess.CalledProcessError:
        pytest.skip("perf not permitted")
    more = count(build(tmp_path, "s+=1;" * 30))
    assert more - base >= 30
