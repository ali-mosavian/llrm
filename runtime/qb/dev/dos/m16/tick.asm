;; name: B$TICKON, B$TICKOFF, B$TICKISR
;; desc: the clock tick (interrupt 1Ch, 18.2 times a second) calling the
;;       runtime's music
;;
;; chng: oct/26 written [ali]
;; obs.: the handler keeps every register, puts the program's DGROUP in ds and
;;       es, calls the C routine B$MUSICTICK (music.c), and chains.
;;       LL$SET_VECTOR remembers the old handler and puts it back when the
;;       program ends, so there is no way off but that. B$TICKON is called once.

                .model  medium, pascal
                .386
                option  proc:private

                include ../../../dos/m16/qb.inc

                extrn   LL$VECTOR:far
                extrn   LL$SET_VECTOR:far
                extrn   B$MUSICTICK:far

CLOCK_TICK      equ     1Ch

.code
previous        dd      0

;; B$TICKISR - the clock tick handler: play the music's tick, then chain.
;;
;; Calls B$MUSICTICK with the program's data segment in ds and es, then jumps to
;; the handler it replaced.
;;
;; Args:
;;     None.
;;
;; Returns:
;;     Does not return to its caller: it ends by chaining to the handler it
;;         replaced.
;;
;; Saves:    all registers (pusha, ds, es)
;; Clobbers: flags
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

;; B$TICKON - take interrupt 1Ch.
;;
;; Saves the vector it replaces in `previous`, then points the vector at
;; B$TICKISR. Called once.
;;
;; Args:
;;     None.
;;
;; Saves:    none
;; Clobbers: ax dx, flags
;; Patches:  previous, in the code segment.
B$TICKON        proc    public

                push    CLOCK_TICK
                call    LL$VECTOR
                add     sp, 2
                mov     W cs:previous, ax
                mov     W cs:previous+2, dx
                ;; LL$SET_VECTOR (number, handler), C's order
                push    S B$TICKISR
                push    O B$TICKISR
                push    CLOCK_TICK
                call    LL$SET_VECTOR
                add     sp, 6
                ret
B$TICKON        endp

;; B$TICKOFF - nothing: LL$SET_VECTOR puts the vector back when the program ends.
B$TICKOFF       proc    public

                ret
B$TICKOFF       endp
                end
