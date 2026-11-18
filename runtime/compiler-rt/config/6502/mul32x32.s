; Hand written. void __mul32x32(uint32_t a, uint32_t b, uint32_t *hi, uint32_t *lo): unsigned 32x32 -> 64, the SF_MUL32X32 hook
; (-DSF_MUL32X32_6502). llrm-6502 A:X-first layout: a in A:X and rc2:rc3, b in rc4:rc5 and rc6:rc7, hi and lo at the caller's stack (rc0:rc1)+0..3.
; Shift and add with the product shifting right: the smaller operand drives, one byte at a time, a zero byte is a byte move.
; Scratch: A X Y, rc2..rc7, rc16..rc25.

.setcpu "6502"
.export __mul32x32

rc0 = $80
rc2 = $82
rc3 = $83
rc4 = $84
rc5 = $85
rc6 = $86
rc7 = $87
m0 = $82                ; rc2..rc5: the multiplicand
d0 = $90                ; rc16..rc19: the driver; byte j becomes product byte j
ph = $94                ; rc20, rc21
pl = $96                ; rc22, rc23
h0 = $98                ; rc24, rc25, rc6, rc7: product bytes 4..7 (h3 lives in A in the loop)
h1 = $99
h2 = $86
h3 = $87

__mul32x32:
    ldy rc4                 ; b
    sty d0
    ldy rc5
    sty d0+1
    ldy rc6
    sty d0+2
    ldy rc7
    sty d0+3
    ldy rc3                 ; a from A:X and rc2:rc3 into rc2..rc5
    sty rc5
    ldy rc2
    sty rc4
    sta rc2
    stx rc3
    ldy #3                  ; hi and lo, the pointers ph and pl
:   lda (rc0),y
    sta ph,y
    dey
    bpl :-
    ldx #3                  ; drive with the smaller operand
:   lda d0,x
    cmp m0,x
    bne :+
    dex
    bpl :-
:   bcc start
    ldx #3
:   lda d0,x
    ldy m0,x
    sta m0,x
    sty d0,x
    dex
    bpl :-
start:
    lda #0
    sta h0
    sta h1
    sta h2
    tax
nbyte:
    ldy d0,x
    beq zero
    ldy #8
    lsr d0,x
step:
    bcc noadd
    sta h3
    clc
    lda h0
    adc m0
    sta h0
    lda h1
    adc m0+1
    sta h1
    lda h2
    adc m0+2
    sta h2
    lda h3
    adc m0+3
noadd:
    ror a
    ror h2
    ror h1
    ror h0
    ror d0,x
    dey
    bne step
next:
    inx
    cpx #4
    bne nbyte
    ldy #3
    sta (ph),y
    dey
    lda h2
    sta (ph),y
    dey
    lda h1
    sta (ph),y
    dey
    lda h0
    sta (ph),y
    ldy #3
:   lda d0,y
    sta (pl),y
    dey
    bpl :-
    rts
zero:
    ldy h0
    sty d0,x
    ldy h1
    sty h0
    ldy h2
    sty h1
    sta h2
    lda #0
    beq next
