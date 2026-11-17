# Tiên Nghịch roster: design (draft)

Status: draft, not built. No character beyond Thunder and Frost exists yet. Revised 2026-11-17: the cast is drawn on
PixelLab in HD pixel art to the art direction (`art/README.md`), not by the engine's 64 px painter (`pixfx-fighter`,
`SCALE` 1.6).

## Intent

Every main character of Tiên Nghịch (Renegade Immortal) is a PixelLab fighter as detailed as the references in
`art/README.md`. Names, faces, outfits and palettes are the arena's own drawings of the book's characters (see the
pixfx decision: mechanics may follow the datamine; art is original).

## Size tiers

All on the same pixel grid. 256 is PixelLab's largest canvas, so there is one tier; a beast or a boss bigger than a
fighter is drawn in parts or by other means, never by enlarging a sprite.

| Tier | Canvas | Body height | Used for |
|---|---|---|---|
| Standard | 256 | about 240 | every fighter of the cast |

Elders and sect heads are Standard with heavier robes, a staff or a headdress, not a taller canvas, so any two of the
cast can meet in a duel.

## Cast (names to confirm against the book before any art is made)

MEDIUM confidence, from memory of the novel, not yet checked against a source:

- Vương Lâm (Wang Lin), the protagonist
- Lý Mộ Uyển (Li Muwan)
- Tư Đồ Nam (Situ Nan)
- Hồng Điệp (Hong Die)
- Lưu Mi (Liu Mei)
- Thiên Vận Tử (Tian Yun Zi)

Checked against a source (see each character's spec):

- Thất Thải Đạo Nhân (the Seven-Colored Daoist), first soul of the Seven-Colored Immortal Venerable: in the duel
  (frost's seat, art only), `2026-11-17-seven-colored-daoist-design.md`. Vương Lâm is the cultivator in thunder's.

The full list comes from a source pass (wiki or the book's character index) before the roster is final; names not confirmed there are dropped, never guessed.

## Open decisions (owner)

1. Art roster only, Thunder/Frost replaced by two cast members, or a full playable cast (rules, retrain, new season). Unanswered.
2. Whether the duel's bodies and reaches change with the full-density sprites (a rules change: goldens, retrain, new
   season), or only the view does.
