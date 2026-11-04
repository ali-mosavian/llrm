; ours var1 (shift-add) in llvm-mos's own __mulsi3 ABI: a in A:X:rc2:rc3, b in rc4..rc7, result A:X:rc2:rc3. Same algorithm as mul32.s VAR 1.
.include "imag.inc"
.text
.globl __mulsi3
__mulsi3:
    sta __rc8
    stx __rc9
    lda #0
    sta __rc10
    sta __rc11
    sta __rc12
    sta __rc13
.Lloop:
    lsr __rc7
    ror __rc6
    ror __rc5
    ror __rc4
    bcc .Lns
    clc
    lda __rc10
    adc __rc8
    sta __rc10
    lda __rc11
    adc __rc9
    sta __rc11
    lda __rc12
    adc __rc2
    sta __rc12
    lda __rc13
    adc __rc3
    sta __rc13
.Lns:
    asl __rc8
    rol __rc9
    rol __rc2
    rol __rc3
    lda __rc4
    ora __rc5
    ora __rc6
    ora __rc7
    bne .Lloop
    lda __rc12
    sta __rc2
    lda __rc13
    sta __rc3
    ldx __rc11
    lda __rc10
    rts
