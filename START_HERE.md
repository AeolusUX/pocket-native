# Start here

Pocket Native does not work like an emulator. There is no app to install
on the handheld and no game included. A PC rebuilds one of your own game
dumps at a time into a folder for the SD card, and that is the part most
people miss.

Only Pokémon Omega Ruby and Alpha Sapphire have been tested and tuned.
Other games may or may not boot.

## What you need

- An R36S (or a close clone) with ArkOS. Other systems are untested.
- A PC for the build: Linux with podman or Docker, or Windows with
  Docker Desktop or Podman Desktop. On Windows it has to be running
  before you start.
- Your own dump of the game: a decrypted `.3ds`/`.cci`, or a `.cia`
  along with your console's `aes_keys.txt`.

## Building

From the repository's folder:

```bash
scripts/build.sh path/to/game.3ds
```

With a `.cia`:

```bash
scripts/build.sh path/to/game.cia --keys path/to/aes_keys.txt
```

On Windows, in PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build.ps1 path\to\game.3ds
powershell -ExecutionPolicy Bypass -File scripts\build.ps1 path\to\game.cia -Keys path\to\aes_keys.txt
```

The first build sets up the build container, which downloads a few
hundred MB and takes a while. Each game's code is recompiled, which
takes several minutes for a big game. Later builds reuse what they can.

The result is a `Pocket Native` folder in `build/`. Building more games
into the same folder adds them next to each other, one launcher each. A
game with no profile yet builds with `(no profile yet)` after its name
and is named after its title ID.

## Installing

1. Copy the `Pocket Native` folder into `ports` on the card's game
   partition (`roms/ports`, or `roms2/ports` with two cards).
2. Restart EmulationStation. Each game shows up under Ports.

A tap of FN opens the menu in game and pauses it, FN + Start quits.
Saves and the game's log go in
`Pocket Native/pocket-native/games/<title id>/data`.

## If it goes wrong

**The build says the image is encrypted.** The dump has to be decrypted
first, or built from the `.cia` with `--keys` instead.

**The Windows build cannot reach Docker.** Docker Desktop is not
running. Start it, let it finish starting, and run the build again.

**The game is not under Ports.** The folder is in the wrong place, or
EmulationStation has not been restarted. The whole `Pocket Native`
folder goes inside `ports`, not just the files in it.

**A game other than Omega Ruby or Alpha Sapphire does not start, or
runs badly.** That is expected for now. Its `log.txt`, in the game's
`data` folder above, says what the emulator ran into.
