; Entry points of the f32 arithmetic for the llrm-6502 convention (rc2-first): a in rc2..rc5, b's low two bytes in
; rc6:rc7 and its high two bytes in the caller's stack words at (rc0:rc1)+0 and +1; result A:X:rc2:rc3.
; The cores in sf32.s and sf32_muldiv.s read b whole from rc6..rc9, and rc8/rc9 belong to the caller, so these
; shims copy the stack bytes into rc8/rc9 and give the old values back. When the convention changes (A:X first),
; only this file changes.

.setcpu "6502"
.import sf_add_c, sf_sub_c, sf_mul_c, sf_div_c
.export __addsf3, __subsf3, __mulsf3, __divsf3

rc0 = $80
rc4 = $84
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
    lda rc8
    sta sv8
    lda rc9
    sta sv9
    ldy #0
    lda (rc0),y
    sta rc8
    iny
    lda (rc0),y
    sta rc9
    rts
