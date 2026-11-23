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