        TITLE   FRAME - BASIC procedure frames (B$ENRA, B$EXSA)
;***
; FRAME - BASIC procedure frames
;
;Purpose:
;       A compiled SUB or FUNCTION calls B$ENRA before its body and B$EXSA
;       after.  Between them BP addresses the frame QB's inc/stack.inc draws:
;
;               BP+0    previous BP
;               BP-2    previous BASIC frame (b$curframe)
;               BP-4    SI
;               BP-6    DI
;               BP-8    bytes of locals
;               BP-10   GOSUB count
;               ...     locals, zeroed
;
;       The error handler walks the frames through b$curframe.  Strings
;       local to the procedure are freed by the procedure, with B$STDL.
;       The same bytes as the entries of BCOM45's rtenexit.
;
;       Kept in assembly: the entries take the caller's frame apart and
;       leave BP and SP as the procedure uses them, which C cannot do.
;******************************************************************************
        .MODEL  MEDIUM
        .386

        EXTRN   _cur_level:WORD         ; b$curlevel, in nhstutil.c

        PUBLIC  B$ENRA
        PUBLIC  B$EXSA
        PUBLIC  _b_curframe

_DATA   SEGMENT WORD PUBLIC 'DATA'
_b_curframe     DW      0               ; frame of the BASIC procedure running
b$return        DD      0               ; where the entry returns to
_DATA   ENDS

        .CODE   FRAME_TEXT
        ASSUME  DS:DGROUP

;***
;B$ENRA - Enter a BASIC procedure
;
;Purpose:
;       Build the frame and zero its locals.
;
;Entry:
;       CX      Bytes of locals, even
;       BX      Temporary string slots (not used: B$STDL frees them)
;
;Exit:
;       BP      The new frame
;       SP      Below the locals
;
;Uses:
;       AX, CX, DI
;
;Exceptions:
;       None.
;****
B$ENRA  PROC    FAR
        POP     WORD PTR b$return       ; our return address, off the stack
        POP     WORD PTR b$return+2
        XOR     AX,AX
        PUSH    BP
        MOV     BP,SP
        PUSH    _b_curframe             ; previous BASIC frame
        PUSH    SI
        PUSH    DI
        PUSH    CX                      ; bytes of locals
        PUSH    AX                      ; GOSUB count
        SUB     SP,CX
        MOV     _b_curframe,BP
        INC     _cur_level
        MOV     DI,SP
        PUSH    ES
        PUSH    DS
        POP     ES
        SHR     CX,1
        XOR     AX,AX
        REP     STOSW                   ; zero the locals
        POP     ES
        JMP     DWORD PTR b$return
B$ENRA  ENDP

;***
;B$EXSA - Exit a BASIC procedure
;
;Purpose:
;       Take the frame down and return to the caller.
;
;Entry:
;       BP      The frame B$ENRA built
;       DX:AX   The function's result, kept
;
;Exit:
;       SI, DI, BP, b$curframe as the caller had them
;
;Uses:
;       None.
;****
B$EXSA  PROC    FAR
        POP     WORD PTR b$return       ; our return address, off the stack
        POP     WORD PTR b$return+2
        DEC     _cur_level
        LEA     SP,[BP-6]
        POP     DI
        POP     SI
        POP     _b_curframe
        POP     BP
        JMP     DWORD PTR b$return
B$EXSA  ENDP

        END
