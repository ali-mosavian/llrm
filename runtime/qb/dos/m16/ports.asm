;; name: B$OUTB, B$OUTW, B$INB
;; desc: I/O ports
;;
;; args: ax:word port, dx:word value | a byte or a word to the port: for a word the index is the low byte and the datum the high one, as the VGA's index/data pairs take it
;; retn: B$INB ax -> the byte
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (regparm3: ax, dx, cx), so no thunk is needed; the QB
;;       runtime's device code (dev*.c, gfxdev.c) is C around these. Target 386+ with an FPU.

                .model  medium, pascal
                .386
                .387
                option  proc:private

                include qb.inc

.code
;;::::::::::::::
;; B$OUTB (ax: port, dx: value)
B$OUTB          proc    public

                xchg    ax, dx                  ;; dx the port, al the byte
                out     dx, al
                ret
B$OUTB          endp

;;::::::::::::::
;; B$OUTW (ax: port, dx: value)
B$OUTW          proc    public

                xchg    ax, dx                  ;; dx the port, ax the word
                out     dx, ax
                ret
B$OUTW          endp

;;::::::::::::::
;; B$INB (ax: port)
B$INB           proc    public

                mov     dx, ax
                in      al, dx
                xor     ah, ah
                ret
B$INB           endp
                end
