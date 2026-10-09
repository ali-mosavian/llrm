        TITLE   VARARGS - Entries whose stack block has the call's own length
;***
; VARARGS - B$DDIM, B$RDIM
;
;Purpose:
;       DIM and REDIM of a dynamic array pass every dimension's
;       bounds on the stack, so the entry pops as many bytes as the call
;       pushed (QB rt/dynamic.asm CleanStack).  A register convention cannot
;       say that, so these keep BASIC's pushes and C does the work
;       (array_dim, array.c).
;
;       The block, last pushed first:
;
;               descriptor, rank + features << 8, element size,
;               then each dimension's upper and lower bound
;******************************************************************************
        .MODEL  MEDIUM
        .386

        EXTRN   ARRAY_DIM:FAR

        PUBLIC  B$DDIM
        PUBLIC  B$RDIM

DIM_ALLOCATE    EQU     0               ; enum DimMode in array.h
DIM_REALLOCATE  EQU     1

        .CODE   VARARGS_TEXT

B$DDIM  PROC    FAR
        MOV     AX,DIM_ALLOCATE
        JMP     SHORT DIMENSION
B$DDIM  ENDP

B$RDIM  PROC    FAR
        MOV     AX,DIM_REALLOCATE
B$RDIM  ENDP

;***
;DIMENSION - Run array_dim on the caller's block, then pop it
;
;Entry:
;       AX      The DimMode
;       The block above the return address
;
;Exit:
;       The block gone: 6 bytes and 4 for each dimension
;
;Uses:
;       AX, BX, DX (array_dim's)
;****
DIMENSION       PROC    FAR
        PUSH    BP
        MOV     BP,SP
        LEA     DX,[BP+6]               ; the block, past BP and the return
        CALL    FAR PTR ARRAY_DIM
        MOV     BX,[BP+8]               ; rank + features << 8
        XOR     BH,BH
        SHL     BX,2
        ADD     BX,6                    ; bytes the caller pushed
        POP     BP
        POP     AX                      ; return offset
        POP     DX                      ; return segment
        ADD     SP,BX
        PUSH    DX
        PUSH    AX
        RETF
DIMENSION       ENDP

        END
