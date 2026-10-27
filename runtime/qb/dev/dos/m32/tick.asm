;; name: B$TICKON, B$TICKOFF, B$TICKISR
;; desc: the clock tick (interrupt 1Ch, 18.2 times a second) calling the
;;       runtime's music, flat
;;
;; chng: oct/26 written [ali]
;; obs.: DOS/32A reflects 1Ch to a protected-mode handler (DPMI 0204h, 0205h).
;;       The handler keeps every register, puts the flat data selector (its own
;;       stack's) in ds and es, calls the C routine B$MUSICTICK (play.c), and
;;       chains. B$TICKON is called once, B$TICKOFF when the program ends: DOS/32A
;;       reflects a protected-mode 1Ch handler into the real-mode vector, and
;;       warns on the program's standard output (9003) if one is left there.

                .386
                .model  flat

                extrn   B$MUSICTICK:near

                public  B$TICKON
                public  B$TICKOFF
                public  B$TICKISR

CLOCK_TICK      equ     1Ch
DPMI_GET_VECTOR equ     0204h
DPMI_SET_VECTOR equ     0205h

.data
previous        dd      0                       ;; the replaced handler: offset
                dw      0                       ;; and selector

.code
;; B$TICKISR - the clock tick handler: play the music's tick, then chain.
;;
;; Calls B$MUSICTICK with the program's data selector in ds and es, then jumps
;; to
;; the handler it replaced.
;;
;; Args:
;;     None.
;;
;; Returns:
;;     Does not return to its caller: it ends by chaining to the handler it
;;         replaced.
;;
;; Saves:    all registers (pushad, ds, es)
;; Clobbers: flags
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

;; B$TICKON - take the protected-mode interrupt 1Ch.
;;
;; Saves the vector it replaces in `previous` (DPMI 0204h), then points the
;; vector at B$TICKISR (DPMI 0205h). Called once.
;;
;; Args:
;;     None.
;;
;; Saves:    ebx ecx edx
;; Clobbers: eax, flags
;; Patches:  previous.
B$TICKON        proc uses ebx ecx edx

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
                ret
B$TICKON        endp

;; B$TICKOFF - give back the protected-mode interrupt 1Ch, if B$TICKON took it.
;;
;; Args:
;;     None.
;;
;; Saves:    ebx ecx edx
;; Clobbers: eax, flags
B$TICKOFF       proc uses ebx ecx edx

                cmp     word ptr previous+4, 0
                je      @F
                mov     eax, DPMI_SET_VECTOR
                mov     ebx, CLOCK_TICK
                movzx   ecx, word ptr previous+4
                mov     edx, previous
                int     31h
                mov     previous, 0
                mov     word ptr previous+4, 0
@@:
                ret
B$TICKOFF       endp
                end
