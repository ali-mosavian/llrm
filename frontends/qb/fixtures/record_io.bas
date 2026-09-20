type Header
    size as long
end type

dim fileNumber as integer
dim header as Header
dim text as string

get #fileNumber, , header
put #fileNumber, , text
