//! The HUD's little bitmaps (from the page's `duel-art.js` and `skill-icons.js`): digits, buff icons, trait marks,
//! the letters of the words that float over the skill bar, and where each skill's icon sits in the icon atlases.

use pixfx_duel::data::{School, Trait};

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
    Some(match id {
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
        "attune" => (
            "#c89bff",
            [
                "...#...", "..#.#..", ".#.#.#.", "#.###.#", ".#.#.#.", "..#.#..", "...#...",
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

/// A trait's mark in the corner of its icon, 5 by 5, and its colour: swift chevrons, a heavy weight, a piercing
/// arrow, a lingering drop, a cheap gem.
pub fn trait_mark(t: Trait) -> ([&'static str; 5], &'static str) {
    match t {
        Trait::Swift => (["#..#.", ".#..#", "..#..", ".#..#", "#..#."], "#8dffa8"),
        Trait::Heavy => ([".###.", "..#..", "#####", "#####", "#####"], "#ff9a3a"),
        Trait::Piercing => (["..#..", ".###.", "#.#.#", "..#..", "..#.."], "#ffffff"),
        Trait::Lingering => (["..#..", ".###.", "#####", "#####", ".###."], "#c89bff"),
        Trait::Cheap => (["..#..", ".#.#.", "#...#", ".#.#.", "..#.."], "#ffd84a"),
    }
}

/// The capitals of the words that float over the skill bar (DOUBLE, FREE, AGAIN, RARE, EPIC, LEGENDARY), 5 tall.
pub fn letter(c: char) -> Option<[&'static str; 5]> {
    Some(match c {
        'A' => [".##.", "#..#", "####", "#..#", "#..#"],
        'B' => ["###.", "#..#", "###.", "#..#", "###."],
        'C' => [".###", "#...", "#...", "#...", ".###"],
        'D' => ["###.", "#..#", "#..#", "#..#", "###."],
        'E' => ["####", "#...", "###.", "#...", "####"],
        'F' => ["####", "#...", "###.", "#...", "#..."],
        'G' => [".###", "#...", "#.##", "#..#", ".###"],
        'I' => ["###", ".#.", ".#.", ".#.", "###"],
        'L' => ["#...", "#...", "#...", "#...", "####"],
        'N' => ["#...#", "##..#", "#.#.#", "#..##", "#...#"],
        'O' => [".##.", "#..#", "#..#", "#..#", ".##."],
        'P' => ["###.", "#..#", "###.", "#...", "#..."],
        'R' => ["###.", "#..#", "###.", "#.#.", "#..#"],
        'U' => ["#..#", "#..#", "#..#", "#..#", ".##."],
        'Y' => ["#...#", ".#.#.", "..#..", "..#..", "..#.."],
        _ => return None,
    })
}

/// The icon atlases' columns: a square grid read left to right, top row first.
pub const ICON_COLUMNS: u32 = 5;

/// Where skill `name` of `school` sits in the icon atlases (cell 12, a blue bolt, is spare).
pub fn icon_cell(school: School, name: &str) -> Option<u32> {
    Some(match school {
        School::Thunder => match name {
            "bolt" => 0,
            "palm" => 1,
            "step" => 2,
            "seal" => 3,
            "orb" => 4,
            "bell" => 5,
            "heal" => 6,
            "spark" => 7,
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
            "slashluo" => 2,
            "arsenal" => 7,
            "inchearth" => 2,
            "onestep" => 2,
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
            "fireescape" => 2,
            "flameorigin" => 4,
            "warprint" => 8,
            "earthescape" => 9,
            "celestialslaughter" => 0,
            "ancientcauldron" => 3,
            "eightextremes" => 9,
            "daofusion" => 8,
            _ => return None,
        },
        School::Frost => match name {
            "lotus" => 13,
            "shatter" => 14,
            "icicle" => 15,
            "collapse" => 16,
            "frostfire" => 17,
            "void" => 18,
            "mirror" => 19,
            "ward" => 20,
            "shard" => 21,
            "jet" => 22,
            "flurry" => 23,
            "prison" => 24,
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
            "sevencolor" => 21,
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
        },
    })
}
