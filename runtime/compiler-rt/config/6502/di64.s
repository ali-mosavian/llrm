; 64-bit multiply and divide for the 6502: u64 __muldi3(u64 a, u64 b) and __udivdi3 __umoddi3 __divdi3 __moddi3, llrm-6502 A:X-first layout: a0:a1 in A:X, a2..a7 in rc2..rc7,
; b0..b7 at the caller's stack (rc0:rc1)+0..7 (read only). Result A:X:rc2..rc7. Scratch rc16..rc25 and BSS.
; Multiply: shift and add, one multiplier byte at a time from the lowest to the highest nonzero byte of b.
; Divide: restoring, the divisor is shifted up to the dividend's bit length and steps down one bit at a time; the quotient bits roll into qv. Division by zero gives all ones and the dividend as remainder (lib/int/int64.c).

.setcpu "6502"
.export __muldi3, __udivdi3, __divdi3

r0 = $90                ; rc16..rc23: the product
r1 = $91
r2 = $92
r3 = $93
r4 = $94
r5 = $95
r6 = $96
r7 = $97
a6 = $98                ; rc24, rc25
a7 = $99

.include "di64_div.inc"
.bss
bb:     .res 1
top:    .res 1
.segment "CODE"

__muldi3:
    ldy rc6                 ; the A:X-first layout into the cells the body shifts: a0..a5 in rc2..rc7, a6:a7 in a6:a7
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
    lda #0
    ldx #7
:   sta r0,x
    dex
    bpl :-
    ldy #7                  ; highest nonzero byte of b
:   lda (rc0),y
    bne found
    dey
    cpy #$FF
    bne :-
    beq out                 ; b = 0
found:
    sty top
    ldy #0
byte:
    lda (rc0),y
    sta bb
    ldx #8
nbit:
    lsr bb
    bcc skip
    clc
    lda r0
    adc rc2
    sta r0
    lda r1
    adc rc3
    sta r1
    lda r2
    adc rc4
    sta r2
    lda r3
    adc rc5
    sta r3
    lda r4
    adc rc6
    sta r4
    lda r5
    adc rc7
    sta r5
    lda r6
    adc a6
    sta r6
    lda r7
    adc a7
    sta r7
skip:
    asl rc2
    rol rc3
    rol rc4
    rol rc5
    rol rc6
    rol rc7
    rol a6
    rol a7
    dex
    bne nbit
    cpy top
    beq out
    iny
    jmp byte
out:
    lda r2
    sta rc2
    lda r3
    sta rc3
    lda r4
    sta rc4
    lda r5
    sta rc5
    lda r6
    sta rc6
    lda r7
    sta rc7
    ldx r1
    lda r0
    rts

; ---- divide: N = rc16..rc23 (dividend, then remainder), d = divisor, qv = quotient

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
    rol qv
    rol qv+1
    rol qv+2
    rol qv+3
    rol qv+4
    rol qv+5
    rol qv+6
    rol qv+7
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

retq:
    ldx #5
:   lda qv+2,x
    sta rc2,x
    dex
    bpl :-
    ldx qv+1
    lda qv
    rts
negq:
    sec
    lda #0
    sbc qv
    sta qv
    lda #0
    sbc qv+1
    sta qv+1
    lda #0
    sbc qv+2
    sta qv+2
    lda #0
    sbc qv+3
    sta qv+3
    lda #0
    sbc qv+4
    sta qv+4
    lda #0
    sbc qv+5
    sta qv+5
    lda #0
    sbc qv+6
    sta qv+6
    lda #0
    sbc qv+7
    sta qv+7
    rts

__udivdi3:
    jsr load
    jsr udm
    jmp retq
__divdi3:
    jsr sload
    jsr udm
    lda sgn
    eor sgd
    beq :+
    lda dz                  ; no negation when dividing by zero: the quotient stays all ones
    bne :+
    jsr negq
:   jmp retq
