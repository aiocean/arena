# Seven-Colored Daoist (Thất Thải Đạo Nhân): design (draft)

Status: in the duel (2026-11-17). He fights in the frost seat against Vương Lâm (the cultivator) in thunder's:
art only, no rule changed. The HD look, the 21 core clips and his sheets are made (below).
Art is the arena's own drawing of the book's character (`2026-11-17-xianni-roster-design.md`); the clips follow `2026-11-17-fighter-clips-design.md`.

## From the book

Checked against the novel (chapters 1695-1702, 1749-1768 in translation; Baidu Baike, "Su Dao"):

- **Origin.** Su Dao, master of the Seven Dao Sect, became the Seven-Colored Immortal Venerable and made the Cave
  World with the Boundary Setting Compass. Betrayed by Fan Shanmeng and mortally wounded by Lian Daofei, he split
  into three souls and seven spirits. The Seven-Colored Daoist is the first soul, the strongest, and **inherited all
  of the Venerable's spells**. The second soul is Zhan Laogui, the third Xie Qing (who holds the memories and becomes
  Wang Lin's second disciple). Among the seven spirits are Situ Nan and Immortal Lord Qingshui.
- **Aim.** Devour the other two souls and become the whole Venerable again.
- **Manner.** Ruthless, commanding (he forced the Venerable's concubines to serve him), confident, laughing as he
  finds his foe; he flees only once, after the Li Guang Bow's arrows.
- **Spells.** The **Seven-Colored Dao Lotus**: seven petals, one colour each, closing into a bud around him as a
  shield, its petals also blades ("Seven-Colored Dao Lotus, lotus gathers the soldiers"); Wang Lin's blade broke two
  petals. **Seven layers of light screens.** The **Nine Suns** around his body, his strongest spell. The
  **Seven-Colored Divine Void Nail**, made to kill third-step cultivators.
- **End.** The Li Guang Bow left a fist-sized hole in his chest; Zhan Laogui devoured him.
- **Not in the book:** his face, hair or clothes. The look below is ours.

Vietnamese names other than Vương Lâm, Tư Đồ Nam, Liên Đạo Phi and Tạ Thanh are transliterated from the English and
unchecked against a Vietnamese translation.

## Look

- Tall, slender, upright, chin raised; an ivory and pale silver daoist robe to the floor, a tapering triangle; very
  wide sleeves hiding the hands; a charcoal sash; no weapon.
- **Seven-petal lotus crown** on the topknot, as wide as the head, one colour per petal (red, orange, yellow, green,
  blue, indigo, violet): the mark that must read at any size.
- Hems and cuffs banded in the seven colours, muted; the full colours are kept for his effects.
- A middle-aged man: high cheekbones, narrow cold eyes, thin moustache and goatee, an arrogant set to the mouth.
- Story details for effects, not the sprite: irises cycling the seven colours; a mist-soft edge (he is one soul, not
  a whole being); a lit hole in the chest after a heavy hit.

## Size

HD, as every fighter now (`art/README.md`): a 256x256 PixelLab v3 canvas, the figure about 240 px tall, the same
height as the HD cultivator (`e87e46ec-fb25-4da9-9260-522d28aa2c4c`, 241 px). The 128 states below are the design
master (crown, face, robe), not the game's art; the HD master below was drawn from it. The HD density test
`e2025e8c-a3b0-48b8-a4c6-24e16540d230` (240 px) follows the first draft's prompt, so its crown and face are not his.

## Made

- PixelLab character group `541f9302-ec34-4f10-b152-480d09474d6f`, Pro Flash, 128x128, side view, 8 directions,
  styled on the cultivator's south rotation (outline, detail, shading; not its colours).
  - `Idle` (`f1676955-0b07-46b0-99e3-219650fdd45b`): the first draw. The crown came out a hairpin and the face young.
  - `Idle v2` (`099d4273-7317-4114-bcc0-0fd403233be9`): **the master.** The lotus crown and the man's face fixed,
    robe unchanged. A few light stray pixels in the moustache at 1x; touch up on the east rotation.
- Export: `art/seven-colored-daoist/design-128/` (both states and `metadata.json`).
- **HD master:** v3 group of `8166316c-b5b4-4340-9a35-e2139dbfeb50` (the HD trial below), state `Idle v2`
  (`fe6dba0b-192b-46be-928a-96f164e872b3`): the robe made ivory and the crown's petals brighter. The crown is still a
  ring of petals round the topknot, not a fanned lotus; it reads at 1x. Exported to `art/seven-colored-daoist/src/`
  (`Idle/rotations`, `Idle/animations/<clip>/east`, `metadata.json`), one take a clip.

## HD trial (2026-11-17)

One prompt, three PixelLab routes at 256, judged on the east rotation beside the HD cultivator (241 px tall, 109 wide):

| Route | Id | Height x width | Verdict |
|---|---|---|---|
| `create_character` v3, 9 generations | `8166316c-b5b4-4340-9a35-e2139dbfeb50` | 243 x 110, 46 colours | **Chosen** by the owner (2026-11-17: v3, Pro only if v3 disappoints). Same density, outline and shading as the cultivator (both v3), a man with moustache and goatee, 8 directions. The crown is a ring of petals round the topknot rather than a fanned lotus, and the robe is grey-green rather than ivory. |
| `create_character_pro_flash`, style image the HD cultivator, 17 generations | `9700a824-873d-4eba-ae1a-ef73dddc8b0b` | 243 x 86, 64 colours | Rejected: the face came out young and beardless again, the crown small. The style image holds at 256, but the design drifts. |
| `create_image_pro`, references `Idle v2` and the HD cultivator, 10 generations | job `39fa14db-b966-45a8-bf3c-52024d98c3c1` | 231 x 98, 55 colours | The best design (fanned crown, the face), but a single image in three-quarter view, not a character: no directions, no animation. Kept as the concept to steer the v3 sprite. |

All three are opaque (no part-transparent pixel).

## Core clips (HD, east)

21 clips, 178 frames, all opaque, on the 256 canvas. Which PixelLab animation mode made each, after trying them:

| Mode | Clips | Cost | How it did |
|---|---|---|---|
| `pixminimax` | stance-idle, jump, duck, overhead, thrust, dash, guard, meditate, frozen, shocked, launched, tumble, knockdown, defeat, roll, getting-up, win | 1 generation a clip | **Best and cheapest.** Holds the face, crown and profile; follows the action text. Two first takes failed (duck drew a second fighter flying past; overhead never came down) and were redone with "alone, no other figures, no effects" and frame-by-frame wording. |
| `v3` | walk, slash, cast | 8 a clip (4 at 4 frames) | Good when the text says "strictly from the side, right-facing profile"; without it the body turned to the viewer (cast, idle) and the guard came out holding something. |
| `skeleton-v3`, template `taking-punch` | hit | 3 | A clean recoil. Its `breathing-idle` drew a breath puff and a fighting stance: not used. |

For the next character: **pixminimax first** for every clip, v3 only where pixminimax fails twice.

## Sheets

`site/assets/fighters/seven-colored-daoist.json` and `seven-colored-daoist-frost.png`, `-frost-1.png` (two pages,
173 frames, the figure 242 px tall), built by `art/seven-colored-daoist/tools/sheet.rs`: the cultivator's tool with
`--keep-colors` (his red and violet petals and hems pass its crimson test and would turn to the school's colour),
`--head 64` (30 rows are only his crown) and a `spark` strip (the cyan strip's white-core step ate his white robe).
The page draws each school's own fighter (`site/assets/duel-hero.js`, `FIGHTERS`).

Built from `src/Idle/animations` staged with these frames left out, then `--keep-colors --head 64 --strip
shocked=spark`:

- `defeat/frame_000`, `getting-up/frame_000`: the standing master PixelLab keeps as frame 0; the clip would pop to
  standing (`pick.rs` plays `getting-up` from its first frame as `Sit`).
- `slash/frame_008`, `overhead/frame_007`, `launched/frame_003`: an effect drawn in (a white streak, an orange arc,
  dark smears). Stripping by colour would take the crown's orange petal too.

`jump` crouches, rises and lands, but its highest frame is low; the page's `air` lifts it.

## Smoother clips (2026-11-17)

The first sheet (5-9 frames a clip at 10 fps, each opening on the standing master PixelLab keeps as frame 0) read as
choppy and late: a pose shorter than its clip showed 3-4 frames, and every action began with 0.1 s of standing.
Remade with `pixminimax` at 16 frames (8 for `hit`, `guard`, `frozen`; 2 generations a 16-frame clip) and
`keep_first_frame=false`, played at **20 fps** (`--fps 20`; a layout's fps is its own, so the cultivator is unchanged).
`src/picks.json` lists which take each clip came from and the frames left out (generator effects: `slash` 12-15,
`overhead` 6, `cast` 6-7, `hit` 0-1; standing frames: `defeat` 0, `getting-up` 0-1). `shocked` stays the 9-frame take
(the 8-frame one drew yellow lightning). 293 frames; the sheet tool now writes sheets of at most 256 colours as
indexed PNGs, 1.2 MB instead of 6.2 MB, pixel for pixel the same.

## Clips

He fights unarmed and by spell, never with fist or foot: an arrogant daoist does not kick. That sets him apart from
the cultivator. On top of the core and tier 1 of the clips spec:

| Clip | What he does | His skill |
|---|---|---|
| `slash`, `overhead`, `thrust` | a sweep of the sleeve, a sleeve brought down, a palm thrust | core blade clips, drawn as sleeve blows |
| `cast-point` | a finger pointed; a nail of void leaves it | `voidnail` |
| `dash` / `signature-sevencolor` | he streaks through the foe as a spear of seven colours | `sevencolor` |
| `guard` / `signature-lotus` | the seven petals close into a bud around him; a held blow loses a petal | the Dao Lotus |
| `ultimate` | arms raised; nine suns light one by one around him | the Nine Suns |
| `win` | the lotus opens above his head; he looks down on the foe | |
| `defeat` | a hole of light in his chest; he fades to mist (code) | the Li Guang Bow, his end |

The Dao Lotus as a guard and the Nine Suns as an ultimate are new skills: a rule change, the owner's call
(`CLAUDE.md`, "Traps").

## Next

1. Check a hit frame in a real fight: his white robe against the white hit flash (`art/README.md`, "Colour of a
   character").
2. Tier 1 of the clips spec (pixminimax): `walk-back`, `fly`, `cast-summon`, `cast-palm`, `cast-prayer`,
   `cast-cross`, `hit-light`, `hit-heavy`, `guard-hit`.
3. His signature clips: `cast-point` (`voidnail`; `thrust` already points a finger), the lotus guard, the Nine Suns.
