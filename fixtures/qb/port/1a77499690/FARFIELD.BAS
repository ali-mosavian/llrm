option explicit
type Item
pad as integer
value as integer
end type
declare sub consume (number as integer)
dim shared items() as Item
sub invoke (byval index as integer)
consume items(index).value
end sub
