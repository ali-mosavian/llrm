; expf and logf for the 6502, hand written, bit-exact with lib/math/mathf.c. They run a small interpreter: the arithmetic is a
; byte-coded program over eight work cells and a constant table, each step calling sf_add_c/sf_sub_c/sf_mul_c/sf_div_c or
; the int conversions, so a polynomial costs four bytes per operation instead of a call sequence. Special values, the
; exponent split and the final power-of-two scaling are plain 6502 code in front of and behind the program.
; ABI as sf32_abi.s: x in A:X and rc2:rc3 (moved to rc2..rc5 on entry), result A:X:rc2:rc3. State lives in BSS (not reentrant), so nothing depends on the
; zero-page cells the arithmetic routines clobber.

.setcpu "6502"
.include "sf32.inc"
.import sf_add_c, sf_sub_c, sf_mul_c, sf_div_c, sf_floatsisf_c, sf_fixsfsi_c
.export expf, logf

; program steps: op, dst, a, b; operand 0..7 is a work cell, 8.. a constant copied behind them; $ff ends
.macro FADD xd, xa, xb
    .byte 0, xd, xa, xb
.endmacro
.macro FSUB xd, xa, xb
    .byte 1, xd, xa, xb
.endmacro
.macro FMUL xd, xa, xb
    .byte 2, xd, xa, xb
.endmacro
.macro FDIV xd, xa, xb
    .byte 3, xd, xa, xb
.endmacro
.macro CVTF xd, xa
    .byte 4, xd, xa, 0
.endmacro
.macro CVTI xd, xa
    .byte 5, xd, xa, 0
.endmacro

.bss
regs:   .res 4 * 18
ip:     .res 2
rop:    .res 1
rdst:   .res 1
rb:     .res 1
xb:     .res 4
kl:     .res 1
kh:     .res 1
sv8:    .res 1
sv9:    .res 1

.segment "CODE"

; ---- interpreter: A:X = program address
run:
    sta ip
    stx ip+1
rnext:
    lda ip
    sta rc16
    lda ip+1
    sta rc17
    ldy #0
    lda (rc16),y
    cmp #$ff
    beq rdone
    sta rop
    iny
    lda (rc16),y
    sta rdst
    iny
    lda (rc16),y
    asl a
    asl a
    tax
    iny
    lda (rc16),y
    asl a
    asl a
    sta rb
    ldy #0
    jsr getop
    ldx rb
    ldy #4
    jsr getop
    clc
    lda ip
    adc #4
    sta ip
    bcc :+
    inc ip+1
:   ldx rop
    jsr callop
    sta rc16                ; result A:X:rc2:rc3 into the destination cell
    stx rc17
    lda rdst
    asl a
    asl a
    tay
    lda rc16
    sta regs,y
    lda rc17
    sta regs+1,y
    lda rc2
    sta regs+2,y
    lda rc3
    sta regs+3,y
    jmp rnext
rdone:
    rts
getop:                      ; cell at X to rc2..rc5 (Y = 0) or rc6..rc9 (Y = 4)
    lda regs,x
    sta rc2,y
    lda regs+1,x
    sta rc3,y
    lda regs+2,x
    sta rc4,y
    lda regs+3,x
    sta rc5,y
    rts
callop:
    lda optab_hi,x
    pha
    lda optab_lo,x
    pha
    rts
optab_lo:
    .lobytes sf_add_c-1, sf_sub_c-1, sf_mul_c-1, sf_div_c-1, sf_floatsisf_c-1, sf_fixsfsi_c-1
optab_hi:
    .hibytes sf_add_c-1, sf_sub_c-1, sf_mul_c-1, sf_div_c-1, sf_floatsisf_c-1, sf_fixsfsi_c-1

; ---- helpers
isnan:                      ; carry set when rc2..rc5 is a NaN
    lda rc5
    and #$7f
    cmp #$7f
    bne nonan
    lda rc4
    bpl nonan
    and #$7f
    ora rc3
    ora rc2
    beq nonan
    sec
    rts
nonan:
    clc
    rts
retarg:                     ; return rc2..rc5
    ldy rc2
    ldx rc3
    lda rc4
    sta rc2
    lda rc5
    sta rc3
    tya
    rts
nanret:                     ; quiet NaN of the argument
    lda rc4
    ora #$40
    sta rc4
    jmp retarg
putreg:                     ; A:X:rc2:rc3 into cell Y*4
    sta regs,y
    txa
    sta regs+1,y
    lda rc2
    sta regs+2,y
    lda rc3
    sta regs+3,y
    rts
mulc:                       ; rc2..rc5 *= the constant whose bytes 3 and 2 are A and X (low bytes 0); result in rc2..rc5
    sta rc9
    stx rc8
    lda #0
    sta rc6
    sta rc7
    jsr sf_mul_c
    ldy rc2
    sty rc4
    ldy rc3
    sty rc5
    sta rc2
    stx rc3
    rts

