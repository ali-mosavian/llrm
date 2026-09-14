; SUB TscSnap (hi AS LONG, lo AS LONG): the time-stamp counter, for timing.
; BASIC passes both by near reference and pops its own arguments (Pascal).
.586
TSCSNAP_TEXT segment word public use16 'CODE'
        public  TSCSNAP
TSCSNAP proc    far
        push    bp
        mov     bp,sp
        push    bx
        rdtsc
        mov     bx,[bp+8]
        mov     [bx],edx
        mov     bx,[bp+6]
        mov     [bx],eax
        pop     bx
        pop     bp
        ret     4
TSCSNAP endp
TSCSNAP_TEXT ends
        end
