option explicit

type Game
    doorCount as integer
    platCount as integer
    trigCount as integer
end type

type Door
    padding(29) as integer
    link as integer
    targeted as integer
    tail(18) as integer
end type

type Plat
    padding(18) as integer
    kind as integer
    targeted as integer
    state as integer
    tail(26) as integer
end type

type Trigger
    model as integer
    kind as integer
    target as integer
    name as integer
    kill as integer
    state as integer
    left as integer
    tail(30) as integer
end type

declare sub fireDoor ( g as Game, byval link as integer, door() as Door )
declare sub fireTrig ( g as Game, byval k as integer, door() as Door, trig() as Trigger, plat() as Plat )

sub useTargets ( g as Game, byval id as integer, door() as Door, trig() as Trigger, plat() as Plat )
    dim k as integer

    if ( id = 0 ) then exit sub
    for k = 0 to g.doorCount - 1
        if ( door(k).targeted = id ) then fireDoor g, door(k).link, door()
    next k
    for k = 0 to g.platCount - 1
        if ( plat(k).kind = 1 and plat(k).targeted = id and plat(k).state = 0 ) then plat(k).state = 3
    next k
    for k = 0 to g.trigCount - 1
        if ( trig(k).name = id ) then
            select case trig(k).kind
                case 2
                    if ( trig(k).state <> 4 ) then
                        trig(k).left = trig(k).left - 1
                        if ( trig(k).left <= 0 ) then fireTrig g, k, door(), trig(), plat()
                    end if
                case 0, 1
                    if ( trig(k).state = 0 ) then fireTrig g, k, door(), trig(), plat()
                case 7
                    trig(k).state = 5
                case 8
                    fireTrig g, k, door(), trig(), plat()
                case 9
                    if ( trig(k).state = 0 ) then trig(k).state = 5
                case 10
                    fireTrig g, k, door(), trig(), plat()
            end select
        end if
    next k
end sub

dim gameState as Game
dim doors(1) as Door
dim triggers(1) as Trigger
dim plats(1) as Plat
useTargets gameState, 1, doors(), triggers(), plats()
end
