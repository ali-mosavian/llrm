; ca65 driver for the FP8 routines: a over 16 values from A0, b over 0..255, exit code = mismatches (max 255)
.export _main
.include "op.inc"
.include "exp8.inc"
rc2 = $82
rc3 = $83
ptr = $a0
fail = $a4
got = $a5
va = $a6
vb = $a7
_main:
    lda #<exp8
    sta ptr
    lda #>exp8
    sta ptr+1
    lda #0
    sta fail
    sta vb
    lda #A0
    sta va
loop:
    lda va
    sta rc2
    lda vb
    sta rc3
    jsr OP
    sta got
    ldy #0
    lda (ptr),y
    cmp #$7e
    bne exact
    lda got                 ; any NaN
    and #$7c
    cmp #$7c
    bne bad
    lda got
    and #3
    bne next
bad:
    lda fail
    cmp #255
    beq next
    inc fail
    jmp next
exact:
    cmp got
    bne bad
next:
    inc ptr
    bne :+
    inc ptr+1
:   inc vb
    bne loop
    inc va
    lda va
    sec
    sbc #A0
    cmp #16
    bcc loop
    lda fail
    ldx #0
    rts
