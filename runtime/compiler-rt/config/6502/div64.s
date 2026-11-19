; Hand written. void __sf_div64(void): the f64 quotient step, the SF_DIV64_6502 hook of sf64.c. Reads sf_d64_ma and sf_d64_mb (uint64_t, little
; endian), leaves q = floor(ma * 2^56 / mb) in sf_d64_q and the remainder in sf_d64_r: 57 steps of "q = q << 1 | (r >= mb); if r >= mb: r -= mb;
; r <<= 1". The operands go through these cells and not the arguments, so the routine does not depend on the compiler's argument layout.
; r lives in zero page (rc2..rc7, rc16, rc17), q in rc18..rc25: the compare runs from the top byte and ends at the first byte that differs; the
; carry that falls out of the compare (clear) or the subtract (set) is the quotient bit that rolls into q.
; Scratch: A X, rc2..rc7, rc16..rc25.

.setcpu "6502"
.export __sf_div64, sf_d64_ma, sf_d64_mb, sf_d64_q, sf_d64_r

.bss
sf_d64_ma: .res 8
sf_d64_mb: .res 8
sf_d64_q:  .res 8
sf_d64_r:  .res 8

.segment "CODE"

r0 = $82
r1 = $83
r2 = $84
r3 = $85
r4 = $86
r5 = $87
r6 = $90
r7 = $91
q0 = $92
q1 = $93
q2 = $94
q3 = $95
q4 = $96
q5 = $97
q6 = $98
q7 = $99

__sf_div64:
    lda sf_d64_ma+0
    sta r0
    lda sf_d64_ma+1
    sta r1
    lda sf_d64_ma+2
    sta r2
    lda sf_d64_ma+3
    sta r3
    lda sf_d64_ma+4
    sta r4
    lda sf_d64_ma+5
    sta r5
    lda sf_d64_ma+6
    sta r6
    lda sf_d64_ma+7
    sta r7
    lda #0
    sta q0
    sta q1
    sta q2
    sta q3
    sta q4
    sta q5
    sta q6
    sta q7
    ldx #57
step:
    lda r7
    cmp sf_d64_mb+7
    bne decided
    lda r6
    cmp sf_d64_mb+6
    bne decided
    lda r5
    cmp sf_d64_mb+5
    bne decided
    lda r4
    cmp sf_d64_mb+4
    bne decided
    lda r3
    cmp sf_d64_mb+3
    bne decided
    lda r2
    cmp sf_d64_mb+2
    bne decided
    lda r1
    cmp sf_d64_mb+1
    bne decided
    lda r0
    cmp sf_d64_mb+0
decided:
    bcc shift
    sec
    lda r0
    sbc sf_d64_mb+0
    sta r0
    lda r1
    sbc sf_d64_mb+1
    sta r1
    lda r2
    sbc sf_d64_mb+2
    sta r2
    lda r3
    sbc sf_d64_mb+3
    sta r3
    lda r4
    sbc sf_d64_mb+4
    sta r4
    lda r5
    sbc sf_d64_mb+5
    sta r5
    lda r6
    sbc sf_d64_mb+6
    sta r6
    lda r7
    sbc sf_d64_mb+7
    sta r7
shift:
    rol q0
    rol q1
    rol q2
    rol q3
    rol q4
    rol q5
    rol q6
    rol q7
    asl r0
    rol r1
    rol r2
    rol r3
    rol r4
    rol r5
    rol r6
    rol r7
    dex
    beq done
    jmp step
done:
    lda q0
    sta sf_d64_q+0
    lda r0
    sta sf_d64_r+0
    lda q1
    sta sf_d64_q+1
    lda r1
    sta sf_d64_r+1
    lda q2
    sta sf_d64_q+2
    lda r2
    sta sf_d64_r+2
    lda q3
    sta sf_d64_q+3
    lda r3
    sta sf_d64_r+3
    lda q4
    sta sf_d64_q+4
    lda r4
    sta sf_d64_r+4
    lda q5
    sta sf_d64_q+5
    lda r5
    sta sf_d64_r+5
    lda q6
    sta sf_d64_q+6
    lda r6
    sta sf_d64_r+6
    lda q7
    sta sf_d64_q+7
    lda r7
    sta sf_d64_r+7
    rts
