option explicit
dim shared head() as integer
dim shared link() as integer
dim shared group() as integer
function take (byval order as integer, byval wanted as integer) as integer
dim block as integer, previous as integer
block = head(order)
previous = -1
while block >= 0
if group(block) = wanted then
if previous >= 0 then link(previous) = link(block) else head(order) = link(block)
take = block
exit function
end if
previous = block
block = link(block)
wend
take = -1
end function
