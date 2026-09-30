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

Captured 2026-09-29, verbatim:

- `getitems-13009-4278390-620.json` — `IStoreBrowseService/GetItems/v1` with `data_request: {}`,
  asking three ids each as `appid`, `bundleid` and `packageid` (nine answers, in request order):
  13009 is a bundle only (Iceborne Digital Deluxe), 4278390 is nothing under any kind (a removed
  app), and 620 is BOTH app 620 (Portal 2) and package 620, whose `store_url_path` is another
  app's page. A refused probe answers `success: 15` with an empty name and a path built from the
  id alone (`app/0/`, `bundle/4278390/`), which is why a path is taken only under `success: 1`.
