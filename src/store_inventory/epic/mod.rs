//! A snapshot of Epic's catalogue. **Not built — notes only.**
//!
//! Deliberately left as bullet points: Epic's data model is not understood well enough here to
//! commit to a shape, and Steam is the half being built first. What follows is what has been
//! measured, so the next person starts from facts rather than from a blank page.
//!
//! # Epic is the easy half, and here is why
//!
//! - **The catalogue is 26 times smaller.** 7,008 base games against Steam's 187,144. Everything
//!   Epic sells, including add-ons and editions, is 27,695.
//! - **So a full sweep is 176 requests.** Paging is capped hard at 40 per request — asking for
//!   100, 200 or 1000 returns exactly 40 — but at that size the whole base-game catalogue is a
//!   few minutes' work. All 27,695 offers is 693 requests.
//! - **It is one keyless GraphQL POST**, `store.epicgames.com/graphql`, no account and no key.
//! - **Tags arrive as id AND name in the same response.** Steam returns tag ids and needs a
//!   separate 446-entry dictionary joined onto them; Epic needs no join at all.
//! - **There is no band problem, because there is no band.** `criticReviews`, `productRating`
//!   and `reviews` are absent from the schema — not empty, absent. Epic publishes no review data
//!   through this API, so none of the population trouble that complicates the Steam side exists
//!   here. A quality signal for an Epic title has to come from the Steam side by identifier, or
//!   not at all.
//!
//! # What a sweep yields, per element
//!
//! `title`, `id`, `namespace`, `developerDisplayName`, `publisherDisplayName`, `releaseDate` and
//! `effectiveDate` as ISO-8601 with an explicit `Z`, `seller`, `tags { id name }`,
//! `categories { path }`, `price(country:)`, `countriesBlacklist`, `offerMappings { pageSlug }`,
//! `productSlug`, `isCodeRedemptionOnly`, `prePurchase` and `status`.
//!
//! # Two traps already found
//!
//! - **Unreleased titles carry a sentinel date**, `2099-12-29T05:00:00.000Z`, rather than null.
//!   Sorting by release date without catching it puts every unreleased game at the end of time.
//! - **`price` needs a `country` argument.** Without one the field looks absent, which is a wrong
//!   "field does not exist" that already cost one investigation an afternoon.
//!
//! # Why a sweep beats asking about the games we hold
//!
//! Matching an account's games to Epic's catalogue could be done by searching Epic for each title
//! we hold. A bulk sweep plus a LOCAL match gets the same answer and tells Epic nothing about
//! anyone: the queries describe the catalogue, not the person. Prefer the sweep for that reason
//! alone, quite apart from it being fewer requests and reusable.
