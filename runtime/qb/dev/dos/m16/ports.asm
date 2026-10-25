;; name: B$OUTB, B$OUTW, B$INB, B$CLI, B$STI
;; desc: I/O ports and the interrupt flag
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (regparm3: ax, dx, cx), so no thunk is needed;
;;       the QB runtime's device code (dev*.c, gfxdev.c) is C around these.
;;       Target 386+ with an FPU.

                .model  medium, pascal
                .386
                .387
                option  proc:private

                include ../../../dos/m16/qb.inc

.code
;; B$OUTB - write a byte to a port.
;;
;; Args:
;;     port  (ax): the port number.
;;     value (dx): the byte, in its low byte.
;;
;; Saves:    none
;; Clobbers: ax dx
B$OUTB          proc    public

                xchg    ax, dx                  ;; dx the port, al the byte
                out     dx, al
                ret
B$OUTB          endp

;; B$OUTW - write a word to a port.
;;
;; For the VGA's index/data pairs the index is the low byte and the datum the
;; high one.
;;
;; Args:
;;     port  (ax): the port number.
;;     value (dx): the word, in its low word.
;;
;; Saves:    none
;; Clobbers: ax dx
B$OUTW          proc    public

                xchg    ax, dx                  ;; dx the port, ax the word
                out     dx, ax
                ret
B$OUTW          endp

;; B$INB - read a byte from a port.
;;
;; Args:
;;     port (ax): the port number.
;;
;; Returns:
;;     ax: the byte, the rest zero.
;;
;; Saves:    none
;; Clobbers: ax dx, flags
B$INB           proc    public

                mov     dx, ax
                in      al, dx
                xor     ah, ah
                ret
B$INB           endp

;; B$CLI - clear the interrupt flag.
;;
;; Args:
;;     None.
;;
;; Saves:    none
;; Clobbers: flags
B$CLI           proc    public

                cli
                ret
B$CLI           endp

;; B$STI - set the interrupt flag.
;;
;; Args:
;;     None.
;;
;; Saves:    none
;; Clobbers: flags
B$STI           proc    public

                sti
                ret
B$STI           endp
                end
