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
;; obs.: the frame is the one QB's inc/stack.inc draws, below bp:
;;
;;           bp+0    previous bp
;;           bp-2    previous BASIC frame (b$curframe)
;;           bp-4    si
;;           bp-6    di
;;           bp-8    bytes of locals
;;           bp-10   GOSUB count
;;           ...     the locals, zeroed
;;
;;       The error handler finds the procedures through b$curframe and
;;       the bytes of locals. Strings local to the procedure are freed by
;;       the procedure, with B$STDL, before it calls B$EXSA.
;;
;;       Assembly because the entries take the caller's frame apart and
;;       leave bp and sp as the procedure then uses them.

                .model  medium, pascal
                .386
                option  proc:private

                include qb.inc

                extrn   c cur_level:word        ;; b$curlevel, nhstutil.c

                public  c b_curframe

FR_BFRAME       equ     -2
FR_SI           equ     -4
FR_DI           equ     -6
FR_LOCALS       equ     -8
FR_GOSUB        equ     -10
FR_SIZE         equ     10                      ;; the header, bp-10 .. bp-1

.data
b_curframe      word    0                       ;; frame of the procedure running

.code
;;::::::::::::::
;; B$ENRA ()
B$ENRA          proc    public

                pop     bx                      ;; our caller's way back,
                pop     dx                      ;; off the stack

                push    bp
                mov     bp, sp
                sub     sp, FR_SIZE

                mov     ax, b_curframe          ;; chain this frame to the last
                mov     [bp+FR_BFRAME], ax
                mov     [bp+FR_SI], si
                mov     [bp+FR_DI], di
                mov     [bp+FR_LOCALS], cx
                mov     W [bp+FR_GOSUB], 0
                mov     b_curframe, bp
                inc     cur_level

                sub     sp, cx                  ;; the locals, zeroed by dwords
                mov     di, sp
                push    es
                push    ds
                pop     es
                xor     eax, eax
                shr     cx, 1
                shr     cx, 1                   ;; cf: a word is left over
                rep     stosd
                adc     cx, cx
                rep     stosw
                pop     es

                push    dx
                push    bx
                ret
B$ENRA          endp

;;::::::::::::::
;; B$EXSA ()
B$EXSA          proc    public

                pop     bx                      ;; our caller's way back
                pop     cx

                dec     cur_level
                mov     di, [bp+FR_DI]
                mov     si, [bp+FR_SI]
                push    W [bp+FR_BFRAME]
                pop     b_curframe

                leave
                push    cx
                push    bx
                ret
B$EXSA          endp
                end
