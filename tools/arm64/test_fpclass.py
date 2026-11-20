"""llvm.is.fpclass: every one of the 1024 masks, on f32 and f64, over the edge values and random bits, is the class
table of the IEEE value. Needs LLRM_ARM64_LLC (the llrm-arm64-llc binary) and unicorn."""
import os, random, subprocess, tempfile
import pytest

unicorn = pytest.importorskip('unicorn')
from unicorn import Uc, UC_ARCH_ARM64, UC_MODE_ARM
from unicorn.arm64_const import UC_ARM64_REG_CPACR_EL1, UC_ARM64_REG_Q0, UC_ARM64_REG_SP, UC_ARM64_REG_W0, UC_ARM64_REG_X30

LLC = os.environ.get('LLRM_ARM64_LLC')
HEADER = ('target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"\n'
          'target triple = "aarch64-unknown-linux-gnu"\n'
          'declare i1 @llvm.is.fpclass.f32(float, i32)\ndeclare i1 @llvm.is.fpclass.f64(double, i32)\n')
FORMATS = {32: ('f', 'float', 8, 23), 64: ('d', 'double', 11, 52)}


def classify(bits, width):
    _, _, e, m = FORMATS[width]
    sign, exp, frac = bits >> (width - 1) & 1, bits >> m & (1 << e) - 1, bits & (1 << m) - 1
    if exp == (1 << e) - 1:
        return (0x4 if sign else 0x200) if frac == 0 else (0x2 if frac >> (m - 1) & 1 else 0x1)
    if exp == 0:
        return (0x20 if sign else 0x40) if frac == 0 else (0x10 if sign else 0x80)
    return 0x8 if sign else 0x100


def values(width):
    _, _, e, m = FORMATS[width]
    ones, inf = (1 << m) - 1, ((1 << e) - 1) << m
    base = [0, 1, ones, 1 << m - 1, inf, inf | 1, inf | 1 << m - 1, inf | ones, 1 << m, 1 << m | 1, inf - 1,
            0x3f800000 if width == 32 else 0x3ff0000000000000]
    rng = random.Random(1)
    out = {b | s << width - 1 for b in base for s in (0, 1)} | {rng.getrandbits(width) for _ in range(40)}
    return sorted(out)


@pytest.mark.skipif(not LLC, reason='LLRM_ARM64_LLC is not set')
def test_every_mask_of_a_float_and_a_double_is_the_class_of_the_value():
    text = HEADER
    for m in range(1024):
        for width, (pre, ty, _, _) in FORMATS.items():
            text += (f'define i32 @{pre}{m}({ty} %x) {{\n %r = call i1 @llvm.is.fpclass.f{width}({ty} %x, i32 {m})\n'
                     f' %z = zext i1 %r to i32\n ret i32 %z\n}}\n')
    with tempfile.TemporaryDirectory() as out:
        source, obj, raw = (os.path.join(out, n) for n in ('t.ll', 't.o', 't.bin'))
        open(source, 'w').write(text)
        subprocess.run([LLC, '-Os', '-o', obj, source], check=True)
        subprocess.run(['aarch64-linux-gnu-objcopy', '-O', 'binary', '-j', '.text', obj, raw], check=True)
        names = subprocess.run(['aarch64-linux-gnu-nm', obj], check=True, capture_output=True, text=True).stdout
        syms = {f[2]: int(f[0], 16) for f in (l.split() for l in names.splitlines()) if len(f) == 3 and f[1] in 'Tt'}
        code = open(raw, 'rb').read()
    base, back = 0x10000, 0x90000
    emu = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
    emu.mem_map(base, 0x100000)
    emu.mem_write(base, code)
    emu.mem_map(0x400000, 0x10000)
    emu.reg_write(UC_ARM64_REG_CPACR_EL1, 0x300000)
    wrong = []
    for width, (pre, _, _, _) in FORMATS.items():
        for m in range(1024):
            for bits in values(width):
                emu.reg_write(UC_ARM64_REG_SP, 0x410000)
                emu.reg_write(UC_ARM64_REG_X30, back)
                emu.reg_write(UC_ARM64_REG_Q0, bits)
                try:
                    emu.emu_start(base + syms[f'{pre}{m}'], back, count=200)  # a stray branch is a failure, not a hang
                except unicorn.UcError:
                    wrong.append((pre, m, hex(bits), 'did not run'))
                    continue
                if emu.reg_read(UC_ARM64_REG_W0) & 1 != bool(classify(bits, width) & m):
                    wrong.append((pre, m, hex(bits)))
    assert not wrong, wrong[:10]
