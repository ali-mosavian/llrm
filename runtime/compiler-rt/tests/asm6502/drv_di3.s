; ca65 driver for the 64-bit shifts: rc0:rc1 points at each record's stack word (the count), A:X and rc2..rc7 get w0..w3
.import __ashldi3, __lshrdi3, __ashrdi3
.export _main
.include "vec.inc"
.include "op.inc"
rc0 = $80
rc2 = $82
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
    ldy #7
:   lda (ptr),y
    sta rc2-2,y
    dey
    cpy #1
    bne :-
    clc
    lda ptr
    adc #8
    sta rc0
    lda ptr+1
    adc #0
    sta rc0+1
    lda (ptr),y
    tax
    dey
    lda (ptr),y
    jsr OP
    sta res
    stx res+1
    ldx #5
:   lda rc2,x
    sta res+2,x
    dex
    bpl :-
    ldy #10
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
    adc #18
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
