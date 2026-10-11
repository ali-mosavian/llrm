;; name: gdfill.asm
;; desc: the box fill of the graphics screen, `op mem, imm` patched once per
;; primitive
;;
;; chng: oct/26 split from fill.asm [ali]
;; obs.: the patched bytes are in the code, written through the flat data
;; selector; the registers are C's; gdfill.inc has the structures

                .386
                .model  flat

                public  B$FSEL
                public  B$FBOX


                include gdfill.inc

.code
;; B$FSEL - set the box fill's operation and pattern, once per primitive.
;;
;; Args:
;;     operation (eax): 0 set, 1 and, 2 or, 3 xor.
;;     pattern   (dl): the byte to fill with.
;;
;; Saves:    ebx
;; Clobbers: eax, flags
;; Patches:  the opcode, mod/rm and immediate of the box fill's loops.
B$FSEL          proc    uses ebx
                mov     ebx, eax
                and     ebx, 3
                mov     ax, word ptr dword_ops[ebx * 2]
                mov     word ptr [dword_op], ax
                mov     ax, word ptr byte_ops[ebx * 2]
                mov     word ptr [byte_op], ax
                movzx   eax, dl
                imul    eax, 01010101h
                mov     dword ptr [dword_imm], eax
                mov     dword ptr [set_pattern], eax
                mov     byte ptr [set_flag], 0
                test    ebx, ebx
                jnz     @F
                mov     byte ptr [set_flag], 1
@@:
                mov     byte ptr [byte_imm], dl
                ;; the patched bytes are not in the prefetch queue
                jmp     short @F
@@:
                ret
B$FSEL          endp

;; B$FBOX - fill rows of a box with the operation B$FSEL set.
;;
;; A row is an edge byte (if `edges`), `middle` bytes, then an edge byte.
;; A row starts `steps[0]` after the one before it when that is even,
;; `steps[1]` when it is odd (`phase` says which the first is).
;;
;; Args:
;;     box (eax): ptr to a FillBox.
;;
;; Saves:    ebx ecx edx esi edi ebp
;; Clobbers: eax, flags
B$FBOX          proc    uses ebx ecx edx esi edi ebp
                cld
                mov     ebx, eax
                mov     esi, [ebx].FillBox.dst
                mov     ebp, [ebx].FillBox.rows
@@row:
                mov     edi, esi
                cmp     [ebx].FillBox.edges, 0
                je      @F
                mov     ecx, [ebx].FillBox.left
                mov     dl, [edi]
                and     dl, cl
                xor     dl, ch
                mov     [edi], dl
                inc     edi
@@:
                mov     ecx, [ebx].FillBox.middle
                cmp     [set_flag], 0
                jne     @@set
                mov     edx, ecx
                shr     ecx, 2
                jz      @@tail
@@dwords:
;; mov dword ptr [edi], imm32; the opcode and mod/rm patched
dword_op        db      0C7h, 07h
dword_imm       dd      0
                add     edi, 4
                dec     ecx
                jnz     @@dwords
@@tail:
                and     edx, 3
                jz      @@right
@@bytes:
byte_op         db      0C6h, 07h               ;; mov byte ptr [edi], imm8
byte_imm        db      0
                inc     edi
                dec     edx
                jnz     @@bytes
                jmp     short @@right
;; a set is UGL's hlinef: dwords, then the bytes left
@@set:
                mov     eax, [set_pattern]
                mov     edx, ecx
                shr     ecx, 2
                rep     stosd
                mov     ecx, edx
                and     ecx, 3
                rep     stosb
@@right:
                cmp     [ebx].FillBox.edges, 0
                je      @F
                mov     ecx, [ebx].FillBox.right
                mov     dl, [edi]
                and     dl, cl
                xor     dl, ch
                mov     [edi], dl
@@:
                mov     eax, [ebx].FillBox.phase
                add     esi, [ebx + eax].FillBox.steps
                xor     eax, 4
                mov     [ebx].FillBox.phase, eax
                dec     ebp
                jnz     @@row
                ret
B$FBOX          endp

;; the opcode and the mod/rm of each operation, for a dword and for a byte

set_pattern     dd      0
set_flag        db      0
dword_ops       dw      07C7h, 2781h, 0F81h, 3781h
byte_ops        dw      07C6h, 2780h, 0F80h, 3780h
                end
