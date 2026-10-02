' Guard the runtime-selected record before accessing either field.
type Coord
    x as long
    y as long
end type
dim points(2) as Coord
dim slot as integer, iteration as integer
dim stepX as long, stepY as long
read slot, stepX, stepY
if slot < 0 then end
if slot > 2 then end
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
