dim token(0 to 3) as string
dim answer as integer

select case token(0)
    case "display.xres", "display.yres"
        answer = 1
    case else
        answer = 2
end select
