type Header
    size as long
end type

dim fileNumber as integer
dim recordNumber as long
dim header as Header

seek #fileNumber, recordNumber
get #fileNumber, recordNumber, header
put #fileNumber, recordNumber, header
