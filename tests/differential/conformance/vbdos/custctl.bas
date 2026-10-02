option explicit

declare sub SetProperty
declare sub InvokeEvent
declare sub InvokeMethod

dim control_id as integer
dim caption_text as string

SetProperty caption_text, byval control_id, byval 60
InvokeMethod byval 25, byval 2, byval control_id, byval 3
InvokeEvent byval control_id, byval 2
