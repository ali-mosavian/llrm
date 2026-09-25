option explicit

dim replyValue as integer
dim inputValue as string

msgbox "Compatibility dialog", 1, "VBDOS"
replyValue = msgbox("Choose a result", 4, "VBDOS")
inputValue = inputbox$("Type a value", "VBDOS", "default", 1, 1)
