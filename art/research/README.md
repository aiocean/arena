# Art research

What was looked into before the art direction (`art/README.md`) was settled, kept so the choice need not be made
again from nothing. Dated 2026-11-17; the market of image models moves fast, so the generator lists age quickly.

## 1. Pixel density: how big a fighter is drawn

The owner wanted a pixel level "not too low", because a small sprite cannot express anything. Three levels were
compared on our own PixelLab characters, then a fourth, HD, was generated to see it.

| Level | Figure height | Seen in | What it can show |
|---|---|---|---|
| 16-bit | 16-48 px | Stardew Valley, Celeste, Undertale, Terraria, Hyper Light Drifter, Sea of Stars | an iconic shape; a face is a dot or two. Good for exploration games, where a figure is 1/8 to 1/6 of the screen |
| Mid | about 60 px | the arena before 2026-11-17 (128 sprites halved) | a readable pose, but the face is a few dots and trims turn to noise |
| Classic fighter | about 120 px | Street Fighter III: Third Strike, Garou, Metal Slug, Darkstalkers (screen about 384x224, figure half its height) | face, hairpin, sash and hem; poses read at once |
| HD | about 240 px | King of Fighters XIII | eyes, brows, lips, locks of hair, sleeve folds, each stripe of a trim: a face can act. Close to illustration, and the dearest to animate |

**Chosen: HD** (owner, 2026-11-17, after seeing the pictures below). What it costs: a picture about 500 px tall for a
fighter at 45% of it, so the stage, effects and UI are drawn at that density too; the rules' lengths at `UNIT` 4
(arena-dc, with a retrain); sheets split into pages under 4096 px a side; on PixelLab only `v3` reaches 256 and there is
no style lock there.

What players praise as "good pixel art" is less the size than: one density, one palette and one outline across every
asset; animation with weight (anticipation, hit stop, recovery); colour with intent (the fighters stand off the
ground, each side its own colours); silhouettes that tell each move apart.

### The pictures (`density/`)

| File | Shows |
|---|---|
| `compare.png` | the same Thất Thải Đạo Nhân at about 30, 60 and 120 px, enlarged to one height: what each level keeps |
| `tiers_same_scale.png` | a 32 px farmer drawn at that size, the Daoist at 60 and at 120 px, all at x3: their real sizes side by side |
| `hd_same_scale.png` | the 128 Daoist beside the 256 HD test (side and front), at x2 |
| `hd_head.png` | the head at 120 px and at HD, enlarged to one size: the detail HD adds |
| `hd_pair.png` | the two HD references side by side, 240 and 241 px |

The 30 and 60 px pictures were made from the 128 sprite the way the old `sheet.rs` halved it (each 2x2 block the colour
most of its opaque pixels share, the darker on a tie). A sprite drawn small from the start (the farmer) reads better
than one shrunk to that size.

