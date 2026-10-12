;; name: B$MOVE, B$FILL, B$COPY, B$FILLB, B$CMPB
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
;; `from`, as a loop would, or when there are fewer than 4.
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
                ;; under a dword: bytes alone, no split
                cmp     cx, 4
                jb      @F
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

;; B$FILLB - set `count` bytes at `to` to the byte `value`.
;;
;; By dwords and then the bytes left.
;;
;; Args:
;;     to    (ax): near ptr.
;;     value (dx): the byte, in dl.
;;     count (cx): bytes.
;;
;; Saves:    bx cx dx di es
;; Clobbers: eax, flags
B$FILLB         proc    public uses bx cx dx di es

                push    ds
                pop     es
                mov     di, ax
                movzx   eax, dl
                mov     ah, al
                mov     edx, eax
                shl     eax, 16
                or      eax, edx                ;; the byte four times; imul costs 9 to 40 clocks
                mov     bx, cx
                cld
                shr     cx, 2
                rep     stosd
                mov     cx, bx
                and     cx, 3
                rep     stosb
                ret
B$FILLB         endp

;; B$CMPB - compare `count` bytes at `first` and `second`, as signed bytes: BCOM45
;; compares so (CHR$(200) < "a"), the flags of its REPE CMPSB read with JG and JL.
;;
;; By dwords until one differs, then the four bytes it is in.
;;
;; Args:
;;     first  (ax): near ptr.
;;     second (dx): near ptr.
;;     count  (cx): bytes.
;;
;; Returns:  ax: -1, 0 or 1 as `first` sorts before, with or after `second`.
;; Saves:    bx cx si di es
;; Clobbers: flags
B$CMPB          proc    public uses bx cx si di es

                push    ds
                pop     es
                mov     si, ax
                mov     di, dx
                mov     bx, cx
                cld
                shr     cx, 2                   ;; ZF set when no dwords: nothing differs yet
                repe    cmpsd
                je      @F
                sub     si, 4
                sub     di, 4
                mov     cx, 4
                jmp     lastbytes
@@:
                mov     cx, bx
                and     cx, 3
lastbytes:
                repe    cmpsb
                mov     ax, 0
                je      @F
                mov     ax, 1
                jg      @F
                mov     ax, -1
@@:
                ret
B$CMPB          endp
                end
