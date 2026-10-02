' QB45 compatibility source.
dim colors(0 to 3) as long

screen 1
colors(0) = 0
colors(1) = 1
colors(2) = 2
colors(3) = 3
palette using colors(0)
pcopy 0, 0
screen 0
print "PASS palette"
end
