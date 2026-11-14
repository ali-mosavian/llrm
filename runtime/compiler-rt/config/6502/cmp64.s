; f64 comparisons for the 6502: int __cmpdf2 __eqdf2 __nedf2 __ltdf2 __ledf2 (1 when unordered) and __gedf2 __gtdf2 (-1 when
; unordered), the compiler-rt contract of lib/fp/f64/cmp.c. llrm-6502 A:X-first layout (as di64.s): a0:a1 in A:X, a2..a7 in rc2..rc7,
; b0..b7 at the caller's stack (rc0:rc1)+0..7 (read only). Result -1, 0 or 1 in A:X. Scratch rc16, rc17, rc24, rc25.
; A NaN operand is tested first, on the exponent bytes; the operands then compare as sign and magnitude from the top byte down and
; stop at the first byte that differs (zeros of either sign are equal).

.setcpu "6502"
.export __cmpdf2, __eqdf2, __nedf2, __ltdf2, __ledf2, __gedf2, __gtdf2

rc0 = $80
rc2 = $82
rc3 = $83
rc4 = $84
rc5 = $85
rc6 = $86
rc7 = $87
un = $90                ; rc16: the result for an unordered pair
tmp = $91               ; rc17
a6 = $98                ; rc24
a7 = $99                ; rc25

.segment "CODE"

__cmpdf2:
__eqdf2:
__nedf2:
__ltdf2:
__ledf2:
    ldy #1
    bne common
__gedf2:
__gtdf2:
    ldy #$FF
common:
    sty un
    ldy rc6                 ; the A:X-first layout into the cells the body reads: a0..a5 in rc2..rc7, a6:a7 in a6:a7
    sty a6
    ldy rc7
    sty a7
    ldy rc5
    sty rc7
    ldy rc4
    sty rc6
    ldy rc3
    sty rc5
    ldy rc2
    sty rc4
    sta rc2
    stx rc3
    lda a7                  ; a NaN: exponent all ones and a mantissa
    and #$7F
    cmp #$7F
    bne a_ok
    lda a6
    and #$F0
    cmp #$F0
    bne a_ok
    lda a6
    and #$0F
    ora rc2
    ora rc3
    ora rc4
    ora rc5
    ora rc6
    ora rc7
    beq a_ok
    jmp unordered
a_ok:
    ldy #7                  ; b7
    lda (rc0),y
    and #$7F
    cmp #$7F
    bne b_ok
    dey
    lda (rc0),y             ; b6
    and #$F0
    cmp #$F0
    bne b_ok
    lda (rc0),y
    and #$0F
:   dey
    ora (rc0),y
    cpy #0
    bne :-
    cmp #0
    beq b_ok
    jmp unordered
b_ok:
    ldy #7
    lda (rc0),y
    eor a7
    bmi signs               ; the signs differ
    lda a7                  ; the same sign: the magnitudes, top byte first
    cmp (rc0),y
    bne decided
    lda a6
    dey
    cmp (rc0),y
    bne decided
    lda rc7
    dey
    cmp (rc0),y
    bne decided
    lda rc6
    dey
    cmp (rc0),y
    bne decided
    lda rc5
    dey
    cmp (rc0),y
    bne decided
    lda rc4
    dey
    cmp (rc0),y
    bne decided
    lda rc3
    dey
    cmp (rc0),y
    bne decided
    lda rc2
    dey
    cmp (rc0),y
    bne decided
equal:
    lda #0
    tax
    rts
decided:                    ; C: the magnitude of a is the greater
    bcs greater
    lda a7                  ; smaller magnitude: less if positive
    bmi plus1
minus1:
    lda #$FF
    tax
    rts
greater:
    lda a7
    bmi minus1
plus1:
    lda #1
    ldx #0
    rts
signs:                      ; two zeros are equal whatever their signs; otherwise the negative one is less
    lda a7
    and #$7F
    ora a6
    ora rc2
    ora rc3
    ora rc4
    ora rc5
    ora rc6
    ora rc7
    ldy #6
:   ora (rc0),y
    dey
    cpy #$FF
    bne :-
    sta tmp
    ldy #7
    lda (rc0),y
    and #$7F
    ora tmp
    beq equal
    lda a7
    bmi minus1
    bpl plus1
unordered:
    lda un
    bmi minus1
    bpl plus1
