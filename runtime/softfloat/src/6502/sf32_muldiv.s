; IEEE-754 binary32 multiply and divide for the 6502, hand written; links with sf32.s (rounding, packing, shifts).
; Same ABI and scratch budget as sf32.s: a in rc2..rc5, b in rc6..rc9, result A:X:rc2:rc3.
;
; The tricks: mantissas are normalized once (denormals too) so both loops see 24-bit values with bit 23 set;
; multiply is Woz's shift-add on a 6-byte right-shifting product, one ror chain per bit, the add carry re-enters
; through the chain's carry; divide is a restoring loop whose quotient shifts into the same cells that become the
; rounding block, the trial subtraction lives in A/X/Y and is committed only on success; the exponent is a signed
; 16-bit sum that is range-checked once, in the shared tail.

.setcpu "6502"
.include "sf32.inc"
.import sf_round, sf_align, sf_retinf, sf_quiet
.export sf_mul_c, sf_div_c

; cells (rc16.. are free scratch in both routines)
e16lo = rc2             ; result exponent, signed 16 bit
e16hi = rc3
opf   = rc24            ; 0 multiply, 1 divide; special-value path only

.segment "CODE"

sf_mul_c:
    lda #0
    beq mdentry
sf_div_c:
    lda #1
mdentry:
    sta opf
    ldx #0
    jsr unp
    beq mdsp
    sty rc25                ; normalization shift of a
    ldx #4
    jsr unp
    beq mdsp
    lda opf
    beq mulbody
    jmp divbody
mdsp:
    jmp mdspec

; ---- multiply: C = rc16..18, M = rc20..22, P = rc19 (high) rc23 rc24 (low)
mulbody:
    tya
    clc
    adc rc25
    adc #127                ; 127 + na + nb
    sta rc4
    lda rc19
    clc
    adc rc23
    ldx #0
    bcc :+
    inx
:   sec
    sbc rc4
    sta e16lo
    txa
    sbc #0
    sta e16hi
    lda #0
    sta rc19
    sta rc23
    sta rc24
    ldx #24
    clc
mloop:
    ror rc19
    ror rc23
    ror rc24
    ror rc22
    ror rc21
    ror rc20                ; carry = multiplier bit
    bcc mnext
    clc
    lda rc24
    adc rc16
    sta rc24
    lda rc23
    adc rc17
    sta rc23
    lda rc19
    adc rc18
    sta rc19
mnext:
    dex
    bne mloop
    bcc mfin                ; the adds came one place early: V' = 2V; no carry out means V' < 2^48 and P is the mantissa
    ror rc19                ; product >= 2: shift once more, exponent up
    ror rc23
    ror rc24
    ror rc22
    ror rc21
    ror rc20
    inc e16lo
    bne mfin
    inc e16hi
mfin:
    lda rc23
    sta rc18
    lda rc24
    sta rc17
    lda rc21
    ora rc20
    beq :+
    lda #1
:   ora rc22
    sta rc16                ; guard byte with the sticky bit
    jmp mdtail

; ---- divide: R = rc23..25 (low first), D = rc20..22, quotient rc16..19 (the rounding block)
divbody:
    tya
    sec
    sbc rc25
    clc
    adc #127                ; 127 + nb - na, always 104..150
    sta rc4
    lda rc19
    clc
    adc rc4
    ldx #0
    bcc :+
    inx
:   sec
    sbc rc23
    sta e16lo
    txa
    sbc #0
    sta e16hi
    lda rc16
    sta rc23
    lda rc17
    sta rc24
    lda rc18
    sta rc25
    lda #0
    sta rc17
    sta rc18
    sta rc19
    lda #$40                ; sentinel: it reaches bit 32 and falls out of rc19 after exactly 26 quotient bits
    sta rc16
dloop:
    sec
    lda rc23
    sbc rc20
    tax
    lda rc24
    sbc rc21
    tay
    lda rc25
    sbc rc22
    bcc dno                 ; R < D: quotient bit 0 (carry clear)
    sta rc25
    sty rc24
    stx rc23                ; quotient bit 1 (carry set)
dno:
    rol rc16
    rol rc17
    rol rc18
    rol rc19
    bcs ddone
    asl rc23
    rol rc24
    rol rc25
    bcc dloop
    sec                     ; R overflowed 24 bits: the next bit is 1 and R - D is exact modulo 2^24
    lda rc23
    sbc rc20
    sta rc23
    lda rc24
    sbc rc21
    sta rc24
    lda rc25
    sbc rc22
    sta rc25
    sec
    bcs dno
