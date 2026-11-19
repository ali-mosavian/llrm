; Turns umul8x8r16 (LHS in A, RHS in ptr1 low, result X/A) into
; unsigned hand_umul8x8r16(unsigned char a, unsigned char b): a on the C stack, b in A.
        .export _hand_umul8x8r16
        .import umul8x8r16, popa
        .importzp ptr1
_hand_umul8x8r16:
        sta     ptr1
        jsr     popa
        jmp     umul8x8r16
