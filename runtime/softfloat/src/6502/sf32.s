; IEEE-754 binary32 for the 6502, hand written. Round to nearest even, denormals, inf, NaN.
; ABI (proposed to llrm-target-6502): a in rc2..rc5, b in rc6..rc9 (little endian, rc8/rc9 read only),
; result in A:X:rc2:rc3 (byte 0..3). Scratch: rc2..rc7 and rc16..rc25; rc0, rc1 and rc8..rc15 are never written.
;
; The tricks: signs and exponents come out of two shifts (asl a / rol a leaves the exponent in A and the sign in
; the carry); a mantissa is three bytes plus a guard byte, the guard byte holds the rounding bits and a sticky bit
; in its bit 0; alignment moves whole bytes, then at most seven single bits, with the lost bits found by one mask
; test instead of a sticky test per bit; operands are never swapped (the block to shift is chosen by an X offset);
; a negative difference is negated afterwards instead of comparing magnitudes; normalization counts bytes first.

.setcpu "6502"
.export sf_add_c, sf_sub_c, sf_round, sf_align, sf_retinf, sf_quiet
.ifdef STACKB
.export __addsf3, __subsf3, in89, out89
.endif

.include "sf32.inc"

.segment "CODE"

sf_sub_c:
.ifdef STACKB
__subsf3:
.endif
    ldx #$80
    bne addcore
sf_add_c:
.ifdef STACKB
__addsf3:
.endif
    ldx #0
addcore:
    txa
    pha                     ; subtract flag, kept on the stack until both operands are unpacked
    BHI0
    asl a
    BHI1
    rol a
    beq bden
    cmp #$ff
    bne :+
    jmp spent
:   sta eb
    BHI0
    ora #$80
    bne bman
bden:
    lda #1
    sta eb
    BHI0
    and #$7f
bman:
    sta blkB+3
    lda rc7
    sta blkB+2
    lda rc6
    sta blkB+1
    lda #0
    sta blkB+0
    sta blkA+0
    lda rc4
    asl a
    lda rc5
    rol a
    beq aden
    cmp #$ff
    bne :+
    jmp spent
:   sta ea
    lda rc4
    ora #$80
    bne aman
aden:
    lda #1
    sta ea
    lda rc4
    and #$7f
aman:
    sta blkA+3
    lda rc3
    sta blkA+2
    lda rc2
    sta blkA+1
    lda rc5                 ; signs: sa = a's, sb = b's xor the subtract flag
    and #$80
    sta sa
    pla
    BHI1_EOR
    and #$80
    sta sb
; ---- align the operand with the smaller exponent
    sec
    lda ea
    sbc eb
    beq aligned
    bcs shiftb
    eor #$ff
    adc #1                  ; carry is clear here: A = eb - ea
    ldx #0
    ldy eb
    sty re
    jsr sf_align
    jmp aligned2
shiftb:
    ldx #4
    ldy ea
    sty re
    jsr sf_align
    jmp aligned2
aligned:
    lda ea
    sta re
aligned2:
; ---- add or subtract the aligned mantissas
    lda sa
    eor sb
    bmi dosub
    lda sa
    sta rsg
    clc
    lda blkA+0
    adc blkB+0
    sta blkA+0
    lda blkA+1
    adc blkB+1
    sta blkA+1
    lda blkA+2
    adc blkB+2
    sta blkA+2
    lda blkA+3
    adc blkB+3
    sta blkA+3
    bcs :+
    jmp sf_round
:   ror blkA+3              ; carry out: shift right one, the lost bit joins the sticky bit
    ror blkA+2
    ror blkA+1
    ror blkA+0
    bcc :+
    lda blkA+0
    ora #1
    sta blkA+0
:   inc re
    jmp sf_round
dosub:
    sec
    lda blkA+0
    sbc blkB+0
    sta blkA+0
    lda blkA+1
    sbc blkB+1
    sta blkA+1
    lda blkA+2
    sbc blkB+2
    sta blkA+2
    lda blkA+3
    sbc blkB+3
    sta blkA+3
    bcs subpos
    sec                     ; negative: negate the four bytes, the sign is b's
    lda #0
    sbc blkA+0
    sta blkA+0
    lda #0
    sbc blkA+1
    sta blkA+1
    lda #0
    sbc blkA+2
    sta blkA+2
    lda #0
    sbc blkA+3
    sta blkA+3
    lda sb
    sta rsg
    jmp subnorm
subpos:
    lda sa
    sta rsg
subnorm:
    lda blkA+3              ; zero result is +0
    ora blkA+2
    ora blkA+1
    ora blkA+0
    bne :+
    jmp retzero
:   lda blkA+3
    bne bitnorm
byteloop:                   ; top byte clear: move up a whole byte while the exponent allows
    lda re
    cmp #9
    bcc bitnorm
    sbc #8
    sta re
    lda blkA+2
    sta blkA+3
    lda blkA+1
    sta blkA+2
    lda blkA+0
    sta blkA+1
    lda #0
    sta blkA+0
    lda blkA+3
    beq byteloop
bitnorm:                    ; top byte in A, exponent in X, shift left while the top bit is clear and the exponent allows
    ldx re
    lda blkA+3
    bmi bndone
