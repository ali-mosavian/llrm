type Vector
    x as single
    y as single
    count as long
end type

dim vertices(0 to 3) as Vector
dim i as integer
i = 2
vertices(i).count = vertices(i).count + 1&
