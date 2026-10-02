' Two independent fields of a selected record should remain values in a loop.
type Coord
    x as long
    y as long
end type
dim points(2) as Coord
dim slot as integer, iteration as integer
dim stepX as long, stepY as long
read slot, stepX, stepY
points(slot).x = 0
points(slot).y = 0
for iteration = 1 to 7
    points(slot).x = points(slot).x + stepX
    points(slot).y = points(slot).y + stepY
next iteration
print points(slot).x
print points(slot).y
print "DONE"
data 1, 3, -5
