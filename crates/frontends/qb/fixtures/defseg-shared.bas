sub selectSegment (byval segment as integer)
    def seg = segment
end sub

function readByte (byval offset as long) as integer
    readByte = peek(offset)
end function
