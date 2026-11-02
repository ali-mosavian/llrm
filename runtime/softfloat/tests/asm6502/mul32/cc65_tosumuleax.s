; ours (v2y: byte loop with a sentinel cell, r3 in Y) in cc65's tosumuleax slot: left operand on the C stack (4 bytes, low first), right operand
; in A:X:sreg:sreg+1, result in A:X:sreg:sreg+1, the stack operand popped. Same algorithm as src/6502/mulsi3.S.
.export tosumuleax, tosmuleax
.importzp sp, sreg
.segment "ZEROPAGE"
mb:  .res 4     ; multiplier bytes (right operand), low first
ma:  .res 4     ; multiplicand (left operand), shifted left in place
mr:  .res 3     ; result bytes 0..2, byte 3 in Y
mm:  .res 1     ; sentinel cell
.segment "CODE"
tosmuleax:
tosumuleax:
    sta mb
    stx mb+1
    lda sreg
    sta mb+2
    lda sreg+1
    sta mb+3
    ldy #3
:   lda (sp),y
    sta ma,y
    dey
    bpl :-
    lda #0
    sta mr
    sta mr+1
    sta mr+2
    tay
    ldx #$fc
by: lda mb+4,x          ; mb+4+$fc wraps to mb+0..3
    beq mv
    sec
    ror a
    sta mm
bt: bcc nb
    clc
    lda mr
    adc ma
    sta mr
    lda mr+1
    adc ma+1
    sta mr+1
    lda mr+2
    adc ma+2
    sta mr+2
    tya
    adc ma+3
    tay
nb: asl ma
    rol ma+1
    rol ma+2
    rol ma+3
    lsr mm
    bne bt
    beq nx
mv: lda ma+2
    sta ma+3
    lda ma+1
    sta ma+2
    lda ma
    sta ma+1
    lda #0
    sta ma
nx: inx
    bne by
    sty sreg+1
    lda mr+2
    sta sreg
    clc
    lda sp              ; pop the stack operand
    adc #4
    sta sp
    bcc :+
    inc sp+1
:   ldx mr+1
    lda mr
    rts
