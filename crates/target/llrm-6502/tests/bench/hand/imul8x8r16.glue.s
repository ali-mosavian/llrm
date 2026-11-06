; Turns imul8x8r16 (LHS in A, RHS in ptr1 low, result X/A) into
; int hand_imul8x8r16(signed char a, signed char b): a on the C stack, b in A.
        .export _hand_imul8x8r16
        .import imul8x8r16, popa
        .importzp ptr1
_hand_imul8x8r16:
        sta     ptr1
        jsr     popa
        jmp     imul8x8r16
