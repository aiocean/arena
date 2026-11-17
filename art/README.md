# Art direction

How every fighter and every spell effect in the arena is drawn, so a new one stands beside the others without looking
pasted in.

**Pixel is how the arena is shown, not how it is drawn (owner, 2026-11-17).** Art is generated smooth, a clean
vector-style illustration, by image models through OpenRouter (game-factory `tools/openrouter-image`, Gemini for
clips), never asked for as pixel art, and kept smooth in the game; the pixel layer at render makes the pixels.
PixelLab is no longer used. A new fighter is a master in the fighting stance, approved through the layer before any
clip is paid for, then the 20 clips of `pick::CLIPS` facing east, 8 key poses a picture with the master as the
reference (about $0.07 a picture). The PixelLab sheets on the page stay until each is redrawn.

Where it stands: the two fighters on the page are still the PixelLab sheets, shown pixel for pixel (the game samples
them nearest, `ImagePlugin::default_nearest`, and enlarges the whole picture by a whole number,
`crates/arena-game/src/layout.rs`). The pixel layer is not in the game's render yet; the spell effects are already
drawn on the new route ("Spell effects drawn by an image model", below) and sampled smoothly.

This file is for the people and agents who make the art. The player docs are `docs/` (published on the site). The
generators looked at and the pictures that decided the route: `art/research/README.md`.

## Making a fighter

1. **The master.** One picture of the fighter in its fighting stance (weapon in hand, feet set), side view facing
   east, smooth, on a plain ground that keys out, at the cast's height: about 6.5 heads tall, a slender wuxia figure.
   Put it through the pixel layer beside the other fighters at the game's density and have the owner approve it
   before any clip is generated.
2. **The clips.** The 20 clips of `CLIPS` (`crates/arena-duel/src/pick.rs`), facing east:

   `stance-idle walk dash jump duck guard slash overhead cast hit knockdown launched tumble getting-up roll frozen
   shocked meditate win defeat`

   One picture holds 8 key poses in a 4 x 2 grid, read left to right then top to bottom, with the master given as the
   reference image, so the design (hair, trims, the weapon's hand and side) stays the master's. A 16-frame clip is two
   pictures, the second continuing the first; the short clips (`hit`, `guard`, `frozen`, `shocked`) one.
3. **Clip to clip.** The game cuts from one clip to the next without blending, so the clips must meet: every clip
   starts from the master's stance; every clip that ends standing (`stance-idle`, `walk`, strikes, `cast`, `guard`,
   `hit`, `getting-up`, `roll`, `duck`, `win`) ends back in it, said on its last frame's line; loops (`stance-idle`,
   `walk`, `tumble`, `shocked`) end where they start; `knockdown` ends lying as `defeat` and `getting-up` begin.
   `launched` and `knockdown` start in the air, thrown by a blow: no standing frame at their start.
