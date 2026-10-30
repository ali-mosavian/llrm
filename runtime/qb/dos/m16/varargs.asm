;; name: B$DDIM, B$RDIM, B$RDMP, B$CLOS, B$COLR, B$LOCT, B$CSCN
;; desc: entries whose stack block has the length of the call
;;
;; args: B$DDIM, B$RDIM, B$RDMP [in] (last pushed first)
;;                      ad:word           | the array descriptor
;;                      rankFeat:word     | rank, features << 8
;;                      elem:word         | element size
;;                      upper, lower:word | each dimension's bounds, the one
;;                                        | next to elem being the first stored
;;       B$CLOS         [in] count:word    | channels to close, 0: all
;;                      channel:word...    | the channels
;;       B$COLR, B$LOCT, B$CSCN
;;                      [in] count:word    | words that follow
;;                      (flag:word, value:word)...
;;                                         | per argument; no value where the
;;                                         | flag is 0
;; retn: none; the entry pops its whole block
;;
;; chng: oct/26 written [ali]
;; obs.: DIM and REDIM push every dimension's bounds, and CLOSE every
;;       channel, so the callee pops as many bytes as the call pushed (QB
;;       rt/dynamic.asm CleanStack, rt/dkutil.asm B$CLOS). A register
;;       convention cannot say that, so these keep BASIC's pushes; C does
;;       the work (array_dim in array.c, file_close in file.c) and takes
;;       the block's address in dx.  COLOR, LOCATE and SCREEN push a flag and
;;       a value for each argument given, and the count of those words (QB
;;       rt/gwscr.asm), so they too keep BASIC's pushes.

                .model  medium, pascal
                .386
                option  proc:private

                include qb.inc

;; fastcall: ax, dx (the C side, regparm3, takes them so); the name is
;; @array_dim@4
array_dim       proto   far fastcall :word, :word
file_close      proto   far fastcall :word, :word

screen_color    proto   far fastcall :word
screen_locate   proto   far fastcall :word
screen_mode     proto   far fastcall :word

DIM_ALLOCATE    equ     0                       ;; enum DimMode, array.h
DIM_REALLOCATE  equ     1
DIM_PRESERVE    equ     2

;; the DIM entries differ in the mode alone
DIMPROC         macro   entry:req, mode:req
entry           proc    public\
                        elem:word, rankFeat:word, ad:ptr word

                ;; the block, past bp and the return
                invoke  array_dim, mode, addr ad

                movzx   ebx, B rankFeat         ;; rank
                lea     bx, [4*ebx+6]           ;; bytes the caller pushed
                jmp     popblock
entry           endp
                endm

.code
;; B$DDIM, B$RDIM - DIM (allocate) and REDIM (reallocate) a dynamic array.
;;
;; The block is popped by the entry (popblock): the call pushed a word each
;; for the array descriptor, rank and features, and element size, then four
;; bytes for each dimension.
;;
;; Args:
;;     ad       (bp+6):  the array descriptor, the start of the block.
;;     rankFeat (bp+8):  rank, features << 8.
;;     elem     (bp+10): element size.
;;     bounds   (bp+12): upper, lower words for each dimension, the one next
;;                       to elem being the first stored.
;;
;; Saves:    none (bp is popped by popblock)
;; Clobbers: ax bx dx, flags
                DIMPROC B$DDIM, DIM_ALLOCATE
                DIMPROC B$RDIM, DIM_REALLOCATE
                DIMPROC B$RDMP, DIM_PRESERVE

;; B$CLOS - CLOSE the channels given, or all of them.
;;
;; The block is popped by the entry (popblock).
;;
;; Args:
;;     count    (bp+6): channels to close, 0: all.
;;     channels (bp+8): that many words, the channel numbers.
;;
;; Saves:    none (bp is popped by popblock)
;; Clobbers: ax bx dx, flags
B$CLOS          proc    public\
                        channel:word, count:word

                invoke  file_close, addr channel, count

                movzx   ebx, count
                lea     bx, [2*ebx+2]           ;; bytes the caller pushed
                jmp     popblock
B$CLOS          endp

;; the screen statements differ in the C routine alone
BLOCKPROC       macro   entry:req, handler:req
entry           proc    public\
                        count:word

                ;; the count and the words past it
                invoke  handler, addr count

                movzx   ebx, count
                lea     bx, [2*ebx+2]           ;; bytes the caller pushed
                jmp     popblock
entry           endp
                endm

;; B$COLR, B$LOCT, B$CSCN - COLOR, LOCATE and SCREEN with the arguments given.
;;
;; The block is popped by the entry (popblock).
;;
;; Args:
;;     count (bp+6): words that follow.
;;     words (bp+8): (flag, value) for each argument; no value where the
;;                   flag is 0.
;;
;; Saves:    none (bp is popped by popblock)
;; Clobbers: ax bx dx, flags
                BLOCKPROC B$COLR, screen_color
                BLOCKPROC B$LOCT, screen_locate
                BLOCKPROC B$CSCN, screen_mode

;; popblock - return from an entry, popping its block of arguments.
;;
;; Args:
;;     bytes (bx): the bytes the caller pushed.
;;     saved (sp): the entry's bp, then the far return address.
;;
;; Returns:
;;     To the caller, with the block popped.
;;
;; Saves:    none
;; Clobbers: ax dx, flags
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
