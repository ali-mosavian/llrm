.text
.globl report, _report, input_read, _input_read, report_, input_read_
report:
_report:
input_read:
_input_read:
 ret
# llrm's default convention takes the value in EAX, and the harness reads it there: a place of its own
report_:
input_read_:
 ret
# libc stand-ins: dword string ops, then the tail bytes
.globl memset, memcpy, memmove
memset:
 push %edi
 mov 8(%esp),%edi
 movzbl 12(%esp),%eax
 imul $0x01010101,%eax,%eax
 mov 16(%esp),%ecx
 mov %ecx,%edx
 shr $2,%ecx
 rep stosl
 mov %edx,%ecx
 and $3,%ecx
 rep stosb
 mov 8(%esp),%eax
 pop %edi
 ret
memcpy:
memmove:
 push %esi
 push %edi
 mov 12(%esp),%edi
 mov 16(%esp),%esi
 mov 20(%esp),%ecx
 cmp %esi,%edi
 jbe 1f
 lea (%esi,%ecx),%eax
 cmp %eax,%edi
 jae 1f
 # overlapping with dst above src: copy backwards, dwords from the end, then the head bytes
 std
 lea -4(%esi,%ecx),%esi
 lea -4(%edi,%ecx),%edi
 mov %ecx,%edx
 shr $2,%ecx
 rep movsl
 mov %edx,%ecx
 and $3,%ecx
 add $3,%esi
 add $3,%edi
 rep movsb
 cld
 jmp 2f
1:
 mov %ecx,%edx
 shr $2,%ecx
 rep movsl
 mov %edx,%ecx
 and $3,%ecx
 rep movsb
2:
 mov 12(%esp),%eax
 pop %edi
 pop %esi
 ret
