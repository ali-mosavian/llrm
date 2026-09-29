	.386p
	.387
	ifndef	??version
?debug	macro
	endm
publicdll macro	name
	public	name
	endm
$comm	macro	name,dist,size,count
	comm	dist name:BYTE:count*size
	endm
	else
$comm	macro	name,dist,size,count
	comm	dist name[size]:BYTE:count
	endm
	endif
	?debug	V 300h
	?debug	S "pfaggee.c"
	?debug	C E9996A3D5D09706661676765652E63
	?debug	C E9996A3D5D0470662E68
	?debug	C E9996A3D5D0863616C6C65652E68
	?debug	C E9996A3D5D0763617365732E68
PFAGGEE_TEXT	segment byte public use16 'CODE'
PFAGGEE_TEXT	ends
DGROUP	group	_DATA,_BSS
	assume	cs:PFAGGEE_TEXT,ds:DGROUP
_DATA	segment word public use16 'DATA'
d@	label	byte
d@w	label	word
_DATA	ends
_BSS	segment word public use16 'BSS'
b@	label	byte
b@w	label	word
_BSS	ends
PFAGGEE_TEXT	segment byte public use16 'CODE'
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
P_S1	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_s1,ax
	mov	al,byte ptr [bp+8]
	mov	byte ptr DGROUP:_gv_s1,al
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_s1,ax
	pop	bp
	ret	6
P_S1	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
P_S2	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gh_s2,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_s2,ax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_s2,ax
	pop	bp
	ret	6
P_S2	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
P_S3	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gh_s3,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_s3,ax
	mov	al,byte ptr [bp+10]
	mov	byte ptr DGROUP:_gv_s3+2,al
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_s3,ax
	pop	bp
	ret	8
P_S3	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
P_S4	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gh_s4,ax
	mov	eax,dword ptr [bp+8]
	mov	dword ptr DGROUP:_gv_s4,eax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_s4,ax
	pop	bp
	ret	8
P_S4	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
P_S5	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+14]
	mov	word ptr DGROUP:_gh_s5,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_s5,ax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gv_s5+2,ax
	mov	al,byte ptr [bp+12]
	mov	byte ptr DGROUP:_gv_s5+4,al
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_s5,ax
	pop	bp
	ret	10
P_S5	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
P_S7	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+16]
	mov	word ptr DGROUP:_gh_s7,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_s7,ax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gv_s7+2,ax
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gv_s7+4,ax
	mov	al,byte ptr [bp+14]
	mov	byte ptr DGROUP:_gv_s7+6,al
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_s7,ax
	pop	bp
	ret	12
P_S7	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
P_S8	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+16]
	mov	word ptr DGROUP:_gh_s8,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_s8,ax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gv_s8+2,ax
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gv_s8+4,ax
	mov	ax,word ptr [bp+14]
	mov	word ptr DGROUP:_gv_s8+6,ax
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_s8,ax
	pop	bp
	ret	12
P_S8	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
P_S9	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+18]
	mov	word ptr DGROUP:_gh_s9,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_s9,ax
	mov	ax,word ptr [bp+10]
	mov	word ptr DGROUP:_gv_s9+2,ax
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gv_s9+4,ax
	mov	ax,word ptr [bp+14]
	mov	word ptr DGROUP:_gv_s9+6,ax
	mov	al,byte ptr [bp+16]
	mov	byte ptr DGROUP:_gv_s9+8,al
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_s9,ax
	pop	bp
	ret	14
P_S9	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
P_SCI	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	mov	ax,word ptr [bp+12]
	mov	word ptr DGROUP:_gh_sci,ax
	mov	ax,word ptr [bp+8]
	mov	word ptr DGROUP:_gv_sci,ax
	mov	al,byte ptr [bp+10]
	mov	byte ptr DGROUP:_gv_sci+2,al
	mov	ax,word ptr [bp+6]
	mov	word ptr DGROUP:_gt_sci,ax
	pop	bp
	ret	8
P_SCI	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
R_S1	proc	far
	call	far ptr _trash
	mov	al,byte ptr DGROUP:_in_s1
	ret	
R_S1	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
R_S2	proc	far
	call	far ptr _trash
	mov	ax,word ptr DGROUP:_in_s2
	ret	
R_S2	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
R_S3	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	les	bx,dword ptr [bp+6]
	mov	ax,word ptr DGROUP:_in_s3
	mov	word ptr es:[bx],ax
	mov	al,byte ptr DGROUP:_in_s3+2
	mov	byte ptr es:[bx+2],al
	mov	dx,word ptr [bp+8]
	mov	ax,word ptr [bp+6]
	pop	bp
	ret	4
R_S3	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
R_S4	proc	far
	call	far ptr _trash
	mov	eax,dword ptr DGROUP:_in_s4
	shld	edx,eax,16
	ret	
R_S4	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
R_S5	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	les	bx,dword ptr [bp+6]
	mov	ax,word ptr DGROUP:_in_s5
	mov	word ptr es:[bx],ax
	mov	ax,word ptr DGROUP:_in_s5+2
	mov	word ptr es:[bx+2],ax
	mov	al,byte ptr DGROUP:_in_s5+4
	mov	byte ptr es:[bx+4],al
	mov	dx,word ptr [bp+8]
	mov	ax,word ptr [bp+6]
	pop	bp
	ret	4
