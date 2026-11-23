# Weapon models (Blender)

The game's weapons are built by script, so they can be rebuilt or tweaked any time.

    blender --background --python app/tools/blender/build_weapons.py
    blender --background --python app/tools/blender/build_weapons.py -- katana scattergun
    blender --background --python app/tools/blender/build_weapons.py -- --no-renders

(Blender 4.2 or newer; made with 5.3. On this PC: "C:\Program Files\Blender Foundation\Blender 5.3\blender.exe".)

What it writes:

- `app/assets/weapons/<name>.glb`: the model the game loads (built into the program on the next `cargo build`).
- `app/assets/weapons/<name>_far.glb`: a simplified copy (about 30% of the triangles) the game draws for fighters
  more than 9 m from the camera.
- `app/assets/weapons/textures/<name>.png`: the weapon's baked texture (512 x 512): hand-painted-style wood grain
  with darker edges, metal with wear, lighter worn edges and darker recesses (edge highlights and ambient
  occlusion are painted in), brass, leather and cord wraps, rayskin, engraving. Made by `weapons_tex.py` (Cycles
  bake of procedural paint recipes after a smart UV unwrap). The game puts four to a layer of its texture array.
- `app/assets/weapons/icons/<name>.png`: the HUD and loadout icon, rendered from the model (grey; the game tints it).
- `app/shots/weapons/<name>.png`: a four-view turntable picture for review; `_all.png` has them all.

Files:

- `weapons_tex.py`: the paint recipes (per palette material), unwrapping and baking.
- `weapons_lib.py`: the building blocks (bevelled profiles, swept blades with an edge bevel, tubes, rings,
  booleans), the palette of stylised materials (metal, wood, leather wrap, brass...), export and rendering.
- `build_weapons.py`: one function per weapon, with its measurements in metres.

Conventions the game relies on:

- Blender axes: +X is the weapon's right side, +Y forward (barrel or blade), +Z up. The game reads x right, y up,
  z forward.
- The origin is where the hand holds it: the top of a gun's grip (its middle runs through the fist), the middle
  of a blade's handle.
- Blade reach matches the game: the katana's point at 1.28 m, the greatsword's at 1.72 m, the dagger's at 0.5 m.
- Moving parts are their own objects: `pump` (the scattergun's fore-end; it slides back along -Y by
  `PUMP_TRAVEL`, held by the front hand at `PUMP_HOLD`, both in `src/weapons.rs`) and `cylinder` (the revolver's;
  its origin is the pivot and it turns about Y as shots are fired and while reloading).
- Lenses use the `glass` material and blades' cutting edges `steel_edge`: the game draws both unlit (a glint, and a
  bright edge line that keeps blades readable at a distance).
- In-game pictures of each weapon held: `app/shots/weapons/in_game` (test switch `DUEL_HOLD=<weapon number>`).
- Keep each model under about 4,000 triangles (the test in `src/weapons.rs` checks 300 to 4,000): every fighter's
  weapon is drawn each frame.

After changing a model, run `cargo test` (it checks every model and icon loads, the reach, the moving parts, and
that each gun's front hand reaches where it holds the gun).

# Fighter accessories (Blender)

Each fighter's signature accessory is built by `build_accessories.py`, fitted onto that fighter's own body:

    cargo test --release dump_fighter_refs -- --ignored      (once: writes app/tools/blender/ref/)
    blender --background --factory-startup --python app/tools/blender/build_accessories.py
    blender --background --factory-startup --python app/tools/blender/build_accessories.py -- striker

- Corvin Vale: a brass warden badge (a shield with a star) on a leather backing, on the coat's left breast.
- Sable Rook: a knitted scarf with pale stripes wrapped twice round the neck over the hood's collar, knotted
  at the front left, two tails down the chest. It takes the player's colour; the tails sway.
- Bram "Anvil" Kask: a pauldron of three dark hammered iron plates laid over the left shoulder's own curve
  like scales (fitted by ray casts round the arm; in the game each vertex copies the bones of the skin under
  it, so it bends with the shoulder and stays inside the arm's outline), a leather strap across the chest, and a smith's apron on a belt
  (it hugs the belly, hangs straight from the widest point, follows the thighs and sways).
- Ilya Fen: a hood over the head and shoulders with a thin trim along its front edge (over the brow, down
  to the jaw): a soft accent in the player's colour, brighter only while Afterimage is on (her hair is under
  the hood).

What it writes:

- `app/assets/characters/acc_<name>.glb`: the accessory, each piece its own mesh (`badge`, `scarf`,
  `scarf_tail_*`, `pauldron`, `strap`, `apron`, `belt`, `hood`, `hood_trim`). `ACCESSORIES` in
  `src/chars.rs` says which bones each piece follows and how much it sways; material names say what it is
  (`glow` is drawn unlit in the player's colour, `brass`/`iron` as metal, `leather`/`apron` as leather).
- `app/assets/characters/accessories.png`: the four 512 x 512 bakes in one image, a quarter each (ranger top
  left, huntress top right, brawler bottom left, striker bottom right); each model's UVs point into its quarter.
- `app/shots/character-art/accessories/<name>.png`: four views of the accessory on the body.

Conventions: the game's model space for the fighter (metres, y up, the fighter faces +z, their left is +x),
read from `ref/anchors.json` (bone positions) and `ref/<name>.obj` (the body, in its rest T-pose). Pieces are
fitted with ray casts and an "outside surface" shrink-wrap, so nothing sinks into the body.
