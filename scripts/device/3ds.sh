#!/bin/bash
# 3ds.sh <rom>: plays a game Pocket Native was built with from
# EmulationStation's 3DS list, for a system whose command runs this with
# %ROM%, from the ROM there instead of a copy beside the game
id=$(od -An -tx8 -j 264 -N 8 "$1" | tr -d ' ' | tr a-f A-F)
exec "$(dirname "$0")/run.sh" "$id" "$1"
