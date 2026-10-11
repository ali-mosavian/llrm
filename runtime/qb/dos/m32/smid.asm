;; name: B$SMID
;; desc: the MID$ statement, MID$(target, start, max) = source
;;
;; chng: oct/26 written [ali]
;; obs.: QB rt/mid.asm: the same bounds.  It copies bytes because the source may
;;       be the target; so do we unless there are 8 or more and the target is
;;       not 0 to 3 bytes above the source.

                .386
                .model  flat

                public  B$SMID
                extrn   B$ERROR:near
                extrn   str_tmp_free_:near

.code
;; B$SMID - overwrite the target from `start` with up to `max` bytes of the
;; source, as many as the target has left and the source has; a temporary
;; source is freed.
;;
;; Args:
;;     target (eax): the data of a fixed-length target, else the string's
;;                   descriptor.
;;     width  (edx): bytes of a fixed-length target, or 0 for a string.
;;     source (ebx): ptr to the source's descriptor.
;;     max    (ecx): the most bytes to copy.
;;     start  (sp+4): 1-based; popped.
;;
;; Saves:    esi edi
;; Clobbers: eax ebx ecx edx, flags
;; Errors:   5 (illegal function call) for a `start` or `max` out of range.
B$SMID          proc

                push    esi
                push    edi
                mov     edi, eax
                mov     esi, ebx
                mov     ebx, [esp+12]           ;; start
                cmp     ebx, 1
                jl      bad
                test    ecx, ecx
                jl      bad
                test    edx, edx
                jnz     @F
                mov     edx, [edi]              ;; a string: its length and data
                mov     edi, [edi+4]
@@:
                dec     ebx
                cmp     ebx, edx
                jae     bad
                add     edi, ebx                ;; where the copy starts
                sub     edx, ebx                ;; room left in the target
                cmp     edx, ecx
                jbe     @F
                mov     edx, ecx
@@:
                cmp     edx, [esi]
                jbe     @F
                mov     edx, [esi]
@@:
                mov     ecx, edx
                mov     eax, esi                ;; the source, to free
                mov     esi, [esi+4]
                cld
                cmp     ecx, 8
                jb      @F                      ;; a few bytes: not worth the split
                mov     ebx, edi
                sub     ebx, esi
                cmp     ebx, 4
                jb      @F                      ;; the target 0 to 3 above: bytes, as a loop would
                mov     ebx, ecx
                shr     ecx, 2
                rep     movsd
                mov     ecx, ebx
                and     ecx, 3
@@:
                rep     movsb
                call    str_tmp_free_
                pop     edi
                pop     esi
                ret     4
bad:
                mov     eax, 5
                call    B$ERROR
B$SMID          endp
                end
