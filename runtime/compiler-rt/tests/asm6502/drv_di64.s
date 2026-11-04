; ca65 driver for the 64-bit multiply/divide: rc0:rc1 = record + 6 (a6:a7 then b), rc2..rc7 = a0..a5
.export _main
.include "vec.inc"
.include "op.inc"
rc0 = $80
rc2 = $82
rc8 = $88
ptr = $a0
cntr = $a2
fail = $a4
res = $b0
_main:
    lda #<vec
    sta ptr
    lda #>vec
    sta ptr+1
    lda #<nrec
    sta cntr
    lda #>nrec
    sta cntr+1
    lda #0
    sta fail
loop:
    ldy #5
:   lda (ptr),y
    sta rc2,y
    dey
    bpl :-
    clc
    lda ptr
    adc #6
    sta rc0
    lda ptr+1
    adc #0
    sta rc0+1
    lda #$5a
    sta rc8
    jsr OP
    sta res
    stx res+1
    ldx #5
:   lda rc2,x
    sta res+2,x
    dex
    bpl :-
    lda rc8                 ; callee-saved
    cmp #$5a
    bne bad
    ldy #16
    ldx #0
:   lda (ptr),y
    cmp res,x
    bne bad
    iny
    inx
    cpx #8
    bne :-
    beq next
bad:
    lda fail
    cmp #255
    beq next
    inc fail
next:
    clc
    lda ptr
    adc #24
    sta ptr
    bcc :+
    inc ptr+1
:   lda cntr
    bne :+
    dec cntr+1
:   dec cntr
    lda cntr
    ora cntr+1
    bne loop
    lda fail
    ldx #0
    rts
