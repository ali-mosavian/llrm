"""kernel_bytes: text of the kernel's functions plus its literal pools.

Since 2026-11-05 the .rodata.cst* sections count too (an fp constant loaded
with adrp+ldr costs its 8 pool bytes); before, only text symbols counted and
a pool-using column read smaller than it is.

Code that no symbol covers counts too (a static helper whose object carries no symbol of its own: release
main's queens read 44 B, the helper's 220 B uncovered); nops and zero words between functions are padding.
"""
import subprocess


def pool_bytes(obj):
    t = 0
    for line in subprocess.run(['aarch64-linux-gnu-size', '-A', obj], capture_output=True, text=True).stdout.splitlines():
        f = line.split()
        if len(f) >= 2 and f[0].startswith('.rodata.cst'):
            t += int(f[1])
    return t


def uncovered_bytes(obj, ranges):
    """Bytes of instructions in .text that lie in none of `ranges` ((start, end) pairs); nop and zero words are padding."""
    t = 0
    for line in subprocess.run(['aarch64-linux-gnu-objdump', '-d', '-j', '.text', obj], capture_output=True, text=True).stdout.splitlines():
        f = line.split('\t')
        if len(f) < 3 or not f[0].strip().endswith(':'):
            continue
        at, word = int(f[0].strip()[:-1], 16), f[1].strip()
        if int(word, 16) == 0 or f[2].split()[0] == 'nop':
            continue
        if not any(lo <= at < hi for lo, hi in ranges):
            t += 4
    return t


def kernel_bytes(obj):
    t, ranges = 0, []
    for line in subprocess.run(['aarch64-linux-gnu-nm', '-S', '--defined-only', obj], capture_output=True, text=True).stdout.splitlines():
        f = line.split()
        if len(f) == 4 and f[2] in 'tT':
            ranges.append((int(f[0], 16), int(f[0], 16) + int(f[1], 16)))
            if f[3] not in ('main', 'report') and not f[3].startswith('mem'):
                t += int(f[1], 16)
    return t + uncovered_bytes(obj, ranges) + pool_bytes(obj)