; rc8 and rc9 belong to the caller: the wrappers give them back
logf:
    jsr ax2cells
    jsr save89
    jsr logf_c
    jmp rest89
expf:
    jsr ax2cells
    jsr save89
    jsr expf_c
    jmp rest89
ax2cells:               ; x from A:X and rc2:rc3 into rc2..rc5, through Y
    ldy rc3
    sty rc5
    ldy rc2
    sty rc4
    sta rc2
    stx rc3
    rts
save89:
    lda rc8
    sta sv8
    lda rc9
    sta sv9
    rts
rest89:
    sta rc4
    lda sv8
    sta rc8
    lda sv9
    sta rc9
    lda rc4
    rts

; ---- logf: x = 2^k (1 + f), f in [sqrt(1/2) - 1, sqrt(2) - 1]
logf_c:
    jsr isnan
    bcc :+
    jmp nanret
:   lda rc5
    and #$7f
    ora rc4
    ora rc3
    ora rc2
    bne :+
    lda #$80                ; log(0) = -inf
    sta rc2
    lda #$ff
    sta rc3
    lda #0
    tax
    rts
:   lda rc5
    bpl :+
    lda #$c0                ; negative: NaN
    sta rc2
    lda #$7f
    sta rc3
    lda #0
    tax
    rts
:   cmp #$7f
    bne :+
    lda rc4
    bpl :+
    jmp retarg              ; +inf
:   lda rc5
    bne lnorm
    lda rc4
    bmi lnorm
    lda #0                  ; denormal: scale by 2^25, k = -25
    sta rc6
    sta rc7
    sta rc8
    lda #$4c
    sta rc9
    jsr sf_mul_c
    sta xb
    stx xb+1
    lda rc2
    sta xb+2
    lda rc3
    sta xb+3
    ldx #$e7
    ldy #$ff
    bne lgot
lnorm:
    ldx #3
:   lda rc2,x
    sta xb,x
    dex
    bpl :-
    ldx #0
    ldy #0
lgot:
    stx kl
    sty kh
    lda xb+2                ; k += biased exponent - 127
    asl a
    lda xb+3
    rol a
    sec
    sbc #127
    ldy #0
    bcs :+
    dey
:   clc
    adc kl
    sta kl
    tya
    adc kh
    sta kh
    lda xb+2                ; mantissa with exponent 127
    ora #$80
    sta xb+2
    lda #$3f
    sta xb+3
    sec                     ; above sqrt(2): halve it and count one more
    lda #$f3
    sbc xb
    lda #$04
    sbc xb+1
    lda #$b5
    sbc xb+2
    lda #$3f
    sbc xb+3
    bcs lf
    lda xb+2
    and #$7f
    sta xb+2
    inc kl
    bne lf
    inc kh
lf: ldx #3                  ; f = m - 1
:   lda xb,x
    sta rc2,x
    dex
    bpl :-
    lda #0
    sta rc6
    sta rc7
    lda #$80
    sta rc8
    lda #$3f
    sta rc9
    jsr sf_sub_c
    ldy #0
    jsr putreg
    lda kl                  ; k as a 32-bit int in cell 6
    sta regs+24
    lda kh
    sta regs+25
    sta regs+26
    sta regs+27
    ldx #4*7-1
:   lda logc,x
    sta regs+32,x
    dex
    bpl :-
    lda #<logprog
    ldx #>logprog
    jsr run
    lda regs+26
    sta rc2
    lda regs+27
    sta rc3
    ldx regs+25
    lda regs+24
    rts

; ---- expf: x = k ln2 + r, exp(x) = 2^k (1 + r + r r P(r))
expf_c:
    jsr isnan
    bcc :+
    jmp nanret
:   lda rc5
    and #$7f
    sta rc7                 ; |x| high byte
    lda rc5
    bmi eneg
    sec                     ; x > 88.7228394 -> inf
    lda #$18
    sbc rc2
    lda #$72
    sbc rc3
    lda #$b1
    sbc rc4
    lda #$42
    sbc rc7
    bcs etiny
    lda #$80
    sta rc2
    lda #$7f
    sta rc3
    lda #0
    tax
    rts
eneg:
    sec                     ; x < -103.972084 -> 0
    lda #$b5
    sbc rc2
    lda #$f1
    sbc rc3
    lda #$cf
    sbc rc4
    lda #$42
    sbc rc7
    bcs etiny
    lda #0
    tax
    sta rc2
    sta rc3
    rts
etiny:
    lda rc7
    cmp #$32
    bcs enorm
    ldx #3                  ; |x| < 2^-27: 1 + x
