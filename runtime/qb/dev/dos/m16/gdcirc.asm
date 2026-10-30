;; name: gdcirc.asm
;; desc: the eight points of a circle step, planar and CGA
;;
;; chng: oct/26 split from fill.asm [ali]
;; obs.: the patched bytes are in the code, written through cs; the registers
;; are C's; gdfill.inc has the structures

                .model  medium, pascal
                .386
                option  proc:private

                include gdfill.inc

.code
FillOct         struc
                base            dword   ?
                row             word   4 dup (?)
                col             word   4 dup (?)
                pm              word   4 dup (?)
                color           word   ?
FillOct         ends

PTP             macro   r, c
                mov     di, word ptr [bx].FillOct.base
                add     di, [bx].FillOct.row[2*r]
                add     di, [bx].FillOct.col[2*c]
                mov     ah, byte ptr [bx].FillOct.pm[2*c]
                mov     al, 8
                out     dx, ax
                mov     al, es:[di]
                mov     es:[di], cl
                endm

PTC             macro   r, c
                mov     di, word ptr [bx].FillOct.base
                add     di, [bx].FillOct.row[2*r]
                add     di, [bx].FillOct.col[2*c]
                mov     al, es:[di]
                mov     ah, al
                xor     ah, cl
                and     ah, byte ptr [bx].FillOct.pm[2*c]
                xor     al, ah
                mov     es:[di], al
                endm

;; B$FOCP - plot the eight points of a circle step, planar.
;;
;; The bit mask by `out`, the latches loaded by the read, the colour
;; written.
;;
;; Args:
;;     oct (ax): ptr to a FillOct.
;;
;; Saves:    bx cx dx di es
;; Clobbers: ax, flags
B$FOCP          proc    public uses bx cx dx di es
                mov     bx, ax
                les     di, [bx].FillOct.base
                mov     cl, byte ptr [bx].FillOct.color
                mov     dx, 3CEh
                PTP     0, 0
                PTP     2, 2
                PTP     2, 3
                PTP     0, 1
                PTP     1, 1
                PTP     3, 3
                PTP     3, 2
                PTP     1, 0
                ret
B$FOCP          endp

;; B$FOCC - plot the eight points of a circle step, CGA.
;;
;; Each byte is blended under the mask of its pixel.
;;
;; Args:
;;     oct (ax): ptr to a FillOct.
;;
;; Saves:    bx cx dx di es
;; Clobbers: ax, flags
B$FOCC          proc    public uses bx cx dx di es
                mov     bx, ax
                les     di, [bx].FillOct.base
                mov     cl, byte ptr [bx].FillOct.color
                PTC     0, 0
                PTC     2, 2
                PTC     2, 3
                PTC     0, 1
                PTC     1, 1
                PTC     3, 3
                PTC     3, 2
                PTC     1, 0
                ret
B$FOCC          endp
                end
