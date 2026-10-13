;; name: LL$OPEN, LL$CREATE, LL$READ, LL$SEEK, LL$WRITE_FILE, LL$CLOSE, LL$EXIT, LL$MORE, LL$CONSOLE_READ_KEY, LL$CONSOLE_KEY_READY
;; desc: the OS layer shared by C, Nib and BASIC programs: x86-m32, DOS/32A
;;
;; args: the interface of runtime/shared/interface.toml, in cdecl32: arguments in dword slots from
;;       [esp+4]; ebx, esi and edi kept
;; retn: eax -> the result, or the DOS error code negated
;;
;; chng: oct/26 restyled, symbols renamed to LL$ [ali]
;; obs.: DOS/32A takes a flat DS:EDX buffer for the file calls, so none needs a transfer buffer.
;;       One object per group, as the m16 layer (OS_GROUPS).

;; The operating-system routines std.os declares (os.nib), in cdecl32: arguments in dword slots from
;; [esp+4], EBX, ESI and EDI kept, a result in EAX. DOS/32A takes a flat DS:EDX buffer for the file
;; calls, so none needs a transfer buffer.
.386
.model flat

;; Which groups of the interface this object holds. A library has one object per group, each
;; assembled with -DOS_GROUPS=<its bit>, so a program links only what it calls; assembled with no
;; definition the file holds every group.
G_CORE          equ     1
G_CONSOLE       equ     2
ifndef OS_GROUPS
OS_GROUPS       equ     3
endif

;; What DOS's device information (IOCTL 44h) says of a handle: bit 7 is a character device.
DEVICE_BIT      equ     80h
CTRL_Z          equ     1Ah

if OS_GROUPS and G_CORE
public LL$OPEN
public LL$CREATE
public LL$READ
public LL$SEEK
public LL$WRITE_FILE
public LL$CLOSE
public LL$EXIT
public LL$MORE
public LL$STACK_LOW
endif
if OS_GROUPS and G_CONSOLE
public LL$CONSOLE_READ_KEY
public LL$CONSOLE_KEY_READY
endif

;; HEAP_BYTES comes from os.toml, DOS_* from the OS's facts: the assembler is told both.

if OS_GROUPS and G_CORE
.data
;; The lowest ESP a checked function may reach (-fsanitize=stack). Start-up sets it.
LL$STACK_LOW    dd      0
;; The heap's next free byte and the end of its arena: set on the first LL$MORE, which asks the
;; extender for the arena.
heap_next       dd      0
heap_end        dd      0
;; The request being served and the arena size being tried, which DPMI 0501h's register use leaves in memory.
heap_ask        dd      0
heap_try        dd      0
endif
.code
if OS_GROUPS and G_CORE
;;::::::::::::::
;; LL$MORE(bytes: usize) -> *near mut u8: `bytes` more of the heap at its end, or 0 when it runs out.
;; The heap grows inside an arena taken from the extender (DPMI 0501h, allocate memory block): the first
;; call takes HEAP_BYTES, or the largest half of it the extender has, down to the request. When the arena
;; cannot hold a request the next is another arena (at least the request), and the grant is at an address
;; that need not follow the last one's: the heap chains arenas.
LL$MORE         proc
                mov     ecx, dword ptr [esp+4]
                mov     eax, heap_next
                test    eax, eax
                jz      take
                lea     edx, [eax + ecx]
                cmp     edx, heap_end
                ja      take
                mov     heap_next, edx
                ret
take:
                push    ebx
                push    esi
                push    edi
;; DPMI 0501h returns the block's handle in SI:DI, so the request and the size being tried stay in memory.
                mov     heap_ask, ecx
                mov     edx, HEAP_BYTES
                cmp     edx, ecx
                jae     sized
                mov     edx, ecx
sized:
                mov     heap_try, edx
arena:
                mov     ebx, heap_try
                shr     ebx, 16
                mov     ecx, heap_try
                and     ecx, 0FFFFh
                mov     eax, DOS_DPMI_ALLOC
                int     DOS_DPMI_INT
                jnc     arena_got
                mov     edx, heap_try
                shr     edx, 1
                cmp     edx, heap_ask
                jb      arena_refused
                mov     heap_try, edx
                jmp     arena
arena_refused:
                pop     edi
                pop     esi
                pop     ebx
                xor     eax, eax
                ret
arena_got:
                shl     ebx, 16
                mov     bx, cx
                mov     eax, ebx
                add     ebx, heap_try
                mov     heap_end, ebx
                mov     edx, eax
                add     edx, heap_ask
                mov     heap_next, edx
                pop     edi
                pop     esi
                pop     ebx
                ret
LL$MORE         endp

;; The DOS file calls. Each returns what DOS does, or when DOS sets carry, its error code negated.

