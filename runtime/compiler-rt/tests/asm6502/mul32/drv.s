.export _main
.import mul32
.include "vec.inc"
ptr=$a0
fail=$a4
cnt=$a5
a0=$b0
b0=$b4
r0=$b8
_main:
    lda #<vec
    sta ptr
    lda #>vec
    sta ptr+1
    lda #0
    sta fail
    lda #NV
    sta cnt
next:
    ldy #7
:   lda (ptr),y
    sta a0,y
    dey
    bpl :-
    jsr mul32
    ldy #8
.repeat 4, i
    lda r0+i
    cmp (ptr),y
    beq :+
    inc fail
:   iny
.endrepeat
    clc
    lda ptr
    adc #12
    sta ptr
    bcc :+
    inc ptr+1
:   dec cnt
    bne next
    lda fail
    rts
