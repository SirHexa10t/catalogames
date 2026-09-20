//! Every tag Steam publishes, as an enum.
//!
//! GENERATED — do not hand-edit; regenerate and review the diff:
//!
//! ```text
//! cargo run --release --features tools --bin steam_tags > src/inventory/steam/tags.rs
//! ```
//!
//! Tags are an enum rather than strings so that a misspelt tag cannot compile and a
//! `match` over them can be exhaustive. The cost is that a tag Valve adds is unknown
//! here until this is regenerated — which is why the game generator treats an
//! unrecognised tag as a failure rather than discarding it.
//!
//! Variant names are the display names with non-alphanumerics removed and each word
//! capitalised; names beginning with a digit take a leading underscore, since Rust
//! forbids an identifier starting with one.

#![allow(clippy::upper_case_acronyms, non_camel_case_types)]

/// A Steam tag. 446 of them, as of the last regeneration.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Tag {
    /// "Strategy", tag id 9.
    Strategy,
    /// "Action", tag id 19.
    Action,
    /// "Adventure", tag id 21.
    Adventure,
    /// "Design & Illustration", tag id 84.
    DesignIllustration,
    /// "Utilities", tag id 87.
    Utilities,
    /// "Free to Play", tag id 113.
    FreeToPlay,
    /// "RPG", tag id 122.
    RPG,
    /// "Massively Multiplayer", tag id 128.
    MassivelyMultiplayer,
    /// "Indie", tag id 492.
    Indie,
    /// "Early Access", tag id 493.
    EarlyAccess,
    /// "Casual", tag id 597.
    Casual,
    /// "Simulation", tag id 599.
    Simulation,
    /// "Racing", tag id 699.
    Racing,
    /// "Sports", tag id 701.
    Sports,
    /// "Video Production", tag id 784.
    VideoProduction,
    /// "Photo Editing", tag id 809.
    PhotoEditing,
    /// "Animation & Modeling", tag id 872.
    AnimationModeling,
    /// "Audio Production", tag id 1027.
    AudioProduction,
    /// "Education", tag id 1036.
    Education,
    /// "Software Training", tag id 1445.
    SoftwareTraining,
    /// "Trains", tag id 1616.
    Trains,
    /// "Music", tag id 1621.
    Music,
    /// "Platformer", tag id 1625.
    Platformer,
    /// "Metroidvania", tag id 1628.
    Metroidvania,
    /// "Dogs", tag id 1637.
    Dogs,
    /// "Dog", tag id 1638.
    Dog,
    /// "Building", tag id 1643.
    Building,
    /// "Driving", tag id 1644.
    Driving,
    /// "Tower Defense", tag id 1645.
    TowerDefense,
    /// "Hack and Slash", tag id 1646.
    HackAndSlash,
    /// "Western", tag id 1647.
    Western,
    /// "Satire", tag id 1651.
    Satire,
    /// "Relaxing", tag id 1654.
    Relaxing,
    /// "Zombies", tag id 1659.
    Zombies,
    /// "Survival", tag id 1662.
    Survival,
    /// "FPS", tag id 1663.
    FPS,
    /// "Puzzle", tag id 1664.
    Puzzle,
    /// "Match 3", tag id 1665.
    Match3,
    /// "Card Game", tag id 1666.
    CardGame,
    /// "Horror", tag id 1667.
    Horror,
    /// "Moddable", tag id 1669.
    Moddable,
    /// "4X", tag id 1670.
    _4X,
    /// "Superhero", tag id 1671.
    Superhero,
    /// "Aliens", tag id 1673.
    Aliens,
    /// "Typing", tag id 1674.
    Typing,
    /// "RTS", tag id 1676.
    RTS,
    /// "Turn-Based", tag id 1677.
    TurnBased,
    /// "War", tag id 1678.
    War,
    /// "Heist", tag id 1680.
    Heist,
    /// "Pirates", tag id 1681.
    Pirates,
    /// "Fantasy", tag id 1684.
    Fantasy,
    /// "Co-op", tag id 1685.
    CoOp,
    /// "Stealth", tag id 1687.
    Stealth,
    /// "Ninja", tag id 1688.
    Ninja,
    /// "Classic", tag id 1693.
    Classic,
    /// "Open World", tag id 1695.
    OpenWorld,
    /// "Third Person", tag id 1697.
    ThirdPerson,
    /// "Point & Click", tag id 1698.
    PointClick,
    /// "Crafting", tag id 1702.
    Crafting,
    /// "Tactical", tag id 1708.
    Tactical,
    /// "Surreal", tag id 1710.
    Surreal,
    /// "Psychedelic", tag id 1714.
    Psychedelic,
    /// "Roguelike", tag id 1716.
    Roguelike,
    /// "Hex Grid", tag id 1717.
    HexGrid,
    /// "MOBA", tag id 1718.
    MOBA,
    /// "Comedy", tag id 1719.
    Comedy,
    /// "Dungeon Crawler", tag id 1720.
    DungeonCrawler,
    /// "Psychological Horror", tag id 1721.
    PsychologicalHorror,
    /// "Action RTS", tag id 1723.
    ActionRTS,
    /// "Sokoban", tag id 1730.
    Sokoban,
    /// "Voxel", tag id 1732.
    Voxel,
    /// "Unforgiving", tag id 1733.
    Unforgiving,
    /// "Fast-Paced", tag id 1734.
    FastPaced,
    /// "Hidden Object", tag id 1738.
    HiddenObject,
    /// "Turn-Based Strategy", tag id 1741.
    TurnBasedStrategy,
    /// "Story Rich", tag id 1742.
    StoryRich,
    /// "Fighting", tag id 1743.
    Fighting,
    /// "Basketball", tag id 1746.
    Basketball,
    /// "Comic Book", tag id 1751.
    ComicBook,
    /// "Rhythm", tag id 1752.
    Rhythm,
    /// "Skateboarding", tag id 1753.
    Skateboarding,
    /// "MMORPG", tag id 1754.
    MMORPG,
    /// "Space", tag id 1755.
    Space,
    /// "Great Soundtrack", tag id 1756.
    GreatSoundtrack,
    /// "Perma Death", tag id 1759.
    PermaDeath,
    /// "Board Game", tag id 1770.
    BoardGame,
    /// "Arcade", tag id 1773.
    Arcade,
    /// "Shooter", tag id 1774.
    Shooter,
    /// "PvP", tag id 1775.
    PvP,
    /// "Espionage", tag id 1776.
    Espionage,
    /// "Steampunk", tag id 1777.
    Steampunk,
    /// "Based On A Novel", tag id 3796.
    BasedOnANovel,
    /// "Side Scroller", tag id 3798.
    SideScroller,
    /// "Visual Novel", tag id 3799.
    VisualNovel,
    /// "Sandbox", tag id 3810.
    Sandbox,
    /// "Real Time Tactics", tag id 3813.
    RealTimeTactics,
    /// "Third-Person Shooter", tag id 3814.
    ThirdPersonShooter,
    /// "Exploration", tag id 3834.
    Exploration,
    /// "Post-apocalyptic", tag id 3835.
    PostApocalyptic,
    /// "First-Person", tag id 3839.
    FirstPerson,
    /// "Local Co-Op", tag id 3841.
    LocalCoOp,
    /// "Online Co-Op", tag id 3843.
    OnlineCoOp,
    /// "Lore-Rich", tag id 3854.
    LoreRich,
    /// "Multiplayer", tag id 3859.
    Multiplayer,
    /// "2D", tag id 3871.
    _2D,
    /// "Precision Platformer", tag id 3877.
    PrecisionPlatformer,
    /// "Competitive", tag id 3878.
    Competitive,
    /// "Old School", tag id 3916.
    OldSchool,
    /// "Cooking", tag id 3920.
    Cooking,
    /// "Immersive", tag id 3934.
    Immersive,
    /// "Sci-fi", tag id 3942.
    SciFi,
    /// "Gothic", tag id 3952.
    Gothic,
    /// "Rail Shooter", tag id 3954.
    RailShooter,
    /// "Character Action Game", tag id 3955.
    CharacterActionGame,
    /// "Roguelite", tag id 3959.
    Roguelite,
    /// "Pixel Graphics", tag id 3964.
    PixelGraphics,
    /// "Epic", tag id 3965.
    Epic,
    /// "Physics", tag id 3968.
    Physics,
    /// "Survival Horror", tag id 3978.
    SurvivalHorror,
    /// "Historical", tag id 3987.
    Historical,
    /// "Combat", tag id 3993.
    Combat,
    /// "Retro", tag id 4004.
    Retro,
    /// "Difficult", tag id 4026.
    Difficult,
    /// "Parkour", tag id 4036.
    Parkour,
    /// "Dragons", tag id 4046.
    Dragons,
    /// "Magic", tag id 4057.
    Magic,
    /// "Thriller", tag id 4064.
    Thriller,
    /// "Anime", tag id 4085.
    Anime,
    /// "Minimalist", tag id 4094.
    Minimalist,
    /// "Combat Racing", tag id 4102.
    CombatRacing,
    /// "Action-Adventure", tag id 4106.
    ActionAdventure,
    /// "Cyberpunk", tag id 4115.
    Cyberpunk,
    /// "Funny", tag id 4136.
    Funny,
    /// "Transhumanism", tag id 4137.
    Transhumanism,
    /// "Cinematic", tag id 4145.
    Cinematic,
    /// "World War II", tag id 4150.
    WorldWarII,
    /// "Class-Based", tag id 4155.
    ClassBased,
    /// "Beat 'em up", tag id 4158.
    BeatEmUp,
    /// "Real-Time", tag id 4161.
    RealTime,
    /// "Kids", tag id 4162.
    Kids,
    /// "Atmospheric", tag id 4166.
    Atmospheric,
    /// "Military", tag id 4168.
    Military,
    /// "Medieval", tag id 4172.
    Medieval,
    /// "Realistic", tag id 4175.
    Realistic,
    /// "Singleplayer", tag id 4182.
    Singleplayer,
    /// "Chess", tag id 4184.
    Chess,
    /// "Addictive", tag id 4190.
    Addictive,
    /// "3D", tag id 4191.
    _3D,
    /// "Cartoony", tag id 4195.
    Cartoony,
    /// "Trading", tag id 4202.
    Trading,
    /// "Action RPG", tag id 4231.
    ActionRPG,
    /// "Short", tag id 4234.
    Short,
    /// "Loot", tag id 4236.
    Loot,
    /// "Episodic", tag id 4242.
    Episodic,
    /// "Stylized", tag id 4252.
    Stylized,
    /// "Shoot 'Em Up", tag id 4255.
    ShootEmUp,
    /// "Spaceships", tag id 4291.
    Spaceships,
    /// "Futuristic", tag id 4295.
    Futuristic,
    /// "Colorful", tag id 4305.
    Colorful,
    /// "Turn-Based Combat", tag id 4325.
    TurnBasedCombat,
    /// "City Builder", tag id 4328.
    CityBuilder,
    /// "Dark", tag id 4342.
    Dark,
    /// "Gore", tag id 4345.
    Gore,
    /// "Grand Strategy", tag id 4364.
    GrandStrategy,
    /// "Assassin", tag id 4376.
    Assassin,
    /// "Abstract", tag id 4400.
    Abstract,
    /// "JRPG", tag id 4434.
    JRPG,
    /// "CRPG", tag id 4474.
    CRPG,
    /// "Choose Your Own Adventure", tag id 4486.
    ChooseYourOwnAdventure,
    /// "Co-op Campaign", tag id 4508.
    CoOpCampaign,
    /// "Farming", tag id 4520.
    Farming,
    /// "Dwarves", tag id 4535.
    Dwarves,
    /// "Quick-Time Events", tag id 4559.
    QuickTimeEvents,
    /// "Cartoon", tag id 4562.
    Cartoon,
    /// "Alternate History", tag id 4598.
    AlternateHistory,
    /// "Dark Fantasy", tag id 4604.
    DarkFantasy,
    /// "Swordplay", tag id 4608.
    Swordplay,
    /// "Top-Down Shooter", tag id 4637.
    TopDownShooter,
    /// "Violent", tag id 4667.
    Violent,
    /// "Wargame", tag id 4684.
    Wargame,
    /// "Economy", tag id 4695.
    Economy,
    /// "Replay Value", tag id 4711.
    ReplayValue,
    /// "Cute", tag id 4726.
    Cute,
    /// "2D Fighter", tag id 4736.
    _2DFighter,
    /// "Character Customization", tag id 4747.
    CharacterCustomization,
    /// "Twin Stick Shooter", tag id 4758.
    TwinStickShooter,
    /// "Spectacle fighter", tag id 4777.
    SpectacleFighter,
    /// "Top-Down", tag id 4791.
    TopDown,
    /// "Mechs", tag id 4821.
    Mechs,
    /// "6DOF", tag id 4835.
    _6DOF,
    /// "4 Player Local", tag id 4840.
    _4PlayerLocal,
    /// "Capitalism", tag id 4845.
    Capitalism,
    /// "Billiards", tag id 4852.
    Billiards,
    /// "Parody ", tag id 4878.
    Parody,
    /// "Bullet Hell", tag id 4885.
    BulletHell,
    /// "Romance", tag id 4947.
    Romance,
    /// "2.5D", tag id 4975.
    _25D,
    /// "Naval Combat", tag id 4994.
    NavalCombat,
    /// "Dystopian ", tag id 5030.
    Dystopian,
    /// "eSports", tag id 5055.
    ESports,
    /// "Procedural Generation", tag id 5125.
    ProceduralGeneration,
    /// "Score Attack", tag id 5154.
    ScoreAttack,
    /// "Dinosaurs", tag id 5160.
    Dinosaurs,
    /// "Cold War", tag id 5179.
    ColdWar,
    /// "Psychological", tag id 5186.
    Psychological,
    /// "Blood", tag id 5228.
    Blood,
    /// "Sequel", tag id 5230.
    Sequel,
    /// "God Game", tag id 5300.
    GodGame,
    /// "Mod", tag id 5348.
    Mod,
    /// "Family Friendly", tag id 5350.
    FamilyFriendly,
    /// "Destruction", tag id 5363.
    Destruction,
    /// "Conspiracy", tag id 5372.
    Conspiracy,
    /// "2D Platformer", tag id 5379.
    _2DPlatformer,
    /// "World War I", tag id 5382.
    WorldWarI,
    /// "Time Attack", tag id 5390.
    TimeAttack,
    /// "3D Platformer", tag id 5395.
    _3DPlatformer,
    /// "Benchmark", tag id 5407.
    Benchmark,
    /// "Beautiful", tag id 5411.
    Beautiful,
    /// "Programming", tag id 5432.
    Programming,
    /// "Hacking", tag id 5502.
    Hacking,
    /// "Puzzle Platformer", tag id 5537.
    PuzzlePlatformer,
    /// "Arena Shooter", tag id 5547.
    ArenaShooter,
    /// "Emotional", tag id 5608.
    Emotional,
    /// "Detective", tag id 5613.
    Detective,
    /// "Collectathon", tag id 5652.
    Collectathon,
    /// "Modern", tag id 5673.
    Modern,
    /// "Remake", tag id 5708.
    Remake,
    /// "Team-Based", tag id 5711.
    TeamBased,
    /// "Mystery", tag id 5716.
    Mystery,
    /// "Baseball", tag id 5727.
    Baseball,
    /// "Robots", tag id 5752.
    Robots,
    /// "Gun Customization", tag id 5765.
    GunCustomization,
    /// "Science", tag id 5794.
    Science,
    /// "Bullet Time", tag id 5796.
    BulletTime,
    /// "Isometric", tag id 5851.
    Isometric,
    /// "Walking Simulator", tag id 5900.
    WalkingSimulator,
    /// "Tennis", tag id 5914.
    Tennis,
    /// "Dark Humor", tag id 5923.
    DarkHumor,
    /// "Reboot", tag id 5941.
    Reboot,
    /// "Mining", tag id 5981.
    Mining,
    /// "Horses", tag id 6041.
    Horses,
    /// "Noir", tag id 6052.
    Noir,
    /// "Elves", tag id 6054.
    Elves,
    /// "Logic", tag id 6129.
    Logic,
    /// "Birds", tag id 6214.
    Birds,
    /// "Inventory Management", tag id 6276.
    InventoryManagement,
    /// "Diplomacy", tag id 6310.
    Diplomacy,
    /// "Crime", tag id 6378.
    Crime,
    /// "Choices Matter", tag id 6426.
    ChoicesMatter,
    /// "3D Fighter", tag id 6506.
    _3DFighter,
    /// "Pinball", tag id 6621.
    Pinball,
    /// "Time Manipulation", tag id 6625.
    TimeManipulation,
    /// "Nudity", tag id 6650.
    Nudity,
    /// "1990's", tag id 6691.
    _1990s,
    /// "Mars", tag id 6702.
    Mars,
    /// "PvE", tag id 6730.
    PvE,
    /// "Hand-drawn", tag id 6815.
    HandDrawn,
    /// "Poker", tag id 6835.
    Poker,
    /// "Nonlinear", tag id 6869.
    Nonlinear,
    /// "Naval", tag id 6910.
    Naval,
    /// "Martial Arts", tag id 6915.
    MartialArts,
    /// "Rome", tag id 6948.
    Rome,
    /// "Multiple Endings", tag id 6971.
    MultipleEndings,
    /// "Golf", tag id 7038.
    Golf,
    /// "Real-Time with Pause", tag id 7107.
    RealTimeWithPause,
    /// "Party", tag id 7108.
    Party,
    /// "Party Game", tag id 7178.
    PartyGame,
    /// "Female Protagonist", tag id 7208.
    FemaleProtagonist,
    /// "Linear", tag id 7250.
    Linear,
    /// "Skiing", tag id 7309.
    Skiing,
    /// "Bowling", tag id 7328.
    Bowling,
    /// "Base Building", tag id 7332.
    BaseBuilding,
    /// "Local Multiplayer", tag id 7368.
    LocalMultiplayer,
    /// "Sniper", tag id 7423.
    Sniper,
    /// "Lovecraftian", tag id 7432.
    Lovecraftian,
    /// "Controller", tag id 7481.
    Controller,
    /// "Dice", tag id 7556.
    Dice,
    /// "Grid-Based Movement", tag id 7569.
    GridBasedMovement,
    /// "Offroad", tag id 7622.
    Offroad,
    /// "Narrative", tag id 7702.
    Narrative,
    /// "1980s", tag id 7743.
    _1980s,
    /// "Dwarf", tag id 7918.
    Dwarf,
    /// "Artificial Intelligence", tag id 7926.
    ArtificialIntelligence,
    /// "Soundtrack", tag id 7948.
    Soundtrack,
    /// "Software", tag id 8013.
    Software,
    /// "TrackIR", tag id 8075.
    TrackIR,
    /// "Minigames", tag id 8093.
    Minigames,
    /// "Level Editor", tag id 8122.
    LevelEditor,
    /// "Music-Based Procedural Generation", tag id 8253.
    MusicBasedProceduralGeneration,
    /// "Investigation", tag id 8369.
    Investigation,
    /// "Runner", tag id 8666.
    Runner,
    /// "Resource Management", tag id 8945.
    ResourceManagement,
    /// "Hentai", tag id 9130.
    Hentai,
    /// "Underwater", tag id 9157.
    Underwater,
    /// "Immersive Sim", tag id 9204.
    ImmersiveSim,
    /// "Trading Card Game", tag id 9271.
    TradingCardGame,
    /// "Demons", tag id 9541.
    Demons,
    /// "Dating Sim", tag id 9551.
    DatingSim,
    /// "Hunting", tag id 9564.
    Hunting,
    /// "Dynamic Narration", tag id 9592.
    DynamicNarration,
    /// "Animals", tag id 9626.
    Animals,
    /// "Snow", tag id 9803.
    Snow,
    /// "Life Sim", tag id 10235.
    LifeSim,
    /// "Transportation", tag id 10383.
    Transportation,
    /// "Memes", tag id 10397.
    Memes,
    /// "Trivia", tag id 10437.
    Trivia,
    /// "Samurai", tag id 10617.
    Samurai,
    /// "Time Travel", tag id 10679.
    TimeTravel,
    /// "Party-Based RPG", tag id 10695.
    PartyBasedRPG,
    /// "Supernatural", tag id 10808.
    Supernatural,
    /// "Split Screen", tag id 10816.
    SplitScreen,
    /// "Interactive Fiction", tag id 11014.
    InteractiveFiction,
    /// "Boss Rush", tag id 11095.
    BossRush,
    /// "Vehicular Combat", tag id 11104.
    VehicularCombat,
    /// "Mouse Only", tag id 11123.
    MouseOnly,
    /// "Villain Protagonist", tag id 11333.
    VillainProtagonist,
    /// "Vikings", tag id 11634.
    Vikings,
    /// "Tutorial", tag id 12057.
    Tutorial,
    /// "Sexual Content", tag id 12095.
    SexualContent,
    /// "Boxing", tag id 12190.
    Boxing,
    /// "Management", tag id 12472.
    Management,
    /// "Vampires", tag id 12686.
    Vampires,
    /// "Solitaire", tag id 13070.
    Solitaire,
    /// "Tanks", tag id 13276.
    Tanks,
    /// "Archery", tag id 13382.
    Archery,
    /// "Sailing", tag id 13577.
    Sailing,
    /// "Experimental", tag id 13782.
    Experimental,
    /// "Game Development", tag id 13906.
    GameDevelopment,
    /// "Turn-Based Tactics", tag id 14139.
    TurnBasedTactics,
    /// "Nostalgia", tag id 14720.
    Nostalgia,
    /// "Intentionally Awkward Controls", tag id 14906.
    IntentionallyAwkwardControls,
    /// "Flight", tag id 15045.
    Flight,
    /// "Conversation", tag id 15172.
    Conversation,
    /// "Philosophical", tag id 15277.
    Philosophical,
    /// "Fishing", tag id 15564.
    Fishing,
    /// "Motocross", tag id 15868.
    Motocross,
    /// "Silent Protagonist", tag id 15954.
    SilentProtagonist,
    /// "Mythology", tag id 16094.
    Mythology,
    /// "Gambling", tag id 16250.
    Gambling,
    /// "Space Sim", tag id 16598.
    SpaceSim,
    /// "Time Management", tag id 16689.
    TimeManagement,
    /// "Werewolves", tag id 17015.
    Werewolves,
    /// "Strategy RPG", tag id 17305.
    StrategyRPG,
    /// "Lemmings", tag id 17337.
    Lemmings,
    /// "Tabletop", tag id 17389.
    Tabletop,
    /// "Asynchronous Multiplayer", tag id 17770.
    AsynchronousMultiplayer,
    /// "Cats", tag id 17894.
    Cats,
    /// "Pool", tag id 17927.
    Pool,
    /// "FMV", tag id 18594.
    FMV,
    /// "Cycling", tag id 19568.
    Cycling,
    /// "Submarine", tag id 19780.
    Submarine,
    /// "Dark Comedy", tag id 19995.
    DarkComedy,
    /// "Wolves", tag id 20486.
    Wolves,
    /// "Underground", tag id 21006.
    Underground,
    /// "Language Learning", tag id 21635.
    LanguageLearning,
    /// "Tactical RPG", tag id 21725.
    TacticalRPG,
    /// "VR", tag id 21978.
    VR,
    /// "Agriculture", tag id 22602.
    Agriculture,
    /// "Mini Golf", tag id 22955.
    MiniGolf,
    /// "Cleaning", tag id 23491.
    Cleaning,
    /// "Word Game", tag id 24003.
    WordGame,
    /// "Touch-Friendly", tag id 25085.
    TouchFriendly,
    /// "Wuxia", tag id 25959.
    Wuxia,
    /// "Political Sim", tag id 26921.
    PoliticalSim,
    /// "Voice Control", tag id 27758.
    VoiceControl,
    /// "Snowboarding", tag id 28444.
    Snowboarding,
    /// "Souls-like", tag id 29482.
    SoulsLike,
    /// "Nature", tag id 30358.
    Nature,
    /// "Fox", tag id 30927.
    Fox,
    /// "Text-Based", tag id 31275.
    TextBased,
    /// "Otome", tag id 31579.
    Otome,
    /// "Deckbuilding", tag id 32322.
    Deckbuilding,
    /// "Mahjong", tag id 33572.
    Mahjong,
    /// "Job Simulator", tag id 35079.
    JobSimulator,
    /// "Falling Blocks", tag id 37376.
    FallingBlocks,
    /// "Combat Flight Simulator", tag id 37799.
    CombatFlightSimulator,
    /// "Sexual Themes", tag id 40500.
    SexualThemes,
    /// "Jump Scare", tag id 42089.
    JumpScare,
    /// "Dialogue Heavy", tag id 42152.
    DialogueHeavy,
    /// "Coding", tag id 42329.
    Coding,
    /// "Action Roguelike", tag id 42804.
    ActionRoguelike,
    /// "LGBTQ+", tag id 44868.
    LGBTQ,
    /// "Zoo", tag id 46348.
    Zoo,
    /// "Wrestling", tag id 47827.
    Wrestling,
    /// "Rugby", tag id 49213.
    Rugby,
    /// "Cult", tag id 52406.
    Cult,
    /// "On-Rails Shooter", tag id 56690.
    OnRailsShooter,
    /// "Electronic Music", tag id 61357.
    ElectronicMusic,
    /// "Spelling", tag id 71389.
    Spelling,
    /// "Farming Sim", tag id 87918.
    FarmingSim,
    /// "Shop Keeper", tag id 91114.
    ShopKeeper,
    /// "Jet", tag id 92092.
    Jet,
    /// "Skating", tag id 96359.
    Skating,
    /// "Assassins", tag id 97070.
    Assassins,
    /// "Cozy", tag id 97376.
    Cozy,
    /// "Elf", tag id 102530.
    Elf,
    /// "8-bit Music", tag id 117648.
    _8BitMusic,
    /// "Bikes", tag id 123332.
    Bikes,
    /// "ATV", tag id 129761.
    ATV,
    /// "Gaming", tag id 150626.
    Gaming,
    /// "Cricket", tag id 158638.
    Cricket,
    /// "Battle Royale", tag id 176981.
    BattleRoyale,
    /// "Faith", tag id 180368.
    Faith,
    /// "Instrumental Music", tag id 189941.
    InstrumentalMusic,
    /// "Mystery Dungeon", tag id 198631.
    MysteryDungeon,
    /// "Motorbike", tag id 198913.
    Motorbike,
    /// "Colony Sim", tag id 220585.
    ColonySim,
    /// "BMX", tag id 252854.
    BMX,
    /// "Automation", tag id 255534.
    Automation,
    /// "Musou", tag id 323922.
    Musou,
    /// "Hockey", tag id 324176.
    Hockey,
    /// "Rock Music", tag id 337964.
    RockMusic,
    /// "Looter Shooter", tag id 353880.
    LooterShooter,
    /// "Snooker", tag id 363767.
    Snooker,
    /// "Clicker", tag id 379975.
    Clicker,
    /// "Traditional Roguelike", tag id 454187.
    TraditionalRoguelike,
    /// "Foxes", tag id 507423.
    Foxes,
    /// "Wholesome", tag id 552282.
    Wholesome,
    /// "Incremental", tag id 560542.
    Incremental,
    /// "Hardware", tag id 603297.
    Hardware,
    /// "Idler", tag id 615955.
    Idler,
    /// "Hero Shooter", tag id 620519.
    HeroShooter,
    /// "Bullet Heaven", tag id 723991.
    BulletHeaven,
    /// "Social Deduction", tag id 745697.
    SocialDeduction,
    /// "Xianxia", tag id 760247.
    Xianxia,
    /// "Escape Room", tag id 769306.
    EscapeRoom,
    /// "360 Video", tag id 776177.
    _360Video,
    /// "Card Battler", tag id 791774.
    CardBattler,
    /// "Volleyball", tag id 847164.
    Volleyball,
    /// "Asymmetric VR", tag id 856791.
    AsymmetricVR,
    /// "Decorating", tag id 889937.
    Decorating,
    /// "Creature Collector", tag id 916648.
    CreatureCollector,
    /// "Boomer Shooter", tag id 1023537.
    BoomerShooter,
    /// "Auto Battler", tag id 1084988.
    AutoBattler,
    /// "Roguelike Deckbuilder", tag id 1091588.
    RoguelikeDeckbuilder,
    /// "Outbreak Sim", tag id 1100686.
    OutbreakSim,
    /// "Automobile Sim", tag id 1100687.
    AutomobileSim,
    /// "Medical Sim", tag id 1100688.
    MedicalSim,
    /// "Open World Survival Craft", tag id 1100689.
    OpenWorldSurvivalCraft,
    /// "Extraction Shooter", tag id 1199779.
    ExtractionShooter,
    /// "Hobby Sim", tag id 1220528.
    HobbySim,
    /// "Organizing", tag id 1239876.
    Organizing,
    /// "Football (Soccer)", tag id 1254546.
    FootballSoccer,
    /// "Football (American)", tag id 1254552.
    FootballAmerican,
    /// "Desktop Companion", tag id 1320952.
    DesktopCompanion,
    /// "Capybaras", tag id 1352486.
    Capybaras,
}