bnloop:
    cpx #2
    bcc bndone              ; exponent 1: the result is denormal
    dex
    asl blkA+0
    rol blkA+1
    rol blkA+2
    rol a
    bpl bnloop
bndone:
    sta blkA+3
    stx re
; ---- round to nearest even on the guard byte, pack
sf_round:
    lda blkA+0
    cmp #$80
    bcc pack
    bne up
    lda blkA+1
    and #1
    beq pack
up: inc blkA+1
    bne pack
    inc blkA+2
    bne pack
    inc blkA+3
    bne pack
    lda #$80                ; the mantissa wrapped: 1.0 at the next exponent
    sta blkA+3
    inc re
pack:
    lda re
    cmp #255
    bcs sf_retinf
    ldy blkA+3
    bmi :+
    lda #0                  ; denormal: exponent field 0
:   lsr a                   ; A = exponent >> 1, carry = exponent bit 0
    ora rsg
    sta rc3
    lda blkA+3
    and #$7f
    bcc :+
    ora #$80
:   sta rc2
    ldx blkA+2
    lda blkA+1
    rts
sf_retinf:
    lda rsg
    ora #$7f
    sta rc3
    lda #$80
    sta rc2
    lda #0
    tax
    rts
retzero:
    lda #0
    sta rc2
    sta rc3
    tax
    rts

; ---- shift block (X = 0 for A, 4 for B) right by A bits (1..255); lost bits end up in bit 0 of the guard byte
sf_align:
    tay
    cpy #32
    bcs alfar
al8:
    cpy #8
    bcc albits
    lda blkA+0,x            ; one byte: guard = m0, sticky if the old guard was not zero
    beq :+
    lda blkA+1,x
    ora #1
    bne :++
:   lda blkA+1,x
:   sta blkA+0,x
    lda blkA+2,x
    sta blkA+1,x
    lda blkA+3,x
    sta blkA+2,x
    lda #0
    sta blkA+3,x
    tya
    sec
    sbc #8
    tay
    jmp al8
albits:
    tya
    beq aldone
    lda masks-1,y           ; the bits about to fall out of the guard byte
    and blkA+0,x
    sta tmp
:   lsr blkA+3,x
    ror blkA+2,x
    ror blkA+1,x
    ror blkA+0,x
    dey
    bne :-
    lda tmp
    beq aldone
    lda blkA+0,x
    ora #1
    sta blkA+0,x
aldone:
    rts
alfar:                      ; 32 bits or more: everything becomes the sticky bit
    lda blkA+1,x
    ora blkA+2,x
    ora blkA+3,x
    ora blkA+0,x
    beq :+
    lda #1
:   sta blkA+0,x
    lda #0
    sta blkA+1,x
    sta blkA+2,x
    sta blkA+3,x
    rts

masks:
    .byte $01, $03, $07, $0f, $1f, $3f, $7f

; ---- inf and NaN operands: a in rc2..rc5, b in rc6..rc9, the subtract flag on the stack
.ifdef STACKB
spent:                      ; the rare path keeps the cores' convention: b's high bytes copied into rc8/rc9 and given back
    pla
    sta rc25
    jsr in89
    lda rc25
    jsr special_a
    jmp out89
in89:
    lda rc8
    sta sv8
    lda rc9
    sta sv9
    ldy #0
    lda (rc0),y
    sta rc8
    iny
    lda (rc0),y
    sta rc9
    rts
out89:
    sta rc4                 ; result bytes A:X:rc2:rc3 stay, rc4 is scratch
    lda sv8
    sta rc8
    lda sv9
    sta rc9
    lda rc4
    rts
.bss
sv8:    .res 1
sv9:    .res 1
.segment "CODE"
special:
    pla
special_a:
.else
spent:
special:
    pla
.endif
    and #$80
    sta fl
    ldx #0
    jsr cls
    sty ca
    ldx #4
    jsr cls                 ; Y = class of b
    lda ca
    cmp #2
    bne :+
    ldx #0
    beq sf_quiet
:   cpy #2
    bne :+
    ldx #4
    bne sf_quiet
:   cpy #1
    bne reta                ; b finite: a is the infinity
    lda ca
    beq retb                ; a finite, b infinite
    lda rc5                 ; both infinite: opposite effective signs are invalid
    eor rc9
    eor fl
    bpl reta
    lda #$c0
    sta rc2
    lda #$7f
    sta rc3
    lda #0
    tax
    rts
sf_quiet:
    lda rc4,x
    ora #$40
    sta rc4,x
    lda #0
    sta fx
    beq retx
reta:
    ldx #0
    stx fx
    beq retx
retb:
    ldx #4
    lda fl
    sta fx
retx:                       ; return operand at offset X, sign flipped by fx
    lda rc5,x
    eor fx
    sta rc5,x
    ldy rc2,x
    lda rc3,x
    sta tmp
    lda rc4,x
    sta rc2
    lda rc5,x
    sta rc3
    ldx tmp
    tya
    rts
; class of the operand at offset X (0 a, 4 b) in Y: 0 finite, 1 inf, 2 nan
cls:
    ldy #0
    lda rc5,x
    and #$7f
    cmp #$7f
    bne cx
    lda rc4,x
    bpl cx
    iny
    and #$7f
    ora rc3,x
    ora rc2,x
    beq cx
    iny
cx: rts
