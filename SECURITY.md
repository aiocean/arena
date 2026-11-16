# Reporting a problem privately

Some bugs are worth more to a player than to everyone: a rule a bot can exploit, a fight that replays differently on
another machine, a weights file that crashes or stalls the loader or the runner. Report those privately, through
[a security advisory](https://github.com/aiocean/arena/security/advisories/new), not an issue, so no bot is trained on
them before they are fixed.

Say what is needed to see it happen: the season tag (or commit), the seed, the two bots (weights files or built-in
names), and what happens against what you expected.

A fix that changes the rules waits for the next season once a season has started (`docs/tournament.md`); the
maintainers decide whether a bug is serious enough to stop a season.
