defint a-z
type Snake
    row as integer
    lives as integer
    score as integer
end type
declare sub Touch (a() as Snake)
dim a(1 to 2) as Snake
Touch a()
end
sub Touch (a() as Snake)
    a(1).row = 25
end sub
