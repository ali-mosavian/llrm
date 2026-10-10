;; name: B$TICKON, B$TICKISR
;; desc: the clock tick (interrupt 1Ch, 18.2 times a second) calling the runtime's music, flat
;;
;; args: B$TICKON (none)                      | takes the protected-mode interrupt 1Ch
;; retn: B$TICKISR does not return to its caller: it ends by chaining to the handler it replaced
;;
;; chng: oct/26 written [ali]
;; obs.: DOS/32A reflects 1Ch to a protected-mode handler (DPMI 0204h, 0205h). The handler keeps every
;;       register, puts the flat data selector (its own stack's) in ds and es, calls the C routine
;;       B$MUSICTICK (play.c), and chains. B$TICKON is called once; the extender puts its vector back
;;       when the program ends.

                .386
                .model  flat

                extrn   B$MUSICTICK:near

                public  B$TICKON
                public  B$TICKISR

CLOCK_TICK      equ     1Ch
DPMI_GET_VECTOR equ     0204h
DPMI_SET_VECTOR equ     0205h

.data
previous        dd      0                       ;; the replaced handler: offset
                dw      0                       ;; and selector

.code
;;::::::::::::::
;; B$TICKISR ()
B$TICKISR       proc

                pushad
                push    ds
                push    es
                mov     ax, ss
                mov     ds, ax
                mov     es, ax
                cld
                call    B$MUSICTICK
                pop     es
                pop     ds
                popad
                jmp     fword ptr ss:previous
B$TICKISR       endp

;;::::::::::::::
;; B$TICKON ()
B$TICKON        proc

                push    ebx
                push    ecx
                push    edx
                mov     eax, DPMI_GET_VECTOR
                mov     ebx, CLOCK_TICK
                int     31h
                mov     previous, edx
                mov     word ptr previous+4, cx
                mov     eax, DPMI_SET_VECTOR
                mov     ebx, CLOCK_TICK
                mov     ecx, cs
                mov     edx, offset B$TICKISR
                int     31h
                pop     edx
                pop     ecx
                pop     ebx
                ret
B$TICKON        endp
                end
