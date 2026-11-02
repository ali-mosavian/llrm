#!/usr/bin/env python3
"""Writes llvmmos_mulsi3_v2.s and _v3.s: shift-add 32x32->32 multiply in llvm-mos's __mulsi3 ABI (a in A:X:rc2:rc3, b in rc4..rc7, result A:X:rc2:rc3).
v2b: v2 without the zero-byte skip (smaller).
v2: one byte loop, multiplier in a sentinel cell (one lsr+bne per bit, no zero test), a shifted in place, zero bytes skipped by moving a up a byte.
v3: the multiplier bytes unrolled: byte k adds only into result bytes k..3 from a copy of a's low 4-k bytes (the discarded high part is never computed)."""
HEAD = '.include "imag.inc"\n.text\n.globl __mulsi3\n__mulsi3:\n    sta __rc8\n    stx __rc9\n'
A = ['__rc8', '__rc9', '__rc2', '__rc3']; R = ['__rc10', '__rc11', '__rc12', '__rc13']; Wk = ['__rc14', '__rc15', '__rc16', '__rc17']; M = '__rc18'
def clr(): return ''.join('    sta %s\n' % r for r in R)
def tail(): return '    lda __rc12\n    sta __rc2\n    lda __rc13\n    sta __rc3\n    ldx __rc11\n    lda __rc10\n    rts\n'
def v3():
    s = HEAD + '    lda #0\n' + clr()
    for k in range(4):
        w = 4 - k
        s += '    lda __rc%d\n    beq .Lz%d\n    sta %s\n    sec\n    ror %s\n' % (4 + k, k, M, M)
        for i in range(w): s += '    lda %s\n    sta %s\n' % (A[i], Wk[i])
        s += '.Lb%d:\n    bcc .Ln%d\n    clc\n' % (k, k)
        for i in range(w): s += '    lda %s\n    adc %s\n    sta %s\n' % (R[k + i], Wk[i], R[k + i])
        s += '.Ln%d:\n' % k
        s += '    asl %s\n' % Wk[0] + ''.join('    rol %s\n' % Wk[i] for i in range(1, w))
        s += '    lsr %s\n    bne .Lb%d\n.Lz%d:\n' % (M, k, k)
    return s + tail()
def v2():
    s = HEAD + '    lda #0\n' + clr() + '    ldx #0\n.Lby:\n    lda __rc4,x\n    beq .Lmv\n    sta %s\n    sec\n    ror %s\n.Lb:\n    bcc .Ln\n    clc\n' % (M, M)
    for i in range(4): s += '    lda %s\n    adc %s\n    sta %s\n' % (R[i], A[i], R[i])
    s += '.Ln:\n    asl %s\n' % A[0] + ''.join('    rol %s\n' % A[i] for i in range(1, 4))
    s += '    lsr %s\n    bne .Lb\n    beq .Lnx\n.Lmv:\n' % M
    s += '    lda %s\n    sta %s\n    lda %s\n    sta %s\n    lda %s\n    sta %s\n    lda #0\n    sta %s\n' % (A[2], A[3], A[1], A[2], A[0], A[1], A[0])
    s += '.Lnx:\n    inx\n    cpx #4\n    bne .Lby\n'
    return s + tail()
def v2b():
    s = HEAD + '    lda #0\n' + clr() + '    ldx #0\n.Lby:\n    lda __rc4,x\n    sta %s\n    sec\n    ror %s\n.Lb:\n    bcc .Ln\n    clc\n' % (M, M)
    for i in range(4): s += '    lda %s\n    adc %s\n    sta %s\n' % (R[i], A[i], R[i])
    s += '.Ln:\n    asl %s\n' % A[0] + ''.join('    rol %s\n' % A[i] for i in range(1, 4))
    s += '    lsr %s\n    bne .Lb\n    inx\n    cpx #4\n    bne .Lby\n' % M
    return s + tail()
open('llvmmos_mulsi3_v2b.s', 'w').write(v2b())
open('llvmmos_mulsi3_v2.s', 'w').write(v2())
open('llvmmos_mulsi3_v3.s', 'w').write(v3())
