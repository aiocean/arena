// Where each school's skill icons sit in the icon atlases (site/assets/skills-32.png and skills-24.png, made by
// site/art/skill-icons.sh from the sheet skill-icons.prompt.txt asks for): a square grid read left to right, top row
// first, in the prompt's order. Each school maps names to its own cells; cell 12, a blue bolt, is spare.

/** The atlas's columns, and its rows. */
export const ICON_COLUMNS = 5;

/** Each school's spells to their cell in the atlas. */
export const ICON_CELLS = {
  thunder: { bolt: 0, palm: 1, step: 2, seal: 3, orb: 4, bell: 5, heal: 6, spark: 7, burst: 8, quake: 9, hex: 10, tribulation: 11,
    fireball: 4, gravity: 3, earthsplit: 9, finger: 0, jithunder: 0, gather: 3, ancientgod: 1, slashluo: 2, arsenal: 7, inchearth: 2, onestep: 2, mountain: 9, heartbeat: 8, lightshield: 5, nineturn: 8, indestructible: 6, ancientprotect: 10, vermilion: 4,
    ancientthunder: 3, demonfinger: 0, kunjiwhip: 8, tigerflag: 2,
    fingerdeath: 0, heavenlychop: 1, fireescape: 2, flameorigin: 4, warprint: 8, earthescape: 9, celestialslaughter: 0,
  },
  frost: {
    lotus: 13, shatter: 14, icicle: 15, collapse: 16, frostfire: 17, void: 18, mirror: 19, ward: 20, shard: 21,
    jet: 22, flurry: 23, prison: 24,
    summonrain: 23, summonwind: 22, beans: 18, yinmoon: 16, soulsearch: 18, spiritchange: 21, bodyfix: 14, restriction: 24, plumblossom: 13, lifedeath: 14, karma: 19, dreamback: 20, dreamdao: 18, flowingmoon: 13, remnantnight: 15, heavenflip: 16, thirdeye: 21, sevencolor: 21, umbrella: 17, devour: 16, starfall: 14, seize: 21, yellowspring: 22,
    timerestrict: 24, lifedeathrestrict: 17, soulbanner: 18, servitude: 16, voidnail: 21,
    ghostsummon: 18, soulvortex: 16, soulrefine: 13, starrotation: 14, meritspirit: 20, bodyformation: 13, truefalse: 16,
  },
};
