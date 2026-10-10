;; name: B$DDIM, B$RDIM, B$CLOS, B$COLR, B$LOCT, B$CSCN
;; desc: entries whose stack block has the length of the call, flat
;;
;; args: B$DDIM, B$RDIM [in] (last pushed first, a dword slot each)
;;                      ad:dword          | the array descriptor
;;                      rankFeat:dword    | rank, features << 8 (the low word)
;;                      elem:dword        | element size
;;                      upper, lower:dword| each dimension's bounds, the one next to elem being the first
;;                                        | stored
;;       B$CLOS         [in] count:dword   | channels to close, 0: all
;;                      channel:dword...   | the channels
;;       B$COLR, B$LOCT, B$CSCN
;;                      [in] count:dword   | slots that follow
;;                      (flag, value)...   | per argument; no value where the flag is 0
;; retn: none; the entry pops its whole block
;;
;; chng: oct/26 written [ali]
;; obs.: as the real-mode entries (the callee pops as many slots as the call pushed): a register
;;       convention cannot say that, so these keep BASIC's pushes; C does the work (array_dim in
;;       array.c, file_close in file.c, the screen routines in screen.c) and takes the block's
;;       address.  A value is the low 16 bits of its slot.  The entries change eax, ebx, ecx and edx.

                .386
                .model  flat

                public  B$DDIM
                public  B$RDIM
                public  B$CLOS
                public  B$COLR
                public  B$LOCT
                public  B$CSCN

                extrn   @array_dim@4:near
                extrn   @file_close@4:near
                extrn   @screen_color@2:near
                extrn   @screen_locate@2:near
                extrn   @screen_mode@2:near

SLOT            equ     4
DIM_ALLOCATE    equ     0                       ;; enum DimMode, array.h
DIM_REALLOCATE  equ     1

.code
;;::::::::::::::
;; the DIM entries differ in the mode alone: the block, past the return, and then the rank's bytes
DIMPROC         macro   entry:req, mode:req

entry           proc

                mov     eax, mode
                lea     edx, [esp+SLOT]         ;; the block: descriptor first
                call    @array_dim@4
                movzx   ecx, byte ptr [esp+2*SLOT]      ;; rank
                lea     ecx, [8*ecx+3*SLOT]     ;; bytes the caller pushed
                jmp     popblock
entry           endp
                endm

;;::::::::::::::
;; B$DDIM (...), B$RDIM (...)
                DIMPROC B$DDIM, DIM_ALLOCATE
                DIMPROC B$RDIM, DIM_REALLOCATE

;;::::::::::::::
;; B$CLOS (channel:dword..., count:dword)
B$CLOS          proc

                lea     eax, [esp+2*SLOT]       ;; the channels, just past the count
                movzx   edx, word ptr [esp+SLOT]        ;; the count
                call    @file_close@4
                movzx   ecx, word ptr [esp+SLOT]
                lea     ecx, [4*ecx+SLOT]       ;; bytes the caller pushed
                jmp     popblock
B$CLOS          endp

;; the screen statements differ in the C routine alone
BLOCKPROC       macro   entry:req, handler:req

entry           proc

                lea     eax, [esp+SLOT]         ;; the count and the slots past it
                call    handler
                movzx   ecx, word ptr [esp+SLOT]
                lea     ecx, [4*ecx+SLOT]       ;; bytes the caller pushed
                jmp     popblock
entry           endp
                endm

;;::::::::::::::
;; B$COLR (...), B$LOCT (...), B$CSCN (...)
                BLOCKPROC B$COLR, @screen_color@2
                BLOCKPROC B$LOCT, @screen_locate@2
                BLOCKPROC B$CSCN, @screen_mode@2

;;::::::::::::::
;; popblock (ecx: bytes) :  return, popping ecx bytes
popblock        proc

                pop     edx                     ;; return address
                add     esp, ecx
                jmp     edx
popblock        endp
                end
