;; name: B$ENRA, B$EXSA
;; desc: BASIC procedure frames
;;
;; args: B$ENRA [in] cx:word           | bytes of locals, even
;;                   bx:word           | temporary string slots (unused: the
;;                                     | procedure frees its strings itself,
;;                                     | with B$STDL)
;;       B$EXSA [in] bp                | the frame B$ENRA built
;;                   dx:ax             | the function's result, kept
;; retn: B$ENRA bp -> the new frame, sp below its locals
;;       B$EXSA si, di, bp and b$curframe as the caller had them
;;
;; chng: oct/26 written [ali]
;; obs.: the frame is QB's (inc/stack.inc):
;;
;;           bp+0    previous bp
;;           bp-2    previous BASIC frame (b$curframe)
;;           bp-4    si
;;           bp-6    di
;;           bp-8    bytes of locals
;;           bp-10   GOSUB count
;;           ...     the locals, zeroed
;;
;;       The error handler walks the frames through b$curframe. These
;;       are the instructions of BCOM45's rtenexit, B$ENRA and B$EXSA,
;;       except that the locals are zeroed by dwords. They stay in
;;       assembly because they take the caller's frame apart and leave
;;       bp and sp as the procedure uses them, which C cannot do.

                .model  medium, pascal
                .386
                option  proc:private

                include qb.inc

                extrn   c cur_level:word        ;; b$curlevel, nhstutil.c

                public  c b_curframe

.data
b_curframe      word    0                       ;; frame of the procedure running
retaddr         dword   0                       ;; where the entries return to

.code
;;::::::::::::::
;; B$ENRA ()
B$ENRA          proc    public

                pop     W retaddr               ;; our return, off the stack
                pop     W retaddr+2

                xor     eax, eax
                push    bp
                mov     bp, sp
                push    b_curframe              ;; previous BASIC frame
                push    si
                push    di
                push    cx                      ;; bytes of locals
                push    ax                      ;; GOSUB count
                sub     sp, cx
                mov     b_curframe, bp
                inc     cur_level

                push    es                      ;; zero the locals
                push    ds
                pop     es
                mov     di, sp
                shr     cx, 1
                shr     cx, 1                   ;; dwords; cf: a word is left
                rep     stosd
                adc     cx, cx
                rep     stosw
                pop     es

                jmp     D retaddr
B$ENRA          endp

;;::::::::::::::
;; B$EXSA ()
B$EXSA          proc    public

                pop     W retaddr               ;; our return, off the stack
                pop     W retaddr+2

                dec     cur_level
                lea     sp, [bp-6]
                pop     di
                pop     si
                pop     b_curframe
                pop     bp

                jmp     D retaddr
B$EXSA          endp
                end
