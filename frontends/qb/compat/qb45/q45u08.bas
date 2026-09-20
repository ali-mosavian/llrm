' QB45 compatibility source.
type Vertex
    x as integer
    y as integer
    label as string * 4
end type

dim firstVertex as Vertex
dim secondVertex as Vertex

firstVertex.x = &H1234
firstVertex.y = &H5678
firstVertex.label = "node"
secondVertex = firstVertex
firstVertex.x = 0

def seg = varseg(secondVertex)
if secondVertex.x = &H1234 and secondVertex.y = &H5678 and secondVertex.label = "node" and len(secondVertex) = 8 and peek(varptr(secondVertex)) = &H34 and peek(varptr(secondVertex) + 1) = &H12 then
    def seg
    print "PASS udt"
else
    def seg
    print "FAIL udt copy"
end if
end
