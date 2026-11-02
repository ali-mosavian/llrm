; IEEE-754 binary32 multiply and divide for the 6502, hand written; links with sf32.s (rounding, packing, shifts).
; Same ABI and scratch budget as sf32.s: a in rc2..rc5, b in rc6..rc9, result A:X:rc2:rc3.
;
; The tricks: mantissas are normalized once (denormals too) so both loops see 24-bit values with bit 23 set;
; multiply is Woz's shift-add on a 4-byte window (product top byte in A, one multiplier byte below it, which fills
; with product bits), the add carry re-enters through the chain's carry; divide is a restoring loop whose quotient shifts into the same cells that become the
; rounding block, the trial subtraction lives in A/X/Y and is committed only on success; the exponent is a signed
; 16-bit sum that is range-checked once, in the shared tail.

.setcpu "6502"
.include "sf32.inc"
.import sf_round, sf_align, sf_retinf, sf_quiet
.export sf_mul_c, sf_div_c
.ifdef STACKB
.export __mulsf3, __divsf3
.import in89, out89
.endif

; cells (rc16.. are free scratch in both routines)
e16lo = rc2             ; result exponent, signed 16 bit
e16hi = rc3
opf   = rc24            ; 0 multiply, 1 divide; special-value path only


.ifdef SF_MUL_LUT
; add (row byte) * M[j] into the sum at cell lo and the cells above it
.macro QPROD j, lo
    ldy rc20+j
    sec
    lda (rc2),y
    sbc (rc6),y
    tax
    lda (rc4),y
    sbc (rc24),y
    tay
    txa
    clc
    adc lo
    sta lo
    tya
    QUP lo
.endmacro
.macro QUP lo                ; A + carry into the cell after lo, carry rippling up to rc19
.if lo = rc23
    adc rc16
    sta rc16
    bcc :+
    inc rc17
    bne :+
    inc rc18
    bne :+
    inc rc19
:
.elseif lo = rc16
    adc rc17
    sta rc17
    bcc :+
    inc rc18
    bne :+
    inc rc19
:
.elseif lo = rc17
    adc rc18
    sta rc18
    bcc :+
    inc rc19
:
.else
    adc rc19
    sta rc19
.endif
.endmacro
.endif

.segment "CODE"

sf_mul_c:
.ifdef STACKB
__mulsf3:
.endif
    lda #0
    beq mdentry
sf_div_c:
.ifdef STACKB
__divsf3:
.endif
    lda #1
mdentry:
    sta opf
    ldx #0
    jsr unp
    beq mdsp
    sty rc25                ; normalization shift of a
    ldx #4
.ifdef STACKB
    jsr unpb
.else
    jsr unp
.endif
    beq mdsp
    lda opf
    beq mulbody
    jmp divbody
.ifdef STACKB
mdsp:
    jsr in89
    jsr mdspec
    jmp out89
.else
mdsp:
    jmp mdspec
