;; name: B$TICKON, B$TICKISR
;; desc: the clock tick (interrupt 1Ch, 18.2 times a second) calling the runtime's music
;;
;; args: B$TICKON (none)                      | takes interrupt 1Ch
;; retn: B$TICKISR does not return to its caller: it ends by chaining to the handler it replaced
;;
;; chng: oct/26 written [ali]
;; obs.: the handler keeps every register, puts the program's DGROUP in ds and es, calls the
;;       C routine B$MUSICTICK (music.c), and chains. LL$SET_VECTOR remembers the old handler
;;       and puts it back when the program ends, so there is no way off but that. B$TICKON is
;;       called once.

                .model  medium, pascal
                .386
                option  proc:private

                include qb.inc

                extrn   LL$VECTOR:far
                extrn   LL$SET_VECTOR:far
                extrn   B$MUSICTICK:far

CLOCK_TICK      equ     1Ch

.code
previous        dd      0

;;::::::::::::::
;; B$TICKISR ()
B$TICKISR       proc    far

                pusha
                push    ds
                push    es
                mov     ax, DGROUP
                mov     ds, ax
                mov     es, ax
                cld
                call    B$MUSICTICK
                pop     es
                pop     ds
                popa
                jmp     cs:previous
B$TICKISR       endp

;;::::::::::::::
;; B$TICKON ()
B$TICKON        proc    public

                push    CLOCK_TICK
                call    LL$VECTOR
                add     sp, 2
                mov     W cs:previous, ax
                mov     W cs:previous+2, dx
                push    S B$TICKISR             ;; LL$SET_VECTOR (number, handler), C's order
                push    O B$TICKISR
                push    CLOCK_TICK
                call    LL$SET_VECTOR
                add     sp, 6
                ret
B$TICKON        endp
                end
