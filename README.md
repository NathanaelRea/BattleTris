
![BattleTris](usr/src/art/btstartup2.png)

# BattleTris

BattleTris is a two-player networked game based on Tetris.   Players
collect  money  to  purchase  weapons,  which  in  turn  make the other
player's game more difficult.  Examples of weapons include flipping the
opponent's  screen  upside  down,  swapping  boards with your opponent,
"spying" on your opponent, giving your opponent disjointed pieces, etc.
Each  player's  record  is  maintained  in  a database, and players are
ranked based on their performance.  
If a player wants to hone his or her
skills, he or she may also play the computer (though not for ranking).

## History

BattleTris was written at Brown University as a CS32 final project in
spring 1994 by Bryan Cantrill, Charlie Hoecker and Mike Shapiro.
It was revived several times between 1994 and 2001, and then
exhumed in 2026 by Adam Leventhal.  (A fuller history -- including
the inspiration for BattleTris in Wesleyan Tetris -- can be found 
[here](https://bcantrill.dtrace.org/2026/05/25/a-portentous-reunion/).)

## Requirements

BattleTris is a bit of a time
capsule of Unix on the desktop ca. 1994: it was originally written for
Solaris on SPARC, using [X11](https://en.wikipedia.org/wiki/X_Window_System) and
[Motif](https://en.wikipedia.org/wiki/Motif_(software)).
This version works on both 
MacOS (via [XQuartz](https://www.xquartz.org/) and OpenMotif)
and on Linux.  To compile BattleTris, you should be able to 
(more or less) run `configure`.

Note that BattleTris dates from a time that the highest resolution
monitors were 1600x1280; those on modern (higher resolution) displays
may find that the resolution of output needs to be manually lowered to
make the game playable.

To play against someone else, you will need to be able to directly 
connect to one another's IP address, and each of you will need to connect
to a host running an instance of `btserverd` (which can be found in
`usr/src/daemons`).  This keeps a player database that can be manipulated
with `btref`.

To play against the computer, you do not need to be networked at all;
run `BattleTris -X`.

## Gameplay

After connection is established, each player begins by  playing  Tetris
normally.   The  difference is that in addition to the standard pieces,
there exists a die piece.  This is a one block piece that has  a  value
from 1 to 6 pips.  Whenever a player gets a line, his or her "funds" go
up by the number of pips in the line (it's important  to  note  that  a
"double"  earns  twice the number of pips in the double, a triple earns
triple, and a tetris earns quadruple).  It is also  important  to  note
that  there  is a small probability that the piece will be a one by one
happy face.  Should the player get a line with the happy face  on  that
turn,  their  funds  will  increase by 150.  If they miss, however, the
happy face will turn into a frown.

The opportunity to spend funds comes whenever the two  players  between
them  get 20 lines.  At this time, both players go to a weapons bazaar,
where they each purchase weapons to make the other's game  more  diffi‐
cult.   Examples  of weapons are flipping the opponent's screen upside-
down, giving them disjointed pieces, depriving them of long pieces, de‐
pleting opponent's funds, etc.

Once  both  players  have left the bazaar, play continues.  Players may
launch weapons by pressing the number key which corresponds to the num‐
ber  of  the weapon in the arsenal (which is displayed on-screen).  The
weapon will last for a specific duration, measured in  lines.   Typical
durations range from 3 to 30 lines.

The first player to die loses.

## Future directions

### Graphics

The intent is to keep game play more or less as it was in ~1994,
including the use of Motif.  It would be entirely reasonable to rewrite
BattleTris for modernity, of course, but this version will remain true
to its mid-1990s roots.

### Networking

An entirely reasonable enhancement would be to allow play over the
open Internet by proxying gameplay through an Internet-facing service.
This would require some modest changes to the networking aspects of
the game, but should be otherwise straightforward.

### Sounds 

The BattleTris sounds -- a major component of the original game that 
players of the era will remember -- have not yet been recovered.
We are not totally out of ideas of where they may linger, but if you
happen to be in possession of BattleTris audio files (or the backup
tapes which might contain them?), you would be a hero down at the 
GenX retirement village.

