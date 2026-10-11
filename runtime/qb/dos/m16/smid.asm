;; name: B$SMID
;; desc: the MID$ statement, MID$(target, start, max) = source
;;
;; chng: oct/26 written [ali]
;; obs.: QB rt/mid.asm: the same bounds.  It copies bytes because the source may
;;       be the target; so do we unless there are 8 or more and the target is
;;       not 0 to 3 bytes above the source.  Our callers pop the stack.

                .model  medium, pascal
                .386
                option  proc:private

                include qb.inc

extrn           B$ERROR:far
extrn           B$STDALCTMP:far

.code
;; B$SMID - overwrite the target from `start` with up to `max` bytes of the
;; source, as many as the target has left and the source has; a temporary
;; source is freed.
;;
;; Args:
;;     width  (ax): bytes of a fixed-length target, or 0 for a string
;;     source (dx): near ptr to the source's descriptor.
;;     max    (cx): the most bytes to copy.
;;     target (sp+4): far ptr; the data of a fixed-length target, else the
;;                    string's descriptor (offset only).
;;     start  (sp+8): 1-based.
;;
;; Saves:    bp si di es
;; Clobbers: ax bx cx dx, flags
;; Errors:   5 (illegal function call) for a `start` or `max` out of range.
B$SMID          proc    public

                push    bp
                mov     bp, sp
                push    si
                push    di
                push    es
                mov     si, dx
                mov     dx, [bp+10]             ;; start
                cmp     dx, 1
                jl      bad
                test    cx, cx
                jl      bad
                mov     di, ax                  ;; width
                mov     ax, [bp+6]              ;; target
                test    di, di
                jnz     @F
                mov     bx, ax                  ;; a string: its length and data
                mov     di, [bx]
                mov     ax, [bx+2]
@@:
                dec     dx
                cmp     dx, di
                jae     bad
                add     ax, dx                  ;; where the copy starts
                sub     di, dx                  ;; room left in the target
                cmp     di, cx
                jbe     @F
                mov     di, cx
@@:
                cmp     di, [si]
                jbe     @F
                mov     di, [si]
@@:
                mov     cx, di
                mov     di, ax
                push    ds
                pop     es
                push    si                      ;; the source, to free
                mov     si, [si+2]
                cld
                cmp     cx, 8
                jb      @F                      ;; a few bytes: not worth the split
                mov     ax, di
                sub     ax, si
                cmp     ax, 4
                jb      @F                      ;; the target 0 to 3 above: bytes, as a loop would
                mov     bx, cx
                shr     cx, 2
                rep     movsd
                mov     cx, bx
                and     cx, 3
@@:
                rep     movsb
                pop     ax
                call    B$STDALCTMP
                pop     es
                pop     di
                pop     si
                pop     bp
                ret
bad:
                mov     ax, 5
                call    B$ERROR
B$SMID          endp
                end
