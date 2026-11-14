; Shared by di64.s and umod64.s: the 64-bit divide's operand cells, loader, sign handling and negations. Each object assembles its own copy.
rc0 = $80
rc2 = $82
rc3 = $83
rc4 = $84
rc5 = $85
rc6 = $86
rc7 = $87
N = $90                ; rc16..rc23: the dividend, then the remainder
.bss
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

load:
    sta N
    stx N+1
    ldx #5
:   lda rc2,x
    sta N+2,x
    dex
    bpl :-
    ldy #0
    ldx #0
:   lda (rc0),y
    sta d,x
    iny
    inx
    cpx #8
    bne :-
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

