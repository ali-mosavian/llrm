; 8-bit float (OCP FP8 E5M2: sign, 5 exponent bits bias 15, 2 mantissa bits; denormals, inf, NaN; round to nearest even)
; add, sub, mul, div for the 6502, hand written. Args rc2 and rc3, result A, scratch rc2..rc7 and rc16..rc25.
;
; The tricks: the low seven bits are a monotonic code, so |a| < |b| and "is inf or NaN" are integer compares and
; rounding up is "code + 1", which carries from the mantissa into the exponent and from the largest finite value into
; infinity by itself; denormal operands are normalized on unpack (exponents go below 1) so no operation has a denormal
; case; lost bits are jammed into bit 0 of the significand, four places under the guard bit, instead of a sticky flag;
; every operation ends in one packer that works on a left-aligned 8-bit significand.

.setcpu "6502"
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
.export __fp8_add, __fp8_sub, __fp8_mul, __fp8_div

sa  = rc16              ; operand a: sign ($80/0), exponent (signed, below 1 for denormals), significand 4..7 (0 for zero)
ea  = rc17
ma  = rc18
sb  = rc19
eb  = rc20
mb  = rc21
bm  = rc22              ; b aligned
cd  = rc23              ; code under construction
rsg = rc4               ; packer inputs: sign, exponent of the leading one (signed), significand with its one in bit 7
pe  = rc6
pm  = rc7
t1  = rc24              ; |a| and |b| as codes
t2  = rc25

.segment "CODE"

__fp8_sub:
    lda rc3
    eor #$80
    sta rc3
__fp8_add:
    jsr mags
    bcc :+
    ldx #0
    jmp special
:   lda t2                  ; order by magnitude: the code order is the value order
    cmp t1
    bcc :+
    beq :+
    ldx rc2
    lda rc3
    sta rc2
    stx rc3
:   jsr unpab
    lda sa
    sta rsg
    lda ea
    sta pe
    sec
    sbc eb
    tay                     ; exponent difference
    lda ma
    asl a
    asl a
    asl a
    asl a
    asl a
    sta pm
    lda mb
    asl a
    asl a
    asl a
    asl a
    asl a
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

; ---- pack: sign rsg, exponent of the leading one pe (signed, may be <= 0), significand pm with its leading one in bit 7
pk:
    lda pe
    bmi pden
    beq pden
    cmp #31
    bcs pinf
pnorm:
    sec
    sbc #1
    asl a
    asl a
    sta cd
    lda pm
    lsr a
    lsr a
    lsr a
    lsr a
    lsr a
    clc
    adc cd
    sta cd                  ; (e-1)*4 + top three bits
    lda pm
    and #$10                ; guard bit
    beq pdone
    lda pm
    and #$0f
    bne pup
    lda cd
    lsr a
    bcc pdone
pup:
    inc cd
pdone:
    lda cd
    cmp #$7c
    bcc :+
pinf:
    lda #$7c
:   ora rsg
    rts
pden:                       ; below the normal range: shift into the denormal position, 1 - pe places
    lda #1
    sec
    sbc pe
    tay
    ldx #0
    jsr shr
    lda #1
    jmp pnorm

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

; ---- multiply
__fp8_mul:
    jsr mags
    bcc :+
    ldx #1
    jmp special
:   jsr unpab
    lda sa
    eor sb
    sta rsg
    lda ma
    beq zret
    lda mb
    beq zret
    lda #0                  ; P = ma * mb, 16..49
    ldx #3
:   lsr mb
    bcc :+
    clc
    adc ma
:   asl ma
    dex
    bne :--
    sta pm
    lda ea
    clc
    adc eb
    sec
    sbc #15
    sta pe
    lda pm
    cmp #32
    bcs :+
    asl pm                  ; leading one at bit 4: three places up, exponent as is
    asl pm
    asl pm
    jmp pk
:   asl pm                  ; leading one at bit 5: two places up, exponent one higher
    asl pm
    inc pe
    jmp pk
zret:
    lda rsg
    rts

; ---- |a| and |b| as codes into t1 and t2; carry set when either is an infinity or a NaN
mags:
    lda rc2
    and #$7f
    sta t1
    lda rc3
    and #$7f
    sta t2
    cmp #$7c
    bcs :+
    lda t1
    cmp #$7c
:   rts

; ---- infinities and NaNs: X = 0 add, 1 multiply, 2 divide
special:
    lda t1
    cmp #$7d
    bcs retnan
    lda t2
    cmp #$7d
    bcs retnan
    cpx #0
    bne notadd
    lda t1                  ; the infinity wins; two of opposite sign are invalid
    cmp #$7c
    bne retb
    lda t2
    cmp #$7c
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
    beq retnan
    lda t2
    beq retnan
spinf:
    lda rsg
    ora #$7c
    rts
divsp:
    lda t1
    cmp #$7c
    bne divz
    lda t2
    cmp #$7c
    bne spinf
retnan:
    lda #$7e                ; inf / inf
    rts
divz:                       ; finite / inf = 0
    lda rsg
    rts

; ---- divide
__fp8_div:
    jsr mags
    bcc :+
    ldx #2
    jmp special
:   jsr unpab
    lda sa
    eor sb
    sta rsg
    lda mb
    bne :+
    lda ma                  ; x / 0: infinity, 0 / 0: NaN
    beq retnan
    lda #$7c
    ora rsg
    rts
:   lda ma
    bne :+
    lda rsg
    rts
:   ldy #5                  ; q = floor(16 ma / mb), five restoring steps
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
    adc #14
    sta pe
    lda t1
    cmp #16
    bcs :+
    asl a                   ; ratio below one: leading one at bit 3, four places up
    asl a
    asl a
    asl a
    jmp dpm
:   asl a                   ; ratio at least one: leading one at bit 4, three places up, exponent one higher
    asl a
    asl a
    inc pe
dpm:
    cpx #0
    beq :+
    ora #1
:   sta pm
    jmp pk

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
    lsr a
    lsr a
    and #$1f
    sta rc17,x
    lda t1
    and #3
    ldy rc17,x
    bne :++
    inc rc17,x              ; denormal: exponent 1, no implicit one
    cmp #0
    beq :+++
:   cmp #4
    bcs :++
    asl a
    dec rc17,x
    jmp :-
:   ora #4
:   sta rc18,x
    rts
