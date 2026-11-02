; ca65 driver for the FP8 routines: a over 16 values from A0, b over 0..255, exit code = mismatches (max 255)
.export _main
.include "op.inc"
.include "exp8.inc"
.ifndef FP8_M
FP8_M = 2
.endif
INF = ((1 << (7 - FP8_M)) - 1) << FP8_M
QNAN = INF | (1 << (FP8_M - 1))
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
    cmp #$ff                ; not tested (inf or NaN operand in a build without them)
    beq next
    cmp #QNAN
    bne exact
    lda got                 ; any NaN
    and #$7f
    cmp #INF + 1
    bcs next
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