ddone:
    ldy #0                  ; sticky: any remainder
    lda rc23
    ora rc24
    ora rc25
    beq :+
    ldy #1
:   ldx #0                  ; align the 26-bit quotient to bit 31: 6 shifts, or 7 when the ratio was < 1
dnorm:
    lda rc19
    bmi dnormd
    asl rc16
    rol rc17
    rol rc18
    rol rc19
    inx
    bne dnorm
dnormd:
    cpx #7
    bne dnd2
    lda e16lo
    bne dnd1
    dec e16hi
dnd1:
    dec e16lo
dnd2:
    tya
    ora rc16
    sta rc16
; ---- shared tail: block rc16..19 normalized, e16 = biased exponent of its top bit, sign from rc5/rc9
mdtail:
    lda rc5
    eor rc9
    and #$80
    sta rsg
    lda e16hi
    bmi mden
    bne mdinf
    lda e16lo
    beq mden
    cmp #255
    bcs mdinf
    sta re
    jmp sf_round
mdinf:
    jmp sf_retinf
mden:                       ; exponent <= 0: shift right by 1 - e, into the denormal range
    sec
    lda #1
    sbc e16lo
    tay
    lda #0
    sbc e16hi
    beq :+
    ldy #255
:   tya
    ldx #0
    jsr sf_align
    lda #1
    sta re
    jmp sf_round

; ---- unpack operand at offset X (0 a, 4 b): mantissa rc16+X.., exponent rc19+X, Y = normalization shift
; returns Z set for zero, inf or NaN (the special path), else A = exponent
unp:
    ldy #0
    lda rc4,x
    asl a
    lda rc5,x
    rol a
    beq undn
    cmp #$ff
    beq unret
    sta rc19,x
    lda rc4,x
    ora #$80
    sta rc18,x
    lda rc3,x
    sta rc17,x
    lda rc2,x
    sta rc16,x
    lda rc19,x
unret:
    rts
undn:
    lda rc4,x
    and #$7f
    sta rc18,x
    ora rc3,x
    ora rc2,x
    beq unret               ; zero
    lda rc3,x
    sta rc17,x
    lda rc2,x
    sta rc16,x
    lda #1
    sta rc19,x
unl:
    lda rc18,x
    bmi unok
    asl rc16,x
    rol rc17,x
    rol rc18,x
    iny
    bne unl
unok:
    lda #1
    rts

; ---- zero, inf and NaN operands
; class of the operand at offset X in Y: 0 finite nonzero, 1 zero, 2 inf, 3 nan
mcls:
    ldy #0
    lda rc4,x
    asl a
    lda rc5,x
    rol a
    beq mzero
    cmp #$ff
    bne mcx
    ldy #2
    bne mman
mzero:
    ldy #1
mman:
    lda rc4,x
    and #$7f
    ora rc3,x
    ora rc2,x
    beq mcx
    cpy #1
    beq mden2
    ldy #3
    rts
mden2:
    ldy #0
mcx:
    rts
mdspec:
    ldx #0
    jsr mcls
    sty rc17
    ldx #4
    jsr mcls
    sty rc18
    lda rc17
    cmp #3
    bne :+
    ldx #0
    beq mq
:   lda rc18
    cmp #3
    bne :+
    ldx #4
mq: jmp sf_quiet
:   lda rc5
    eor rc9
    and #$80
    sta rc19
    lda opf
    bne dvsp
    lda rc17                ; multiply: zero times inf is invalid, any other inf gives inf, else zero
    clc
    adc rc18
    cmp #3
    beq inval
    lda rc17
    cmp #2
    beq rinf
    lda rc18
    cmp #2
    beq rinf
    bne rzero
dvsp:                       ; divide: 0/0 and inf/inf invalid, inf/x and x/0 inf, else zero
    lda rc17
    beq :+
    cmp rc18
    beq inval
:   lda rc17
    cmp #2
    beq rinf
    lda rc18
    cmp #2
    beq rzero
    cmp #1
    beq rinf
rzero:
    lda #0
    sta rc2
    lda rc19
    sta rc3
    lda #0
    tax
    rts
rinf:
    lda rc19
    sta rc5
    jmp sf_retinf
inval:
    lda #$c0
    sta rc2
    lda #$7f
    sta rc3
    lda #0
    tax
    rts
