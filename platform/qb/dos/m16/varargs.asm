;; name: B$DDIM, B$RDIM, B$CLOS
;; desc: entries whose stack block has the length of the call
;;
;; args: B$DDIM, B$RDIM [in] (last pushed first)
;;                      ad:word           | the array descriptor
;;                      rankFeat:word     | rank, features << 8
;;                      elem:word         | element size
;;                      upper, lower:word | each dimension's bounds, the one
;;                                        | next to elem being the first stored
;;       B$CLOS         [in] count:word    | channels to close, 0: all
;;                      channel:word...    | the channels
;; retn: none; the entry pops its whole block
;;
;; chng: oct/26 written [ali]
;; obs.: DIM and REDIM push every dimension's bounds, and CLOSE every
;;       channel, so the callee pops as many bytes as the call pushed (QB
;;       rt/dynamic.asm CleanStack, rt/dkutil.asm B$CLOS). A register
;;       convention cannot say that, so these keep BASIC's pushes; C does
;;       the work (array_dim in array.c, file_close in file.c) and takes
;;       the block's address in dx.

                .model  medium, pascal
                .386
                option  proc:private

                include qb.inc

                extrn   c array_dim:far
                extrn   c file_close:far

DIM_ALLOCATE    equ     0                       ;; enum DimMode, array.h
DIM_REALLOCATE  equ     1

.code
;;::::::::::::::
;; B$DDIM (...)
B$DDIM          proc    public

                mov     ax, DIM_ALLOCATE
                jmp     short dimension
B$DDIM          endp

;;::::::::::::::
;; B$RDIM (...)
B$RDIM          proc    public

                mov     ax, DIM_REALLOCATE
B$RDIM          endp

;;::::::::::::::
;; dimension (ax: mode) :  array_dim on the caller's block, then pops it
dimension       proc

                push    bp
                mov     bp, sp

                lea     dx, [bp+6]              ;; the block, past bp and the return
                call    array_dim

                movzx   ebx, B [bp+8]           ;; rank
                lea     bx, [4*ebx+6]           ;; bytes the caller pushed
                jmp     short popblock
dimension       endp

;;::::::::::::::
;; B$CLOS (count:word, channel:word...)
B$CLOS          proc    public

                push    bp
                mov     bp, sp

                lea     ax, [bp+8]              ;; the channels
                mov     dx, [bp+6]              ;; how many
                call    file_close

                movzx   ebx, W [bp+6]
                lea     bx, [2*ebx+2]           ;; bytes the caller pushed
B$CLOS          endp

;;::::::::::::::
;; popblock (bx: bytes, bp pushed) :  return to the caller, popping bx bytes
popblock        proc

                pop     bp
                pop     ax                      ;; return offset
                pop     dx                      ;; return segment
                add     sp, bx
                push    dx
                push    ax
                ret
popblock        endp
                end
