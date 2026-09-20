# Test fixtures

`games-index.html` and `bundle-page.html` each carry a **verbatim capture** of
the JSON Humble Bundle embedded in the corresponding live page on 2026-09-11,
wrapped in a minimal HTML document. The JSON is unedited; only the surrounding
markup is synthetic, which keeps the files reviewable (~60KB and ~85KB instead
of ~540KB each) while still exercising the real extraction path.

Two things to know before trusting them:

- **They are one region's capture.** Humble shapes this payload by requester
  geography — the payload carries `isEuCountry`, `ipInChina` and a currency —
  so a capture taken elsewhere will differ in pricing and availability fields.
  Bundle and game titles are not expected to vary, and nothing parsed here
  depends on the region-specific fields.
- **They are frozen, and that is the point.** They pin the schema as it stood
  on the capture date so the suite runs offline and deterministically. They
  cannot tell you that Humble has *changed* the schema since — only the live
  test (`cargo test -- --ignored`) does that.

To recapture, save a page and keep the inner text of its
`<script type="application/json">` block: `landingPage-json-data` on `/games`,
`webpack-bundle-page-data` on a bundle page.

## `gamelib/`

Unlike the files above, these are **hand-written to the documented response
shape**, not captures — they describe one person's own library, and a real one
would be someone's purchase history checked into a public repository.

- `steam-owned-games.json` — `IPlayerService/GetOwnedGames/v1`, with
  `include_appinfo=1`. Deliberately uneven: a played game, an unplayed one
  (`playtime_forever: 0`), and one row with no playtime field at all, because
  those three must not collapse into each other.
- `steam-wishlist.json` — `IWishlistService/GetWishlist/v1`, which publishes
  app-ids and ranks and no titles.
- `steam-withheld.json` — `{"response":{}}`, **verbatim**, as the live endpoint
  answered on 2026-09-16 for a profile whose game details are not public. This
  is the response that must never be written out as an empty snapshot.
- `legendary-list.json` — `legendary list --json`, carrying every field of
  Legendary's game model plus the nested `dlcs` list that must stay out of the
  game count.

`fanatical/mcp-search-bundles.sse` is a **verbatim capture** of one
`search_products` answer from 2026-09-17, cut down to five of its thirty-one
results and to the fields this crate reads. It holds one of each shape that
matters: a fixed-price `game-bundle` whose contents list includes the bundle
itself, another whose count and list agree, a `mystery-bundle` that must be
excluded by its declared type rather than by its name, and two pick-and-mix
bundles that the catalogue already supplies and must therefore not be listed
twice.