.endif

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
.ifdef SF_MUL_LUT
; quarter squares: x*y = q1[x+y] - q2[255-x+y], q1[n] = n*n/4, q2[n] = (n-255)^2/4; nine 8x8 products, 6-byte sum
; pointers (zero page, lo byte = the row's byte of a): rc2 q1 lo, rc4 q1 hi, rc6 q2 lo, rc24 q2 hi
; sum P0 = mp0 (bss), P1 = rc23, P2..P5 = rc16..rc19
    lda rc5                 ; sign, exponent, a's upper bytes: the pointers reuse their cells
    BHI1_EOR
    and #$80
    pha
    lda e16lo
    pha
    lda e16hi
    pha
    lda rc18
    pha
    lda rc17
    pha
    lda #>q1lo
    sta rc3
    lda #>q1hi
    sta rc5
    lda #>q2lo
    sta rc7
    lda #>q2hi
    sta rc25
    lda rc16
    ldx #0
    stx rc16
    stx rc17
    stx rc18
    stx rc19
    jsr qrow0
    pla
    jsr qptr
    QPROD 0, rc23
    QPROD 1, rc16
    QPROD 2, rc17
    pla
    jsr qptr
    QPROD 0, rc16
    QPROD 1, rc17
    QPROD 2, rc18
    pla
    sta e16hi
    pla
    sta e16lo
    lda rc19
    bmi :+                  ; product >= 2: exponent up
    asl rc23                ; else one place left, as the shift-add leaves it
    rol rc16
    rol rc17
    rol rc18
    rol rc19
    jmp :++
:   inc e16lo
    bne :+
    inc e16hi
:   lda rc23
    ora mp0
    beq :+
    lda #1
    ora rc16
    sta rc16
:   pla
    sta rsg
    jmp mdtail2
qptr:
    sta rc2
    sta rc4
    eor #$ff
    sta rc6
    sta rc24
    rts
qrow0:                      ; row 0 into the zeroed sum: its first product is stored, not added
    jsr qptr
    ldy rc20
    sec
    lda (rc2),y
    sbc (rc6),y
    sta mp0
    lda (rc4),y
    sbc (rc24),y
    sta rc23
    QPROD 1, rc23
    QPROD 2, rc16
    rts
.pushseg
.bss
mp0: .res 1
.segment "SFLUT"         ; page aligned (linker config)
q1lo: .repeat 512, n
    .byte <(n * n / 4)
.endrepeat
q1hi: .repeat 512, n
    .byte >(n * n / 4)
.endrepeat
q2lo: .repeat 512, n
    .byte <((n - 255) * (n - 255) / 4)
.endrepeat
q2hi: .repeat 512, n
    .byte >((n - 255) * (n - 255) / 4)
.endrepeat
.popseg
.else
    lda rc16                ; C - 1: each add enters with the multiplier bit (1) as its carry
    bne :++
    lda rc17
    bne :+
    dec rc18
:   dec rc17
:   dec rc16
    lda #0                  ; P2 lives in A
    sta rc23
    sta rc24
    ldx #$fd                ; rc23,x walks the multiplier bytes rc20..rc22 (zero-page wrap)
    clc
mloop:
.repeat 8
    ror a
    ror rc23
    ror rc24
    ror rc23,x              ; carry = multiplier bit; product bits fill the byte from the top
    bcc :+
    tay
    lda rc24
    adc rc16
    sta rc24
    lda rc23
    adc rc17
    sta rc23
    tya
    adc rc18
:
.endrepeat
    inx
    beq :+
    jmp mloop
:   bcc mfin                ; the adds came one place early: V' = 2V; no carry out means V' < 2^48 and P is the mantissa
    ror a                   ; product >= 2: shift once more, exponent up
    ror rc23
    ror rc24
    ror rc22
    ror rc21
    inc e16lo
    bne mfin
    inc e16hi
mfin:
    sta rc19
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
.endif

; ---- divide: R = rc23, rc24, A (low first), D = rc20..22, quotient rc17..19, guard and sticky rc16 (the rounding block)
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
    lda rc16                ; R = rc23, rc24 and A (top byte); D = rc20..22
    sta rc23
    lda rc17
    sta rc24
    ldy #1                  ; three 8-bit quotient groups, each collected in rc4 behind a sentinel bit, go to rc16+X
    sty rc4
    ldx #3
    lda rc18
    cmp rc22                ; ratio < 1: one step down, so the first quotient bit is always 1
    bne :+
    ldy rc24
    cpy rc21
    bne :+
    ldy rc23
    cpy rc20
:   bcs dsub
    ldy e16lo
    bne :+
    dec e16hi
:   dec e16lo
    jmp dnext
dloop:
    cmp rc22                ; compare first: the top byte nearly always decides, and only a quotient bit of 1 pays for the subtract
    bne dcmp
    ldy rc24
    cpy rc21
    bne dcmp
    ldy rc23
    cpy rc20
dcmp:
    bcc dno                 ; R < D: quotient bit 0 (carry clear)
dsub:
    tay                     ; carry is set: R - D, and the quotient bit 1 is the carry left by the last sbc
    lda rc23
    sbc rc20
    sta rc23
    lda rc24
    sbc rc21
    sta rc24
    tya
    sbc rc22
dno:
    rol rc4
    bcs dgroup
dnext:
    asl rc23
    rol rc24
    rol a
    bcc dloop
    tay                     ; R overflowed 24 bits: the next bit is 1 and R - D is exact modulo 2^24
    lda rc23
    sbc rc20
    sta rc23
    lda rc24
    sbc rc21
    sta rc24
    tya
    sbc rc22
    sec
    bcs dno
dgroup:                     ; the sentinel fell out: rc4 holds the group
    ldy rc4
    sty rc16,x
    ldy #1
    sty rc4
    dex
    bne dnext
    asl rc23                ; guard and sticky from 2R against D
    rol rc24
    rol a
    bcs dabove
    cmp rc22
    bne :+
    ldy rc24
    cpy rc21
    bne :+
    ldy rc23
    cpy rc20
:   bcc dbelow
    beq dhalf
dabove:
    lda #$81
    bne dst
dhalf:
    lda #$80
    bne dst
dbelow:
    ora rc24
    ora rc23
    beq dst
    lda #1
dst:
    sta rc16
; ---- shared tail: block rc16..19 normalized, e16 = biased exponent of its top bit, sign from rc5/rc9
mdtail:
    lda rc5
    BHI1_EOR
    and #$80
    sta rsg
mdtail2:
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

.ifdef STACKB
; b without rc8/rc9: its low bytes in rc6:rc7, its high bytes in the caller's stack words; same results as unp at X = 4
unpb:
    ldy #0
    lda (rc0),y
    asl a
    iny
    lda (rc0),y
    rol a
    beq ubn
    cmp #$ff
    beq ubret
    sta rc23
    dey
    lda (rc0),y
    ora #$80
    sta rc22
    lda rc7
    sta rc21
    lda rc6
    sta rc20
    lda rc23
ubret:
    rts
ubn:
    dey
    lda (rc0),y
    and #$7f
    sta rc22
    ora rc7
    ora rc6
    beq ubret
    lda rc7
    sta rc21
    lda rc6
    sta rc20
    lda #1
    sta rc23
ubl:
    lda rc22
    bmi ubok
    asl rc20
    rol rc21
    rol rc22
    iny
    bne ubl
ubok:
    lda #1
    rts
.endif

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
