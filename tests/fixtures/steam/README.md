# Steam store fixtures

Captured 2026-09-15 for app **1202130** (Starship Troopers: Terran Command), verbatim:

- `1202130-appdetails.json` — `api/appdetails` with the crate's named filter list.
- `1202130-appreviews.json` — `appreviews` with `language=all&purchase_type=all`,
  `num_per_page=0`.
- `1202130-deck.json` — the Steam Deck compatibility report (`resolved_category` 2, Playable).
- `1202130-page.html` — the store page under `l=english` with the age-gate cookie set. Kept
  whole (181 KB) rather than trimmed: the parser runs an HTML parser over the entire document,
  and a trimmed page would test a document Steam never served.

One region's capture, one day's figures — the same caveats as `../README.md`.
