; 8-bit float, IEEE-style: sign, E exponent bits (bias 2^(E-1)-1, the top value reserved for inf and NaN), M mantissa
; bits, E + M = 7, denormals, round to nearest even. add, sub, mul, div for the 6502, hand written.
; Args rc2 and rc3, result A, scratch rc2..rc7 and rc16..rc25.
;
; Build options (ca65 -D name=value):
;   FP8_M        mantissa bits, 1..4 (default 2: E5M2; 3 gives E4M3-shaped IEEE, 4 E3M4, 1 E6M1)
;   FP8_SPECIAL  1 (default): inf and NaN operands and results. 0: not handled, inputs must be finite and a divisor of
;                zero gives infinity, 0/0 too; an overflowing result still comes out as the inf code
;   FP8_DENORM   1 (default): denormal operands and results. 0: flushed to zero (the sign is kept)
;   FP8_MULTAB  1: the significand product comes from a table of 2^(M+1) squared bytes (256 B for E4M3), M <= 3
;   FP8_ADD, FP8_SUB, FP8_MUL, FP8_DIV  1 (default) to build the routine, 0 to leave it out (SUB needs ADD)
;
; The tricks: the low seven bits are a monotonic code, so |a| < |b| and "is inf or NaN" are integer compares and
; rounding up is "code + 1", which carries from the mantissa into the exponent and from the largest finite value into
; infinity by itself; denormal operands are normalized on unpack (exponents go below 1) so no operation has a denormal
; case; lost bits are jammed into bit 0 of the significand, under the guard bit, instead of a sticky flag; every
; operation ends in one packer that works on a left-aligned 8-bit significand.

.setcpu "6502"
.ifndef FP8_M
FP8_M = 2
.endif
.ifndef FP8_SPECIAL
FP8_SPECIAL = 1
.endif
.ifndef FP8_DENORM
FP8_DENORM = 1
.endif
.ifndef FP8_ADD
FP8_ADD = 1
.endif
.ifndef FP8_SUB
FP8_SUB = 1
.endif
.ifndef FP8_MUL
FP8_MUL = 1
.endif
.ifndef FP8_DIV
FP8_DIV = 1
.endif
.ifndef FP8_MULTAB
FP8_MULTAB = 0
.endif
.if (FP8_M < 1) .or (FP8_M > 4)
.fatal "FP8_M must be 1..4: the 8-bit working significand needs three bits below the guard bit"
.endif
.if FP8_MULTAB .and (FP8_M > 3)
.fatal "FP8_MULTAB needs FP8_M <= 3: the significand product must fit a byte"
.endif
.if FP8_SUB .and (.not FP8_ADD)
.fatal "FP8_SUB needs FP8_ADD"
.endif

M     = FP8_M
E     = 7 - M
BIAS  = (1 << (E - 1)) - 1
EMAX  = (1 << E) - 1
INF   = EMAX << M       ; magnitude code of infinity; above it are NaNs
QNAN  = INF | (1 << (M - 1))
HID   = 1 << M          ; the implicit one of a significand
SIGSH = 7 - M           ; left alignment of a significand in the 8-bit working byte
GUARD = 1 << (SIGSH - 1)
REST  = GUARD - 1

rc2 = $82
rc3 = $83
rc4 = $84
rc6 = $86
rc7 = $87
rc16 = $90
rc17 = $91
rc18 = $92
rc19 = $93
rc20 = $94
rc21 = $95
rc22 = $96
rc23 = $97
rc24 = $98
rc25 = $99
.if FP8_ADD
.export __fp8_add
.endif
.if FP8_SUB
.export __fp8_sub
.endif
.if FP8_MUL
.export __fp8_mul
.endif
.if FP8_DIV
.export __fp8_div
.endif

