;; name: B$OUTB, B$OUTW, B$INB, B$CLI, B$STI
;; desc: I/O ports and the interrupt flag
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (the target's own convention: eax, edx, ebx,
;;       ecx), so no thunk is needed.

                .386
                .model  flat

                public  B$OUTB
                public  B$OUTW
                public  B$INB
                public  B$CLI
                public  B$STI

.code
;; B$OUTB - write a byte to a port.
;;
;; Args:
;;     port  (eax): the port number.
;;     value (edx): the byte, in its low byte.
;;
;; Saves:    none
;; Clobbers: none
B$OUTB          proc

                xchg    eax, edx                ;; dx the port, al the byte
                out     dx, al
                xchg    eax, edx                ;; edx as it came
                ret
B$OUTB          endp

;; B$OUTW - write a word to a port.
;;
;; For the VGA's index/data pairs the index is the low byte and the datum the
;; high one.
;;
;; Args:
;;     port  (eax): the port number.
;;     value (edx): the word, in its low word.
;;
;; Saves:    none
;; Clobbers: none
B$OUTW          proc

                xchg    eax, edx                ;; dx the port, ax the word
                out     dx, ax
                xchg    eax, edx
                ret
B$OUTW          endp

;; B$INB - read a byte from a port.
;;
;; Args:
;;     port (eax): the port number.
;;
;; Returns:
;;     eax: the byte, the rest zero.
;;
;; Saves:    edx
;; Clobbers: eax, flags
B$INB           proc uses edx

                mov     edx, eax
                xor     eax, eax
                in      al, dx
                ret
B$INB           endp

;; B$CLI - clear the interrupt flag.
;;
;; Args:
;;     None.
;;
;; Saves:    none
;; Clobbers: flags
B$CLI           proc

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
B$STI           proc

                sti
                ret
B$STI           endp
                end
