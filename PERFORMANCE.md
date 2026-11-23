# Performance (v0.2.0, 23 November 2026)

Measured with the built-in performance run (test switch `DUEL_BENCH`): an 8-fighter Free-for-All, all bots,
frame rate unlocked (no vsync, no limit, no dynamic resolution), counted after 5 s to settle.

Test PC: NVIDIA GeForce RTX 3060 (a mid-range card), Intel Core i9-14900K (a high-end processor), Windows 11.

| Run | 3D picture | Average | 1% low | Frames slower than 144 fps |
| --- | --- | --- | --- | --- |
| High, 1080p pixel count, 60 s, new weapon models | 1663x1247 (2.07 MP) | 341 fps | 230 fps | 0.00% |
| High, 1080p pixel count, one 30-minute match (old weapon shapes) | 1663x1247 | 309 fps | 174 fps | 0.19% |
| Steam Deck preset, 1280x800, 60 s | 1152x720 | 785 fps | 403 fps | 0.00% |
| Steam Deck preset, tiny picture (processor cost only) | 346x216 | 960 fps (1.04 ms) | 538 fps (1.86 ms) | 0.00% |

Processor time per frame: game logic 0.06 ms; preparing the 3D picture 0.8 to 0.9 ms (weapons out of view are
skipped, and fighters more than 9 m away get a simplified weapon model).

## Target: 144 fps on a mid-range PC

Met with room to spare: 230 fps at the 1% low on an RTX 3060 at 1080p High. The processor here is faster than a
mid-range one, but the game uses about 1 ms of processor time a frame, so even a processor three times slower
stays near 3 ms, well inside the 6.9 ms a 144 fps frame allows.

## Target: 60 fps on the Steam Deck (estimated, not measured on a Deck)

No Steam Deck was available, so this is an estimate from the runs above. The Deck's graphics are roughly 6 to 8
times slower than the RTX 3060, and its processor about 3 times slower per core than the test PC's. Scaling the
Steam Deck preset's graphics share (about 0.2 ms average, 0.6 ms at the 1% low) by 8, and the processor share
(1.04 ms average, 1.86 ms at the 1% low) by 3, gives roughly 3 ms average and 6 ms at the 1% low, against the
16.7 ms a 60 fps frame allows. The Deck also starts with a 60 fps limit and dynamic resolution on. Worth
confirming on a real Deck.

## Memory

- The game's own lists (effects, replays, histories, logs) stay flat through a 30-minute match: checked every
  frame of a simulated 30-minute, 8-bot match by the test `soak_tests::a_30_minute_match_keeps_every_list_bounded`,
  and every 10 s in the 30-minute run.
- The whole program: 1,138 MB committed / 370 MB in RAM after the first minute, 1,163 MB / 391 MB after 30
  minutes (+25 MB, most of it in the first 10 minutes, then nearly flat). Most of the committed memory is the
  graphics driver and the textures it holds.

## Running it again

    set DUEL_AUTOPLAY=1
    set DUEL_BOTS=7
    set DUEL_QUALITY=2            (0 Low, 1 Medium, 2 High, 3 Ultra, 4 Steam Deck)
    set DUEL_RENDER_SCALE=1.299   (draws a 1280x960 window's 3D picture at 1080p's pixel count)
    set DUEL_BENCH=60,C:\temp\bench.txt
    duel_arena.exe
