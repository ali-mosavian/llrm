;; name: gdcirc.asm
;; desc: the eight points of a circle step, planar and CGA
;;
;; chng: oct/26 split from fill.asm [ali]
;; obs.: the patched bytes are in the code, written through the flat data
;; selector; the registers are C's; gdfill.inc has the structures

                .386
                .model  flat

                public  B$FOCP
                public  B$FOCC


                include gdfill.inc

.code
FillOct         struc
                base            dword   ?
                row             dword   4 dup (?)
                col             dword   4 dup (?)
                pm              dword   4 dup (?)
                color           dword   ?
FillOct         ends

PTP             macro   r, c
                mov     edi, [ebx].FillOct.base
                add     edi, [ebx].FillOct.row[4*r]
                add     edi, [ebx].FillOct.col[4*c]
                mov     ah, byte ptr [ebx].FillOct.pm[4*c]
                mov     al, 8
                out     dx, ax
                mov     al, es:[edi]
                mov     es:[edi], cl
                endm

PTC             macro   r, c
                mov     edi, [ebx].FillOct.base
                add     edi, [ebx].FillOct.row[4*r]
                add     edi, [ebx].FillOct.col[4*c]
                mov     al, es:[edi]
                mov     ah, al
                xor     ah, cl
                and     ah, byte ptr [ebx].FillOct.pm[4*c]
                xor     al, ah
                mov     es:[edi], al
                endm

;; B$FOCP - plot the eight points of a circle step, planar.
;;
;; The bit mask by `out`, the latches loaded by the read, the colour
;; written.
;;
;; Args:
;;     oct (eax): ptr to a FillOct.
;;
;; Saves:    ebx ecx edx edi
;; Clobbers: eax, flags
B$FOCP          proc    uses ebx ecx edx edi
                mov     ebx, eax
                mov     cl, byte ptr [ebx].FillOct.color
                mov     edx, 3CEh
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
;;     oct (eax): ptr to a FillOct.
;;
;; Saves:    ebx ecx edx edi
;; Clobbers: eax, flags
B$FOCC          proc    uses ebx ecx edx edi
                mov     ebx, eax
                mov     cl, byte ptr [ebx].FillOct.color
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
