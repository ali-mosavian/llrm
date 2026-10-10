;; name: B$MOVE, B$FILL, B$COPY
;; desc: string moves by dwords
;;
;; args: ax:ptr BlockOp | dst, src: bytes forward; or by bytes if `value` is not 0 (an EGA latch copy holds one byte); B$FILL: dst, words of a 16-bit value
;; retn: none
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (regparm3: ax, dx, cx), so no thunk is needed; the QB
;;       runtime's device code (dev*.c, gfxdev.c) is C around these. Target 386+ with an FPU.

                .model  medium, pascal
                .386
                .387
                option  proc:private

                include qb.inc

Regs            struc
                rax             word    ?
                rbx             word    ?
                rcx             word    ?
                rdx             word    ?
                rbp             word    ?
                res             word    ?
                rflags          word    ?
Regs            ends

BlockOp         struc
                dst             dword   ?
                src             dword   ?
                count           word    ?               ;; bytes, or words for B$FILL
                value           word    ?               ;; for B$FILL
BlockOp         ends

.code
;;::::::::::::::
;; B$MOVE (ax: op)
B$MOVE          proc    public

                push    si
                push    di
                push    ds
                push    es
                mov     bx, ax
                les     di, [bx].BlockOp.dst
                mov     cx, [bx].BlockOp.count
                mov     dx, [bx].BlockOp.value
                lds     si, [bx].BlockOp.src
                cld
                test    dx, dx
                jnz     @F
                mov     ax, cx
                shr     cx, 2
                rep     movsd
                mov     cx, ax
                and     cx, 3
@@:
                rep     movsb
                pop     es
                pop     ds
                pop     di
                pop     si
                ret
B$MOVE          endp

;;::::::::::::::
;; B$FILL (ax: op)
B$FILL          proc    public

                push    di
                push    es
                mov     bx, ax
                les     di, [bx].BlockOp.dst
                movzx   eax, [bx].BlockOp.value
                mov     edx, eax
                shl     edx, 16
                or      eax, edx                ;; the word twice
                mov     cx, [bx].BlockOp.count
                cld
                shr     cx, 1
                rep     stosd
                jnc     @F
                stosw
@@:
                pop     es
                pop     di
                ret
B$FILL          endp
                ;;::::::::::::::
;; B$COPY (ax: to, dx: from, cx: count)
;; near bytes forward, dwords and then what is left; bx, cx, dx, si, di and es are kept
B$COPY          proc    public

                push    bx
                push    cx
                push    si
                push    di
                push    es
                push    ds
                pop     es
                mov     di, ax
                mov     si, dx
                cld
                mov     ax, di
                sub     ax, si
                cmp     ax, 4
                jb      @F                      ;; `to` 0 to 3 above `from`: byte by byte, as a loop would
                mov     bx, cx
                shr     cx, 2
                rep     movsd
                mov     cx, bx
                and     cx, 3
@@:
                rep     movsb
                pop     es
                pop     di
                pop     si
                pop     cx
                pop     bx
                ret
B$COPY          endp
                end
