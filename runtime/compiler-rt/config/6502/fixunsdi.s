; u64 __fixunssfdi(float), u64 __fixunsdfdi(double): truncate toward zero; below 1 and negative give 0, 2^64 and up (and a NaN) saturate.
; The mantissa goes to v (rc16..rc23), then __ashldi3_a / __lshrdi3_a shift it by the exponent less the mantissa's width (shift64.inc).
; The C members were 181 (f32) and 220 B (f64), each with a 64-bit call and a stack argument for the shift. llrm-6502 A:X-first layout: the float
; or double in A:X:rc2..rc7 (low byte first), the result there too. Scratch rc16..rc25.

.setcpu "6502"
.export __fixunssfdi, __fixunsdfdi
.import __ashldi3_a, __lshrdi3_a
.include "shift64.inc"
rc3 = $83
rc6 = $86
rc7 = $87

.segment "CODE"

; double: 53-bit mantissa, e = exponent - 1023 in 0..63
__fixunsdfdi:
    ldy rc7
    bmi zero
    sta v
    stx v+1
    ldx #3
:   lda rc2,x
    sta v+2,x
    dex
    bpl :-
    lda rc6
    and #$0F
    ora #$10
    sta v+6
    lda #0
    sta v+7
    tya                     ; the exponent, 11 bits: H = rc7 >> 4, L = rc7 << 4 | rc6 >> 4
    lsr a
    lsr a
    lsr a
    lsr a
    sta fill
    lda rc6
    lsr a
    lsr a
    lsr a
    lsr a
    sta cnt
    tya
    asl a
    asl a
    asl a
    asl a
    ora cnt
    sec
    adc #0                  ; e = (H - 4) * 256 + L + 1
    sta cnt
    lda fill
    adc #$FC
    bcc zero
    bne big
    lda cnt
    cmp #64
    bcs big
    ldx #52
    bne tail

zero:
    lda #0
    beq setall
big:
    lda #$FF
setall:
    ldx #5
:   sta rc2,x
    dex
    bpl :-
    tax
    rts

; float: 24-bit mantissa, e = exponent - 127 in 0..63
__fixunssfdi:
    ldy rc3
    bmi zero                ; negative
    sta v
    stx v+1
    lda rc2
    ora #$80
    sta v+2
    lda #0
    ldx #4
:   sta v+3,x
    dex
    bpl :-
    lda rc2
    asl a                   ; the exponent: bit 0 from rc2, the rest from rc3
    tya
    rol a
    sec
    sbc #127
    bcc zero                ; below 1
    cmp #64
    bcs big
    ldx #23

; A = e, X = the mantissa's width less 1: shift left by e - X, or right by X - e
tail:
    stx cnt
    sec
    sbc cnt
    bcs left
    eor #$FF
    adc #1
    jmp __lshrdi3_a
left:
    jmp __ashldi3_a