4. **Frame data.** A strike's or a cast's picture numbers which frames wind up, which one is the release (the blow
   lands, the spell leaves the hand) and which recover, as long as the rules keep each open ("Frame data and the
   sprite", below).
5. **Look at every frame.** A model draws an effect, a second figure, a cut-off weapon, a hand crossing into the next
   cell: redraw that picture. Check the details that drift between pictures against the master.
6. **The sheet.** Cut the grid, key the ground, find the feet, scale the whole character once, and write the sheet
   the game reads (below) into `crates/arena-game/assets/fighters/`. The tool for this route is still to be written
   (`art/tools/sheet.rs`); the PixelLab characters' own `art/<character>/tools/sheet.rs` built today's sheets.

## Style

| | Rule |
|---|---|
| Drawing | A clean vector-style illustration: crisp shapes, soft cel shading in a few steps a material, shadows cooler and darker, highlights warmer; no glow, no haze, no effects drawn into a fighter |
| Outline | One dark outline round the silhouette; inner lines a darker shade of the area. The pixel layer draws the outline the screen shows |
| Light | From the front and above, the same in every frame; the fighter is mirrored for the west side, light included |
| Edges | Smooth: anti-aliased, part-transparent edges are kept; the layer decides the pixels |
| Proportions | Slender wuxia figures, about 6.5 heads tall, long robes; feet visible or the hem ending at the floor |

### Colour of a character

- One **main colour** (the robe) and one **accent** (a lining, a sash, a trim). The two fighters of a duel must not share
  a main colour, and each must stand out against the stage's dark night ground.
- The **look's colour** is the accent: a look's sheet turns the accent (the cultivator's crimson lining) into the
  look's glow (Thunder `#ffe066`, Frost `#a6ecff`). A new character keeps its accent in one hue family, used
  nowhere else on the body, so that recolour hits it and nothing else.
- **White is the hit flash**: the painter draws a struck fighter all white. A character mostly in white (Thất Thải Đạo
  Nhân) needs its dark hair, sash and outline to carry the flash, or a flash that is not white. Check a hit frame.
- Effects (qi, fire, lightning) are not drawn into a fighter: they are sheets of their own, baked from `arena-vfx`'s
  recipes (`crates/arena-game/assets/vfx`) or drawn by an image model (`art/vfx/<name>/`, "Spell effects drawn by an
  image model" below), and played by `crates/arena-game/src/vfx` in the colours of the effect palettes in
  `crates/arena-duel/src/data.rs` (`Tint`). The character's own colours are never reused for an effect's glow.

## How the PixelLab characters on the page were made (history)

The two fighters on the page today were made on PixelLab before the rule above. Their sheets stay until each is
redrawn on the route above; this is how they were made, for reading them, not for a new character.

1. `create_character`, mode `v3` (the only mode that draws 256; 9 generations), size 256, view `side`, outline
   "single color black outline", detail "high detail". Describe outfit, hair, weapon, materials and colours in the
   prompt as the cultivator's does (`cultivator/src/metadata.json`), and end it with "no aura, no glow": a glow gets
   drawn into every frame and has to be stripped. There is no style lock at 256 (`pro` and `style_character_id`
   stop at 128), so the prompt and the side-by-side check are what keep the cast alike. All 8 directions are made;
   the duel uses east, the rest serve portraits and the site.
2. Look at the east rotation at 1x and 2x beside both references. Same height, outline and shading, or start over.
   Fixing this is cheap here and expensive after 20 animations.
2b. **The stance.** Every clip is generated from the character's rotation, so the rotation must be its fighting
   stance: weapon already in hand, feet set. If the design is drawn at rest (a sword in its scabbard), make a
   `create_character_state` "in a ready fighting stance, weapon drawn" (about 25 generations at 256) and animate that
   state. Otherwise the weapon pops between scabbard and hand at every change of clip.
3. Animate **east only**, every clip the painter picks from (`crates/arena-duel/src/pick.rs`, `CLIPS`):

   `stance-idle walk dash jump duck guard slash overhead cast hit knockdown launched tumble getting-up roll frozen
   shocked meditate win defeat` (20)

   **The smooth standard (owner, 2026-11-17), for every character:**

   - `animate_character` mode **`pixminimax`**, **16 frames** a clip (2 generations at 256), 8 for the short ones
     (`hit`, `guard`, `frozen`, `shocked`; 1 generation), **`keep_first_frame=false`**: PixelLab otherwise keeps the
     rotation as frame 0, and every action opens on 0.1 s of standing still, which reads as lag. About 40
     generations a character.
   - The sheet plays at **20 fps** (`sheet.rs --fps 20`). 5 to 9 frames at 10 fps read as choppy: a pose shorter
     than its clip showed 3 or 4 frames.
   - **Clip to clip.** The painter cuts from one clip to the next without blending, so the clips must meet: every clip
     starts from the stance (it does, being drawn from the rotation); every clip that ends standing (`stance-idle`,
     `walk`, strikes, `cast`, `guard`, `hit`, `getting-up`, `roll`, `duck`, `win`) ends back in the stance, said in its
     text ("then returns to the starting stance"); loops (`stance-idle`, `walk`, `tumble`, `shocked`) end where they
     start; `knockdown` ends lying as `defeat` and `getting-up` begin.
   - The text: "side view facing right, alone, no other figures, no effects"; for a strike, which frames wind up
     (the first half: `pick.rs` plays it by the wind-up's progress) and which land (the second half).
   - Look at every frame. Leave out a frame the generator drew an effect into, and a standing frame at the start of
     `defeat` or `getting-up`; write what was left out in `src/picks.json` and keep the cleaned frames in `src/`.
   - `launched` and `knockdown` start in the air, thrown by a blow: the body is already flying when the painter
     cuts to them (`launched` while it rises, `knockdown` again from its first frame as it falls), so every frame
     still standing at their start is left out, up to the first one where the body is clearly thrown back. Left in,
     the fighter flies off standing upright and only then falls.
   - Where pixminimax fails twice: `v3` (8 frames at most at 256, one generation a frame), with "strictly from the
     side, right-facing profile" or the body turns to the viewer.

   Every clip keeps the character on the same canvas with the feet in the same place, so one feet point serves every
   clip. These are the required core; the
   optional clips, each falling back to a core one, are in `docs/specs/2026-11-17-fighter-clips-design.md`.
4. Export (`get_character` → download) into `art/<character>/src/` and build the sheets with the character's
   `tools/sheet.rs` into `crates/arena-game/assets/fighters/` (built into the game): one layout `<name>.json` and the look's pages, `<name>-<look>.png`, then `<name>-<look>-1.png` and on (each under
   4096 pixels a side). Each character ships one look today: the cultivator Thunder, the Seven-Colored Daoist Frost.
   An effect the generator drew into a clip is taken out there, not left for the page.

## The sheet the game reads

- PNG, RGBA, transparent ground, one row per clip, frames trimmed, on pages. A look's pages share one layout JSON.
- The JSON (`crates/arena-game/src/sheet.rs`, `Layout`): `pages` (or `size` for one page), `fps`, each clip's cells
  `{page, x, y, w, h, dx, dy}`, where `dx, dy` is the cell's top-left measured from the **feet** (centre of the idle
  frame's bottom row), and `portrait`, the head box for the HUD.
- Clip names are the contract with the painter. A sheet with a clip renamed, missing or empty does not load
  (`sheet::parse`, `LayoutError`).
- A weapon or anything else that follows a hand is placed per frame (`cultivator/tools/attach.rs`, `socket.rs`), not
  by eye.

## Frame data and the sprite

The sprite is the rules: where a fighter can be hit and where its blow or bolt comes from are measured from its frames,
not set by hand. `cargo run -p arena-game --example frame_boxes -- crates/arena-duel/src/frames_data.rs` reads every
look's sheet and writes, for every clip of `CLIPS` and every frame, the hurtboxes, the hitbox of a blow, the hand
and each clip's active frame, into the rules' table (`arena_duel::frames`). So a redrawn clip is a
rule change: run it again, re-pin the duel's goldens, measure with the ladder, and bump `arena_link::RULES_VERSION`.
`cargo run -p arena-game --example hitbox_review -- <out dir>` draws review shots of real fights stopped on the
frames that show the boxes. A skill's windup a foe can counter and its recover it can punish are drawn for as long as
the rules keep them open: a change to a skill's `windup` or `recover` re-checks its clip, and a redrawn clip re-checks
the numbers. `crates/arena-duel/tests/frame_sprite.rs` holds it: every wind-up has a frame of its own, and every
skill's timing in each look is pinned beside its clip's frames with two verdicts, `choppy` (a wind-up under 10 frames
a second) and `choppy_recover` (a follow-through under 10), so a change to one side without the other fails it. The
follow-through plays stretched over the rules' recover, as the wind-up does over its windup (`pick`), so the drawn
body is open exactly as long as the fighter is punishable; a clip with few frames after its active one then shows them
slowly, which `choppy_recover` names: those clips want more follow-through frames.

## Spell effects drawn by an image model

An effect is a hybrid of code and sprite (owner, 2026-11-17): the sprite keeps only the drawn shape, a few frames of
it, smooth (no pixel art: the renderer makes any pixels); what code can do (a spin, a flight, a scale in or out, a
fade, a tail) is code. Each effect is a folder `art/vfx/<name>/`: its prompts (one a look, in that look's colours, on
pure black), the pictures generated (`generated/`), the frames keyed from them (`frames/<look>/NN.png`, by
`art/vfx/tools/key.rs`) and `motion.json` (its id, fps, loop, anchor and size, and the motion code lays on it).
`cargo -Zscript art/vfx/tools/vfx.rs --all art/vfx crates/arena-game/assets/drawn` packs every effect into the
game's sheets; `arena_vfx::drawn` reads them and `crates/arena-game/src/vfx/drawn.rs` plays them. A drawn effect
plays in place of its baked recipe.

## Known faults to redraw

- **The Seven-Colored Daoist's (Frost's) `slash`** is drawn backwards: from frame 6 the arm sweeps out behind him while
  his face and body turn to the foe (`assets/fighters/seven-colored-daoist.json`, `slash`), so the blow's hand
  reaches away from what it hits. Mirroring the clip as a stand-in was tried (2026-11-18) and does not read: mirrored,
  the arm reaches the foe but the face and chest turn away from it, and the six stance frames before the swing would
  face away too (mirroring only frames 6-11 flips the head mid-clip). It stays as drawn until it is redrawn facing
  east with the arm sweeping forward (the image key is out of its monthly limit until 2026-12-01); the rules' reach
  box covers the blow meanwhile. Its `cast` reads right: both hands push toward the foe, from frame 10.
- **Totems, minions, traps and mines have no pictures**: the game draws each as a flat block of its owner's colour
  with a life bar over it (`crates/arena-game/src/units.rs`), a pale bar beside the fighters in a fight
  (`review/visual-qa`, seed 2). Each kind wants a drawn sprite, made like a spell's effect (`art/vfx/`), once the
  image key has its limit back (2026-12-01).

## Before a character ships

- [ ] The master through the pixel layer beside the cast at the game's density: same height, outline and shading;
  approved by the owner before any clip is generated.
- [ ] Every clip in `pick::CLIPS` present, 16 frames (8 for the short ones) at 20 fps, feet steady across a clip (no
  sliding while idle).
- [ ] Each clip's first frame is the stance and the clips that end standing end in it: step through one clip into
  the next on the page.
- [ ] No effect glow left in the frames.
- [ ] Each look's sheet: the accent recoloured, nothing else changed.
- [ ] `frame_boxes` run again and its table reviewed (`hitbox_review`): the boxes sit on the body and the blade.
- [ ] A hit frame on the dark stage: the white flash reads.
- [ ] The poses read at the size the page shows them: slash, overhead and cast have three different silhouettes.
- [ ] The sprite fits the body the rules give it (`body_spec` in `data.rs`: height `body`, half width `half_width`).
  A sword drawn longer than the blow's reach makes hits look like they miss, and shorter makes them look like they
  land from afar.
