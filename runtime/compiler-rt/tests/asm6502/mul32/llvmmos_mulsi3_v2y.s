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
    tay
    ldx #$fc
.Lby:
    lda __rc8,x
    beq .Lmv
    sec
    ror a
    sta __rc18
.Lb:
    bcc .Ln
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
    tya
    adc __rc3
    tay
.Ln:
    asl __rc8
    rol __rc9
    rol __rc2
    rol __rc3
    lsr __rc18
    bne .Lb
    beq .Lnx
.Lmv:
    lda __rc2
    sta __rc3
    lda __rc9
    sta __rc2
    lda __rc8
    sta __rc9
    lda #0
    sta __rc8
.Lnx:
    inx
    bne .Lby
    lda __rc12
    sta __rc2
    sty __rc3
    ldx __rc11
    lda __rc10
    rts
