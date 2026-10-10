;; name: B$DIV0, B$TRAPS
;; desc: the divide fault: an integer division by zero is the error Division by zero
;;
;; args: B$TRAPS (none)                       | takes processor exception 0
;; retn: B$DIV0 does not return: the error is raised
;;
;; chng: oct/26 written [ali]
;; obs.: the extender (DPMI 0203h) enters the handler with the exception's frame on the stack: its own way
;;       back, an error code, then the faulting program's eip, cs, eflags, esp and ss.  The handler makes the
;;       fault resume at B$DIV0, which raises the error on the program's own stack; the extender puts the old
;;       handler back when the program ends.

                .386
                .model  flat

                extrn   B$ERROR:near

                public  B$DIV0
                public  B$TRAPS

BE_DIVIDE0      equ     11
DPMI_SET_EXCEPTION equ  0203h
DIVIDE_EXCEPTION   equ  0
FRAME_EIP       equ     12                      ;; the faulting eip, past the way back and the error code

.code
;;::::::::::::::
;; B$DIV0 ()
B$DIV0          proc

                sti
                mov     eax, BE_DIVIDE0
                call    B$ERROR
B$DIV0          endp

;;::::::::::::::
;; the exception handler: the fault resumes at B$DIV0
divide_fault    proc

                mov     dword ptr [esp+FRAME_EIP], offset B$DIV0
                retf
divide_fault    endp

;;::::::::::::::
;; B$TRAPS ()
B$TRAPS         proc

                push    ebx
                push    ecx
                push    edx
                mov     eax, DPMI_SET_EXCEPTION
                mov     ebx, DIVIDE_EXCEPTION
                mov     ecx, cs
                mov     edx, offset divide_fault
                int     31h
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$TRAPS         endp
                end
