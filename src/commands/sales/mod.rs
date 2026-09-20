//! The `sales` command: what the bundle sites are selling today, and what was read last time.
//!
//! Named for the command because that is exactly who uses it — [`humble`], [`fanatical`] and
//! [`cache`] have one caller between them, the `sales` run. A wider name was tried and did not
//! earn itself: [`crate::steam`] IS mined the same way, but it serves rendering, the inventory
//! and two generators as well, so grouping it here would have put a crate-wide client inside one
//! command's folder.
//!
//! One module per site, and **they are not behind a shared trait**. What they have in common is
//! the shape of the answer — a [`crate::Listing`] of [`crate::Bundle`]s — and nothing else: Humble
//! publishes JSON embedded in a page and no Steam app-ids at all, while Fanatical answers a
//! JSON-RPC service over server-sent events and publishes ids for most of what it sells. A trait
//! over two members that share no mechanism would be a vocabulary for talking about the
//! difference rather than a way of hiding it; [`crate::cli::Vendor`] does the choosing instead.
//!
//! Each site's terms are its own business and are documented in its own module. Read them before
//! shipping anything that runs these.

pub mod cache;
pub mod fanatical;
pub mod humble;