impl Tag {
    /// Steam's own id for this tag.
    #[rustfmt::skip]
    pub fn id(self) -> u32 {
        match self {
            Self::Strategy => 9,
            Self::Action => 19,
            Self::Adventure => 21,
            Self::DesignIllustration => 84,
            Self::Utilities => 87,
            Self::FreeToPlay => 113,
            Self::RPG => 122,
            Self::MassivelyMultiplayer => 128,
            Self::Indie => 492,
            Self::EarlyAccess => 493,
            Self::Casual => 597,
            Self::Simulation => 599,
            Self::Racing => 699,
            Self::Sports => 701,
            Self::VideoProduction => 784,
            Self::PhotoEditing => 809,
            Self::AnimationModeling => 872,
            Self::AudioProduction => 1027,
            Self::Education => 1036,
            Self::SoftwareTraining => 1445,
            Self::Trains => 1616,
            Self::Music => 1621,
            Self::Platformer => 1625,
            Self::Metroidvania => 1628,
            Self::Dogs => 1637,
            Self::Dog => 1638,
            Self::Building => 1643,
            Self::Driving => 1644,
            Self::TowerDefense => 1645,
            Self::HackAndSlash => 1646,
            Self::Western => 1647,
            Self::Satire => 1651,
            Self::Relaxing => 1654,
            Self::Zombies => 1659,
            Self::Survival => 1662,
            Self::FPS => 1663,
            Self::Puzzle => 1664,
            Self::Match3 => 1665,
            Self::CardGame => 1666,
            Self::Horror => 1667,
            Self::Moddable => 1669,
            Self::_4X => 1670,
            Self::Superhero => 1671,
            Self::Aliens => 1673,
            Self::Typing => 1674,
            Self::RTS => 1676,
            Self::TurnBased => 1677,
            Self::War => 1678,
            Self::Heist => 1680,
            Self::Pirates => 1681,
            Self::Fantasy => 1684,
            Self::CoOp => 1685,
            Self::Stealth => 1687,
            Self::Ninja => 1688,
            Self::Classic => 1693,
            Self::OpenWorld => 1695,
            Self::ThirdPerson => 1697,
            Self::PointClick => 1698,
            Self::Crafting => 1702,
            Self::Tactical => 1708,
            Self::Surreal => 1710,
            Self::Psychedelic => 1714,
            Self::Roguelike => 1716,
            Self::HexGrid => 1717,
            Self::MOBA => 1718,
            Self::Comedy => 1719,
            Self::DungeonCrawler => 1720,
            Self::PsychologicalHorror => 1721,
            Self::ActionRTS => 1723,
            Self::Sokoban => 1730,
            Self::Voxel => 1732,
            Self::Unforgiving => 1733,
            Self::FastPaced => 1734,
            Self::HiddenObject => 1738,
            Self::TurnBasedStrategy => 1741,
            Self::StoryRich => 1742,
            Self::Fighting => 1743,
            Self::Basketball => 1746,
            Self::ComicBook => 1751,
            Self::Rhythm => 1752,
            Self::Skateboarding => 1753,
            Self::MMORPG => 1754,
            Self::Space => 1755,
            Self::GreatSoundtrack => 1756,
            Self::PermaDeath => 1759,
            Self::BoardGame => 1770,
            Self::Arcade => 1773,
            Self::Shooter => 1774,
            Self::PvP => 1775,
            Self::Espionage => 1776,
            Self::Steampunk => 1777,
            Self::BasedOnANovel => 3796,
            Self::SideScroller => 3798,
            Self::VisualNovel => 3799,
            Self::Sandbox => 3810,
            Self::RealTimeTactics => 3813,
            Self::ThirdPersonShooter => 3814,
            Self::Exploration => 3834,
            Self::PostApocalyptic => 3835,
            Self::FirstPerson => 3839,
            Self::LocalCoOp => 3841,
            Self::OnlineCoOp => 3843,
            Self::LoreRich => 3854,
            Self::Multiplayer => 3859,
            Self::_2D => 3871,
            Self::PrecisionPlatformer => 3877,
            Self::Competitive => 3878,
            Self::OldSchool => 3916,
            Self::Cooking => 3920,
            Self::Immersive => 3934,
            Self::SciFi => 3942,
            Self::Gothic => 3952,
            Self::RailShooter => 3954,
            Self::CharacterActionGame => 3955,
            Self::Roguelite => 3959,
            Self::PixelGraphics => 3964,
            Self::Epic => 3965,
            Self::Physics => 3968,
            Self::SurvivalHorror => 3978,
            Self::Historical => 3987,
            Self::Combat => 3993,
            Self::Retro => 4004,
            Self::Difficult => 4026,
            Self::Parkour => 4036,
            Self::Dragons => 4046,
            Self::Magic => 4057,
            Self::Thriller => 4064,
            Self::Anime => 4085,
            Self::Minimalist => 4094,
            Self::CombatRacing => 4102,
            Self::ActionAdventure => 4106,
            Self::Cyberpunk => 4115,
            Self::Funny => 4136,
            Self::Transhumanism => 4137,
            Self::Cinematic => 4145,
            Self::WorldWarII => 4150,
            Self::ClassBased => 4155,
            Self::BeatEmUp => 4158,
            Self::RealTime => 4161,
            Self::Kids => 4162,
            Self::Atmospheric => 4166,
            Self::Military => 4168,
            Self::Medieval => 4172,
            Self::Realistic => 4175,
            Self::Singleplayer => 4182,
            Self::Chess => 4184,
            Self::Addictive => 4190,
            Self::_3D => 4191,
            Self::Cartoony => 4195,
            Self::Trading => 4202,
            Self::ActionRPG => 4231,
            Self::Short => 4234,
            Self::Loot => 4236,
            Self::Episodic => 4242,
            Self::Stylized => 4252,
            Self::ShootEmUp => 4255,
            Self::Spaceships => 4291,
            Self::Futuristic => 4295,
            Self::Colorful => 4305,
            Self::TurnBasedCombat => 4325,
            Self::CityBuilder => 4328,
            Self::Dark => 4342,
            Self::Gore => 4345,
            Self::GrandStrategy => 4364,
            Self::Assassin => 4376,
            Self::Abstract => 4400,
            Self::JRPG => 4434,
            Self::CRPG => 4474,
            Self::ChooseYourOwnAdventure => 4486,
            Self::CoOpCampaign => 4508,
            Self::Farming => 4520,
            Self::Dwarves => 4535,
            Self::QuickTimeEvents => 4559,
            Self::Cartoon => 4562,
            Self::AlternateHistory => 4598,
            Self::DarkFantasy => 4604,
            Self::Swordplay => 4608,
            Self::TopDownShooter => 4637,
            Self::Violent => 4667,
            Self::Wargame => 4684,
            Self::Economy => 4695,
            Self::ReplayValue => 4711,
            Self::Cute => 4726,
            Self::_2DFighter => 4736,
            Self::CharacterCustomization => 4747,
            Self::TwinStickShooter => 4758,
            Self::SpectacleFighter => 4777,
            Self::TopDown => 4791,
            Self::Mechs => 4821,
            Self::_6DOF => 4835,
            Self::_4PlayerLocal => 4840,
            Self::Capitalism => 4845,
            Self::Billiards => 4852,
            Self::Parody => 4878,
            Self::BulletHell => 4885,
            Self::Romance => 4947,
            Self::_25D => 4975,
            Self::NavalCombat => 4994,
            Self::Dystopian => 5030,
            Self::ESports => 5055,
            Self::ProceduralGeneration => 5125,
            Self::ScoreAttack => 5154,
            Self::Dinosaurs => 5160,
            Self::ColdWar => 5179,
            Self::Psychological => 5186,
            Self::Blood => 5228,
            Self::Sequel => 5230,
            Self::GodGame => 5300,
            Self::Mod => 5348,
            Self::FamilyFriendly => 5350,
            Self::Destruction => 5363,
            Self::Conspiracy => 5372,
            Self::_2DPlatformer => 5379,
            Self::WorldWarI => 5382,
            Self::TimeAttack => 5390,
            Self::_3DPlatformer => 5395,
            Self::Benchmark => 5407,
            Self::Beautiful => 5411,
            Self::Programming => 5432,
            Self::Hacking => 5502,
            Self::PuzzlePlatformer => 5537,
            Self::ArenaShooter => 5547,
            Self::Emotional => 5608,
            Self::Detective => 5613,
            Self::Collectathon => 5652,
            Self::Modern => 5673,
            Self::Remake => 5708,
            Self::TeamBased => 5711,
            Self::Mystery => 5716,
            Self::Baseball => 5727,
            Self::Robots => 5752,
            Self::GunCustomization => 5765,
            Self::Science => 5794,
            Self::BulletTime => 5796,
            Self::Isometric => 5851,
            Self::WalkingSimulator => 5900,
            Self::Tennis => 5914,
            Self::DarkHumor => 5923,
            Self::Reboot => 5941,
            Self::Mining => 5981,
            Self::Horses => 6041,
            Self::Noir => 6052,
            Self::Elves => 6054,
            Self::Logic => 6129,
            Self::Birds => 6214,
            Self::InventoryManagement => 6276,
            Self::Diplomacy => 6310,
            Self::Crime => 6378,
            Self::ChoicesMatter => 6426,
            Self::_3DFighter => 6506,
            Self::Pinball => 6621,
            Self::TimeManipulation => 6625,
            Self::Nudity => 6650,
            Self::_1990s => 6691,
            Self::Mars => 6702,
            Self::PvE => 6730,
            Self::HandDrawn => 6815,
            Self::Poker => 6835,
            Self::Nonlinear => 6869,
            Self::Naval => 6910,
            Self::MartialArts => 6915,
            Self::Rome => 6948,
            Self::MultipleEndings => 6971,
            Self::Golf => 7038,
            Self::RealTimeWithPause => 7107,
            Self::Party => 7108,
            Self::PartyGame => 7178,
            Self::FemaleProtagonist => 7208,
            Self::Linear => 7250,
            Self::Skiing => 7309,
            Self::Bowling => 7328,
            Self::BaseBuilding => 7332,
            Self::LocalMultiplayer => 7368,
            Self::Sniper => 7423,
            Self::Lovecraftian => 7432,
            Self::Controller => 7481,
            Self::Dice => 7556,
            Self::GridBasedMovement => 7569,
            Self::Offroad => 7622,
            Self::Narrative => 7702,
            Self::_1980s => 7743,
            Self::Dwarf => 7918,
            Self::ArtificialIntelligence => 7926,
            Self::Soundtrack => 7948,
            Self::Software => 8013,
            Self::TrackIR => 8075,
            Self::Minigames => 8093,
            Self::LevelEditor => 8122,
            Self::MusicBasedProceduralGeneration => 8253,
            Self::Investigation => 8369,
            Self::Runner => 8666,
            Self::ResourceManagement => 8945,
            Self::Hentai => 9130,
            Self::Underwater => 9157,
            Self::ImmersiveSim => 9204,
            Self::TradingCardGame => 9271,
            Self::Demons => 9541,
            Self::DatingSim => 9551,
            Self::Hunting => 9564,
            Self::DynamicNarration => 9592,
            Self::Animals => 9626,
            Self::Snow => 9803,
            Self::LifeSim => 10235,
            Self::Transportation => 10383,
            Self::Memes => 10397,
            Self::Trivia => 10437,
            Self::Samurai => 10617,
            Self::TimeTravel => 10679,
            Self::PartyBasedRPG => 10695,
            Self::Supernatural => 10808,
            Self::SplitScreen => 10816,
            Self::InteractiveFiction => 11014,
            Self::BossRush => 11095,
            Self::VehicularCombat => 11104,
            Self::MouseOnly => 11123,
            Self::VillainProtagonist => 11333,
            Self::Vikings => 11634,
            Self::Tutorial => 12057,
            Self::SexualContent => 12095,
            Self::Boxing => 12190,
            Self::Management => 12472,
            Self::Vampires => 12686,
            Self::Solitaire => 13070,
            Self::Tanks => 13276,
            Self::Archery => 13382,
            Self::Sailing => 13577,
            Self::Experimental => 13782,
            Self::GameDevelopment => 13906,
            Self::TurnBasedTactics => 14139,
            Self::Nostalgia => 14720,
            Self::IntentionallyAwkwardControls => 14906,
            Self::Flight => 15045,
            Self::Conversation => 15172,
            Self::Philosophical => 15277,
            Self::Fishing => 15564,
            Self::Motocross => 15868,
            Self::SilentProtagonist => 15954,
            Self::Mythology => 16094,
            Self::Gambling => 16250,
            Self::SpaceSim => 16598,
            Self::TimeManagement => 16689,
            Self::Werewolves => 17015,
            Self::StrategyRPG => 17305,
            Self::Lemmings => 17337,
            Self::Tabletop => 17389,
            Self::AsynchronousMultiplayer => 17770,
            Self::Cats => 17894,
            Self::Pool => 17927,
            Self::FMV => 18594,
            Self::Cycling => 19568,
            Self::Submarine => 19780,
            Self::DarkComedy => 19995,
            Self::Wolves => 20486,
            Self::Underground => 21006,
            Self::LanguageLearning => 21635,
            Self::TacticalRPG => 21725,
            Self::VR => 21978,
            Self::Agriculture => 22602,
            Self::MiniGolf => 22955,
            Self::Cleaning => 23491,
            Self::WordGame => 24003,
            Self::TouchFriendly => 25085,
            Self::Wuxia => 25959,
            Self::PoliticalSim => 26921,
            Self::VoiceControl => 27758,
            Self::Snowboarding => 28444,
            Self::SoulsLike => 29482,
            Self::Nature => 30358,
            Self::Fox => 30927,
            Self::TextBased => 31275,
            Self::Otome => 31579,
            Self::Deckbuilding => 32322,
            Self::Mahjong => 33572,
            Self::JobSimulator => 35079,
            Self::FallingBlocks => 37376,
            Self::CombatFlightSimulator => 37799,
            Self::SexualThemes => 40500,
            Self::JumpScare => 42089,
            Self::DialogueHeavy => 42152,
            Self::Coding => 42329,
            Self::ActionRoguelike => 42804,
            Self::LGBTQ => 44868,
            Self::Zoo => 46348,
            Self::Wrestling => 47827,
            Self::Rugby => 49213,
            Self::Cult => 52406,
            Self::OnRailsShooter => 56690,
            Self::ElectronicMusic => 61357,
            Self::Spelling => 71389,
            Self::FarmingSim => 87918,
            Self::ShopKeeper => 91114,
            Self::Jet => 92092,
            Self::Skating => 96359,
            Self::Assassins => 97070,
            Self::Cozy => 97376,
            Self::Elf => 102530,
            Self::_8BitMusic => 117648,
            Self::Bikes => 123332,
            Self::ATV => 129761,
            Self::Gaming => 150626,
            Self::Cricket => 158638,
            Self::BattleRoyale => 176981,
            Self::Faith => 180368,
            Self::InstrumentalMusic => 189941,
            Self::MysteryDungeon => 198631,
            Self::Motorbike => 198913,
            Self::ColonySim => 220585,
            Self::BMX => 252854,
            Self::Automation => 255534,
            Self::Musou => 323922,
            Self::Hockey => 324176,
            Self::RockMusic => 337964,
            Self::LooterShooter => 353880,
            Self::Snooker => 363767,
            Self::Clicker => 379975,
            Self::TraditionalRoguelike => 454187,
            Self::Foxes => 507423,
            Self::Wholesome => 552282,
            Self::Incremental => 560542,
            Self::Hardware => 603297,
            Self::Idler => 615955,
            Self::HeroShooter => 620519,
            Self::BulletHeaven => 723991,
            Self::SocialDeduction => 745697,
            Self::Xianxia => 760247,
            Self::EscapeRoom => 769306,
            Self::_360Video => 776177,
            Self::CardBattler => 791774,
            Self::Volleyball => 847164,
            Self::AsymmetricVR => 856791,
            Self::Decorating => 889937,
            Self::CreatureCollector => 916648,
            Self::BoomerShooter => 1023537,
            Self::AutoBattler => 1084988,
            Self::RoguelikeDeckbuilder => 1091588,
            Self::OutbreakSim => 1100686,
            Self::AutomobileSim => 1100687,
            Self::MedicalSim => 1100688,
            Self::OpenWorldSurvivalCraft => 1100689,
            Self::ExtractionShooter => 1199779,
            Self::HobbySim => 1220528,
            Self::Organizing => 1239876,
            Self::FootballSoccer => 1254546,
            Self::FootballAmerican => 1254552,
            Self::DesktopCompanion => 1320952,
            Self::Capybaras => 1352486,
        }
    }

