; 64-bit multiply and divide for the 6502: u64 __muldi3(u64 a, u64 b) and __udivdi3 __umoddi3 __divdi3 __moddi3, llrm-6502 rc2-first layout: a0..a5 in rc2..rc7, a6:a7 at the
; caller's stack (rc0:rc1)+0/+1, b0..b7 at (rc0:rc1)+2..9 (read only). Result A:X:rc2..rc7. Scratch rc16..rc25 and BSS.
; Multiply: shift and add, one multiplier byte at a time from the lowest to the highest nonzero byte of b.
; Divide: restoring, the dividend shifts into the remainder and the quotient bits into the dividend's emptied low end;
; leading zero bytes of the dividend are skipped. Division by zero gives all ones and the dividend as remainder (lib/int/int64.c).

.setcpu "6502"
.export __muldi3, __udivdi3, __umoddi3, __divdi3, __moddi3

rc0 = $80
rc2 = $82
rc3 = $83
rc4 = $84
rc5 = $85
rc6 = $86
rc7 = $87
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

.bss
bb:     .res 1
top:    .res 1
d:      .res 8
rem:    .res 8
cnt:    .res 1
sgn:    .res 1
sgd:    .res 1

.segment "CODE"

__muldi3:
    ldy #0
    lda (rc0),y
    sta a6
    iny
    lda (rc0),y
    sta a7
    lda #0
    ldx #7
:   sta r0,x
    dex
    bpl :-
    ldy #9                  ; highest nonzero byte of b
:   lda (rc0),y
    bne found
    dey
    cpy #1
    bne :-
    beq out                 ; b = 0
found:
    sty top
    ldy #2
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

; ---- divide: N = rc16..rc23 (dividend, then quotient), d = divisor, rem = remainder
N = $90

load:
    ldx #5
:   lda rc2,x
    sta N,x
    dex
    bpl :-
    ldy #0
    lda (rc0),y
    sta N+6
    iny
    lda (rc0),y
    sta N+7
    ldy #2
    ldx #0
:   lda (rc0),y
    sta d,x
    iny
    inx
    cpx #8
    bne :-
    rts

udm:
    lda d
    ora d+1
    ora d+2
    ora d+3
    ora d+4
    ora d+5
    ora d+6
    ora d+7
    bne dok
    ldx #7                  ; divide by zero
:   lda N,x
    sta rem,x
    lda #$ff
    sta N,x
    dex
    bpl :-
    rts
dok:
    lda #0
    ldx #7
:   sta rem,x
    dex
    bpl :-
    ldx #7                  ; highest nonzero byte of the dividend
:   lda N,x
    bne dfound
    dex
    bpl :-
    rts                     ; zero divided by anything
dfound:
    txa
    clc
    adc #1
    asl a
    asl a
    asl a
    sta cnt                 ; bits to go
bs: cpx #7                  ; move the dividend up until its top byte is nonzero
    beq iter
    ldy #7
:   lda N-1,y
    sta N,y
    dey
    bne :-
    sty N
    inx
    jmp bs
iter:
    asl N
    rol N+1
    rol N+2
    rol N+3
    rol N+4
    rol N+5
    rol N+6
    rol N+7
    rol rem
    rol rem+1
    rol rem+2
    rol rem+3
    rol rem+4
    rol rem+5
    rol rem+6
    rol rem+7
    bcs sub                 ; 65 bits: certainly not less than d
    sec
    lda rem
    sbc d
    lda rem+1
    sbc d+1
    lda rem+2
    sbc d+2
    lda rem+3
    sbc d+3
    lda rem+4
    sbc d+4
    lda rem+5
    sbc d+5
    lda rem+6
    sbc d+6
    lda rem+7
    sbc d+7
    bcc nosub
sub:
    sec
    lda rem
    sbc d
    sta rem
    lda rem+1
    sbc d+1
    sta rem+1
    lda rem+2
    sbc d+2
    sta rem+2
    lda rem+3
    sbc d+3
    sta rem+3
    lda rem+4
    sbc d+4
    sta rem+4
    lda rem+5
    sbc d+5
    sta rem+5
    lda rem+6
    sbc d+6
    sta rem+6
    lda rem+7
    sbc d+7
    sta rem+7
    inc N
nosub:
    dec cnt
    beq udone
    jmp iter
udone:
    rts

retq:
    ldx #5
:   lda N+2,x
    sta rc2,x
    dex
    bpl :-
    ldx N+1
    lda N
    rts
retr:
    ldx #5
:   lda rem+2,x
    sta rc2,x
    dex
    bpl :-
    ldx rem+1
    lda rem
    rts

negn:
    sec
    lda #0
    sbc N
    sta N
    lda #0
    sbc N+1
    sta N+1
    lda #0
    sbc N+2
    sta N+2
    lda #0
    sbc N+3
    sta N+3
    lda #0
    sbc N+4
    sta N+4
    lda #0
    sbc N+5
    sta N+5
    lda #0
    sbc N+6
    sta N+6
    lda #0
    sbc N+7
    sta N+7
    rts
negd:
    sec
    lda #0
    sbc d
    sta d
    lda #0
    sbc d+1
    sta d+1
    lda #0
    sbc d+2
    sta d+2
    lda #0
    sbc d+3
    sta d+3
    lda #0
    sbc d+4
    sta d+4
    lda #0
    sbc d+5
    sta d+5
    lda #0
    sbc d+6
    sta d+6
    lda #0
    sbc d+7
    sta d+7
    rts
negr:
    sec
    lda #0
    sbc rem
    sta rem
    lda #0
    sbc rem+1
    sta rem+1
    lda #0
    sbc rem+2
    sta rem+2
    lda #0
    sbc rem+3
    sta rem+3
    lda #0
    sbc rem+4
    sta rem+4
    lda #0
    sbc rem+5
    sta rem+5
    lda #0
    sbc rem+6
    sta rem+6
    lda #0
    sbc rem+7
    sta rem+7
    rts

__udivdi3:
    jsr load
    jsr udm
    jmp retq
__umoddi3:
    jsr load
    jsr udm
    jmp retr

sload:                      ; signed: absolute values in N and d, the signs in sgn (dividend) and sgd (divisor)
    jsr load
    lda N+7
    and #$80
    sta sgn
    beq :+
    jsr negn
:   lda d+7
    and #$80
    sta sgd
    beq :+
    jsr negd
:   rts

__divdi3:
    jsr sload
    jsr udm
    lda sgn
    eor sgd
    beq :+
    lda d                   ; no negation when dividing by zero: the quotient stays all ones
    ora d+1
    ora d+2
    ora d+3
    ora d+4
    ora d+5
    ora d+6
    ora d+7
    beq :+
    jsr negn
:   jmp retq
__moddi3:
    jsr sload
    jsr udm
    lda sgn
    beq :+
    jsr negr
:   jmp retr
