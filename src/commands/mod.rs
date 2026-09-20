//! The two things this program does: read what sites are selling, and record what an account owns.
//!
//! Grouped by the work. [`sales`] reads what the bundle sites are offering; [`gamelib`] reads an
//! account's own library and wishlist instead of a catalogue. Each holds only what its own
//! command uses — [`crate::steam`] stayed out because four unrelated callers need it.
//!
//! Both reach out over the network, which is what separates them from everything else in the
//! crate: [`crate::inventory`] and [`crate::store_inventory`] hold what these two brought back,
//! and [`crate::render`] turns it into something to read.

pub mod gamelib;
pub mod sales;