    /// The name Steam displays.
    #[rustfmt::skip]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strategy => "Strategy",
            Self::Action => "Action",
            Self::Adventure => "Adventure",
            Self::DesignIllustration => "Design & Illustration",
            Self::Utilities => "Utilities",
            Self::FreeToPlay => "Free to Play",
            Self::RPG => "RPG",
            Self::MassivelyMultiplayer => "Massively Multiplayer",
            Self::Indie => "Indie",
            Self::EarlyAccess => "Early Access",
            Self::Casual => "Casual",
            Self::Simulation => "Simulation",
            Self::Racing => "Racing",
            Self::Sports => "Sports",
            Self::VideoProduction => "Video Production",
            Self::PhotoEditing => "Photo Editing",
            Self::AnimationModeling => "Animation & Modeling",
            Self::AudioProduction => "Audio Production",
            Self::Education => "Education",
            Self::SoftwareTraining => "Software Training",
            Self::Trains => "Trains",
            Self::Music => "Music",
            Self::Platformer => "Platformer",
            Self::Metroidvania => "Metroidvania",
            Self::Dogs => "Dogs",
            Self::Dog => "Dog",
            Self::Building => "Building",
            Self::Driving => "Driving",
            Self::TowerDefense => "Tower Defense",
            Self::HackAndSlash => "Hack and Slash",
            Self::Western => "Western",
            Self::Satire => "Satire",
            Self::Relaxing => "Relaxing",
            Self::Zombies => "Zombies",
            Self::Survival => "Survival",
            Self::FPS => "FPS",
            Self::Puzzle => "Puzzle",
            Self::Match3 => "Match 3",
            Self::CardGame => "Card Game",
            Self::Horror => "Horror",
            Self::Moddable => "Moddable",
            Self::_4X => "4X",
            Self::Superhero => "Superhero",
            Self::Aliens => "Aliens",
            Self::Typing => "Typing",
            Self::RTS => "RTS",
            Self::TurnBased => "Turn-Based",
            Self::War => "War",
            Self::Heist => "Heist",
            Self::Pirates => "Pirates",
            Self::Fantasy => "Fantasy",
            Self::CoOp => "Co-op",
            Self::Stealth => "Stealth",
            Self::Ninja => "Ninja",
            Self::Classic => "Classic",
            Self::OpenWorld => "Open World",
            Self::ThirdPerson => "Third Person",
            Self::PointClick => "Point & Click",
            Self::Crafting => "Crafting",
            Self::Tactical => "Tactical",
            Self::Surreal => "Surreal",
            Self::Psychedelic => "Psychedelic",
            Self::Roguelike => "Roguelike",
            Self::HexGrid => "Hex Grid",
            Self::MOBA => "MOBA",
            Self::Comedy => "Comedy",
            Self::DungeonCrawler => "Dungeon Crawler",
            Self::PsychologicalHorror => "Psychological Horror",
            Self::ActionRTS => "Action RTS",
            Self::Sokoban => "Sokoban",
            Self::Voxel => "Voxel",
            Self::Unforgiving => "Unforgiving",
            Self::FastPaced => "Fast-Paced",
            Self::HiddenObject => "Hidden Object",
            Self::TurnBasedStrategy => "Turn-Based Strategy",
            Self::StoryRich => "Story Rich",
            Self::Fighting => "Fighting",
            Self::Basketball => "Basketball",
            Self::ComicBook => "Comic Book",
            Self::Rhythm => "Rhythm",
            Self::Skateboarding => "Skateboarding",
            Self::MMORPG => "MMORPG",
            Self::Space => "Space",
            Self::GreatSoundtrack => "Great Soundtrack",
            Self::PermaDeath => "Perma Death",
            Self::BoardGame => "Board Game",
            Self::Arcade => "Arcade",
            Self::Shooter => "Shooter",
            Self::PvP => "PvP",
            Self::Espionage => "Espionage",
            Self::Steampunk => "Steampunk",
            Self::BasedOnANovel => "Based On A Novel",
            Self::SideScroller => "Side Scroller",
            Self::VisualNovel => "Visual Novel",
            Self::Sandbox => "Sandbox",
            Self::RealTimeTactics => "Real Time Tactics",
            Self::ThirdPersonShooter => "Third-Person Shooter",
            Self::Exploration => "Exploration",
            Self::PostApocalyptic => "Post-apocalyptic",
            Self::FirstPerson => "First-Person",
            Self::LocalCoOp => "Local Co-Op",
            Self::OnlineCoOp => "Online Co-Op",
            Self::LoreRich => "Lore-Rich",
            Self::Multiplayer => "Multiplayer",
            Self::_2D => "2D",
            Self::PrecisionPlatformer => "Precision Platformer",
            Self::Competitive => "Competitive",
            Self::OldSchool => "Old School",
            Self::Cooking => "Cooking",
            Self::Immersive => "Immersive",
            Self::SciFi => "Sci-fi",
            Self::Gothic => "Gothic",
            Self::RailShooter => "Rail Shooter",
            Self::CharacterActionGame => "Character Action Game",
            Self::Roguelite => "Roguelite",
            Self::PixelGraphics => "Pixel Graphics",
            Self::Epic => "Epic",
            Self::Physics => "Physics",
            Self::SurvivalHorror => "Survival Horror",
            Self::Historical => "Historical",
            Self::Combat => "Combat",
            Self::Retro => "Retro",
            Self::Difficult => "Difficult",
            Self::Parkour => "Parkour",
            Self::Dragons => "Dragons",
            Self::Magic => "Magic",
            Self::Thriller => "Thriller",
            Self::Anime => "Anime",
            Self::Minimalist => "Minimalist",
            Self::CombatRacing => "Combat Racing",
            Self::ActionAdventure => "Action-Adventure",
            Self::Cyberpunk => "Cyberpunk",
            Self::Funny => "Funny",
            Self::Transhumanism => "Transhumanism",
            Self::Cinematic => "Cinematic",
            Self::WorldWarII => "World War II",
            Self::ClassBased => "Class-Based",
            Self::BeatEmUp => "Beat 'em up",
            Self::RealTime => "Real-Time",
            Self::Kids => "Kids",
            Self::Atmospheric => "Atmospheric",
            Self::Military => "Military",
            Self::Medieval => "Medieval",
            Self::Realistic => "Realistic",
            Self::Singleplayer => "Singleplayer",
            Self::Chess => "Chess",
            Self::Addictive => "Addictive",
            Self::_3D => "3D",
            Self::Cartoony => "Cartoony",
            Self::Trading => "Trading",
            Self::ActionRPG => "Action RPG",
            Self::Short => "Short",
            Self::Loot => "Loot",
            Self::Episodic => "Episodic",
            Self::Stylized => "Stylized",
            Self::ShootEmUp => "Shoot 'Em Up",
            Self::Spaceships => "Spaceships",
            Self::Futuristic => "Futuristic",
            Self::Colorful => "Colorful",
            Self::TurnBasedCombat => "Turn-Based Combat",
            Self::CityBuilder => "City Builder",
            Self::Dark => "Dark",
            Self::Gore => "Gore",
            Self::GrandStrategy => "Grand Strategy",
            Self::Assassin => "Assassin",
            Self::Abstract => "Abstract",
            Self::JRPG => "JRPG",
            Self::CRPG => "CRPG",
            Self::ChooseYourOwnAdventure => "Choose Your Own Adventure",
            Self::CoOpCampaign => "Co-op Campaign",
            Self::Farming => "Farming",
            Self::Dwarves => "Dwarves",
            Self::QuickTimeEvents => "Quick-Time Events",
            Self::Cartoon => "Cartoon",
            Self::AlternateHistory => "Alternate History",
            Self::DarkFantasy => "Dark Fantasy",
            Self::Swordplay => "Swordplay",
            Self::TopDownShooter => "Top-Down Shooter",
            Self::Violent => "Violent",
            Self::Wargame => "Wargame",
            Self::Economy => "Economy",
            Self::ReplayValue => "Replay Value",
            Self::Cute => "Cute",
            Self::_2DFighter => "2D Fighter",
            Self::CharacterCustomization => "Character Customization",
            Self::TwinStickShooter => "Twin Stick Shooter",
            Self::SpectacleFighter => "Spectacle fighter",
            Self::TopDown => "Top-Down",
            Self::Mechs => "Mechs",
            Self::_6DOF => "6DOF",
            Self::_4PlayerLocal => "4 Player Local",
            Self::Capitalism => "Capitalism",
            Self::Billiards => "Billiards",
            Self::Parody => "Parody ",
            Self::BulletHell => "Bullet Hell",
            Self::Romance => "Romance",
            Self::_25D => "2.5D",
            Self::NavalCombat => "Naval Combat",
            Self::Dystopian => "Dystopian ",
            Self::ESports => "eSports",
            Self::ProceduralGeneration => "Procedural Generation",
            Self::ScoreAttack => "Score Attack",
            Self::Dinosaurs => "Dinosaurs",
            Self::ColdWar => "Cold War",
            Self::Psychological => "Psychological",
            Self::Blood => "Blood",
            Self::Sequel => "Sequel",
            Self::GodGame => "God Game",
            Self::Mod => "Mod",
            Self::FamilyFriendly => "Family Friendly",
            Self::Destruction => "Destruction",
            Self::Conspiracy => "Conspiracy",
            Self::_2DPlatformer => "2D Platformer",
            Self::WorldWarI => "World War I",
            Self::TimeAttack => "Time Attack",
            Self::_3DPlatformer => "3D Platformer",
            Self::Benchmark => "Benchmark",
            Self::Beautiful => "Beautiful",
            Self::Programming => "Programming",
            Self::Hacking => "Hacking",
            Self::PuzzlePlatformer => "Puzzle Platformer",
            Self::ArenaShooter => "Arena Shooter",
            Self::Emotional => "Emotional",
            Self::Detective => "Detective",
            Self::Collectathon => "Collectathon",
            Self::Modern => "Modern",
            Self::Remake => "Remake",
            Self::TeamBased => "Team-Based",
            Self::Mystery => "Mystery",
            Self::Baseball => "Baseball",
            Self::Robots => "Robots",
            Self::GunCustomization => "Gun Customization",
            Self::Science => "Science",
            Self::BulletTime => "Bullet Time",
            Self::Isometric => "Isometric",
            Self::WalkingSimulator => "Walking Simulator",
            Self::Tennis => "Tennis",
            Self::DarkHumor => "Dark Humor",
            Self::Reboot => "Reboot",
            Self::Mining => "Mining",
            Self::Horses => "Horses",
            Self::Noir => "Noir",
            Self::Elves => "Elves",
            Self::Logic => "Logic",
            Self::Birds => "Birds",
            Self::InventoryManagement => "Inventory Management",
            Self::Diplomacy => "Diplomacy",
            Self::Crime => "Crime",
            Self::ChoicesMatter => "Choices Matter",
            Self::_3DFighter => "3D Fighter",
            Self::Pinball => "Pinball",
            Self::TimeManipulation => "Time Manipulation",
            Self::Nudity => "Nudity",
            Self::_1990s => "1990's",
            Self::Mars => "Mars",
            Self::PvE => "PvE",
            Self::HandDrawn => "Hand-drawn",
            Self::Poker => "Poker",
            Self::Nonlinear => "Nonlinear",
            Self::Naval => "Naval",
            Self::MartialArts => "Martial Arts",
            Self::Rome => "Rome",
            Self::MultipleEndings => "Multiple Endings",
            Self::Golf => "Golf",
            Self::RealTimeWithPause => "Real-Time with Pause",
            Self::Party => "Party",
            Self::PartyGame => "Party Game",
            Self::FemaleProtagonist => "Female Protagonist",
            Self::Linear => "Linear",
            Self::Skiing => "Skiing",
            Self::Bowling => "Bowling",
            Self::BaseBuilding => "Base Building",
            Self::LocalMultiplayer => "Local Multiplayer",
            Self::Sniper => "Sniper",
            Self::Lovecraftian => "Lovecraftian",
            Self::Controller => "Controller",
            Self::Dice => "Dice",
            Self::GridBasedMovement => "Grid-Based Movement",
            Self::Offroad => "Offroad",
            Self::Narrative => "Narrative",
            Self::_1980s => "1980s",
            Self::Dwarf => "Dwarf",
            Self::ArtificialIntelligence => "Artificial Intelligence",
            Self::Soundtrack => "Soundtrack",
            Self::Software => "Software",
            Self::TrackIR => "TrackIR",
            Self::Minigames => "Minigames",
            Self::LevelEditor => "Level Editor",
            Self::MusicBasedProceduralGeneration => "Music-Based Procedural Generation",
            Self::Investigation => "Investigation",
            Self::Runner => "Runner",
            Self::ResourceManagement => "Resource Management",
            Self::Hentai => "Hentai",
            Self::Underwater => "Underwater",
            Self::ImmersiveSim => "Immersive Sim",
            Self::TradingCardGame => "Trading Card Game",
            Self::Demons => "Demons",
            Self::DatingSim => "Dating Sim",
            Self::Hunting => "Hunting",
            Self::DynamicNarration => "Dynamic Narration",
            Self::Animals => "Animals",
            Self::Snow => "Snow",
            Self::LifeSim => "Life Sim",
            Self::Transportation => "Transportation",
            Self::Memes => "Memes",
            Self::Trivia => "Trivia",
            Self::Samurai => "Samurai",
            Self::TimeTravel => "Time Travel",
            Self::PartyBasedRPG => "Party-Based RPG",
            Self::Supernatural => "Supernatural",
            Self::SplitScreen => "Split Screen",
            Self::InteractiveFiction => "Interactive Fiction",
            Self::BossRush => "Boss Rush",
            Self::VehicularCombat => "Vehicular Combat",
            Self::MouseOnly => "Mouse Only",
            Self::VillainProtagonist => "Villain Protagonist",
            Self::Vikings => "Vikings",
            Self::Tutorial => "Tutorial",
            Self::SexualContent => "Sexual Content",
            Self::Boxing => "Boxing",
            Self::Management => "Management",
            Self::Vampires => "Vampires",
            Self::Solitaire => "Solitaire",
            Self::Tanks => "Tanks",
            Self::Archery => "Archery",
            Self::Sailing => "Sailing",
            Self::Experimental => "Experimental",
            Self::GameDevelopment => "Game Development",
            Self::TurnBasedTactics => "Turn-Based Tactics",
            Self::Nostalgia => "Nostalgia",
            Self::IntentionallyAwkwardControls => "Intentionally Awkward Controls",
            Self::Flight => "Flight",
            Self::Conversation => "Conversation",
            Self::Philosophical => "Philosophical",
            Self::Fishing => "Fishing",
            Self::Motocross => "Motocross",
            Self::SilentProtagonist => "Silent Protagonist",
            Self::Mythology => "Mythology",
            Self::Gambling => "Gambling",
            Self::SpaceSim => "Space Sim",
            Self::TimeManagement => "Time Management",
            Self::Werewolves => "Werewolves",
            Self::StrategyRPG => "Strategy RPG",
            Self::Lemmings => "Lemmings",
            Self::Tabletop => "Tabletop",
            Self::AsynchronousMultiplayer => "Asynchronous Multiplayer",
            Self::Cats => "Cats",
            Self::Pool => "Pool",
            Self::FMV => "FMV",
            Self::Cycling => "Cycling",
            Self::Submarine => "Submarine",
            Self::DarkComedy => "Dark Comedy",
            Self::Wolves => "Wolves",
            Self::Underground => "Underground",
            Self::LanguageLearning => "Language Learning",
            Self::TacticalRPG => "Tactical RPG",
            Self::VR => "VR",
            Self::Agriculture => "Agriculture",
            Self::MiniGolf => "Mini Golf",
            Self::Cleaning => "Cleaning",
            Self::WordGame => "Word Game",
            Self::TouchFriendly => "Touch-Friendly",
            Self::Wuxia => "Wuxia",
            Self::PoliticalSim => "Political Sim",
            Self::VoiceControl => "Voice Control",
            Self::Snowboarding => "Snowboarding",
            Self::SoulsLike => "Souls-like",
            Self::Nature => "Nature",
            Self::Fox => "Fox",
            Self::TextBased => "Text-Based",
            Self::Otome => "Otome",
            Self::Deckbuilding => "Deckbuilding",
            Self::Mahjong => "Mahjong",
            Self::JobSimulator => "Job Simulator",
            Self::FallingBlocks => "Falling Blocks",
            Self::CombatFlightSimulator => "Combat Flight Simulator",
            Self::SexualThemes => "Sexual Themes",
            Self::JumpScare => "Jump Scare",
            Self::DialogueHeavy => "Dialogue Heavy",
            Self::Coding => "Coding",
            Self::ActionRoguelike => "Action Roguelike",
            Self::LGBTQ => "LGBTQ+",
            Self::Zoo => "Zoo",
            Self::Wrestling => "Wrestling",
            Self::Rugby => "Rugby",
            Self::Cult => "Cult",
            Self::OnRailsShooter => "On-Rails Shooter",
            Self::ElectronicMusic => "Electronic Music",
            Self::Spelling => "Spelling",
            Self::FarmingSim => "Farming Sim",
            Self::ShopKeeper => "Shop Keeper",
            Self::Jet => "Jet",
            Self::Skating => "Skating",
            Self::Assassins => "Assassins",
            Self::Cozy => "Cozy",
            Self::Elf => "Elf",
            Self::_8BitMusic => "8-bit Music",
            Self::Bikes => "Bikes",
            Self::ATV => "ATV",
            Self::Gaming => "Gaming",
            Self::Cricket => "Cricket",
            Self::BattleRoyale => "Battle Royale",
            Self::Faith => "Faith",
            Self::InstrumentalMusic => "Instrumental Music",
            Self::MysteryDungeon => "Mystery Dungeon",
            Self::Motorbike => "Motorbike",
            Self::ColonySim => "Colony Sim",
            Self::BMX => "BMX",
            Self::Automation => "Automation",
            Self::Musou => "Musou",
            Self::Hockey => "Hockey",
            Self::RockMusic => "Rock Music",
            Self::LooterShooter => "Looter Shooter",
            Self::Snooker => "Snooker",
            Self::Clicker => "Clicker",
            Self::TraditionalRoguelike => "Traditional Roguelike",
            Self::Foxes => "Foxes",
            Self::Wholesome => "Wholesome",
            Self::Incremental => "Incremental",
            Self::Hardware => "Hardware",
            Self::Idler => "Idler",
            Self::HeroShooter => "Hero Shooter",
            Self::BulletHeaven => "Bullet Heaven",
            Self::SocialDeduction => "Social Deduction",
            Self::Xianxia => "Xianxia",
            Self::EscapeRoom => "Escape Room",
            Self::_360Video => "360 Video",
            Self::CardBattler => "Card Battler",
            Self::Volleyball => "Volleyball",
            Self::AsymmetricVR => "Asymmetric VR",
            Self::Decorating => "Decorating",
            Self::CreatureCollector => "Creature Collector",
            Self::BoomerShooter => "Boomer Shooter",
            Self::AutoBattler => "Auto Battler",
            Self::RoguelikeDeckbuilder => "Roguelike Deckbuilder",
            Self::OutbreakSim => "Outbreak Sim",
            Self::AutomobileSim => "Automobile Sim",
            Self::MedicalSim => "Medical Sim",
            Self::OpenWorldSurvivalCraft => "Open World Survival Craft",
            Self::ExtractionShooter => "Extraction Shooter",
            Self::HobbySim => "Hobby Sim",
            Self::Organizing => "Organizing",
            Self::FootballSoccer => "Football (Soccer)",
            Self::FootballAmerican => "Football (American)",
            Self::DesktopCompanion => "Desktop Companion",
            Self::Capybaras => "Capybaras",
        }
    }
}

