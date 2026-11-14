; 64-bit remainder for the 6502: u64 __umoddi3(u64 a, u64 b) and __moddi3, llrm-6502 A:X-first layout (as di64.s).
; Its own member, so only remainder users pay for it: the step loop does not roll a quotient. Division by zero leaves the dividend.

.setcpu "6502"
.export __umoddi3, __moddi3

.include "di64_div.inc"
.include "di64_align.inc"
lp:
    ldx ln
c1: lda N,x
    cmp d,x
    bne c2
    dex
    bpl c1
    sec
c2: bcc nosub
    sec
    lda N
    sbc d
    sta N
    lda N+1
    sbc d+1
    sta N+1
    lda N+2
    sbc d+2
    sta N+2
    lda N+3
    sbc d+3
    sta N+3
    lda N+4
    sbc d+4
    sta N+4
    lda N+5
    sbc d+5
    sta N+5
    lda N+6
    sbc d+6
    sta N+6
    lda N+7
    sbc d+7
    sta N+7
    sec
nosub:
    lsr d+7
    ror d+6
    ror d+5
    ror d+4
    ror d+3
    ror d+2
    ror d+1
    ror d
    dec cnt
    bne lp
    rts

retr:
    ldx #5
:   lda N+2,x
    sta rc2,x
    dex
    bpl :-
    ldx N+1
    lda N
    rts

__umoddi3:
    jsr load
    jsr udm
    jmp retr

__moddi3:
    jsr sload
    jsr udm
    lda sgn
    beq :+
    jsr negn
:   jmp retr
