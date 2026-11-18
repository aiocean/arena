//! The HUD's little bitmaps (from the page's `duel-art.js` and `skill-icons.js`): digits, buff icons, trait marks,
//! the letters of the words that float over the skill bar, and where each skill's icon sits in the icon atlases.

/// Digits 0 to 9, 3 by 5: the seconds an empty slot still waits, a buff's stacks and the rounds won.
pub const DIGIT: [[&str; 5]; 10] = [
    ["###", "#.#", "#.#", "#.#", "###"],
    [".#.", "##.", ".#.", ".#.", "###"],
    ["##.", "..#", ".#.", "#..", "###"],
    ["##.", "..#", ".#.", "..#", "##."],
    ["#.#", "#.#", "###", "..#", "..#"],
    ["###", "#..", "##.", "..#", "##."],
    [".##", "#..", "###", "#.#", "###"],
    ["###", "..#", ".#.", ".#.", ".#."],
    ["###", "#.#", "###", "#.#", "###"],
    ["###", "#.#", "###", "..#", "##."],
];

/// Each buff's 7 by 7 icon and colour, in the tile under the bars.
pub fn buff(id: &str) -> Option<(&'static str, [&'static str; 7])> {
    Some(match alias(id) {
        "fury" => (
            "#ffc23d",
            [
                "...#...", "..###..", "..###..", "..###..", ".#####.", "...#...", "..###..",
            ],
        ),
        "unyielding" => (
            "#ff6a5a",
            [
                ".##.##.", "#######", "#######", ".#####.", "..###..", "...#...", ".......",
            ],
        ),
        "ward" => (
            "#a6ecff",
            [
                "#######", "#.....#", "#.###.#", "#.###.#", ".#.#.#.", "..#.#..", "...#...",
            ],
        ),
        "barrier" => (
            "#f0cf7a",
            [
                "..###..", ".#...#.", "#.....#", "#.....#", "#.....#", "#######", ".......",
            ],
        ),
        "grit" => (
            "#b8893c",
            [
                "..###..", ".#####.", "##.####", "#######", "####.##", ".#####.", ".......",
            ],
        ),
        "burn" => (
            "#ff9a3a",
            [
                "...#...", "..##...", "..###..", ".####.#", ".######", "#######", ".#####.",
            ],
        ),
        "chill" => (
            "#bff2ff",
            [
                "...#...", ".#.#.#.", "..###..", "#######", "..###..", ".#.#.#.", "...#...",
            ],
        ),
        "shock" => (
            "#ffe14a",
            [
                "....##.", "...##..", "..##...", ".#####.", "...##..", "..##...", ".##....",
            ],
        ),
        "frozen" => (
            "#a6ecff",
            [
                "#######", "#.#...#", "#..#..#", "#...#.#", "#.#...#", "#..#..#", "#######",
            ],
        ),
        "defDown" => (
            "#8f8aa8",
            [
                ".##.##.", "#..#..#", "#..#..#", "#.#...#", ".#.#.#.", "..#.#..", "...#...",
            ],
        ),
        "silence" => (
            "#c9c3e6",
            [
                "#.....#", ".#...#.", "..#.#..", "...#...", "..#.#..", ".#...#.", "#.....#",
            ],
        ),
        "sealed" => (
            "#c9c3e6",
            [
                "..###..", ".#...#.", ".#...#.", "#######", "###.###", "###.###", "#######",
            ],
        ),
        _ => return None,
    })
}

/// The icon atlases' columns: a square grid read left to right, top row first.
pub const ICON_COLUMNS: u32 = 5;

/// The icon a buff id of the damage system shows as: its ailments and controls drawn with the old statuses' art
/// nearest them (a jolt as a shock, a soak as a chill, a sunder as broken defence, a bind or a root as sealed feet,
/// a daze, heavy or dread as a hold, a bleed or a wither as a burn, a glare as the barrier's light, souls and curses
/// as grit and silence).
fn alias(id: &str) -> &str {
    match id {
        "jolt" => "shock",
        "soak" => "chill",
        "sunder" => "defDown",
        "bind" | "root" => "sealed",
        "daze" | "heavy" | "dread" => "frozen",
        "bleed" | "wither" => "burn",
        "glare" => "barrier",
        "souls" => "grit",
        "cursed" => "silence",
        other => other,
    }
}

/// Where skill `name` sits in the icon atlas (cell 12, a blue bolt, is spare): its own cell, else the cell of the
/// motion it borrows (`arena_calc::gem::Gem::spell`), so every skill of the shared pool shows.
pub fn icon_cell(name: &str) -> Option<u32> {
    cell_of(name).or_else(|| cell_of(arena_calc::gem::gem(name)?.spell))
}

/// The atlas's own cell for `name`: the skills drawn for the two old schools.
fn cell_of(name: &str) -> Option<u32> {
    Some(match name {
        "bolt" => 0,
        "palm" => 1,
        "seal" => 3,
        "orb" => 4,
        "bell" => 5,
        "heal" => 6,
        "spark" => 7,
        // the basics: the qi bolt shows as the spark it is drawn as, the qi burst as the burst
        "qibolt" => 7,
        "qiburst" => 8,
        "burst" => 8,
        "quake" => 9,
        "hex" => 10,
        "tribulation" => 11,
        "fireball" => 4,
        "gravity" => 3,
        "earthsplit" => 9,
        "finger" => 0,
        "jithunder" => 0,
        "gather" => 3,
        "ancientgod" => 1,
        "arsenal" => 7,
        "inchearth" => 2,
        "mountain" => 9,
        "heartbeat" => 8,
        "lightshield" => 5,
        "nineturn" => 8,
        "indestructible" => 6,
        "ancientprotect" => 10,
        "vermilion" => 4,
        "ancientthunder" => 3,
        "demonfinger" => 0,
        "kunjiwhip" => 8,
        "tigerflag" => 2,
        "fingerdeath" => 0,
        "heavenlychop" => 1,
        "flameorigin" => 4,
        "warprint" => 8,
        "earthescape" => 9,
        "celestialslaughter" => 0,
        "ancientcauldron" => 3,
        "eightextremes" => 9,
        "daofusion" => 8,
        "lotus" => 13,
        "collapse" => 16,
        "void" => 18,
        "mirror" => 19,
        "ward" => 20,
        "jet" => 22,
        "summonrain" => 23,
        "summonwind" => 22,
        "beans" => 18,
        "yinmoon" => 16,
        "soulsearch" => 18,
        "spiritchange" => 21,
        "bodyfix" => 14,
        "restriction" => 24,
        "plumblossom" => 13,
        "lifedeath" => 14,
        "karma" => 19,
        "dreamback" => 20,
        "dreamdao" => 18,
        "flowingmoon" => 13,
        "remnantnight" => 15,
        "heavenflip" => 16,
        "thirdeye" => 21,
        "umbrella" => 17,
        "devour" => 16,
        "starfall" => 14,
        "seize" => 21,
        "yellowspring" => 22,
        "timerestrict" => 24,
        "lifedeathrestrict" => 17,
        "soulbanner" => 18,
        "servitude" => 16,
        "voidnail" => 21,
        "ghostsummon" => 18,
        "soulvortex" => 16,
        "soulrefine" => 13,
        "starrotation" => 14,
        "meritspirit" => 20,
        "bodyformation" => 13,
        "truefalse" => 16,
        "daoyellow" => 17,
        "springsoul" => 21,
        "starsoul" => 18,
        "spiritsummon" => 16,
        "starcompass" => 22,
        _ => return None,
    })
}