sa  = rc16              ; operand a: sign ($80/0), exponent (signed, below 1 for denormals), significand HID..2*HID-1 (0 for zero)
ea  = rc17
ma  = rc18
sb  = rc19
eb  = rc20
mb  = rc21
bm  = rc22              ; b aligned
cd  = rc23              ; code under construction
rsg = rc16              ; packer inputs: sign (a's cell), exponent of the leading one (signed), significand with its one in bit 7
pe  = rc6
pm  = rc7
t1  = rc24              ; |a| and |b| as codes
t2  = rc25

.segment "CODE"

.if FP8_ADD
.if FP8_SUB
__fp8_sub:
    lda rc3
    eor #$80
    sta rc3
.endif
__fp8_add:
    jsr mags
.if FP8_SPECIAL
    bcc :+
    ldx #0
    jmp special
:
.endif
    lda t2                  ; order by magnitude: the code order is the value order
    cmp t1
    bcc :+
    beq :+
    ldx rc2
    lda rc3
    sta rc2
    stx rc3
:   jsr unpab
    lda ea
    sta pe
    sec
    sbc eb
    tay                     ; exponent difference
    lda ma
.repeat SIGSH
    asl a
.endrepeat
    sta pm
    lda mb
.repeat SIGSH
    asl a
.endrepeat
    sta bm
    tya
    beq :+
    ldx #bm-pm
    jsr shr
:   lda sa
    eor sb
    bmi dosub
    lda pm
    clc
    adc bm
    sta pm
    bcc pk
    ror pm                  ; carry out: shift down, the lost bit is jammed
    bcc :+
    lda pm
    ora #1
    sta pm
:   inc pe
    jmp pk
dosub:
    lda pm
    sec
    sbc bm
    sta pm
    bne nloop
    lda #0                  ; exact cancellation gives +0
    rts
nloop:
    bmi pk
    lda pe
    cmp #2
    bcc pk
    dec pe
    asl pm
    lda pm
    jmp nloop
.endif

; ---- pack: sign rsg, exponent of the leading one pe (signed, may be <= 0), significand pm with its leading one in bit 7
pk:
.if FP8_DENORM = 0
    lda pm                  ; no denormals: anything below the smallest normal, and zero, is a signed zero
    bpl pzero
.endif
    lda pe
.if FP8_DENORM
    bmi pden
    beq pden
.else
    bmi pzero
    beq pzero
.endif
    cmp #EMAX
    bcs pinf
pnorm:
    sec
    sbc #1
.repeat M
    asl a
.endrepeat
    sta cd
    lda pm
.repeat SIGSH
    lsr a
.endrepeat
    clc
    adc cd
    sta cd                  ; (e-1) * 2^M + the significand
    lda pm
    and #GUARD
    beq pdone
    lda pm
    and #REST
    bne pup
    lda cd
    lsr a
    bcc pdone
pup:
    inc cd
pdone:
    lda cd
    cmp #INF
    bcc :+
pinf:
    lda #INF
:   ora rsg
    rts
.if FP8_DENORM
pden:                       ; below the normal range: shift into the denormal position, 1 - pe places
    lda #1
    sec
    sbc pe
    tay
    ldx #0
    jsr shr
    lda #1
    jmp pnorm
.else
pzero:
    lda rsg
    rts
.endif

shr:                        ; cell rc7+X shifted right Y (1..) places, lost bits jammed into bit 0
    cpy #9
    bcc :+
    ldy #8
:   lda rc7,x
:   lsr a
    bcc :+
    ora #1
:   dey
    bne :--
    sta rc7,x
    rts

.if FP8_MUL
__fp8_mul:
.if FP8_SPECIAL
    jsr mags
    bcc :+
    ldx #1
    jmp special
:
.endif
    jsr unpab
    lda sa
    eor sb
    sta sa
    lda ma
    beq zret
    lda mb
    beq zret
    lda ea
    clc
    adc eb
    sec
    sbc #BIAS
    sta pe
.if FP8_MULTAB
    lda ma                  ; P = mtab[ma * 2^(M+1) + mb]
.repeat M + 1
    asl a
.endrepeat
    ora mb
    tax
    lda mtab,x
.elseif M < 4
    lda #0                  ; P = ma * mb
    ldx #M + 1
:   lsr mb
    bcc :+
    clc
    adc ma
:   asl ma
    dex
    bne :--
.endif
.if M < 4
    sta pm
    cmp #(1 << (2 * M + 1))
    bcs :+
.repeat 7 - 2 * M           ; leading one at bit 2M: align it to bit 7, exponent as is
    asl pm
.endrepeat
    jmp pk
:
.repeat 6 - 2 * M           ; leading one at bit 2M+1: one place less, exponent one higher
    asl pm
.endrepeat
    inc pe
    jmp pk
.else
    lda #0                  ; P = ma * mb needs 10 bits: cd:pm
    sta pm
    sta cd
    sta bm
    ldx #M + 1
:   lsr mb
    bcc :+
    lda pm
    clc
    adc ma
    sta pm
    lda cd
    adc bm
    sta cd
:   asl ma
    rol bm
    dex
    bne :--
    ldy #1                  ; shift right to put the leading one in bit 7, jamming what falls off
    lda cd
    cmp #2
    bcc :+
    inc pe
    iny
:   lsr cd
    ror pm
    bcc :+
    lda pm
    ora #1
    sta pm
:   dey
    bne :--
    jmp pk
.endif
zret:
    lda rsg
    rts
.endif

.if FP8_SPECIAL
; ---- |a| and |b| as codes into t1 and t2; carry set when either is an infinity or a NaN
mags:
    lda rc2
    and #$7f
    sta t1
    lda rc3
    and #$7f
    sta t2
    cmp #INF
    bcs :+
    lda t1
    cmp #INF
:   rts

; ---- infinities and NaNs: X = 0 add, 1 multiply, 2 divide
special:
    lda t1
    cmp #INF + 1
    bcs retnan
    lda t2
    cmp #INF + 1
    bcs retnan
    cpx #0
    bne notadd
    lda t1                  ; the infinity wins; two of opposite sign are invalid
    cmp #INF
    bne retb
    lda t2
    cmp #INF
    bne reta
    lda rc2
    eor rc3
    bmi retnan
reta:
    lda rc2
    rts
retb:
    lda rc3
    rts
notadd:
    lda rc2
    eor rc3
    and #$80
    sta rsg
    cpx #1
    bne divsp
    lda t1                  ; inf * 0 is invalid
.if FP8_DENORM
    beq retnan
    lda t2
    beq retnan
.else
    cmp #HID                ; a flushed denormal is a zero
    bcc retnan
    lda t2
    cmp #HID
    bcc retnan
.endif
spinf:
    lda rsg
    ora #INF
    rts
divsp:
    lda t1
    cmp #INF
    bne divz
    lda t2
    cmp #INF
    bne spinf
retnan:
    lda #QNAN               ; inf / inf
    rts
divz:                       ; finite / inf = 0
    lda rsg
    rts
.else
.if FP8_ADD
mags:                       ; |a| and |b| as codes into t1 and t2
    lda rc2
    and #$7f
    sta t1
    lda rc3
    and #$7f
    sta t2
    rts
.endif
.endif

.if FP8_DIV
__fp8_div:
.if FP8_SPECIAL
    jsr mags
    bcc :+
    ldx #2
    jmp special
:
.endif
    jsr unpab
    lda sa
    eor sb
    sta sa
    lda mb
    bne :+
.if FP8_SPECIAL
    lda ma                  ; x / 0: infinity, 0 / 0: NaN
    beq retnan
.endif
    lda #INF
    ora rsg
    rts
:   lda ma
    bne :+
    lda rsg
    rts
:   ldy #M + 3              ; q = floor(2^(M+2) ma / mb), restoring steps
    ldx #0
    stx t1
:   cmp mb
    bcc :+
    sbc mb
:   rol t1
    asl a
    dey
    bne :--
    tax                     ; remainder, nonzero means sticky
    lda ea
    sec
    sbc eb
    clc
    adc #BIAS - 1
    sta pe
    lda t1
    cmp #(1 << (M + 2))
    bcs :+
.repeat 6 - M               ; ratio below one: leading one at bit M+1
    asl a
.endrepeat
    jmp dpm
:
.repeat 5 - M               ; ratio at least one: leading one at bit M+2, exponent one higher
    asl a
.endrepeat
    inc pe
dpm:
    cpx #0
    beq :+
    ora #1
:   sta pm
    jmp pk
.endif

; ---- operand unpack: rc2, rc3 into sa/ea/ma and sb/eb/mb; denormals are normalized (exponent goes below 1)
unpab:
    ldy #0
    ldx #0
    jsr unp
    ldy #1
    ldx #3
unp:                        ; byte rc2+Y into the three cells at rc16+X
    lda rc2,y
    sta t1
    and #$80
    sta rc16,x
    lda t1
.repeat M
    lsr a
.endrepeat
    and #EMAX
    sta rc17,x
    lda t1
    and #HID - 1
    ldy rc17,x
    bne unnorm
    inc rc17,x              ; denormal: exponent 1, no implicit one
.if FP8_DENORM
    cmp #0
    beq unstore
unl:
    cmp #HID
    bcs unstore
    asl a
    dec rc17,x
    jmp unl
.else
    lda #0                  ; flushed to zero
    beq unstore
.endif
unnorm:
    ora #HID
unstore:
    sta rc18,x
    rts

.if FP8_MULTAB
.segment "RODATA"
mtab:
.repeat 2 << M, i
.repeat 2 << M, j
    .byte i * j
.endrepeat
.endrepeat
.endif
