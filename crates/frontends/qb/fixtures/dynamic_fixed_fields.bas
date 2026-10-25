type Pixel
    red as string * 1
    green as string * 1
end type

redim pixels(0 to 2) as Pixel
pixels(0).green = chr$(0)
