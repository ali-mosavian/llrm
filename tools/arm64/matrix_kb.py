"""kernel_bytes: text of the kernel's functions plus its literal pools.

Since 2026-11-05 the .rodata.cst* sections count too (an fp constant loaded
with adrp+ldr costs its 8 pool bytes); before, only text symbols counted and
a pool-using column read smaller than it is.
"""
import subprocess


def pool_bytes(obj):
    t = 0
    for line in subprocess.run(['aarch64-linux-gnu-size', '-A', obj], capture_output=True, text=True).stdout.splitlines():
        f = line.split()
        if len(f) >= 2 and f[0].startswith('.rodata.cst'):
            t += int(f[1])
    return t


def kernel_bytes(obj):
    t = 0
    for line in subprocess.run(['aarch64-linux-gnu-nm', '-S', '--defined-only', obj], capture_output=True, text=True).stdout.splitlines():
        f = line.split()
        if len(f) == 4 and f[2] in 'tT' and f[3] not in ('main', 'report') and not f[3].startswith('mem'): t += int(f[1], 16)
    return t + pool_bytes(obj)
