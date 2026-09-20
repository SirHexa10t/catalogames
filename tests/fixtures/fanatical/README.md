# Fanatical fixtures

Captured 2026-09-14 against the live service.

- `all-en.json` — the `pickandmix` array from `https://www.fanatical.com/api/all/en`,
  trimmed to five bundles spanning the `type` values that matter (`bundle` plus two
  non-game types). Nothing inside an entry is edited; only the list is shortened, and
  the surrounding keys of the 777KB response are dropped.
- `mcp-get-products.sse` — a verbatim MCP response, still SSE-framed, from a
  `tools/call` of `get_products` over three real slugs. It carries the three cases that
  matter: two products with a `steam_id`, and an edition product whose `steam_id` is
  null but which has a `parent` to fall back to.

**Why these two URLs and not others.** They were found by driving a headless browser and
watching what the page actually requested — the served HTML is a JavaScript shell with no
embedded data, so no amount of reading it would have revealed them. That method is not
reproducible from the shipped code, which is exactly why these captures exist: so the next
person does not have to re-run a browser to learn where the data lives.

Product detail comes from Fanatical's MCP server rather than from
`/api/pick-and-mix/<slug>/en`, which also works — see the module documentation for why.
