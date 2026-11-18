; binary32 <-> 32-bit integer for the 6502, hand written; links with sf32.s (rounding). ABI as sf32.s:
; the argument in A:X and rc2:rc3 (moved to rc2..rc5 on entry), the result in A:X:rc2:rc3. Float to int truncates; out of range saturates, NaN gives 0
; (matches lib/fp/f32/fix_si.c lib/fp/f32/float_si.c).

.setcpu "6502"
.include "sf32.inc"
.import sf_round
.export __floatsisf, __floatunsisf, __fixsfsi, __fixunssfsi
.export sf_floatsisf_c, sf_floatunsisf_c, sf_fixsfsi_c, sf_fixunssfsi_c   ; the cores: the argument already in rc2..rc5

.segment "CODE"

; ---- int to float: the integer becomes the rounding block (byte 0 is the guard byte), normalized to bit 31
ax2cells:               ; A:X, rc2:rc3 -> rc2..rc5, through Y
    ldy rc3
    sty rc5
    ldy rc2
    sty rc4
    sta rc2
    stx rc3
    rts

__floatunsisf:
    jsr ax2cells
sf_floatunsisf_c:
    lda #0
    beq fcopy
__floatsisf:
    jsr ax2cells
sf_floatsisf_c:
    lda rc5
    and #$80
fcopy:
    sta rc6                 ; sign
    bne fneg
    lda rc2
    sta rc16
    lda rc3
    sta rc17
    lda rc4
    sta rc18
    lda rc5
    sta rc19
    jmp fabs
fneg:
    sec                     ; negate; 0x80000000 stays and is right as a magnitude
    lda #0
    sbc rc2
    sta rc16
    lda #0
    sbc rc3
    sta rc17
    lda #0
    sbc rc4
    sta rc18
    lda #0
    sbc rc5
    sta rc19
fabs:
    lda rc6
    sta rsg
    ldx #158                ; 127 + 31
    lda rc19
    bne fbits
    ora rc18
    ora rc17
    ora rc16
    bne fbyte
    tax                     ; zero
    sta rc2
    sta rc3
    rts
fbyte:                      ; top byte clear: move up a whole byte
    lda rc19
    bne fbits
    lda rc18
    sta rc19
    lda rc17
    sta rc18
    lda rc16
    sta rc17
    lda #0
    sta rc16
    txa
    sec
    sbc #8
    tax
    bne fbyte
fbits:
    lda rc19
    bmi fdone
:   dex
    asl rc16
    rol rc17
    rol rc18
    rol a
    bpl :-
fdone:
    sta rc19
    stx re
    jmp sf_round

; ---- float to int: |a| truncated into rc16..19, carry set when |a| >= 2^32 (or inf, NaN)
fixmag:
    lda rc4
    asl a
    lda rc5
    rol a
    sec
    sbc #127
    bcc fzero               ; |a| < 1
    cmp #32
    bcs fbig
    tax
    lda rc4
    ora #$80
    sta rc18
    lda rc3
    sta rc17
    lda rc2
    sta rc16
    lda #0
    sta rc19
    cpx #23
    bcs fleft
    stx rc6
    lda #23
    sec
    sbc rc6
    tay                     ; shift right by 23 - e
:   cpy #8
    bcc frbits
    lda rc17
    sta rc16
    lda rc18
    sta rc17
    lda #0
    sta rc18
    tya
    sec
    sbc #8
    tay
    bne :-
frbits:
    tya
    beq fok
:   lsr rc18                ; rc19 is 0 here
    ror rc17
    ror rc16
    dey
    bne :-
fok:
    clc
    rts
fleft:
    txa
    sec
    sbc #23
    tay                     ; shift left by e - 23
    beq fok
:   asl rc16
    rol rc17
    rol rc18
    rol rc19
    dey
    bne :-
    clc
    rts
fzero:
    lda #0
    sta rc16
    sta rc17
    sta rc18
    sta rc19
    clc
    rts
fbig:
    sec
    rts

fnan:                       ; NaN: exponent 255 and a nonzero mantissa (carry set on return)
    lda rc5
    and #$7f
    cmp #$7f
    bne fnot
    lda rc4
    bpl fnot
    and #$7f
    ora rc3
    ora rc2
    bne fnanis
fnot:
    clc
    rts
fnanis:
    sec
    rts

retzero:
    lda #0
    tax
    sta rc2
    sta rc3
    rts

__fixunssfsi:
    jsr ax2cells
sf_fixunssfsi_c:
    lda rc5
    bmi retzero             ; negative, -inf, -NaN
    jsr fixmag
    bcc fret
    jsr fnan
    bcs retzero
    lda #$ff
    tax
    sta rc2
    sta rc3
    rts
fret:
    lda rc18
    sta rc2
    lda rc19
    sta rc3
    ldx rc17
    lda rc16
    rts

__fixsfsi:
    jsr ax2cells
sf_fixsfsi_c:
    jsr fixmag
    bcs sbig
    lda rc19
    bmi ssat                ; 2^31 or more
    lda rc5
    bpl fret
    sec
    lda #0
    sbc rc16
    sta rc16
    lda #0
    sbc rc17
    sta rc17
    lda #0
    sbc rc18
    sta rc18
    lda #0
    sbc rc19
    sta rc19
    jmp fret
sbig:
    jsr fnan
    bcs retzero
ssat:
    lda rc5
    bpl smax
    lda #0
    tax
    sta rc2
    lda #$80
    sta rc3
    lda #0
    rts
smax:
    lda #$ff
    tax
    sta rc2
    lda #$7f
    sta rc3
    lda #$ff
    rts
