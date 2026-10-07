.model small
.386
pw typedef ptr word
pf typedef far ptr byte
ppt typedef ptr pw
.data
v1 pw 0
v2 pf 0
v3 ppt 0
v4 real4 0.0
v5 real8 0.0
v6 qword 0
v7 sbyte 0
v8 sdword 0
v9 sword 0
v10 byte 0
buf byte 10 dup (0)
dbuf dword 5 dup (0)
u union
  ua dw ?
  ub dd ?
u ends
v11 u <>
.code
f proc near
    ret
f endp
end
