; Entry points of the f32 arithmetic for the llrm-6502 convention (A:X first): a in A:X (low word) and rc2:rc3
; (high word), b in rc4:rc5 and rc6:rc7; result A:X:rc2:rc3.
; The cores in sf32.s and sf32_muldiv.s read a from rc2..rc5 and b whole from rc6..rc9, and rc8/rc9 belong to the
; caller, so these shims move the words up (Y only: A and X carry a's low word until the last two stores) and give
; the old rc8/rc9 back.

.setcpu "6502"
.import sf_add_c, sf_sub_c, sf_mul_c, sf_div_c
.export __addsf3, __subsf3, __mulsf3, __divsf3

rc2 = $82
rc3 = $83
rc4 = $84
rc5 = $85
rc6 = $86
rc7 = $87
rc8 = $88
rc9 = $89

.bss
sv8:    .res 1
sv9:    .res 1

.segment "CODE"

__addsf3:
    jsr in89
    jsr sf_add_c
    jmp out89
__subsf3:
    jsr in89
    jsr sf_sub_c
    jmp out89
__mulsf3:
    jsr in89
    jsr sf_mul_c
    jmp out89
__divsf3:
    jsr in89
    jsr sf_div_c
out89:
    sta rc4                 ; result bytes A:X:rc2:rc3 stay, rc4 is scratch
    lda sv8
    sta rc8
    lda sv9
    sta rc9
    lda rc4
    rts
in89:
    ldy rc8
    sty sv8
    ldy rc9
    sty sv9
    ldy rc7
    sty rc9
    ldy rc6
    sty rc8
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
    rts