R_S5	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
R_S7	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	les	bx,dword ptr [bp+6]
	mov	ax,word ptr DGROUP:_in_s7
	mov	word ptr es:[bx],ax
	mov	ax,word ptr DGROUP:_in_s7+2
	mov	word ptr es:[bx+2],ax
	mov	ax,word ptr DGROUP:_in_s7+4
	mov	word ptr es:[bx+4],ax
	mov	al,byte ptr DGROUP:_in_s7+6
	mov	byte ptr es:[bx+6],al
	mov	dx,word ptr [bp+8]
	mov	ax,word ptr [bp+6]
	pop	bp
	ret	4
R_S7	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
R_S8	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	les	bx,dword ptr [bp+6]
	mov	ax,word ptr DGROUP:_in_s8
	mov	word ptr es:[bx],ax
	mov	ax,word ptr DGROUP:_in_s8+2
	mov	word ptr es:[bx+2],ax
	mov	ax,word ptr DGROUP:_in_s8+4
	mov	word ptr es:[bx+4],ax
	mov	ax,word ptr DGROUP:_in_s8+6
	mov	word ptr es:[bx+6],ax
	mov	dx,word ptr [bp+8]
	mov	ax,word ptr [bp+6]
	pop	bp
	ret	4
R_S8	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
R_S9	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	les	bx,dword ptr [bp+6]
	mov	ax,word ptr DGROUP:_in_s9
	mov	word ptr es:[bx],ax
	mov	ax,word ptr DGROUP:_in_s9+2
	mov	word ptr es:[bx+2],ax
	mov	ax,word ptr DGROUP:_in_s9+4
	mov	word ptr es:[bx+4],ax
	mov	ax,word ptr DGROUP:_in_s9+6
	mov	word ptr es:[bx+6],ax
	mov	al,byte ptr DGROUP:_in_s9+8
	mov	byte ptr es:[bx+8],al
	mov	dx,word ptr [bp+8]
	mov	ax,word ptr [bp+6]
	pop	bp
	ret	4
R_S9	endp
   ;	
   ;	AGGREGATES(PARAM) AGGREGATES(RESULT)
   ;	
	assume	cs:PFAGGEE_TEXT
R_SCI	proc	far
	push	bp
	mov	bp,sp
	call	far ptr _trash
	les	bx,dword ptr [bp+6]
	mov	ax,word ptr DGROUP:_in_sci
	mov	word ptr es:[bx],ax
	mov	al,byte ptr DGROUP:_in_sci+2
	mov	byte ptr es:[bx+2],al
	mov	dx,word ptr [bp+8]
	mov	ax,word ptr [bp+6]
	pop	bp
	ret	4
R_SCI	endp
PFAGGEE_TEXT	ends
_BSS	segment word public use16 'BSS'
_gt_sci	label	word
	db	2 dup (?)
_gh_sci	label	word
	db	2 dup (?)
_gv_sci	label	word
	db	3 dup (?)
_gt_s9	label	word
	db	2 dup (?)
_gh_s9	label	word
	db	2 dup (?)
_gv_s9	label	word
	db	9 dup (?)
_gt_s8	label	word
	db	2 dup (?)
_gh_s8	label	word
	db	2 dup (?)
_gv_s8	label	word
	db	8 dup (?)
_gt_s7	label	word
	db	2 dup (?)
_gh_s7	label	word
	db	2 dup (?)
_gv_s7	label	word
	db	7 dup (?)
_gt_s5	label	word
	db	2 dup (?)
_gh_s5	label	word
	db	2 dup (?)
_gv_s5	label	word
	db	5 dup (?)
_gt_s4	label	word
	db	2 dup (?)
_gh_s4	label	word
	db	2 dup (?)
_gv_s4	label	word
	db	4 dup (?)
_gt_s3	label	word
	db	2 dup (?)
_gh_s3	label	word
	db	2 dup (?)
_gv_s3	label	word
	db	3 dup (?)
_gt_s2	label	word
	db	2 dup (?)
_gh_s2	label	word
	db	2 dup (?)
_gv_s2	label	word
	db	2 dup (?)
_gt_s1	label	word
	db	2 dup (?)
_gh_s1	label	word
	db	2 dup (?)
_gv_s1	label	word
	db	1 dup (?)
	?debug	C E9
	?debug	C FA00000000
_BSS	ends
_DATA	segment word public use16 'DATA'
s@	label	byte
_DATA	ends
PFAGGEE_TEXT	segment byte public use16 'CODE'
PFAGGEE_TEXT	ends
	public	R_SCI
	public	R_S9
	public	R_S8
	public	R_S7
	public	R_S5
	public	R_S4
	public	R_S3
	public	R_S2
	public	R_S1
	public	P_SCI
	public	_gt_sci
	public	_gh_sci
	public	_gv_sci
	public	P_S9
	public	_gt_s9
	public	_gh_s9
	public	_gv_s9
	public	P_S8
	public	_gt_s8
	public	_gh_s8
	public	_gv_s8
	public	P_S7
	public	_gt_s7
	public	_gh_s7
	public	_gv_s7
	public	P_S5
	public	_gt_s5
	public	_gh_s5
	public	_gv_s5
	public	P_S4
	public	_gt_s4
	public	_gh_s4
	public	_gv_s4
	public	P_S3
	public	_gt_s3
	public	_gh_s3
	public	_gv_s3
	public	P_S2
	public	_gt_s2
	public	_gh_s2
	public	_gv_s2
	public	P_S1
	public	_gt_s1
	public	_gh_s1
	public	_gv_s1
	extrn	_in_sci:word
	extrn	_in_s9:word
	extrn	_in_s8:word
	extrn	_in_s7:word
	extrn	_in_s5:word
	extrn	_in_s4:word
	extrn	_in_s3:word
	extrn	_in_s2:word
	extrn	_in_s1:word
	extrn	_trash:far
_s@	equ	s@
	end