/// Every tag, ordered by Steam's id.
///
/// Both `id` and `as_str` are exhaustive `match`es, so a variant cannot exist without
/// them; this table is what makes the reverse lookups possible.
#[rustfmt::skip]
pub const TAGS: &[Tag] = &[
    Tag::Strategy,
    Tag::Action,
    Tag::Adventure,
    Tag::DesignIllustration,
    Tag::Utilities,
    Tag::FreeToPlay,
    Tag::RPG,
    Tag::MassivelyMultiplayer,
    Tag::Indie,
    Tag::EarlyAccess,
    Tag::Casual,
    Tag::Simulation,
    Tag::Racing,
    Tag::Sports,
    Tag::VideoProduction,
    Tag::PhotoEditing,
    Tag::AnimationModeling,
    Tag::AudioProduction,
    Tag::Education,
    Tag::SoftwareTraining,
    Tag::Trains,
    Tag::Music,
    Tag::Platformer,
    Tag::Metroidvania,
    Tag::Dogs,
    Tag::Dog,
    Tag::Building,
    Tag::Driving,
    Tag::TowerDefense,
    Tag::HackAndSlash,
    Tag::Western,
    Tag::Satire,
    Tag::Relaxing,
    Tag::Zombies,
    Tag::Survival,
    Tag::FPS,
    Tag::Puzzle,
    Tag::Match3,
    Tag::CardGame,
    Tag::Horror,
    Tag::Moddable,
    Tag::_4X,
    Tag::Superhero,
    Tag::Aliens,
    Tag::Typing,
    Tag::RTS,
    Tag::TurnBased,
    Tag::War,
    Tag::Heist,
    Tag::Pirates,
    Tag::Fantasy,
    Tag::CoOp,
    Tag::Stealth,
    Tag::Ninja,
    Tag::Classic,
    Tag::OpenWorld,
    Tag::ThirdPerson,
    Tag::PointClick,
    Tag::Crafting,
    Tag::Tactical,
    Tag::Surreal,
    Tag::Psychedelic,
    Tag::Roguelike,
    Tag::HexGrid,
    Tag::MOBA,
    Tag::Comedy,
    Tag::DungeonCrawler,
    Tag::PsychologicalHorror,
    Tag::ActionRTS,
    Tag::Sokoban,
    Tag::Voxel,
    Tag::Unforgiving,
    Tag::FastPaced,
    Tag::HiddenObject,
    Tag::TurnBasedStrategy,
    Tag::StoryRich,
    Tag::Fighting,
    Tag::Basketball,
    Tag::ComicBook,
    Tag::Rhythm,
    Tag::Skateboarding,
    Tag::MMORPG,
    Tag::Space,
    Tag::GreatSoundtrack,
    Tag::PermaDeath,
    Tag::BoardGame,
    Tag::Arcade,
    Tag::Shooter,
    Tag::PvP,
    Tag::Espionage,
    Tag::Steampunk,
    Tag::BasedOnANovel,
    Tag::SideScroller,
    Tag::VisualNovel,
    Tag::Sandbox,
    Tag::RealTimeTactics,
    Tag::ThirdPersonShooter,
    Tag::Exploration,
    Tag::PostApocalyptic,
    Tag::FirstPerson,
    Tag::LocalCoOp,
    Tag::OnlineCoOp,
    Tag::LoreRich,
    Tag::Multiplayer,
    Tag::_2D,
    Tag::PrecisionPlatformer,
    Tag::Competitive,
    Tag::OldSchool,
    Tag::Cooking,
    Tag::Immersive,
    Tag::SciFi,
    Tag::Gothic,
    Tag::RailShooter,
    Tag::CharacterActionGame,
    Tag::Roguelite,
    Tag::PixelGraphics,
    Tag::Epic,
    Tag::Physics,
    Tag::SurvivalHorror,
    Tag::Historical,
    Tag::Combat,
    Tag::Retro,
    Tag::Difficult,
    Tag::Parkour,
    Tag::Dragons,
    Tag::Magic,
    Tag::Thriller,
    Tag::Anime,
    Tag::Minimalist,
    Tag::CombatRacing,
    Tag::ActionAdventure,
    Tag::Cyberpunk,
    Tag::Funny,
    Tag::Transhumanism,
    Tag::Cinematic,
    Tag::WorldWarII,
    Tag::ClassBased,
    Tag::BeatEmUp,
    Tag::RealTime,
    Tag::Kids,
    Tag::Atmospheric,
    Tag::Military,
    Tag::Medieval,
    Tag::Realistic,
    Tag::Singleplayer,
    Tag::Chess,
    Tag::Addictive,
    Tag::_3D,
    Tag::Cartoony,
    Tag::Trading,
    Tag::ActionRPG,
    Tag::Short,
    Tag::Loot,
    Tag::Episodic,
    Tag::Stylized,
    Tag::ShootEmUp,
    Tag::Spaceships,
    Tag::Futuristic,
    Tag::Colorful,
    Tag::TurnBasedCombat,
    Tag::CityBuilder,
    Tag::Dark,
    Tag::Gore,
    Tag::GrandStrategy,
    Tag::Assassin,
    Tag::Abstract,
    Tag::JRPG,
    Tag::CRPG,
    Tag::ChooseYourOwnAdventure,
    Tag::CoOpCampaign,
    Tag::Farming,
    Tag::Dwarves,
    Tag::QuickTimeEvents,
    Tag::Cartoon,
    Tag::AlternateHistory,
    Tag::DarkFantasy,
    Tag::Swordplay,
    Tag::TopDownShooter,
    Tag::Violent,
    Tag::Wargame,
    Tag::Economy,
    Tag::ReplayValue,
    Tag::Cute,
    Tag::_2DFighter,
    Tag::CharacterCustomization,
    Tag::TwinStickShooter,
    Tag::SpectacleFighter,
    Tag::TopDown,
    Tag::Mechs,
    Tag::_6DOF,
    Tag::_4PlayerLocal,
    Tag::Capitalism,
    Tag::Billiards,
    Tag::Parody,
    Tag::BulletHell,
    Tag::Romance,
    Tag::_25D,
    Tag::NavalCombat,
    Tag::Dystopian,
    Tag::ESports,
    Tag::ProceduralGeneration,
    Tag::ScoreAttack,
    Tag::Dinosaurs,
    Tag::ColdWar,
    Tag::Psychological,
    Tag::Blood,
    Tag::Sequel,
    Tag::GodGame,
    Tag::Mod,
    Tag::FamilyFriendly,
    Tag::Destruction,
    Tag::Conspiracy,
    Tag::_2DPlatformer,
    Tag::WorldWarI,
    Tag::TimeAttack,
    Tag::_3DPlatformer,
    Tag::Benchmark,
    Tag::Beautiful,
    Tag::Programming,
    Tag::Hacking,
    Tag::PuzzlePlatformer,
    Tag::ArenaShooter,
    Tag::Emotional,
    Tag::Detective,
    Tag::Collectathon,
    Tag::Modern,
    Tag::Remake,
    Tag::TeamBased,
    Tag::Mystery,
    Tag::Baseball,
    Tag::Robots,
    Tag::GunCustomization,
    Tag::Science,
    Tag::BulletTime,
    Tag::Isometric,
    Tag::WalkingSimulator,
    Tag::Tennis,
    Tag::DarkHumor,
    Tag::Reboot,
    Tag::Mining,
    Tag::Horses,
    Tag::Noir,
    Tag::Elves,
    Tag::Logic,
    Tag::Birds,
    Tag::InventoryManagement,
    Tag::Diplomacy,
    Tag::Crime,
    Tag::ChoicesMatter,
    Tag::_3DFighter,
    Tag::Pinball,
    Tag::TimeManipulation,
    Tag::Nudity,
    Tag::_1990s,
    Tag::Mars,
    Tag::PvE,
    Tag::HandDrawn,
    Tag::Poker,
    Tag::Nonlinear,
    Tag::Naval,
    Tag::MartialArts,
    Tag::Rome,
    Tag::MultipleEndings,
    Tag::Golf,
    Tag::RealTimeWithPause,
    Tag::Party,
    Tag::PartyGame,
    Tag::FemaleProtagonist,
    Tag::Linear,
    Tag::Skiing,
    Tag::Bowling,
    Tag::BaseBuilding,
    Tag::LocalMultiplayer,
    Tag::Sniper,
    Tag::Lovecraftian,
    Tag::Controller,
    Tag::Dice,
    Tag::GridBasedMovement,
    Tag::Offroad,
    Tag::Narrative,
    Tag::_1980s,
    Tag::Dwarf,
    Tag::ArtificialIntelligence,
    Tag::Soundtrack,
    Tag::Software,
    Tag::TrackIR,
    Tag::Minigames,
    Tag::LevelEditor,
    Tag::MusicBasedProceduralGeneration,
    Tag::Investigation,
    Tag::Runner,
    Tag::ResourceManagement,
    Tag::Hentai,
    Tag::Underwater,
    Tag::ImmersiveSim,
    Tag::TradingCardGame,
    Tag::Demons,
    Tag::DatingSim,
    Tag::Hunting,
    Tag::DynamicNarration,
    Tag::Animals,
    Tag::Snow,
    Tag::LifeSim,
    Tag::Transportation,
    Tag::Memes,
    Tag::Trivia,
    Tag::Samurai,
    Tag::TimeTravel,
    Tag::PartyBasedRPG,
    Tag::Supernatural,
    Tag::SplitScreen,
    Tag::InteractiveFiction,
    Tag::BossRush,
    Tag::VehicularCombat,
    Tag::MouseOnly,
    Tag::VillainProtagonist,
    Tag::Vikings,
    Tag::Tutorial,
    Tag::SexualContent,
    Tag::Boxing,
    Tag::Management,
    Tag::Vampires,
    Tag::Solitaire,
    Tag::Tanks,
    Tag::Archery,
    Tag::Sailing,
    Tag::Experimental,
    Tag::GameDevelopment,
    Tag::TurnBasedTactics,
    Tag::Nostalgia,
    Tag::IntentionallyAwkwardControls,
    Tag::Flight,
    Tag::Conversation,
    Tag::Philosophical,
    Tag::Fishing,
    Tag::Motocross,
    Tag::SilentProtagonist,
    Tag::Mythology,
    Tag::Gambling,
    Tag::SpaceSim,
    Tag::TimeManagement,
    Tag::Werewolves,
    Tag::StrategyRPG,
    Tag::Lemmings,
    Tag::Tabletop,
    Tag::AsynchronousMultiplayer,
    Tag::Cats,
    Tag::Pool,
    Tag::FMV,
    Tag::Cycling,
    Tag::Submarine,
    Tag::DarkComedy,
    Tag::Wolves,
    Tag::Underground,
    Tag::LanguageLearning,
    Tag::TacticalRPG,
    Tag::VR,
    Tag::Agriculture,
    Tag::MiniGolf,
    Tag::Cleaning,
    Tag::WordGame,
    Tag::TouchFriendly,
    Tag::Wuxia,
    Tag::PoliticalSim,
    Tag::VoiceControl,
    Tag::Snowboarding,
    Tag::SoulsLike,
    Tag::Nature,
    Tag::Fox,
    Tag::TextBased,
    Tag::Otome,
    Tag::Deckbuilding,
    Tag::Mahjong,
    Tag::JobSimulator,
    Tag::FallingBlocks,
    Tag::CombatFlightSimulator,
    Tag::SexualThemes,
    Tag::JumpScare,
    Tag::DialogueHeavy,
    Tag::Coding,
    Tag::ActionRoguelike,
    Tag::LGBTQ,
    Tag::Zoo,
    Tag::Wrestling,
    Tag::Rugby,
    Tag::Cult,
    Tag::OnRailsShooter,
    Tag::ElectronicMusic,
    Tag::Spelling,
    Tag::FarmingSim,
    Tag::ShopKeeper,
    Tag::Jet,
    Tag::Skating,
    Tag::Assassins,
    Tag::Cozy,
    Tag::Elf,
    Tag::_8BitMusic,
    Tag::Bikes,
    Tag::ATV,
    Tag::Gaming,
    Tag::Cricket,
    Tag::BattleRoyale,
    Tag::Faith,
    Tag::InstrumentalMusic,
    Tag::MysteryDungeon,
    Tag::Motorbike,
    Tag::ColonySim,
    Tag::BMX,
    Tag::Automation,
    Tag::Musou,
    Tag::Hockey,
    Tag::RockMusic,
    Tag::LooterShooter,
    Tag::Snooker,
    Tag::Clicker,
    Tag::TraditionalRoguelike,
    Tag::Foxes,
    Tag::Wholesome,
    Tag::Incremental,
    Tag::Hardware,
    Tag::Idler,
    Tag::HeroShooter,
    Tag::BulletHeaven,
    Tag::SocialDeduction,
    Tag::Xianxia,
    Tag::EscapeRoom,
    Tag::_360Video,
    Tag::CardBattler,
    Tag::Volleyball,
    Tag::AsymmetricVR,
    Tag::Decorating,
    Tag::CreatureCollector,
    Tag::BoomerShooter,
    Tag::AutoBattler,
    Tag::RoguelikeDeckbuilder,
    Tag::OutbreakSim,
    Tag::AutomobileSim,
    Tag::MedicalSim,
    Tag::OpenWorldSurvivalCraft,
    Tag::ExtractionShooter,
    Tag::HobbySim,
    Tag::Organizing,
    Tag::FootballSoccer,
    Tag::FootballAmerican,
    Tag::DesktopCompanion,
    Tag::Capybaras,
];
