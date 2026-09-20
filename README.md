# catalogames

Track game bundle deals and sort out your games library.

Everything is a library function. The `catalogames` binary is a thin shell over
the crate, so an embedding project can reach the whole feature set.

## Requirements

- Rust 1.85 or newer (2024 edition). No system libraries: TLS is rustls, so
  there is no OpenSSL to install.

## Using the command line

List the bundles a store is currently selling, each with the games in its
largest tier:

```console
$ cargo run -- sales humblebundle
Beyond the Metroidverse Bundle  ( $10.00 )  [ ends 2026-09-17 04:00 UTC · 8h left ]
  - Salt and Sanctuary    Very Positive 21898 | Very Positive 73   https://store.steampowered.com/app/283640
  - HAAK                  Very Positive 4102 | -                   https://store.steampowered.com/app/1352930
  - Godot 4 Shaders Course                                         https://store.steampowered.com/search/?term=Godot%204%20Shaders%20Course
  ...
```

Each game is matched against [the inventory](#the-inventories). A game found
there shows its all-time and last-30-days verdicts, links to its own store page,
and carries a second indented line with the rest of what is known — release
date, platforms, Steam Deck rating and its most-voted tags. A game that is not
found shows no verdicts and no detail line, and falls back to a search link,
because inventing either would be worse than leaving them out. `-` in the recent
column means Steam published no recent row at all — not a bad score.

```console
  - Starship Troopers: Terran Command   Very Positive 12032 | Very Positive 34   .../app/1202130
    └ Jun 16, 2022 · Windows · Deck playable · Strategy, RTS, Singleplayer, Military, Tactical
```

Written to a terminal, the verdicts are coloured by band and the detail line is grey so the
games themselves carry the eye down the list. Piped anywhere else the output is plain — the
process decides, not the library, because escape codes written into a file are noise in someone
else's data.

Columns are aligned by
[`table_formatter`](https://github.com/SirHexa10t/table_formatter), which measures width in
terminal *cells*. That matters for more titles than it sounds: a CJK glyph is one character and
two cells wide, so counting characters silently pushes a row like `白猫骑士物语/ White Cat Knight`
two cells out of line. Colour codes are excluded from the measurement for the same reason —
they occupy no cells at all.

Every store's games print through the same `render::Preview`, so a game looks
identical whether it was found in a Humble bundle, a Fanatical one, or looked up
directly — and a change to what a line says is made in one place.

Name no store and every store is listed, each under its own heading:

```console
$ cargo run -- sales
[humblebundle.com]

CRPG Pack: Isometric Immersion  ( $23.00 )  [ ends 2026-09-24 04:00 UTC · 7d left ]
  ...

[fanatical.com]

Prestige Collection - Build your Own Bundle (Fall 2026)  ( $7.00/game at 5+ )  [ ends 2026-10-16 07:00 UTC · 29d left ]
  ...
```

Stores are listed one after another rather than merged. Keeping them apart is
what lets a problem be reported under the store it came from, and what stops one
unreachable store from being mistaken for the whole run failing. The heading
appears only when more than one store is listed, so naming a single store prints
exactly what it always did.

### What a bundle costs

The price sits in parentheses after the title, and the deadline in brackets
after that. Both carry a space inside the mark, because tight against a digit a
bracket reads as a leading `1` — `[8h` looks like eighteen hours.

**The two stores are priced differently, and the difference is real rather than
cosmetic.** A Humble tier is a *cumulative partition*: paying the largest tier's
price buys all of its games together, which is the set this crate lists, so the
figure shown is the least those games can be bought for. Dividing it by the game
count would produce an average nobody can pay, because no single game in a
Humble bundle is for sale on its own.

A Fanatical pick-and-mix tier is a *price point over one shared pool*: every
tier draws from all of it, and paying for twenty-five picks really does cost the
stated amount per pick. So that one is shown as a rate, taken at the tier that
gives the most games, which is where the rate is lowest.

The rate names the count it holds from — `$0.95/game at 25+` — because the rate
alone hides the commitment: ninety-five cents a game is a different offer at
five games than at twenty-five. It also does not multiply back to the total,
being rounded to the cent, so the count is what lets a reader work out the real
outlay.

Which shape applies is not a question of which store it is. It is whether the
best tier takes the whole pool: everything for one payment is a whole-bundle
price, a selection out of a larger pool is a rate. Fanatical's own catalogue has
bundles of both kinds, and counting a bundle's tiers to decide would get the
second kind wrong.

A bundle with no published price shows none, which means the store published
none rather than that the bundle is free.

### A product that is really several games

Stores sometimes list a marketing wrapper rather than a product. "Lazy Otter
Double Pack" has no store page, no Steam id, and names no base game, so
searching for the phrase finds nothing — the link this used to print went
nowhere. What it actually delivers is two ordinary games, and the store's own
record says which ones, in the list its page prints under "This bundle
includes:".

Those are looked up in turn, so the pack is named once and the games it holds
are listed beneath it, each with its own reviews and its own store page:

```console
  - Moonscars                Very Positive 1513 | -  https://store.steampowered.com/app/1374970
  - Lazy Otter Double Pack
  --> Slots & Diapers        -                       https://store.steampowered.com/app/4409870
  --> Idle Chapel            -                       https://store.steampowered.com/app/4102010
```

The pack keeps its name, because that is what the bundle sells and what appears
in a cart, and it gets no link, because there is nothing to link to.

Nesting is marked by the bullet, and the bullet is joined to the title by a
**single space** so that it stays part of the title's cell. Two spaces are how
this table separates columns, so a bullet written with two becomes a column of
its own, is padded to the width of the widest bullet, and shifts every title
across — the pack's own included. With one space the dashes are never a column,
and the arrow's two extra characters are exactly the offset that shows the
nesting.

A bundle can also list itself among its own contents. One collection names four
products, the first of which is the collection. That self-reference is dropped
before anything is counted or printed, or the bundle would appear as a game
inside itself, with its own link, one line under its own title.

Two things this deliberately does *not* do. It never flattens the games up to
the top level — a pack stays one entry, so the bundle's own "the store said 19,
we found 19" check keeps working, and the games keep their real app-ids instead
of being renamed into something no inventory lookup would match. And a product
is either a game or stands for several, never both: an edition that is its own
Steam app and also lists what it bundles stays one linkable game.

A pack whose contents could not be looked up is **reported**, not left silent.
It would otherwise print as a bare name with nothing beneath it, which looks
exactly like a pack that worked — a failure indistinguishable from success, and
worse than the wrong link it replaced. Fanatical's own catalogue has a few:
slugs its server does not resolve, including one with a typo in it.

### When a bundle stops being buyable

The deadline follows the price, and the time remaining is coloured by how close
it is:

| Time left | Shown |
|---|---|
| Under 50 hours | **red** |
| Under 5 days | **orange** |
| Beyond that | grey |

Fifty hours is just over two days, so a bundle cannot cross into the last day
unseen overnight. The colour is never the only signal: the words say `8h left`
either way, and piped output carries no colour at all. The remaining time is
stated in one unit, and in hours all the way down through the red band, so two
bundles near their deadlines can be compared without converting anything.

Both stores publish an end date, and neither publishes it the same way. Humble's
is a bare `2026-09-17T04:00:00` with no zone marker; Fanatical's is
`2026-09-24T07:00:00.000Z`. Humble's is UTC, and that is measured rather than
assumed: its page renders its own countdown from that value, and on 2026-09-16
at 19:31 UTC the rendered page said "Offer ends in 8 hours : 28 minutes" for a
bundle whose end date is `2026-09-17T04:00:00`. Read as Pacific it would have
said fifteen and a half hours. A timestamp carrying an explicit offset is
*refused* rather than read as UTC, because silently dropping one is the mistake
this is guarding against.

A bundle with no deadline shows none. That means the store published none, never
that the bundle runs forever — Fanatical's listing genuinely omits it for some
bundles, so the two facts are kept apart. A deadline already in the past reads
`ended`, which is not hypothetical: the captured index in `tests/fixtures/`
holds bundles that expired days after it was taken.

Bundles and games appear in the order the store lists them, which is curation
order rather than alphabetical.

A game's link is the field that starts with `https://`, and detail lines carry
no link:

```console
$ cargo run -- sales humblebundle | grep -oE 'https://[^ ]+'
```

A line that carries no link is a pack's name, and the games it delivers are the
indented lines beneath it. Such a line has no `https://` field, so the command
above finds every real link and nothing else.

**That is the only supported way to parse this output.** Game names contain
spaces, so field *positions* are not stable — a positional parse (`$2`, `$3`)
will misread every multi-word title. It used to be safe to take the *last*
field, and that changed when
[the owned/wanted column](#marking-what-you-already-own-or-want) arrived: a
marked line carries its tags after the link. Match the field, don't count to
it.

### Picking from the listing

On a terminal, `sales` **is** the form. Every bundle is a section that folds
shut, every thing it sells is a checkbox, and what you tick becomes a shell
script that opens those pages.

```text
# saved listing from 2026-09-17 15:07 UTC; rerun with -f to refetch the data

# HumbleBundle game bundles
> Crawling Through the Dungeons  ( $14.00 )  [ ends 2026-10-01 04:00 UTC · 13d left ]
v Narrative Masterpieces  ( $13.00 )  [ ends 2026-09-26 04:00 UTC · 8d left ]
  [ ] Citizen Sleeper          Overwhelmingly Positive 8321 | ...
  [x] Norco                    Very Positive 2914 | ...

# Fanatical game bundles
v Build your own Titanium Collection  ( $4.00/game at 5+ )  [ ends 2026-09-25 07:00 UTC · 7d left ]
  [ ] Portal 2                 Overwhelmingly Positive 1041 | ...
  [x] Lazy Otter Double Pack
    [x] Slots & Diapers        Positive 118 | ...
    [x] Idle Chapel            Mostly Positive 44 | ...
[ Submit ]
```

Each store's bundles sit under a comment naming the store. In a Fanatical title
the words "Build your own" are **green**: they say what kind of bundle it is,
one you pick games out of, and the part that names the bundle is left alone.

A **pack** — one product that delivers several games, with no page of its own —
is the box with boxes indented under it. Ticking the pack ticks all of them,
which is how the pack is sold; clearing it clears them. You can also tick some
and not others, and then the pack's box is clear while the games you picked stay
ticked: the box means *all of these*, and what it opens is pages, so leaving out
a game you already own costs you nothing. The nested lines' own columns sit two
cells right of their neighbours', which is the indent showing through.

Every bundle starts folded, because a run across both stores offers several
hundred games and a form that opens expanded is a wall. The keys are the form's
own and it prints them along the bottom. Every named store is fetched first and
shown in **one** form, since comparing bundles across stores is the point.

Submitting prints what you ticked in the ordinary listing format — prices,
deadlines, colours, the link-is-the-last-field rule, all unchanged — and writes
`catalogames-open.sh` beside you:

```sh
#!/bin/sh
# catalogames: generated link opener
...
DELAY=0.4
FIRST_DELAY=2

open_link() {
    xdg-open "$1" >/dev/null 2>&1 || printf 'could not open %s\n' "$1" >&2
    sleep "$2"
}

# Narrative Masterpieces
open_link 'https://store.steampowered.com/app/703670' "$FIRST_DELAY"  # Citizen Sleeper
open_link 'https://store.steampowered.com/app/1221250' "$DELAY"  # NORCO
```

**Every page goes through one function.** Change `open_link` — a different
browser, a particular profile, a private window, a line printed instead of a
window — and every link changes with it. The delay is a variable above it rather
than a number inside it, because it is the thing most likely to need changing: a
browser already running takes a page in milliseconds, while a cold start can
race its own launch. The first page waits longer for that reason.

Pages open **in the order you picked them**, which is what stops two adjacent
tabs reading as one bundle when they came from two. The bundle each one came
from is written above it in the file.

A page listed twice was picked twice, in two different bundles, and opens twice.
That is the comparison, not a mistake to tidy up — ticking a game in one bundle
never ticks it in another.

The script is written `644`, not executable, and a file this program did not
write is never replaced: it recognises its own output by the name on its second
line. `--out` puts it somewhere else.

`--plain` prints the listing instead. So does any run whose output is not a
terminal, or whose **input** is not one — a form needs somewhere to draw and
someone to answer, so `sales | less` and `sales < /dev/null` both go plain
rather than appearing to hang.

### Marking what you already own or want

```console
cargo run -- sales --account-files /path/to/somewhere
```

A game a listing is selling that your own files already name says so:

```console
catalogames: your own files in ~/.local/share/catalogames: Steam library (3241 games, 2026-09-18), Steam wishlist (37 games, 2026-09-18), Epic library (590 games, 2026-09-18)

  - Portal 2       Very Positive 267k | Very Positive 1801  .../app/620     [owned on Steam]
  - Portal         Very Positive 108k | Very Positive 552   .../app/400     [wishlisted on Steam]
  - Half-Life 2    Overwhelmingly Positive 148k | ...        .../app/220     [owned on Epic?]
  - Dishonored     Very Positive 47k | Very Positive 96      .../app/205100  [owned on Steam, Epic?]
```

**The line itself carries the verdict.** An owned game is drawn **red** and a
wanted one **green**, across the name and the link — there is no reason to buy a
game you have, and noticing that should not require reading anything. Red beats
green: a game on both a library and a wishlist is one you *have*. The reviews
keep their own band colours through it, because they answer a different question
and the answer does not change because you own the game.

The tags themselves are **grey**, in a column of their own after the link. The
colour is the verdict; the column is the detail behind it — which stores, and
how sure — for a reader who has seen the colour and wants to know why. Stores
stack into one bracket, since owning a game on two of them is one fact about it
and not two, and owned is listed before wanted.

The files are the ones [`gamelib`](#your-own-libraries) writes, and
`--account-files` defaults to where it writes them — so a plain `gamelib
steam` and a plain `sales` work together with nothing passed to either. It is
**not** `~/.config`: a snapshot is not configuration, and the same reasoning
keeps `gamelib` out of there.

**The `?` is the interesting part.** Steam publishes an app-id for everything it
owns or wishlists, so a Steam mark is normally the *same number* on both sides
and cannot be wrong. Getting a number for the other side is what varies.
Fanatical publishes Steam ids for what it sells; **Humble publishes none at
all**, so a Humble game's id comes from the
[inventory](#games-you-already-have-somewhere-else) instead — the same
resolution that already chose the reviews, tags and store link on that row, so a
mark built on it is exactly as certain as the rest of the line and is not
hedged.

A `?` means the mark rests on a title comparison that nothing else on the line
rests on. Two ways that happens: the game resolves to no inventory entry at all
(415 entries cover the bundles, not a whole library), or the *library* entry has
no Steam id to compare — which is all of Epic, since Epic publishes only its own
identifier and a title. Titles are matched against Steam's own name for the game
and its aliases wherever one is known, because a vendor's marketing wording is
the messier of the two strings.

**Nothing is fetched.** The files are the answer, and taking them is a separate,
deliberate command. It is a fact and never advice, for the same reasons as
[the Epic table](#games-you-already-have-somewhere-else): a second copy is a
gift, and an old purchase may predate a remaster.

**A wrong directory cannot pass for owning nothing**, which is the failure this
is built to not have: an empty directory and a misspelled one both produce a
listing with no marks on it, and no marks looks perfectly healthy. So the line
above the listing names the directory it looked in, every file it found, every
file it did not, and when each was taken — a claim is about the world as it was
on that date. A file that is present and unusable is reported rather than
skipped; a file that is simply absent is not, because snapshotting one store and
not another is normal.

### The listing is saved, so the next run is instant

Reading every bundle from every store takes minutes; a bundle lasts weeks. So a
run saves what it read, under `$XDG_CACHE_HOME/catalogames` (else
`~/.cache/catalogames`), named for when it was taken — `2026-09-17_1507_sales.json`
— and the next run puts it straight on the screen and says so above the form.
Measured on one store: thirty seconds becomes under one.

What is stored is **instants, never durations**. A deadline is kept as the
moment it falls and the time left is worked out each time the listing is drawn,
so a listing saved two hours ago shows two hours less on every bundle with no
arithmetic anywhere.

A saved listing is used only when all of these hold. Any one of them failing
means fetching again, and the reason is printed:

| Condition | Why |
|---|---|
| Under 24 hours old | Bundles run for weeks; their contents do not change in an afternoon. |
| No bundle in it has ended | A listing about deadlines has no business showing a bundle that ended an hour ago. |
| It covers the stores asked for | Compared as a set, and a store whose fetch failed is not in it — so a run that lost a whole store refetches rather than serving a gap. |
| It was written by this version | These types are not frozen, and a missing field would deserialise into a default rather than failing. |

`-f` overrides all of it.

**What a saved listing cannot tell you**, and this is its real failure mode:
both of those first two tests fire on the saved copy going *wrong*. Neither
fires on the world gaining something new. A listing saved at ten, holding
bundles all good for another week, is reused at noon even though a bundle
launched at eleven — nothing looks amiss and the new bundle is simply absent.
`-f` is the way past it.

If a store had problems when it was read, those are saved with it and shown
again on reload. A partial capture would otherwise come back looking complete,
with no sign that three bundles were missing.

**Problems are printed last**, after the listing, so that what you asked for
comes first and the reasons something is missing come after it. A form says the
count above itself, since the detail arrives after the choosing is over.

**One checkbox per page worth opening.** A multi-game pack gets a box of its
own and a box for each game it delivers, indented under it. Ticking the pack
ticks them all, which is how it is bought; ticking some of them opens only those
pages, which is all a tick actually does.

Cancelling or ticking nothing prints no listing and says which of the two
happened, since an empty listing and an empty selection otherwise look identical.

### Seeing what it is doing

`-v` narrates the run; `-vv` adds detail. Narration goes to **stderr**, so
stdout stays a clean listing you can pipe. The flag works on either side of the
subcommand.

```console
$ cargo run -- sales humblebundle -v
[info] fetching bundle index from https://www.humblebundle.com/games
[info] index lists 13 bundles
[info] (1/13) Beyond the Metroidverse Bundle
[info] (2/13) Crawling Through the Dungeons
...
[info] read 13 of 13 bundles, 0 problem(s)

$ cargo run -- sales humblebundle -vv
[debug] GET https://www.humblebundle.com/games/beyond-metroidverse-bundle
[debug]   HTTP 200 OK
[debug]   570675 bytes
[debug]   read 85669 bytes from <script id="webpack-bundle-page-data">
[debug]   2 tier(s); largest is "bt10" with 7 item(s)
[debug]   resolved 7 game title(s)
[debug]   vendor advertises 7, found 7 (agrees)
[debug] pausing 500ms before the next request
```

Only this crate's records are shown. Dependencies are filtered out — `scraper`'s
HTML tokenizer logs every token it processes, which would bury everything else.

### Other stores

```console
cargo run -- sales fanatical
```

Two kinds of bundle are listed, and they are found in two different places.
**Pick-and-mix** bundles — the "Build your own" ones, where you choose some
number of games out of a pool — come from the catalogue endpoint. **Fixed-price**
bundles, sold whole at one price, are not in that payload at all and are found
through one search call against the same MCP server, which returns every game
bundle the store sells in a single request. The pick-and-mix ones it returns are
dropped by slug, since the catalogue already supplied them with their tier
ladders.

Mystery bundles are excluded by their declared type rather than by their names.
Their contents are random by design, so there is nothing to list, and reading the
type keeps that from being a guess about wording. The declared type settles the
price too: a fixed-price bundle is bought whole however many products its tier
happens to name.

In a Fanatical title, the words "Build your own" are greyed. Every bundle of that
kind carries them, so they distinguish none of them, and the part that names the
bundle is what should carry the eye.

Fanatical is read through two sources, for two different reasons. The bundle
**listing** comes from `api/all/en`, a JSON endpoint their `robots.txt`
affirmatively `Allow`s. The bundle **contents** come from Fanatical's MCP
server, which their `llms.txt` asks clients to prefer over reading pages.

Nothing about that path is generative — MCP is JSON-RPC 2.0 over HTTP POST, so
a client is an HTTP client. No model, no tokens, same bytes every time.

Neither is a scrape of a rendered page, and not for want of trying: the
pick-and-mix page is a JavaScript shell with no embedded data at all. The
endpoints were found by driving a headless browser and watching what the page
requested; the responses are saved in `tests/fixtures/fanatical/` so nobody has
to repeat that.

The payoff is that Fanatical publishes **Steam app ids**, so its games link
straight to their store pages with no name matching at all — no
`storesearch`, no `// REVIEW:` lines, none of the machinery the Humble path
needs. Where a store gives an id it is treated as authoritative and there is no
fallback to matching by title, because falling back is how a near-miss title
attaches the wrong game.

Products with no id are multi-game packs and editions, and both are recovered.
An edition names the base product it is built from, and that is followed. A pack
names nothing, but its record lists what it delivers, so those are looked up in
turn — see above.

A product the service declines to describe **in the region asked for** is still
listed. Its slug is the store's own identifier, so it yields a readable title and
a search link, exactly as a game with no id does anywhere else, rather than
disappearing from a bundle that sells it. The derived title is reported as
derived, because it is the slug tidied up and not the store's own wording.

Fanatical asks that its URLs be used exactly as returned and that it be cited
when its data is displayed, and asks that product descriptions, images and
metadata not be republished in bulk — so none of those fields are read.

### Adding a game to the project's inventory

```console
cargo run -- add_project_entry https://store.steampowered.com/app/440
cargo run -- add_project_entry 440
```

Fetches what Steam says about one app — four paced requests — and writes the
inventory entry for it to **stdout**, ready to paste into
`src/inventory/steam/regular.rs`:

```rust
    // captured 2026-09-16
    SteamGame {
        app_id: 440,
        name: "Team Fortress 2",
        aliases: &[],
        released: "Oct 10, 2007",
        recent: Some(Reviews { approval: 92, count: 6059 }),
        all_time: Reviews { approval: 91, count: 1250978 },
        tags: &[Tag::FreeToPlay, Tag::HeroShooter, Tag::Multiplayer, ...],
        os: Os { windows: true, mac: false, linux: true },
        vr: Vr::None,
        deck: Deck::Playable,
        features: &["Multi-player", "Cross-Platform Multiplayer", ...],
    },
```

The preview line goes to stderr beside it, so a person can see at a glance that
the id was the game they meant while stdout stays pasteable.

The same emitter writes this row and the whole table, so a pasted entry cannot
differ from a generated one — a test renders every committed entry through it
and checks the result appears verbatim in `regular.rs`.

Three things are worth knowing about a pasted row:

- **The capture date rides with it.** `CAPTURED` is a module-level date and the
  recent review row is a thirty-day window measured on it, so a row pasted under
  an older date would quietly make that date wrong for this one entry. The
  comment above the entry records the day it was fetched.
- **It carries no `// REVIEW:` marker, and cannot.** Those mark a title the
  generator resolved by *searching*; here the app-id was given, so there was no
  title match to be unsure about. The two outputs are not interchangeable.
- **A game already in the inventory is refused before anything is fetched.**
  It says which entry already holds that app-id, which is an answer rather than
  a duplicate found by a failing test after the paste.

#### A bare id does not say which store it came from

Every store numbers its own games, so `440` is a different game on each. A link
settles it; a bare id gets asked, as a numbered menu drawn on stderr:

```text
440 is a bare id, and every store numbers its own games.
Pass a store link instead to skip this question.
Which store is that from?
  1) Steam
  2) Epic Games Store  — not yet: the Epic inventory is still a placeholder …
> 1▏
```

The menu is [`terminal_choice`](https://github.com/SirHexa10t/terminal_choice).
Epic is shown and dimmed rather than left out: a disabled option that explains
itself documents what is coming better than its absence does. Where there is no
terminal to ask in — a pipe, a script — the command says so and tells you to
pass the link instead, rather than guessing.

### Your own libraries

```console
cargo run -- gamelib steam
cargo run -- gamelib epic
cargo run -- gamelib steam --dir /path/to/somewhere
```

Writes a point-in-time record of what an account owns. Files land in
`$XDG_DATA_HOME/catalogames` (else `~/.local/share/catalogames`) unless `-d`
says otherwise, and their paths are printed on stdout:

| File | Holds |
|------|-------|
| `game_lib_steam` | Games the Steam account owns, with playtime. |
| `game_wishlist_steam` | The Steam wishlist, with the rank you gave each entry. |
| `game_lib_epic` | Games the Epic account owns. |

Not `~/.config`, which holds the settings you edit and the credentials file. A
snapshot is data this program made: nobody edits it, and deleting it changes
nothing about how the program behaves. Not your home directory either, which is
where these used to land, because a file called `game_lib_steam` loose in a home
directory is one that nothing explains.

They are JSON, written `0600`, sorted by app-id so two snapshots diff into *what
changed* rather than a reshuffle, and each one records where it came from — when
it was taken, which account, which endpoint, and the count the store itself
reported beside the number of entries actually written. Two numbers for one
fact: while they agree, the read was complete.

Nothing here keeps a session. Each run is one read and then the program exits;
there is no token to refresh, no background sync, and no state kept between runs
beyond the files themselves.

Run it with nothing set up and it **asks**, rather than printing instructions
and stopping. Each thing it needs gets a field with its own note above it saying
where to get it, and what you type is checked as you type it:

```text
What catalogames needs, and where to get it

  SteamID: The 17 digits at the end of a profile URL such as https://…
  SteamID: 76561197960287930▏

  Web API key: Register one at https://steamcommunity.com/dev/apikey . It reads…
  Web API key: ▏

  [ Submit ]
  ⚠ "7656119796028793001234…" is not a SteamID: 17 digits beginning 7656119.
```

Submitting writes the file and says so:

```console
catalogames: wrote ~/.config/catalogames/credentials — SteamID and Web API key. Readable only by you.
```

The file lands `600` in a directory made `700`, written atomically, so there is
no moment at which either is readable by anyone else and no way to be left with
half a key. Where there is no terminal to ask in, it prints the same instructions
it always did.

**The key is on screen while you type it.** The form has no masked field, so
that is worth saying rather than glossing: it draws on stderr and clears itself
when it closes, so nothing stays in your scrollback, but it does not survive
somebody reading over your shoulder.

Nothing is written for Epic — Legendary keeps its own credentials and this never
touches its files — so that guide stays a guide. When Legendary *is* installed
and the run fails, it offers to run `legendary auth` for you, defaulting to no.

#### Two things a snapshot will not do

- **It will not record "no answer" as "no games."** Steam answers a request
  about a profile whose game details are private with HTTP 200 and
  `{"response":{}}` — no error, no empty list, nothing a parser can object to.
  Taken at face value that is an account owning nothing, and writing it would
  destroy the previous snapshot. So a response with no `games` key (or no
  `items` key, for a wishlist) is refused and explains which privacy setting to
  change. Only a present-but-empty list is ever written as empty.
- **It will not shrink a library.** A Steam library loses a game only on a
  refund, so a snapshot shorter than the one already on disk is far more likely
  to be the wrong account or a truncated read. The old file is kept and the
  reason is printed. A *wishlist* is exempt, because buying something takes it
  off the list — shrinking is that one's normal behaviour.

#### Steam

Needs your SteamID, and a Web API key from
[Valve's key page](https://steamcommunity.com/dev/apikey) for the owned-games
list. The SteamID may be given as the number or as the profile page it is on,
since that is what you have open; a custom name
(`steamcommunity.com/id/...`) is not the id and is refused rather than guessed
at. Valve's key form asks for a domain name, which is a declaration about your
own use: put your own domain, or `localhost` for a tool on your own machine. The wishlist needs no key at all — Valve serves that one on the SteamID
alone. The key reads public profile data and nothing else: it cannot buy, cannot
change the account, and can be revoked from the page that issued it. No Steam
password is asked for anywhere, and none would be accepted.

Credentials go in `$XDG_CONFIG_HOME/catalogames/credentials` (else
`~/.config/…`) as `name = value` lines, and the file is refused if anyone but
its owner can read it. Every setting can be an environment variable instead
(`CATALOGAMES_STEAM_API_KEY`), which keeps it off disk at the cost of exposing
it to every program this one starts. The guide never suggests putting a key on a
command line, because that would put it in your shell history.

Valve accepts the key as a **query parameter** and nowhere else — not a header,
not a body field — so the one thing that must never be printed is carried by the
part of a request that errors and logs print by default. Two types close that
off crate-wide rather than by remembering: `Error`'s URL fields are a `SafeUrl`,
redacted when it is built, so `Display` and `Debug` both read an already-cleaned
string; and its transport causes are a `Transport`, which strips `reqwest`'s own
copy of the URL on the way in. A raw `String` in either position does not
compile.

Two Steam privacy settings have to be public while the snapshot is taken — "Game
details" for the library, and the same setting governs the wishlist — and
**both can be set back immediately afterwards.** The snapshot is a file on your
own disk and never needs them again.

#### Epic

Epic publishes no API for reading your own library, so this shells out to
[Legendary](https://github.com/derrod/legendary), an open-source Epic client
that already implements their login:

```console
pipx install legendary-gl
legendary auth
```

Legendary is a **runtime** dependency, not a cargo one: nothing is compiled or
downloaded by a build, and it is checked for before use — if it is missing, the
command explains how to install it instead of failing. It is declared under
`[[package.metadata.runtime-dependencies]]` in `Cargo.toml` so a project
packaging this one can prepare the environment from the manifest rather than by
reading the source.

Only Legendary's documented `legendary list --json` output is read. Its
configuration and cached manifests are never opened: those are another project's
private files, their format is not a contract, and a snapshot built from them
would be a guess about someone else's internals. This program never sees an Epic
password and never stores an Epic token.

Legendary's own progress and error messages go straight to your terminal rather
than being captured, so a slow read looks like a slow read instead of a hang.

Signing in leaves a token in Legendary's own configuration, so once it reports
an account there is nothing further to pass in: run `gamelib epic` again. If its
sign-in prompt crashes — it indexes what you typed without checking that you
typed anything, so a bare Enter takes it down — the code is still unused, and
`legendary auth --code <authorizationCode>` hands it over directly. That advice
is printed whenever the sign-in this program offered comes back unsuccessful.

Whether anybody is signed in is asked first, with `legendary status --json`, and
the reason is worth knowing: running `list` while signed out does not fail
politely in the version measured. It raises an unhandled Python error and prints
a full traceback. Asking first turns that into a sentence, and the same answer
carries the account name and Legendary's own count of what it owns, which the
snapshot records beside the number of games actually written.

Epic's wishlist is out of scope for now. Playtime is not recorded, because
Legendary's game list does not publish it.

### Exit codes

| Code | Meaning |
|-----:|---------|
| `0`  | Everything asked for was produced. |
| `1`  | Nothing could be produced at all. |
| `2`  | Some of it was produced, and the rest needs attention. |

Code `2` matters: what reached stdout is usable but incomplete or suspect, and
the reasons are on stderr. It is what you get when one bundle page fails while
the rest succeed, when a consistency check trips, or when `gamelib steam` writes
your library but cannot read your wishlist.

## Using the library

```toml
[dependencies]
catalogames = { version = "0.1", default-features = false }
```

`default-features = false` matters here. The `cli` feature is on by default so
that `cargo run` and `cargo install` work with no flags, which means an ordinary
`catalogames = "0.1"` pulls in the command line's own dependencies —
[`clap`](https://docs.rs/clap) and
[`terminal_choice`](https://github.com/SirHexa10t/terminal_choice) — that a
library consumer has no use for. Opting out drops them.

Measured on 2026-09-16: 170 crates with the feature, 162 without. **That gap is
smaller than it should be, and the reason is a finding rather than a design.**
`table_formatter` declares `clap`, `regex`, `rayon` and `console` as
unconditional dependencies with no feature to opt out of, so a library consumer
of this crate receives them however the `cli` feature is set. Only
`terminal_choice` and its TOML parser are actually dropped by opting out. Gating
those four behind a feature in `table_formatter` is the fix, and it belongs in
that crate.

```rust
let listing = catalogames::humble::Client::new()?.list_bundles()?;

for bundle in &listing.bundles {
    println!("{} ({} games)", bundle.title, bundle.games.len());
}
for problem in &listing.problems {
    eprintln!("warning: {problem}");
}
```

A run reports partial success. One unreachable bundle page does not discard the
other twelve; it becomes an entry in `listing.problems`, so a caller can always
tell a complete listing from a degraded one.

### Progress reporting

The library narrates its work through the [`log`](https://docs.rs/log) facade
and installs no logger of its own, so records land in whatever logging an
embedding project already runs:

```rust
// Any `log` implementation works; this crate's records target `catalogames`.
env_logger::init();
let listing = catalogames::humble::Client::new()?.list_bundles()?;
```

`info` is the narrative (which bundle, how many found); `debug` is the detail
(each request, tier sizes, the count cross-check). Nothing is emitted above
`info`, and nothing is ever written to stdout.

### What Steam says about an app

```rust
let game = catalogames::steam::store::Client::new()?.fetch(1202130)?;
println!("{} — {}", game.name, game.all_time.rating().as_str());
```

The runtime twin of an inventory entry, with owned strings. Fetching and parsing are separate
here too: `steam::store::parse_app_details`, `parse_reviews`, `parse_deck` and
`parse_store_page` take response text, so a caller with its own HTTP stack can keep the parsing.

### Your own libraries

```rust
use catalogames::gamelib;
use std::path::Path;

// One call per store; each returns what it wrote and what it could not.
let report = gamelib::steam::snapshot(Path::new("/tmp/snapshots"));
for path in &report.written {
    println!("wrote {}", path.display());
}
for failure in &report.failures {
    eprintln!("{failure}");     // says which snapshot, and what to do about it
}
```

`steam::library`, `steam::wishlist` and `epic::library` are the single-file
forms. Parsing is separate from fetching here too: `steam::parse_library`,
`steam::parse_wishlist` and `epic::parse_list` take response text and do no I/O,
so a caller holding that JSON from anywhere else can build a `Snapshot` without
this crate making a request or starting a process.

### Store links

```rust
let url = catalogames::steam::search_url("Salt and Sanctuary");
```

`render::listing` already appends these; call it directly if you are formatting
output yourself.

### Bringing your own HTTP client

Fetching and parsing are separate. If you have your own HTTP stack — async,
cached, proxied, rate-limited — use the parsers directly and skip `Client`:

```rust
use catalogames::humble;

let entries = humble::parse_index(&index_html, humble::INDEX_URL)?;
let (bundle, problems) = humble::parse_bundle(&bundle_html, &entries[0])?;
```

`parse_index` and `parse_bundle` do no I/O.

## How bundle contents are determined

Stores sell a bundle in price tiers. Each tier lists its contents in full rather
than only what it adds over the tier below, so the largest tier is the whole
bundle — and that is what `Bundle::games` holds.

"Largest" is found by counting items, never by position: Humble's own tier order
runs most-expensive-first, so the last tier is not the biggest one.

Two details worth knowing when reading the output:

- **Items are not filtered by type.** The Godot bundle's items are typed
  `software`, yet the store advertises them as "13 games". Filtering on the type
  would silently empty that bundle.
- **A repeated title is not a bug.** The 15th-anniversary bundle sells two
  copies of each game — one to keep, one to gift — and advertises the doubled
  count. Both copies are listed, and `Game::machine_name` tells them apart.

## Terms of use — read before deploying this

Humble Bundle has no public API for browsing bundles, so this crate reads the
JSON their pages embed for their own front-end.

**Humble's terms prohibit automated access.** Section (b) of their terms bars
scraping (clause i) and access "through any technology or means other than those
provided or authorized by the Service" (clause xi). Neither clause is
rate-conditioned, so polite pacing does not satisfy them. Their `robots.txt`
carries a prose header prohibiting scraping without written permission — note
that its machine-readable rules do *not* disallow `/games`, so a robots parser
alone reads as permission where the document does not grant it.

This is a licensed open-source library, and shipping it passes that exposure to
downstream users who never agreed to Humble's terms. Whether to use the
`humble` module is a decision for whoever deploys it.

What the client does do, as a floor:

- identifies itself honestly as `catalogames/<version>` rather than imitating a
  browser;
- fetches bundle pages sequentially with a pause between them, never in
  parallel, so a run stays slower than a person clicking the same pages.

A sanctioned alternative exists and is not yet implemented: IsThereAnyDeal
publishes a documented bundles API (free key, attribution required) that indexes
Humble among other stores.

### Steam and Epic, for `gamelib`

Steam's path here is the sanctioned one. The Web API key is issued by Valve for
exactly this, under the [Steam API Terms of Use](https://steamcommunity.com/dev/apiterms),
and the endpoints read are Valve's own published services returning data the
account holder has chosen to make public.

**Epic's terms were not read, and it should be said rather than glossed over.**
Their published agreements sit behind a login wall, so this was written without
having seen the clause that would govern it. What limits the exposure is that
nothing here talks to Epic: it runs Legendary, which the account holder
installed and authenticated themselves, and reads what that prints. Whether to
use the `epic` path is a decision for whoever deploys it, on the same footing as
the `humble` one above.

## How big is Steam

Measured on 2026-09-14: 186,157 games, of which roughly 22,300 have a hundred or more reviews;
the whole catalogue compiles as generated Rust in eight and a half minutes from a 132 MB file,
and the first hundred-review cut compiles in 25 seconds. Numbers, method and caveats — including
why "played by at least 100 people" cannot be measured — are in
[`docs/steam-catalogue.md`](docs/steam-catalogue.md).

## The inventories

`inventory/steam/` holds games as Steam presents them, as data: app-id, name, release date,
recent and all-time review summaries, and player tags. It is split into modules by category —
`regular` for games the tracked bundles currently offer, with room for `delisted` and its kind
alongside — so a consumer asks for the category it means instead of filtering one undifferentiated
pile.

```rust
use catalogames::inventory::steam as steam_inventory;

for game in steam_inventory::all() {
    println!("{} ({})", game.name, game.all_time.count);
}

let salt = steam_inventory::by_app_id(283640);
```

`inventory/epic/` holds an Epic account's games, split by how they were come by rather than by
what they are: `giveaways.rs` is one account's free promotional titles, which is the whole of it.
Generated from a snapshot:

```console
cargo run --release --features tools --bin epic_lookup -- game_lib_epic \
    > src/inventory/epic/giveaways.rs
```

**Two fields per entry, and that is everything Epic publishes.** An identifier and a title. No
reviews, no tags, no release date, no platforms: the store does not put them anywhere a client
can read them. The table holds what is published and invents nothing.

What it is for is not buying a game twice. A bundle offering something already sitting free in an
Epic account is not an offer, and `by_title` asks that question through the same `comparable`
matching the Steam side uses, because Epic writes `Dishonored®: Death of the Outsider™` where a
bundle writes `Dishonored - Death of the Outsider`.

Two invariants there are written down rather than tested, because the conventions elsewhere in
this crate would suggest a test for each and real data refuses both. An identifier has no fixed
shape: of 590 entries in one account, 453 are 32-character hexadecimal catalog ids and 137 are
short codenames. And two entries may collide under `comparable` without anything being wrong —
one account holds `Shadow Tactics: Blades of the Shogun` and `Shadow Tactics Blades of the
Shogun` under different identifiers, the same game granted twice. The invariant is that a
collision is the same game, which no test can check.

### Games you already have somewhere else

A listing marks a game the Epic table also holds:

```console
  - Salt and Sanctuary (on Epic)  Very Positive 21898 | Very Positive 73  .../app/283640
```

Measured on the committed tables: 23 of 415 Steam entries are held free on Epic,
and eight of them turned up in one live Humble listing.

**It reports a fact and never advice.** Not "already owned", not "skip this". An
Epic copy is not a Steam copy: no Steam key, so no achievements, no trading
cards, no cloud saves, no Workshop and no Deck verdict, which is exactly why a
Steam entry carries those fields at all. Somebody may well want the Steam copy
of a game they already own, and that is theirs to decide. Wording it as a fact
also costs less when it is wrong, since a fact is something you check against
your own library in seconds.

Nothing is fetched and nothing is stored. Both inventories are already
committed, so the answer is computed from them: no third file to keep in step,
no requests, and no searching Steam by name.

**It steps aside for your own file.** This table is a snapshot of one account,
so the moment `sales` reads a real Epic library — see
[marking what you already own](#marking-what-you-already-own-or-want) — the note
stops being drawn and the file answers instead. A guess about somebody else's
account is worth nothing beside the reader's own.

The match is exact equality after keeping only alphanumerics, which bridges
`Sundered®: Eldritch Edition` and `Sundered Eldritch Edition` while refusing to
do what once resolved "Ashen" to "Ashen Empires" — that was a prefix accepted
from a ranked search, and `ashen` does not equal `ashenempires`. So it fails
safely: an edition suffix one store drops means no match, which reads as "not
known" rather than as a wrong claim.

The one unsafe case is two different games sharing a name, and it is already
half-loaded. The Epic account holds `PREY`; the Steam table holds no Prey at
all. The pair is absent because one side is missing, not because the rule is
safe. The inventory's own `(YYYY)` convention is the answer, and it names how a
false positive will eventually arrive — two year-disambiguated entries both
keeping the bare title as an alias, one Epic title matching both.
`crossover::ambiguous()` finds that, and its test asserts there is none today,
which is the only moment a detector can be trusted not to cry wolf.

Titles are never filtered on their wording, and the reason is worth keeping. A library of that
size holds soundtracks and add-ons, and the obvious way to spot them is to match the title
against words like "soundtrack" — which flags `Ghostwire Tokyo` and `Samorost 2`, because
"ost" sits inside both. Everything is carried; nothing is guessed at from a name.

`inventory/itch` and `inventory/gamejolt` remain placeholders. They carry no types on purpose:
what those stores publish is not yet known, and a shape invented before there is one real entry
to hold would only have to be undone.

Each entry carries its app-id, name, release date, both review scores, player
tags, and compatibility: operating systems, VR, Valve's Steam Deck rating, and
Steam's feature list (co-op, achievements, controller support, cloud saves…).

### Ratings are stored as a percentage, not a word

An entry stores the **approval percentage** and the review count; the verdict
Steam prints is derived from them, so the two can never drift apart.

```rust
let reviews = game.all_time;                 // Reviews { approval: 89, count: 21898 }
reviews.rating();                            // Rating::VeryPositive
reviews.rating().as_str();                   // "Very Positive"
reviews.rating().color();                    // "#66C0F4"
```

Three things about that translation are measured rather than assumed, and each
one is a way to get it wrong:

- **The review count is load-bearing, not a tie-break.** A game on 99% with 201
  reviews is "Very Positive", not "Overwhelmingly Positive" — the top band needs
  hundreds of reviews. A translator taking only a percentage cannot reproduce
  Steam's system at all.
- **The percentage is floored, never rounded.** Steam assigns the band from the
  exact ratio, so a title on 79.98% is "Mostly Positive" despite displaying as
  80%. Checked against 137 real titles: flooring reproduces Steam's own verdict
  137/137, rounding gets 6 wrong — every one at a boundary.
- **The last thirty days is a different function.** Same percentage bands, looser
  count gates: 80% over 10 reviews reads "Very Positive" in that window and only
  "Positive" all-time. Use `recent_rating()` for a recent summary, `rating()` for
  an all-time one.
- **Two palettes, and only one of them is Steam's.** `Rating::color()` is what Steam renders —
  three colours plus a grey, so it cannot tell "Positive" from "Overwhelmingly Positive".
  `Rating::gradient_color()` is ours: one shade per band, keeping Steam's shape (blue above
  Mixed, Steam's own tan for Mixed, red below) with the extremes most vivid. The listing uses
  the gradient, because ranking at a glance is the point of colouring it at all.

The generator checks its own arithmetic on every run — the derived all-time band
is compared against `review_score`, Valve's own band index, and the derived
recent band against the one printed on the page. A mismatch fails the run rather
than writing a verdict Steam disagrees with.

Colours are Steam's own, and there are **three of them plus a grey, not nine** —
"Positive", "Very Positive" and "Overwhelmingly Positive" all render in the same
blue. They were read off Steam's stylesheet on a particular day, differ by
context, and assume a dark background; they are a snapshot, not a contract.

### Tags are an enum

```rust
use catalogames::inventory::steam::{self as steam_inventory, Tag};

let souls_likes: Vec<_> = steam_inventory::tagged(Tag::SoulsLike).collect();
Tag::SoulsLike.as_str();   // "Souls-like"
Tag::SoulsLike.id();       // 29482
```

All 446 tags Valve publishes, generated into `inventory/steam/tags.rs` so a
misspelt tag cannot compile. Variant names are the display names with
punctuation removed and each word capitalised; the ten that start with a digit
take a leading underscore, because Rust forbids an identifier starting with one
(`Tag::_2D`, `Tag::_4X`, `Tag::_1990s`).

The cost is that a tag Valve adds is unknown until the enum is regenerated —
which is why the game generator treats an unrecognised tag as a failure rather
than quietly dropping it from an entry:

```console
cargo run --release --features tools --bin steam_tags > src/inventory/steam/tags.rs
```

### Editions are aliases, not entries

A store selling "Frostpunk: Game of the Year edition" is selling app 323190 with
extra content — Steam has no separate app for it. Recording it as its own entry
would duplicate every review count and tag under an id that does not exist, so
the store's wording becomes an *alias* on the base entry and `by_name` finds the
entry by either.

Across twelve real cases the edition turned out to be its own app-id only twice.
Three were a package on the base app's page, six were a store bundle, and one
did not exist on Steam at all. So the generator asks whether the base app is
*sold* under that wording — checking `package_groups` first, since that arrives
free with the details it already fetches, and the page only for bundles.

### Two things the data means, precisely

- **"All-time" is every review, all languages, bought *or key-activated*.** Steam's review API
  defaults `purchase_type` to `steam`, which excludes key-activated copies — about a fifth of
  the reviews on a popular title. For a catalogue about bundles those are exactly the copies
  that matter, so both `language=all` and `purchase_type=all` are always passed. A consequence
  worth knowing: the store page shows the `purchase_type=steam` figure, so these counts read
  *higher* than the page they came from. That is deliberate, not drift.
- **"Recent" is a 30-day window, and therefore always stale.** It is a snapshot taken on the
  module's `CAPTURED` date and drifts from that day on. Read it as what the last 30 days looked
  like *then*.
- **`Deck::Unknown` means Valve has not rated the game**, never that it fails. The same goes
  for an absent feature: it means Steam does not list it.

### Regenerating the entries

```console
# resolve what Humble is bundling, by searching Steam for each title
cargo run --release --features tools --bin steam_lookup > src/inventory/steam/regular.rs

# or take the app-ids out of a listing this crate already printed
cargo run --release --features tools --bin steam_lookup -- listing.txt > entries.rs
```

The two modes differ in the only part that is ever wrong: where the app-id came
from. Given a listing, the ids are already known and are taken at their word —
no searching, no title matching, none of the machinery that produced the
`// REVIEW:` lines. Given nothing, they are resolved from Humble's titles by
search, which is the fragile half this tool exists to keep out of the library.

An id in a listing is still *checked*, though, because a store can put something
that is not an app-id in an app-id field: four ids in one Fanatical capture
turned out to be Steam **package** ids that no app answers to, and those are
reported rather than turned into entries. Where Steam's name for an app differs
from the store's, the store's wording is recorded as an alias and the entry is
flagged for a human — usually a year suffix Steam adds (`(2010)`), a `DLC` the
store adds, or an edition wording, but it is also what a wrong id looks like.

`steam_lookup` is a maintenance tool, not part of the library: it is built only under the
non-default `tools` feature so it is neither compiled by default nor shipped by `cargo install`
beside the real CLI.

It prints to stdout and has no write-in-place flag, deliberately. These are *curated* entries,
and reading the diff is the only thing that catches a title resolved to the wrong app-id — so
the workflow forces a human past it. Lines marked `// REVIEW:` are ones the generator could not
settle alone.

Responses are cached on disk (`$CATALOGAMES_CACHE`, else a temp directory), so a throttle part
way through does not cost the whole run, and a parse bug found later can be fixed against saved
responses instead of fetching everything again.

### Why matching a title to an app-id is the hard part

Searching Steam for a bundle's exact wording often returns nothing, and the cause is not what it
looks like. **A dash surrounded by spaces returns zero results**, whatever sits either side of
it — and Steam's own DLC names are overwhelmingly `Game - Something`, which is why DLC-shaped
titles appear to be the problem. Measured, one variable at a time:

| Search term | Results |
|---|---|
| `Warhammer 40,000: Rogue Trader - Season Pass` | 0 |
| `Warhammer 40,000: Rogue Trader Season Pass` | 2 |
| `Rogue Trader: Season Pass` (colon instead) | 2 |
| `Rogue Trader – Season Pass` (en-dash) | 0 |
| `Season Pass` (the words alone) | 10 |
| `Half-Life 2` (hyphen inside a word) | 10 |

So the generator normalises the *separator* and never pattern-matches the words. Dropping
titles containing "DLC" or "Pack" would miss the real cause and throw away findable games.

Telling a base game from DLC is then done *after* the search, not before it: `appdetails`
reports `type` as `game`/`dlc`/`music`/`demo`, which is a fact rather than a guess from a title.

Three more things about Steam's data that are easy to get wrong, all measured:

- **VR is read from category *ids*, not their descriptions.** Descriptions are localized
  ("Single-player" becomes "Einzelspieler" under `l=de`), and of the four VR-ish categories only
  two discriminate: 54 is VR-required, 53 is VR-optional. Half-Life: Alyx carries 54 *and* the
  legacy 31 "VR Support", so VR-only is not the absence of VR-support.
- **Steam Deck comes from its own endpoint.** `appdetails` has no deck field and the page
  renders the badge client-side. The report returns `success: 1` even for app-ids that do not
  exist, so a missing report is detected by the shape of `results`, not by `success`.
- **The `filters=` parameter omits silently.** `basic,release_date` drops `categories`,
  `platforms` and `package_groups` without any error — every compatibility field would simply
  come back empty.

What that check does **not** rule out is the wrong game of the right type — a Game of the Year
edition, a regional re-release, or another entry in the same franchise all pass it happily. So
the generator never silently takes the first result: anything it cannot settle becomes a
`// REVIEW:` line in the output for a human to decide.

### Terms, compared with Humble

Steam is a materially weaker restriction than Humble, which is worth stating plainly rather than
treating the two as equivalent:

- `store.steampowered.com/robots.txt` disallows account, login and share paths. It does **not**
  disallow `/app/` or `/api/`, and carries no prose anti-scraping notice.
- Steam's Online Conduct rules address attacking servers, manipulating reviews and generating
  accounts — not read-only retrieval.
- The Subscriber Agreement's Automation clause opens broadly ("in any manner") but every
  example it gives concerns account, gameplay or marketplace abuse.

There is also a defensibility gradient worth knowing: `appreviews` is
[publicly documented by Valve](https://partner.steamgames.com/doc/store/getreviews);
`appdetails` and `storesearch` are undocumented but unrestricted by robots; tags exist only in
the page HTML.

## Development

```console
cargo test                 # offline; uses captured fixtures
cargo clippy --all-targets
cargo fmt
```

The offline suite never touches the network. It runs against real page captures
in `tests/fixtures/` — see the README there for what they do and do not prove.

### The live test is the only drift detector

```console
cargo test --test live_humble -- --ignored --nocapture
```

The fixtures pin the page schema as it was on the day it was captured. They will
stay green forever after Humble changes the page, because they *are* the old
page. Only the live test can tell you this crate has stopped reading the real
site, so run it before releasing and on a schedule — it is ignored by default
only because it needs the network.

Three consistency checks run on every fetch and surface as `Problem`s rather
than being trusted silently:

- the store's own advertised game count is compared against the games actually
  found, since the two numbers come from different parts of the page;
- every smaller tier is checked to be contained in the largest one, because that
  is the only condition under which "largest tier" means "whole bundle";
- items a tier names but the page does not title are reported instead of
  dropped.

## Adding a store

`src/cli.rs` holds two plain enums, not traits. `Vendor` lists stores that *sell
bundles*, which is what `sales` reads. `Store` lists stores this program can
address by name — the one whose library `gamelib` snapshots, and the one a game
belongs to when `add_project_entry` has only a bare id. They are separate
because the two sets do not overlap, and merging them would offer combinations
on the command line that do not exist. Adding a store means adding a variant and
its match arm in `src/main.rs`; the compiler names every place that needs
touching, and the help text builds its own list from the enum.

Each store gets its **own handler**, sharing only helpers. `humble` and
`fanatical` never meet in one function, and neither do `gamelib::steam` and
`gamelib::epic` — one reads a documented HTTP API, the other runs an external
program, and a shared code path would have to keep pretending those are the same
shape. What *is* shared is the part that should be: every game prints through
one `render::Preview`, and every snapshot uses one `Snapshot` writer.

Note that the sanctioned sources in this space are aggregators rather than
storefronts, so a "one store per implementation" shape is probably the wrong one
to reach for first.

## Layout

| Path | Holds |
|------|-------|
| `src/lib.rs` | Crate root and public re-exports. |
| `src/model.rs` | Store-agnostic types: `Bundle`, `Game`, `Listing`, `Problem`, `Money`, `Price`. |
| `src/error.rs` | `Error`, separating transport failures from schema failures. |
| `src/render.rs` | Plain-text rendering of a `Listing`, including bundle deadlines. |
| `src/clock.rs` | UTC instants and the two stores' timestamp formats. No date crate. |
| `src/steam/` | Talking to Steam: links, app-id parsing, and `store.rs` for fetching one app. |
| `src/inventory/steam/` | Games as data, one module per category; `regular.rs` is generated. |
| `src/inventory/steam/source.rs` | Writing an entry back out as Rust source. Used by both the generator and `add_project_entry`. |
| `src/{epic,itch,gamejolt}_inventory/` | Placeholders for other stores. |
| `src/discounted_sales/fanatical/` | Fanatical: `api/all` for the listing, MCP for contents. |
| `src/gamelib/` | Snapshots of your own libraries: `mod.rs` writes them, `steam.rs` and `epic.rs` read them, `config.rs` finds the credentials. |
| `src/bin/steam_lookup.rs` | Generates `inventory/steam/regular.rs`. Not part of the library. |
| `src/bin/epic_lookup.rs` | Generates `inventory/epic/giveaways.rs` from a snapshot. Not part of the library. |
| `docs/` | Dated measurements: how big Steam is, what it costs to collect. |
| `src/bin/steam_tags.rs` | Generates `inventory/steam/tags.rs`. Not part of the library. |
| `src/discounted_sales/humble/parse.rs` | Pure page → `Bundle` parsing, and the consistency checks. |
| `src/discounted_sales/humble/schema.rs` | Serde mirrors of Humble's embedded JSON. |
| `src/discounted_sales/humble/client.rs` | The networked half: fetching and pacing. |
| `src/cli.rs`, `src/main.rs` | Command line; binary only, so the library does not carry clap. |
| `src/logger.rs` | The `-v` stderr sink. Binary only; consumers bring their own. |
| `src/picker.rs` | The `sales` form. Binary only, for the reason clap is: it draws on a terminal. |
| `src/links.rs` | The shell script that opens the picked pages. Returns text; the binary writes it. |
| `src/cache.rs` | Saved listings: what makes one usable, and what retires it. |
| `src/crossover.rs` | Which Steam games an Epic account already holds. Belongs to neither inventory. |
| `src/write.rs` | Replacing a file atomically. Used by the cache and by `gamelib`. |
| `tests/` | Behaviour tests against captured fixtures, plus the live check. |

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
