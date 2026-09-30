//! Steam's store categories, as Valve publishes them. **Generated — do not edit.**
//!
//! Rewritten by `cargo run --release --features tools --bin steam_catalogue -- <out-dir>`,
//! which fetches the table from Valve before it describes any game. Edit that binary, not
//! this file: the next sweep overwrites whatever is here.

/// Which of Valve's three per-game arrays an id belongs to.
///
/// Recorded once per id rather than per game, which is what lets the `feats` column of
/// `steam-games.tsv` merge the three arrays into one sorted list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// Demo, DLC, Mods — a product kind, never in a game's own feature list.
    Product,
    /// Valve's `supported_player_categoryids`: Single-player, Co-op, and so on.
    PlayerMode,
    /// Valve's `feature_categoryids`: Achievements, Cloud, the accessibility set.
    Feature,
    /// Valve's `controller_categoryids`: full support, DualSense, Steam Input.
    Controller,
    /// A kind Valve has added since this table was generated.
    Unknown,
}

impl Kind {
    /// Valve's own number for the kind.
    const fn of(value: u32) -> Self {
        match value {
            0 => Self::Product,
            1 => Self::PlayerMode,
            2 => Self::Feature,
            3 => Self::Controller,
            _ => Self::Unknown,
        }
    }
}

/// Every category Valve publishes, ascending by id so [`of`] can binary-search it.
///
/// **Names are not unique.** 30 and 51 are both "Steam Workshop", 55 and 56 both
/// "DualShock Controller Support", 57 and 58 both "DualSense Controller Support". This
/// is a lookup from id to name and never the reverse.
pub const CATEGORIES: &[(u32, u32, &str)] = &[
    (1, 1, "Multi-player"),
    (2, 1, "Single-player"),
    (6, 0, "Mods (require HL2)"),
    (7, 0, "Mods (require HL1)"),
    (8, 2, "Valve Anti-Cheat enabled"),
    (9, 1, "Co-op"),
    (10, 0, "Game demo"),
    (13, 2, "Captions available"),
    (14, 2, "Commentary available"),
    (15, 2, "Stats"),
    (16, 2, "Includes Source SDK"),
    (17, 2, "Includes level editor"),
    (18, 3, "Partial Controller Support"),
    (19, 0, "Mods"),
    (20, 1, "MMO"),
    (21, 0, "Downloadable Content"),
    (22, 2, "Steam Achievements"),
    (23, 2, "Steam Cloud"),
    (24, 1, "Shared/Split Screen"),
    (25, 2, "Steam Leaderboards"),
    (27, 1, "Cross-Platform Multiplayer"),
    (28, 3, "Full controller support"),
    (29, 2, "Steam Trading Cards"),
    (30, 2, "Steam Workshop"),
    (31, 2, "VR Support"),
    (32, 2, "Steam Turn Notifications"),
    (35, 2, "In-App Purchases"),
    (36, 1, "Online PvP"),
    (37, 1, "Shared/Split Screen PvP"),
    (38, 1, "Online Co-op"),
    (39, 1, "Shared/Split Screen Co-op"),
    (40, 2, "SteamVR Collectibles"),
    (41, 2, "Remote Play on Phone"),
    (42, 2, "Remote Play on Tablet"),
    (43, 2, "Remote Play on TV"),
    (44, 2, "Remote Play Together"),
    (47, 1, "LAN PvP"),
    (48, 1, "LAN Co-op"),
    (49, 1, "PvP"),
    (50, 2, "Additional High-Quality Audio"),
    (51, 2, "Steam Workshop"),
    (52, 2, "Tracked Controller Support"),
    (53, 2, "VR Supported"),
    (54, 2, "VR Only"),
    (55, 3, "DualShock Controller Support"),
    (56, 3, "DualShock Controller Support"),
    (57, 3, "DualSense Controller Support"),
    (58, 3, "DualSense Controller Support"),
    (59, 3, "Steam Input API Support"),
    (60, 3, "Gamepad Recommended"),
    (61, 2, "HDR available"),
    (62, 2, "Family Sharing"),
    (63, 2, "Steam Timeline"),
    (64, 2, "Adjustable Text Size"),
    (65, 2, "Subtitle Options"),
    (66, 2, "Color Alternatives"),
    (67, 2, "Camera Comfort"),
    (68, 2, "Custom Volume Controls"),
    (69, 2, "Stereo Sound"),
    (70, 2, "Surround Sound"),
    (71, 2, "Narrated Game Menus"),
    (72, 2, "Chat Speech-to-text"),
    (73, 2, "Chat Text-to-speech"),
    (74, 2, "Playable without Timed Input"),
    (75, 2, "Keyboard Only Option"),
    (76, 2, "Mouse Only Option"),
    (77, 2, "Touch Only Option"),
    (78, 2, "Adjustable Difficulty"),
    (79, 2, "Save Anytime"),
    (80, 2, "#category_playable_at_your_own_pace"),
    (81, 2, "#category_playable_without_vision"),
    (82, 2, "#category_contrast_controls"),
];

/// What Valve calls this category and which array it comes from, or `None` for an id
/// this table does not hold.
#[must_use]
pub fn of(categoryid: u32) -> Option<(Kind, &'static str)> {
    CATEGORIES
        .binary_search_by_key(&categoryid, |(id, ..)| *id)
        .ok()
        .map(|at| (Kind::of(CATEGORIES[at].1), CATEGORIES[at].2))
}

/// What Valve calls this category, for a caller that does not need the kind.
#[must_use]
pub fn name(categoryid: u32) -> Option<&'static str> {
    of(categoryid).map(|(_, named)| named)
}