PixelLab characters used: Thất Thải Đạo Nhân draft `f1676955-…` (128), its HD density test `e2025e8c-…` (256, made from
the first draft's prompt, so a woman; the Daoist is a man), Tu Sĩ Hắc Kiếm `29a1ef98-…` (128) and HD `e87e46ec-…`
(256), the farmer `fedaaeac-…` (48, top-down), the warrior `8ca8df71-…` (64, top-down).

## 2. Generators other than PixelLab (hosted)

| Tool | Good at | For the arena |
|---|---|---|
| **PixelLab** (in use) | characters with rotations and animations on a skeleton, so a figure holds across frames; up to 256 (`v3`) | the fighters and their clips |
| **Retro Diffusion** (RD Plus, RD Tile, RD Animation) | clean on-grid pixels, many styles (icons, items, tiles, isometric, turnarounds); RD Plus 64-384 px; about $0.015-0.18 an image, animations $0.07-0.25; API, MCP, also on Scenario | skill icons, UI, tiles, small effects. Its animation is mostly walk, run and idle cycles; 20 consistent fighting clips at 256 not shown |
| **Scenario** | training a model on one's own style; hosts Retro Diffusion and Pixel Snapper (snaps to a grid) | a style lock once the roster is large (PixelLab has none at 256) |
| AutoSprite, Sprite AI, God Mode AI, SpriteCook | animation first: a sheet and a GIF from one image | worth a side-by-side test; HD detail and consistency at 256 unproven |
| Ludo.ai, Layer.ai | every kind of game asset, studio batches | not pixel specialists |
| GPT Image, Gemini (Nano Banana), Flux (through OpenRouter, as game-factory's `concept-artist` does) | concept art, HD portraits, backgrounds | portraits for character select and the HUD, stage backgrounds; never used raw (below) |

## 3. Open models

Still pictures, a base model plus a pixel-art LoRA:

| Model | Base | Notes |
|---|---|---|
| [Pixel Art Sprite LoRA](https://huggingface.co/limbicnation/pixel-art-lora) | FLUX.2-klein-4B | the newest; character sprites on a transparent ground. Recommended pairing in 2026 for sprites, icons, portraits |
| [PIXEL ART REDMOND](https://huggingface.co/artificialguybr/PIXELART-REDMOND-QWENIMAGE) | Qwen-Image-2512 | follows prompts well; heavy |
| [Modern Pixel Art](https://huggingface.co/UmeAiRT/FLUX.1-dev-LoRA-Modern_Pixel_art), [Retro Pixel](https://huggingface.co/prithivMLmods/Retro-Pixel-Flux-LoRA), [pixelart Flux](https://huggingface.co/deAPI-ai/pixelartdotfluxlora/blob/main/README.md) | FLUX.1-dev | "modern pixel" suits HD; FLUX.1-dev's licence limits commercial use, check before shipping |
| [Pixel Art XL](https://www.promptlayer.com/models/pixel-art-xl), [pixel-icon SDXL](https://huggingface.co/lizozom/pixel-icon-lora-sdxl) | SDXL | older and light (about 8 steps with LCM); the icon one for skill icons |

Sheets and animation:

- [pixel_spritesheet_4walk_combat_32x48](https://huggingface.co/Leon1000/pixel_spritesheet_4walk_combat_32x48_v1),
  [4walk_small](https://huggingface.co/svntax-dev/pixel_spritesheet_4walk_small_lora_v1/blob/main/README.md): 32x48
  walk and combat sheets, far too small for HD.
- [Sprite Sheet Diffusion](https://arxiv.org/html/2412.03685v2): research on holding a character across frames; the
  right direction, not a tool yet.
- [sprite-sheet-creator](https://github.com/blendi-remade/sprite-sheet-creator): open code, but calls models on fal.ai.

No open model was found that keeps one fighter consistent across 20 clips the way PixelLab does.

**Every model outside PixelLab and Retro Diffusion draws pixel-looking pictures, not pixel art**: off the grid, with
blended colours. Their output is snapped to a grid and reduced to the palette before use (the engine's `pixfx-pixel`,
Pixel Snapper, or PixelLab's `reduce_colors` and `image_to_pixelart`).

### Running them here

The laptop is an M4 with 16 GB and about 24 GB of free disk (2026-11-17). SDXL with Pixel Art XL (about 7 GB) runs;
FLUX.2-klein-4B with its LoRA (8-10 GB) probably runs, tight on memory; Qwen-Image and FLUX.1-dev belong on the
desktop or a rented GPU. Not tried yet. The proposed first test: the Tu Sĩ Hắc Kiếm prompt through FLUX.2-klein-4B with
the sprite LoRA, four pictures, snapped to the grid, beside the PixelLab HD master.

## Sources

- [Ludo – best AI sprite generators](https://ludo.ai/compare/best-ai-sprite-generators)
- [Cinevva – Best AI Pixel Art and Sprite Generators 2026](https://app.cinevva.com/guides/ai-pixel-art-generators)
- [LlamaGen – best AI pixel art generators 2026](https://llamagen.ai/en-au/articles/best-ai-pixel-art-generators-2026)
- [Scenario – Retro Diffusion Models, The Essentials](https://help.scenario.com/en/articles/retro-diffusion-models-the-essentials/)
- [Retro Diffusion API examples](https://github.com/Retro-Diffusion/api-examples)
- [Retro Diffusion on Replicate](https://replicate.com/retro-diffusion/rd-plus/versions/8fa3491557e024ac62c4f3c0918774c54b948c25f6388c5741862dd4b523e311/api)
- [Choosing a model (aigaming)](https://aigaming.nuilab.org/models/choosing/)
- the Hugging Face and arXiv pages linked in section 3
