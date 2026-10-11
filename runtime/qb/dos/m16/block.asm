;; name: B$MOVE, B$FILL, B$COPY
;; desc: string moves by dwords
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (regparm3: ax, dx, cx), so no thunk is needed;
;;       the QB runtime's device code (dev*.c, gfxdev.c) is C around these.
;;       Target 386+ with an FPU.

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
                ;; bytes, or words for B$FILL
                count           word    ?
                value           word    ?               ;; for B$FILL
BlockOp         ends

.code
;; B$MOVE - copy `count` bytes from `src` to `dst`, forward.
;;
;; By dwords and then the bytes left; by bytes alone if `value` is not 0 (an EGA
;; latch copy holds one byte).
;;
;; Args:
;;     op (ax): near ptr to a BlockOp.
;;
;; Saves:    si di ds es
;; Clobbers: ax bx cx dx, flags
B$MOVE          proc    public uses si di ds es

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
                ret
B$MOVE          endp

;; B$FILL - fill `count` words at `dst` with the 16-bit `value`.
;;
;; Two words at a time, and the last one if `count` is odd.
;;
;; Args:
;;     op (ax): near ptr to a BlockOp.
;;
;; Saves:    di es
;; Clobbers: ax bx cx dx, flags
B$FILL          proc    public uses di es

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
                ret
B$FILL          endp
                
;; B$COPY - copy `count` bytes from `from` to `to`, forward.
;;
;; By dwords and then the bytes left; byte by byte when `to` is 0 to 3 above
;; `from`, as a loop would.
;;
;; Args:
;;     to    (ax): near ptr.
;;     from  (dx): near ptr.
;;     count (cx): bytes.
;;
;; Saves:    bx cx si di es
;; Clobbers: ax, flags
B$COPY          proc    public uses bx cx si di es

                push    ds
                pop     es
                mov     di, ax
                mov     si, dx
                cld
                mov     ax, di
                sub     ax, si
                cmp     ax, 4
                ;; `to` 0 to 3 above `from`: byte by byte, as a loop would
                jb      @F
                mov     bx, cx
                shr     cx, 2
                rep     movsd
                mov     cx, bx
                and     cx, 3
@@:
                rep     movsb
                ret
B$COPY          endp
                end