;;::::::::::::::
;; LL$OPEN(name: *far char, mode: u8) -> i16: a handle to an existing file.
LL$OPEN         proc
                mov     edx, dword ptr [esp+4]
                mov     al, byte ptr [esp+8]
                mov     ah, DOS_OPEN
                int     DOS_INT
                jmp     short checked
LL$OPEN         endp

;;::::::::::::::
;; LL$CREATE(name: *far char) -> i16: a handle to a new, empty file.
LL$CREATE       proc
                mov     edx, dword ptr [esp+4]
                xor     ecx, ecx
                mov     ah, DOS_CREATE
                int     DOS_INT
                jmp     short checked
LL$CREATE       endp

;;::::::::::::::
;; LL$READ(handle: i16, data: *near mut u8, count: usize) -> isize: bytes read, in one call: DOS/32A takes a
;; 32-bit count and returns one. Where DOS sets carry, the error code negated.
LL$READ         proc
                mov     ah, DOS_READ
                jmp     short transfer
LL$READ         endp

;;::::::::::::::
;; LL$SEEK(handle: i16, position: i32, origin: u8) -> i32: the absolute position, or the error code negated.
LL$SEEK         proc
                push    ebx
                movzx   ebx, word ptr [esp+8]
                mov     edx, dword ptr [esp+12]
                mov     ecx, edx
                shr     ecx, 16
                mov     al, byte ptr [esp+16]
                mov     ah, DOS_SEEK
                int     DOS_INT
                pop     ebx
                jc      short seek_failed
                movzx   eax, ax
                movzx   edx, dx
                shl     edx, 16
                or      eax, edx
                ret
seek_failed:
                movzx   eax, ax
                neg     eax
                ret
LL$SEEK         endp

;;::::::::::::::
;; LL$WRITE_FILE(handle: i16, data: *near u8, count: usize) -> isize: bytes written, as LL$READ.
LL$WRITE_FILE   proc
                mov     ah, DOS_WRITE
transfer::
                push    ebx
                movzx   ebx, word ptr [esp+8]
                mov     edx, dword ptr [esp+12]
                mov     ecx, dword ptr [esp+16]
                int     DOS_INT
                pop     ebx
                jnc     short transferred
                movzx   eax, ax
                neg     eax
transferred:
                ret
LL$WRITE_FILE   endp

;;::::::::::::::
;; LL$CLOSE(handle: handle) -> isize
LL$CLOSE        proc
                push    ebx
                movzx   ebx, word ptr [esp+8]
                mov     ah, DOS_CLOSE
                int     DOS_INT
                pop     ebx
                jc      short checked
                xor     eax, eax                ;; DOS leaves AX undefined on success
checked::
                jnc     short done
                movzx   eax, ax
                neg     eax
done:
                ret
LL$CLOSE        endp

endif

if OS_GROUPS and G_CONSOLE
;; The console's keyboard: standard input, so what DOS redirects it follows.

;;::::::::::::::
;; LL$CONSOLE_READ_KEY() -> u8: the keyboard's character, no echo (08h). From redirected
;; input it is the next byte read from the handle, or Ctrl-Z once there are none.
LL$CONSOLE_READ_KEY proc
                push    ebx
                push    ecx
                push    edx
                mov     ax, DOS_IOCTL * 256
                mov     ebx, DOS_STDIN
                int     DOS_INT
                jc      short from_handle
                test    dl, DEVICE_BIT
                jnz     short from_device
from_handle:
                push    eax                     ;; the byte lands here
                mov     ah, DOS_READ
                mov     ebx, DOS_STDIN
                mov     ecx, 1
                mov     edx, esp
                int     DOS_INT
                pop     edx
                jc      short end_of_input
                cmp     eax, 1
                jne     short end_of_input
                movzx   eax, dl
                jmp     short key_done
end_of_input:
                mov     eax, CTRL_Z
                jmp     short key_done
from_device:
                mov     ah, DOS_READ_KEY
                int     DOS_INT
                movzx   eax, al
key_done:
                pop     edx
                pop     ecx
                pop     ebx
                ret
LL$CONSOLE_READ_KEY endp

;;::::::::::::::
;; LL$CONSOLE_KEY_READY() -> bool: DOS's input status (0Bh), 0FFh or 0, as 1 or 0.
LL$CONSOLE_KEY_READY proc
                mov     ah, DOS_KEY_READY
                int     DOS_INT
                movzx   eax, al
                and     eax, 1
                ret
LL$CONSOLE_KEY_READY endp
endif

if OS_GROUPS and G_CORE
;;::::::::::::::
;; LL$EXIT(code: u8): ends the program.
LL$EXIT         proc
                mov     al, byte ptr [esp+4]
                mov     ah, DOS_EXIT
                int     DOS_INT
LL$EXIT         endp
endif

end
