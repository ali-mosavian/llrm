' Fixed record fields should promote independently across loop statements.
type Coord
    x as long
    y as long
end type
dim position as Coord
dim iteration as integer
dim stepX as long, stepY as long
read stepX, stepY
position.x = 0
position.y = 0
for iteration = 1 to 7
    position.x = position.x + stepX
    position.y = position.y + stepY
next iteration
print position.x
print position.y
print "DONE"
data 3, -5
