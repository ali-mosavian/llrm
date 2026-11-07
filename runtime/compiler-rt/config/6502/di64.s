; 64-bit multiply and divide for the 6502: u64 __muldi3(u64 a, u64 b) and __udivdi3 __umoddi3 __divdi3 __moddi3, llrm-6502 rc2-first layout: a0..a5 in rc2..rc7, a6:a7 at the
; caller's stack (rc0:rc1)+0/+1, b0..b7 at (rc0:rc1)+2..9 (read only). Result A:X:rc2..rc7. Scratch rc16..rc25 and BSS.
; Multiply: shift and add, one multiplier byte at a time from the lowest to the highest nonzero byte of b.
; Divide: restoring, the divisor is shifted up to the dividend's bit length and steps down one bit at a time; the quotient bits roll into qv. Division by zero gives all ones and the dividend as remainder (lib/int/int64.c).

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
qv:     .res 8
ln:     .res 1
ld:     .res 1
nb:     .res 1
sh:     .res 1
dz:     .res 1
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

; ---- divide: N = rc16..rc23 (dividend, then remainder), d = divisor, qv = quotient
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
    lda #0
    sta dz
    lda d
    ora d+1
    ora d+2
    ora d+3
    ora d+4
    ora d+5
    ora d+6
    ora d+7
    bne dok
    ldx #7                  ; divide by zero: all ones, the remainder stays the dividend
    lda #$ff
    sta dz
:   sta qv,x
    dex
    bpl :-
    rts
dok:
    lda #0
    ldx #7
:   sta qv,x
    dex
    bpl :-
    ldx #7                  ; N < d: quotient 0, remainder N
:   lda N,x
    cmp d,x
    bne :+
    dex
    bpl :-
    sec
:   bcs align
    rts
align:                      ; s = bitlength(N) - bitlength(d); d <<= s; s + 1 steps
    ldx #7
:   lda N,x
    bne :+
    dex
    bpl :-
:   stx ln
    ldy #0
:   iny
    lsr a
    bne :-
    txa
    asl a
    asl a
    asl a
    sty cnt
    clc
    adc cnt
    sta sh                  ; bitlength(N)
    ldx #7
:   lda d,x
    bne :+
    dex
    bpl :-
:   stx ld
    ldy #0
:   iny
    lsr a
    bne :-
    txa
    asl a
    asl a
    asl a
    sty cnt
    clc
    adc cnt                 ; bitlength(d)
    eor #$ff
    sec
    adc sh
    sta sh                  ; s
    lsr a
    lsr a
    lsr a
    sta nb                  ; bytes to move d up
    beq nomove
    ldx ld
:   txa
    clc
    adc nb
    tay
    lda d,x
    sta d,y
    dex
    bpl :-
    ldx nb
    lda #0
:   dex
    sta d,x
    bne :-
nomove:
    lda sh
    and #7
    beq noshl
    tay
:   asl d
    rol d+1
    rol d+2
    rol d+3
    rol d+4
    rol d+5
    rol d+6
    rol d+7
    dey
    bne :-
noshl:
    lda sh
    clc
    adc #1
    sta cnt
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
retr:
    ldx #5
:   lda N+2,x
    sta rc2,x
    dex
    bpl :-
    ldx N+1
    lda N
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
    lda dz                  ; no negation when dividing by zero: the quotient stays all ones
    bne :+
    jsr negq
:   jmp retq
__moddi3:
    jsr sload
    jsr udm
    lda sgn
    beq :+
    jsr negn
:   jmp retr