:   lda rc2,x
    sta rc6,x
    dex
    bpl :-
    lda #0
    sta rc2
    sta rc3
    lda #$80
    sta rc4
    lda #$3f
    sta rc5
    jmp sf_add_c
enorm:
    ldx #3
:   lda rc2,x
    sta regs,x
    dex
    bpl :-
    lda #0                  ; cell 7: 0.5 with the sign of x, to round the quotient away from zero
    sta regs+28
    sta regs+29
    sta regs+30
    lda rc5
    and #$80
    ora #$3f
    sta regs+31
    ldx #4*10-1
:   lda expc,x
    sta regs+32,x
    dex
    bpl :-
    lda #<expprog
    ldx #>expprog
    jsr run
    lda regs+9              ; scale p by 2^k
    bne eneg2
    lda regs+8
    bpl emid
    jsr ploadp              ; k >= 128: (p * 2^127) * 2
    lda #$7f
    ldx #0
    jsr mulc
    lda #$40
    ldx #0
    jsr mulc
    jmp retarg
eneg2:
    lda regs+8
    cmp #$83
    bcs emid
    jsr ploadp              ; k < -125: (p * 2^(k+100)) * 2^-100
    lda regs+8
    sec
    sbc #29                 ; biased exponent of 2^(k+100)
    lsr a
    ldx #0
    bcc :+
    ldx #$80
:   jsr mulc
    lda #13
    ldx #$80
    jsr mulc
    jmp retarg
emid:                       ; p is in [0.7, 1.5]: the biased exponent plus k stays normal
    lda regs+26
    asl a
    lda regs+27
    rol a
    clc
    adc regs+8
    lsr a
    sta rc3
    lda regs+26
    and #$7f
    bcc :+
    ora #$80
:   sta rc2
    ldx regs+25
    lda regs+24
    rts
ploadp:
    ldx #3
:   lda regs+24,x
    sta rc2,x
    dex
    bpl :-
    rts

R0 = 0
R1 = 1
R2 = 2
R3 = 3
R4 = 4
R5 = 5
R6 = 6
R7 = 7
; log constants
L_2 = 8
L_P0 = 9
L_P1 = 10
L_P2 = 11
L_HALF = 12
L_LN2_1 = 13
L_LN2_2 = 14
logprog:
    FADD R1, R0, L_2        ; s = f / (2 + f)
    FDIV R2, R0, R1
    FMUL R3, R2, R2         ; z
    FMUL R4, R3, L_P2       ; horner
    FADD R4, R4, L_P1
    FMUL R4, R4, R3
    FADD R4, R4, L_P0
    FMUL R4, R3, R4         ; R
    FMUL R5, L_HALF, R0     ; hfsq
    FMUL R5, R5, R0
    FADD R4, R5, R4
    FMUL R4, R2, R4         ; s (hfsq + R)
    CVTF R6, R6             ; dk
    FMUL R7, R6, L_LN2_2
    FADD R7, R4, R7
    FSUB R7, R5, R7
    FSUB R7, R7, R0
    FMUL R6, R6, L_LN2_1
    FSUB R6, R6, R7
    .byte $ff
; exp constants
E_1 = 8
E_LOG2E = 9
E_LN2_1 = 10
E_LN2_2 = 11
E_P0 = 12
E_P1 = 13
E_P2 = 14
E_P3 = 15
E_P4 = 16
E_P5 = 17
expprog:
    FMUL R1, R0, E_LOG2E
    FADD R1, R1, R7
    CVTI R2, R1             ; k
    CVTF R3, R2
    FMUL R4, R3, E_LN2_1
    FSUB R4, R0, R4
    FMUL R5, R3, E_LN2_2
    FSUB R4, R4, R5         ; r
    FMUL R5, R4, E_P5
    FADD R5, R5, E_P4
    FMUL R5, R5, R4
    FADD R5, R5, E_P3
    FMUL R5, R5, R4
    FADD R5, R5, E_P2
    FMUL R5, R5, R4
    FADD R5, R5, E_P1
    FMUL R5, R5, R4
    FADD R5, R5, E_P0
    FMUL R6, R4, R4
    FMUL R6, R6, R5
    FADD R6, R4, R6
    FADD R6, E_1, R6        ; p
    .byte $ff

logc:
    .dword $40000000
    .dword $3f2aaaae
    .dword $3eccbe18
    .dword $3e977308
    .dword $3f000000
    .dword $3f317200
    .dword $35bfbe8e
expc:
    .dword $3f800000
    .dword $3fb8aa3b
    .dword $3f317200
    .dword $35bfbe8e
    .dword $3f000000
    .dword $3e2aaaab
    .dword $3d2aaa75
    .dword $3c088870
    .dword $3ab6a18b
    .dword $39509272
