        TITLE   VARARGS - Entries whose stack block has the call's own length
;***
; VARARGS - B$DDIM, B$RDIM, B$CLOS
;
;Purpose:
;       DIM and REDIM of a dynamic array pass every dimension's
;       bounds on the stack, and CLOSE every channel it closes, so the entry
;       pops as many bytes as the call pushed (QB rt/dynamic.asm CleanStack,
;       rt/dkutil.asm B$CLOS).  A register convention cannot say that, so
;       these keep BASIC's pushes and C does the work (array_dim in array.c,
;       file_close in file.c).
;
;       The DIM block, last pushed first:
;
;               descriptor, rank + features << 8, element size,
;               then each dimension's upper and lower bound
;
;       The CLOSE block: the count, then the channels
;******************************************************************************
        .MODEL  MEDIUM
        .386

        EXTRN   ARRAY_DIM:FAR
        EXTRN   FILE_CLOSE:FAR

        PUBLIC  B$DDIM
        PUBLIC  B$RDIM
        PUBLIC  B$CLOS

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
        JMP     SHORT POPBLOCK
DIMENSION       ENDP

;***
;B$CLOS - CLOSE [#channel [, #channel ...]]
;
;Entry:
;       The count of channels on top of the stack, the channels below it
;
;Exit:
;       The block gone: 2 bytes and 2 for each channel
;
;Uses:
;       AX, BX, DX (file_close's)
;****
B$CLOS  PROC    FAR
        PUSH    BP
        MOV     BP,SP
        LEA     AX,[BP+8]               ; the channels
        MOV     DX,[BP+6]               ; how many
        CALL    FAR PTR FILE_CLOSE
        MOV     BX,[BP+6]
        SHL     BX,1
        ADD     BX,2                    ; bytes the caller pushed
B$CLOS  ENDP

;***
;POPBLOCK - Return to the caller, popping BX bytes of its arguments
;
;Entry:
;       BX      Bytes to pop
;       BP      Pushed by our entry
;****
POPBLOCK        PROC    FAR
        POP     BP
        POP     AX                      ; return offset
        POP     DX                      ; return segment
        ADD     SP,BX
        PUSH    DX
        PUSH    AX
        RETF
POPBLOCK        ENDP

        END
