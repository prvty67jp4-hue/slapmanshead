# Balance: spammer vs precise

Made by `cargo test --release duel_benchmark -- --ignored --nocapture` (src/duel_bench.rs).

Two bots duel 200 times with each weapon (the same weapon on both sides, start 6-35 m apart depending on the weapon, which side is which alternating). Both see the other 0.18 s late, turn and aim the same way with the same wobble, and run at the same speed: only their decisions differ.

- **Spammer**: holds the trigger, strafes and hops while shooting, never pulls recoil down, mashes slashes and guard, kicks at random, throws grenades on cooldown.
- **Precise**: taps (SMG bursts of 4, revolver after its 0.45 s reset), counter-strafes to a stop before each shot, waits for the aim to settle, pulls recoil down, keeps just outside sword reach, punishes whiffs, parries swings it can time, kicks a held guard, dodges projectiles.

A draw (both alive after 45 s) counts as half a win.

| Weapon | Precise wins | Spammer wins | Draws | Precise win rate | Accuracy (precise / spammer) | Parries (P / S) | Whiffs (P / S) |
|---|---|---|---|---|---|---|---|
| Dagger | 170 | 30 | 0 | **85%** | - / - | 695 / 0 | 5 / 515 |
| Katana | 189 | 11 | 0 | **94%** | - / - | 548 / 0 | 3 / 355 |
| Greatsword | 200 | 0 | 0 | **100%** | - / - | 589 / 0 | 0 / 11 |
| Scattergun | 142 | 58 | 0 | **71%** | 100% / 80% | 0 / 0 | 0 / 0 |
| Revolver | 187 | 13 | 0 | **94%** | 93% / 35% | 0 / 0 | 0 / 0 |
| SMG | 197 | 3 | 0 | **98%** | 73% / 17% | 0 / 0 | 0 / 0 |
| Rifle | 180 | 20 | 0 | **90%** | 91% / 52% | 0 / 0 | 0 / 0 |
| Rocket Launcher | 185 | 13 | 2 | **93%** | - / - | 0 / 0 | 0 / 0 |
| Hand Cannon | 155 | 45 | 0 | **78%** | 95% / 47% | 0 / 0 | 0 / 0 |
| Sniper | 173 | 27 | 0 | **86%** | 51% / 7% | 0 / 0 | 0 / 0 |
| Shuriken | 171 | 29 | 0 | **86%** | 27% / 17% | 0 / 0 | 0 / 0 |
| **All** | 1949 | 249 | 2 | **89%** | 69% / 25% | 1832 / 0 | 8 / 881 |

Target: the precise bot wins at least 70% overall and in every melee matchup. Result: 89% overall, 85% in the weakest melee matchup - met.

## What the benchmark changed

- Rifle: a shot's recoil now moves the aim for the *next* shot (the shot itself goes where you aimed), and the kick per quick follow-up is 0.05 rad (was 0.03: that lifted a sprayer's second shot exactly from chest to head, rewarding spraying with headshots). Its "rested" first shot counts from when it's ready to fire again (its 0.45 s cycle alone is longer than the 0.4 s rest).
- SMG: the same next-shot timing, so the first 4 bullets are dead-on.
- Slashing out of a guard keeps the full wind-up (it used to skip most of it, leaving about 0.05 s to react).
- Practice bots pull recoil down in proportion to their skill.

## Weapons in hand match the HUD

`DUEL_GALLERY=held:<weapon number>[:run]` lines up your fighter (so the HUD names the weapon) and a second fighter holding it. Every weapon's model in hand matches the HUD's name and icon:

- `shots/weapons/held_check/_all_ready.png` - all 11 weapons, ready to fire (one picture each in the same folder)
- `shots/weapons/held_check/_all_running.png` - the guns while running
- `shots/weapons/held_check/scattergun_play_1100.png` (before) and `scattergun_play_after.png` (after) - the over-the-shoulder view

The scattergun that "looked like a pistol" was the right model seen end-on: shouldered at cheek height and pointing straight away from the over-the-shoulder camera, its wooden stock hid behind the hood and only the dark receiver showed. Long guns (scattergun, rifle, sniper) are now carried at low ready (muzzle down and a little out to the right) when not shooting, and come up to the shoulder the moment they fire, so their length and pump show from behind.

<!-- section:ledger -->
## Ledger Duels: loadouts against the field

Made by `cargo test --release ledger_balance -- --ignored --nocapture` (src/ledger_tests.rs).

2016 bot duels on a flat floor, every preset loadout against every other the same number of times (each side starting on each end, 7-15 m apart). Both bots are the precise duelling bot from the weapon benchmark (same aim, reactions and movement) with the Ledger layer on top: they eat when hurt and not pressed, follow a meal with a bite, drink a tonic early, throw a special when it can finish or is worth it, swap to a weapon whose special can finish, and swap between blade and gun by range. A draw (both standing after 75 s) counts as half a win. Average duel: 19.6 s.

| Loadout | Set | Main hand | Win rate vs the field |
|---|---|---|---|
| Gravewarden Headsman | Gravewarden | Ashfall Cleaver | **55.0%** |
| Bastion Wall | Bastion | Courtly Sabre | **53.2%** |
| Swiftshade Fangs | Swiftshade | Twin Fangs | **45.1%** |
| Longsight Marksman | Longsight | Ledger Repeater | **46.4%** |
| Court Duellist | - | Courtly Sabre | **51.0%** |
| Foundry Brawler | - | Foundry Scattergun | **49.1%** |
| Lowtown Rattler | Swiftshade | Lowtown Rattler | **50.4%** |
| Border Warden | Longsight | Warden Longbarrel | **44.9%** |
| Bridgewright Rigger | Bridgewright's Rig | Rigger's Hook | **54.8%** |

By set (the loadouts wearing it):

| Set | Win rate vs the field |
|---|---|
| Gravewarden | 55.0% |
| Bastion | 53.2% |
| Swiftshade | 47.8% |
| Longsight | 45.6% |
| No set | 50.1% |
| Bridgewright's Rig | 54.8% |

Target: no loadout above ~55% against the field. Result: the highest is 55.0% - met.

Head to head (row's win rate against the column):

| | Gravewarden | Bastion | Swiftshade | Longsight | Court | Foundry | Lowtown | Border | Bridgewright |
|---|---|---|---|---|---|---|---|---|---|
| Gravewarden | - | 59% | 54% | 62% | 51% | 45% | 62% | 52% | 55% |
| Bastion | 41% | - | 54% | 77% | 50% | 36% | 59% | 59% | 50% |
| Swiftshade | 46% | 46% | - | 12% | 50% | 27% | 61% | 70% | 49% |
| Longsight | 38% | 23% | 88% | - | 66% | 79% | 11% | 43% | 25% |
| Court | 49% | 50% | 50% | 34% | - | 39% | 71% | 57% | 58% |
| Foundry | 55% | 64% | 73% | 21% | 61% | - | 41% | 25% | 52% |
| Lowtown | 38% | 41% | 39% | 89% | 29% | 59% | - | 66% | 42% |
| Border | 48% | 41% | 30% | 57% | 43% | 75% | 34% | - | 30% |
| Bridgewright | 45% | 50% | 51% | 75% | 42% | 48% | 58% | 70% | - |

No staking or wagering of any kind exists in Ledger Duels: the honour ledger only records wins, losses and streaks per loadout.
<!-- /section:ledger -->
