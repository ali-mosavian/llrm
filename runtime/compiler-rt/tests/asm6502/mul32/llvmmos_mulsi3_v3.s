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
    lda __rc4
    beq .Lz0
    sta __rc18
    sec
    ror __rc18
    lda __rc8
    sta __rc14
    lda __rc9
    sta __rc15
    lda __rc2
    sta __rc16
    lda __rc3
    sta __rc17
.Lb0:
    bcc .Ln0
    clc
    lda __rc10
    adc __rc14
    sta __rc10
    lda __rc11
    adc __rc15
    sta __rc11
    lda __rc12
    adc __rc16
    sta __rc12
    lda __rc13
    adc __rc17
    sta __rc13
.Ln0:
    asl __rc14
    rol __rc15
    rol __rc16
    rol __rc17
    lsr __rc18
    bne .Lb0
.Lz0:
    lda __rc5
    beq .Lz1
    sta __rc18
    sec
    ror __rc18
    lda __rc8
    sta __rc14
    lda __rc9
    sta __rc15
    lda __rc2
    sta __rc16
.Lb1:
    bcc .Ln1
    clc
    lda __rc11
    adc __rc14
    sta __rc11
    lda __rc12
    adc __rc15
    sta __rc12
    lda __rc13
    adc __rc16
    sta __rc13
.Ln1:
    asl __rc14
    rol __rc15
    rol __rc16
    lsr __rc18
    bne .Lb1
.Lz1:
    lda __rc6
    beq .Lz2
    sta __rc18
    sec
    ror __rc18
    lda __rc8
    sta __rc14
    lda __rc9
    sta __rc15
.Lb2:
    bcc .Ln2
    clc
    lda __rc12
    adc __rc14
    sta __rc12
    lda __rc13
    adc __rc15
    sta __rc13
.Ln2:
    asl __rc14
    rol __rc15
    lsr __rc18
    bne .Lb2
.Lz2:
    lda __rc7
    beq .Lz3
    sta __rc18
    sec
    ror __rc18
    lda __rc8
    sta __rc14
.Lb3:
    bcc .Ln3
    clc
    lda __rc13
    adc __rc14
    sta __rc13
.Ln3:
    asl __rc14
    lsr __rc18
    bne .Lb3
.Lz3:
    lda __rc12
    sta __rc2
    lda __rc13
    sta __rc3
    ldx __rc11
    lda __rc10
    rts
