;; name: B$DDIM, B$RDIM, B$CLOS, B$COLR, B$LOCT, B$CSCN
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

;; fastcall: ax, dx (the C side, regparm3, takes them so); the name is @array_dim@4
array_dim       proto   far fastcall :word, :word
file_close      proto   far fastcall :word, :word

screen_color    proto   far fastcall :word
screen_locate   proto   far fastcall :word
screen_mode     proto   far fastcall :word

DIM_ALLOCATE    equ     0                       ;; enum DimMode, array.h
DIM_REALLOCATE  equ     1

;; the DIM entries differ in the mode alone
DIMPROC         macro   entry:req, mode:req
entry           proc    public\
                        elem:word, rankFeat:word, ad:ptr word

                invoke  array_dim, mode, addr ad ;; the block, past bp and the return

                movzx   ebx, B rankFeat         ;; rank
                lea     bx, [4*ebx+6]           ;; bytes the caller pushed
                jmp     popblock
entry           endp
                endm

.code
;;::::::::::::::
;; B$DDIM (...), B$RDIM (...)
                DIMPROC B$DDIM, DIM_ALLOCATE
                DIMPROC B$RDIM, DIM_REALLOCATE

;;::::::::::::::
;; B$CLOS (channel:word..., count:word)
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

                invoke  handler, addr count     ;; the count and the words past it

                movzx   ebx, count
                lea     bx, [2*ebx+2]           ;; bytes the caller pushed
                jmp     popblock
entry           endp
                endm

;;::::::::::::::
;; B$COLR (...), B$LOCT (...), B$CSCN (...)
                BLOCKPROC B$COLR, screen_color
                BLOCKPROC B$LOCT, screen_locate
                BLOCKPROC B$CSCN, screen_mode

;;::::::::::::::
;; popblock (bx: bytes, bp pushed by the entry) :  return, popping bx bytes
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
