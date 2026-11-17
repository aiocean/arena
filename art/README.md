# Art direction

How every fighter in the arena is drawn, so a new one stands beside the others without looking pasted in. The arena
is **HD pixel art** (decided 2026-11-17): the detail of the King of Fighters XIII sprites, every pixel still placed on
one grid. The references are two PixelLab characters drawn at 256, side by side in style and height (240 and 241 px):

- **Tu Sĩ Hắc Kiếm HD** (`e87e46ec-fb25-4da9-9260-522d28aa2c4c`), the cultivator redrawn in HD;
- **Thất Thải Đạo Nhân HD test** (`e2025e8c-a3b0-48b8-a4c6-24e16540d230`), a density test. It follows the Daoist's
  first draft, so for his design (crown, face) the master is still the 128 px `Idle v2` state (`099d4273-…`,
  `art/seven-colored-daoist`, `docs/specs/2026-11-17-seven-colored-daoist-design.md`); he is redrawn in HD from it.

A new character is checked against them, at the size the game shows it, before any animation is made. The 128 px
characters (`29a1ef98-…` in `cultivator/src`, the Daoist's 128 states) are the old standard: half the detail, used
for design only, never mixed with HD fighters in one duel.

This file is for the people and agents who make the art. The player docs are `docs/` (published on the site).
Why HD, the pictures that decided it, and the other generators looked at: `art/research/README.md`.

## Pixel density: one art pixel is one screen pixel of the fighter

- A fighter is drawn on a **256 x 256** PixelLab canvas, side view, and its body is about **240 px tall** and 110 to
  130 px wide facing east. This is the detail level of the reference: eyes, brows and lips, single locks of hair,
  folds in a sleeve and each stripe of a trim are drawn, so a face can show what the fighter feels.
- The screen is made for it: a fighter stands about 45% of the picture's height, so the duel's picture is about
  **500 px tall** before the page enlarges it.
- The game shows those pixels **as drawn**: never halved, never scaled by a fraction, never rotated or smoothed. The
  page enlarges the whole picture by a whole number (`image-rendering: pixelated`, `imageSmoothingEnabled = false`).
- Every fighter in one duel has the same density. A smaller or larger character is drawn smaller or larger on the same
  grid (a child on a smaller body, a beast on a bigger canvas), never resized from another size.
- Nothing on the way from PixelLab to the screen changes the pixels' size: the sheet keeps every frame as drawn.

## Style

| | Rule | In the references |
|---|---|---|
| Outline | One near-black outline round the silhouette, 1 px, with no gaps; inner lines a darker shade of the area, not black | near-black edge, as in both 128 px characters (`#05060a`, `#000000`) |
| Shading | Soft cel shading: 3 to 5 steps a material, shadows cooler and darker, highlights warmer; no dithering, no gradients | white robe: off-white, pale grey, blue-grey |
| Light | From the front and above, the same in every frame; the fighter is mirrored for the west side, light included | |
| Colours | At most 64 a character, all opaque: every pixel has alpha 0 or 255 | 64, no part-transparent pixels |
| Detail | High: face, hair strands, folds and trims drawn, but the silhouette stays one readable shape at 1x | |
| Proportions | Slender wuxia figures, about 6.5 heads tall, long robes; feet visible or the hem ending at the floor | |

### Colour of a character

- One **main colour** (the robe) and one **accent** (a lining, a sash, a trim). The two fighters of a duel must not share
  a main colour, and each must stand out against the stage's dark night ground.
- The **school's colour** is the accent: each school's sheet turns the accent (the cultivator's crimson lining) into
  the school's glow (thunder `#ffe066`, frost `#a6ecff`). A new character keeps its accent in one hue family, used
  nowhere else on the body, so that recolour hits it and nothing else.
- **White is the hit flash**: the painter draws a struck fighter all white. A character mostly in white (Thất Thải Đạo
  Nhân) needs its dark hair, sash and outline to carry the flash, or a flash that is not white. Check a hit frame.
- Effects (qi, fire, lightning) are the game's particles (`crates/arena-game/src/vfx`) and take their colours from the effect palettes in
  `crates/pixfx-duel/src/data.rs` (`Tint`). The character's own colours are never reused for an effect's glow.

## Making a character on PixelLab

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
3. Animate **east only**, every clip the painter picks from (`crates/arena-fighter`, `pick::CLIPS`):

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
   `tools/sheet.rs` into `crates/arena-game/assets/fighters/` (built into the game): one layout `<name>.json` and one `<name>-<school>.png` per school.
   An effect the generator drew into a clip is taken out there, not left for the page.

## The sheet the game reads

- PNG, RGBA, transparent ground, one row per clip, frames trimmed. Every school's PNG shares one layout JSON.
- The JSON (`crates/arena-fighter`, `Layout`): `size`, `fps`, each clip's cells `{x, y, w, h, dx, dy}`, where `dx, dy`
  is the cell's top-left measured from the **feet** (centre of the idle frame's bottom row), and `portrait`, the head box
  for the HUD.
- Clip names are the contract with the painter. A sheet with a clip renamed, missing or empty does not load
  (`Sheet::new`, `LayoutError`).
- A weapon or anything else that follows a hand is placed per frame (`cultivator/tools/attach.rs`, `socket.rs`), not
  by eye.

## Before a character ships

- [ ] East rotation beside the references at 1x: same height (±12 px), outline and shading.
- [ ] Every clip in `pick::CLIPS` present, 16 frames (8 for the short ones) at 20 fps, feet steady across a clip (no
  sliding while idle).
- [ ] Each clip's first frame is the stance and the clips that end standing end in it: step through one clip into
  the next on the page.
- [ ] No pixel with alpha other than 0 or 255; at most 64 colours.
- [ ] No effect glow left in the frames.
- [ ] Each school's sheet: the accent recoloured, nothing else changed.
- [ ] A hit frame on the dark stage: the white flash reads.
- [ ] The poses read at the size the page shows them: slash, overhead and cast have three different silhouettes.
- [ ] The sprite fits the body the rules give it (`body_spec` in `data.rs`: height `body`, half width `half_width`).
  A sword drawn longer than the blow's reach makes hits look like they miss, and shorter makes them look like they
  land from afar.
