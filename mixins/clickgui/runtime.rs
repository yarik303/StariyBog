// ClickGUI's core: categories, the module registry, settings and the GUI's own
// state. This file is pure Rust with no game types, so `cargo test` compiles it
// on its own. The game-aware half lives in `clickgui_ui` (drawing and input)
// and `clickgui_fullbright` (the FullBright module's tick).
//
// To add a module, add one `Module` to `default_modules()`, then (if it needs
// a tick) one call in tick.rs. The GUI builds its list and settings page from
// the registry, so nothing else has to change.

use std::cell::RefCell;

pub const ID: &str = "clickgui";
pub const VERSION: &str = "0.1.0";
/// The module id and the key name Options > Mods shows.
pub const KEY: &str = "gui";
pub const DEFAULT_KEY: &str = "key.keyboard.right.shift";
/// FullBright's registry id.
pub const FULLBRIGHT: &str = "fullbright";
/// The ArrayList module's registry id.
pub const ARRAYLIST: &str = "arraylist";
/// The Watermark module's registry id.
pub const WATERMARK: &str = "watermark";
/// The AutoTool module's registry id.
pub const AUTOTOOL: &str = "autotool";
/// The Blink module's registry id.
pub const BLINK: &str = "blink";
/// The AutoWeapon module's registry id.
pub const AUTOWEAPON: &str = "autoweapon";
/// The OffHand module's registry id.
pub const OFFHAND: &str = "offhand";
/// The AutoArmor module's registry id.
pub const AUTOARMOR: &str = "autoarmor";
/// The AutoXP module's registry id.
pub const AUTOXP: &str = "autoxp";
/// The HoleFill module's registry id.
pub const HOLEFILL: &str = "holefill";
/// The Reach module's registry id.
pub const REACH: &str = "reach";
/// The AutoClicker module's registry id.
pub const AUTOCLICKER: &str = "autoclicker";
/// The AimAssist module's registry id.
pub const AIMASSIST: &str = "aimassist";
/// The Surround module's registry id.
pub const SURROUND: &str = "surround";
/// The CrystalAura module's registry id.
pub const CRYSTALAURA: &str = "crystalaura";
/// The NameTags module's registry id.
pub const NAMETAGS: &str = "nametags";
/// The NoFall module's registry id.
pub const NOFALL: &str = "nofall";
/// The ESP module's registry id.
pub const ESP: &str = "esp";
/// The Tracers module's registry id.
pub const TRACERS: &str = "tracers";
/// The HoleESP module's registry id.
pub const HOLEESP: &str = "holeesp";
/// The NoRender module's registry id.
pub const NORENDER: &str = "norender";
/// The Chams module's registry id.
pub const CHAMS: &str = "chams";
/// The ViewModel module's registry id.
pub const VIEWMODEL: &str = "viewmodel";
/// The Animations module's registry id.
pub const ANIMATIONS: &str = "animations";
/// The Ambience module's registry id.
pub const AMBIENCE: &str = "ambience";
/// The Zoom module's registry id.
pub const ZOOM: &str = "zoom";
/// The zooms the starting zoom cycles through, and the wheel's limits: the
/// same values Horizon's own Zoom mod uses.
pub const ZOOM_LEVELS: [&str; 6] = ["2x", "3x", "4x", "6x", "8x", "10x"];
pub const ZOOM_FACTORS: [f32; 6] = [2.0, 3.0, 4.0, 6.0, 8.0, 10.0];
const ZOOM_MIN: f32 = 1.25;
const ZOOM_MAX: f32 = 10.0;
/// Each wheel step multiplies the zoom by this.
const ZOOM_STEP: f32 = 1.25;
/// The Criticals module's registry id.
pub const CRITICALS: &str = "criticals";
/// The Scaffold module's registry id.
pub const SCAFFOLD: &str = "scaffold";
/// The Velocity module's registry id.
pub const VELOCITY: &str = "velocity";
/// The ChestStealer module's registry id.
pub const CHESTSTEALER: &str = "cheststealer";
/// The FastBreak module's registry id.
pub const FASTBREAK: &str = "fastbreak";
/// The Speed module's registry id.
pub const SPEED: &str = "speed";
/// The Step module's registry id.
pub const STEP: &str = "step";
/// The Fly module's registry id.
pub const FLY: &str = "fly";
/// The Phase module's registry id.
pub const PHASE: &str = "phase";
/// The Timer module's registry id.
pub const TIMER: &str = "timer";

/// The Timer module's modes: when its multiplier applies.
pub const TIMER_MODES: [&str; 3] = ["Always", "OnMove", "NoMove"];

/// Scaffold's placement modes, one per deduplicated client implementation:
/// Flux's Normal/Legit/New/AAC/Hypixel, Alan's, Augustus's NewScaffold,
/// Novoline's, Sigma's, Tenacity's, Yuri's, and LiquidBounce's Expand,
/// GodBridge and Breezily techniques. Aoba's scaffold was Flux's Normal with
/// different defaults, and LiquidBounce's plain Normal is covered by Normal.
pub const SCAFFOLD_MODES: [&str; 16] = [
    "Off",
    "Normal",
    "Legit",
    "New",
    "AAC",
    "Hypixel",
    "Alan",
    "NewScaffold",
    "Novoline",
    "Sigma",
    "Tenacity",
    "Yuri",
    "Expand",
    "GodBridge",
    "Breezily",
    "Eagle",
];

/// The Scaffold modes each setting belongs to, by `SCAFFOLD_MODES` index.
const SCAFFOLD_ALL: &[usize] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];
/// Expand is the only mode that reads the Extend slider: the rest predict
/// from their own implementation's step.
const SCAFFOLD_EXTEND: &[usize] = &[12];
/// Eagle places nothing, so it shows only its own four settings.
const SCAFFOLD_EAGLE: &[usize] = &[15];

/// Scaffold's sprint handling, deduplicated from the Alan sprint modes:
/// Bypass behaved like Disabled, and the Watchdog jump/prediction variants
/// only duplicated Watchdog's cancel.
pub const SCAFFOLD_SPRINTS: [&str; 7] = [
    "Off", "Disabled", "Legit", "Matrix", "Verus", "Vulcan", "Watchdog",
];
/// Scaffold's tower handling, deduplicated from the tower folder: NCP,
/// Matrix, Verus and Timer were Vanilla with timing tweaks Horizon's clock
/// lacks, AirJump was AutoJump, and the Kotlin variants were Watchdog.
pub const SCAFFOLD_TOWERS: [&str; 6] = ["Off", "Vanilla", "Legit", "Vulcan", "Snap", "Watchdog"];
/// Scaffold's downwards handling; Watchdog behaved like Normal.
pub const SCAFFOLD_DOWNWARDS: [&str; 3] = ["Off", "Normal", "Verus"];

/// Velocity's modes, deduplicated from LiquidBounce's Modify/Reduce/Reversal/
/// Strafe/Lag/JumpReset and the FDPClient, Alan, Sigma, Tenacity, Flux,
/// Expensive, Azura, Augustus and Cel velocity folders.
///
/// The plain packet scalers (LiquidBounce Modify, Tenacity/Flux/Sigma Packet,
/// Augustus Basic, Azura Simple/Custom, FDP Simple, Alan Standard and Cel's
/// packet modes) are Standard; the hurt-tick motion multipliers (Sigma Motion,
/// Cel Custom Motion, Sentinel and the Matrix family) are Motion; the queues
/// (DelayVelocity, Lag, Sigma Delayed, FDP Delay/Delayed, BufferAbuse and the
/// Watchdog Reduce family) are Delayed; the direction flips (Reversal,
/// MatrixReverse, Bounce, Augustus/Azura Reverse) are Reverse. The remaining
/// modes each add one distinct trick. Full cancels are Standard at 0%. The two
/// timer modes are FDPClient's: Spoof keeps the knockback's motion out of the
/// client, reports the step with a position packet and slows the timer for one
/// tick; GrimC03 leaves the knockback, sends its ground status and a
/// stop-destroy into the air block at the feet, then ramps the timer from 0.8x
/// back to 1x over twenty ticks. The blink-stack, transaction, fake-S08 and
/// 1.17 variants are left out with their containers: their blink buffers and
/// transactions have no equivalent in the ported engine. NewGrim is the 1.21
/// "New Grim" fragment: it cancels
/// the velocity packet, then sends its own PosRot and a stop-destroy at the
/// feet facing down.
pub const VELOCITY_MODES: [&str; 21] = [
    "Off",
    "Standard",
    "Motion",
    "Delayed",
    "Reverse",
    "Tick",
    "JumpReset",
    "Hypixel",
    "Verus",
    "Grim",
    "BlocksMC",
    "Phase",
    "Click",
    "SideStrafe",
    "AAC",
    "AACPush",
    "Polar",
    "Intave",
    "NewGrim",
    "Spoof",
    "GrimC03",
];

/// The Velocity modes each setting belongs to, by `VELOCITY_MODES` index.
const VELO_STANDARD: &[usize] = &[1];
const VELO_MOTION: &[usize] = &[2];
const VELO_REVERSE: &[usize] = &[4];
const VELO_TICK: &[usize] = &[5];
const VELO_JUMP: &[usize] = &[6];
const VELO_HYPIXEL: &[usize] = &[7];
const VELO_VERUS: &[usize] = &[8];
const VELO_GRIM: &[usize] = &[9];
const VELO_BLOCKSMC: &[usize] = &[10];
const VELO_PHASE: &[usize] = &[11];
const VELO_CLICK: &[usize] = &[12];
const VELO_SIDE: &[usize] = &[13];
const VELO_AAC: &[usize] = &[14];
const VELO_AACPUSH: &[usize] = &[15];
const VELO_POLAR: &[usize] = &[16];
const VELO_INTAVE: &[usize] = &[17];
/// Spoof's timer settings.
const VELO_SPOOF: &[usize] = &[19];
/// The chance slider is shared by the modes that roll per knockback.
const VELO_CHANCE: &[usize] = &[1, 4, 6];
/// Only-move gates Standard and Reverse.
const VELO_ONLY_MOVE: &[usize] = &[1, 4];
/// The queue delay serves Delayed and Reverse.
const VELO_DELAY: &[usize] = &[3, 4];

/// Speed's modes, one per distinct client implementation: LiquidBounce's Speed
/// module, the FDPClient speed modes it does not have, then the Alan,
/// Librecraft, KoksCraft, Spectre, Swift and Alan TerrainSpeed modes.
/// Duplicates are folded into the mode they copy: AACHop3.5.0 into
/// AAC, HypixelHop into HypixelBHop, SpartanYPort and SpartanV4043FastFall's
/// 524 re-tune into Spartan, BlocksMCSpeed (Kotlin and Java), SentinelSpeed,
/// NCPSpeed, IntaveHop14 and VerusSpeeds into their base modes, Vulcan's
/// Hop/LowHop/YG/Ground288 into Vulcan286/288/Ground286, and SpectreBHop and
/// SpectreLowHop into SpectreHop. The timer-, packet-, blink- and
/// position-spoof modes (Watchdog, TeleportCubeCraft, Boost, IntaveTimer14,
/// VerusSpeeds Ground/Float, OldMatrixHop, Custom's TimerSpeed/StrafeKnock),
/// the 1.8-only field modes (LatestVerusHop, SpeedGeneric, the AAC YG
/// variants) and the module-level only-in-combat/only-on-potion conditions
/// are left out: their timer cycles, packet queues or attributes have no
/// equivalent in the modes' engine. Alan's Bloxd TerrainSpeed mode is left out
/// too: it depends on Bloxd's resyncphysics payload, its Flight and LongJump
/// modules and chest collision overrides, none of which Horizon exposes.
pub const SPEED_MODES: [&str; 41] = [
    "LegitHop",
    "Custom",
    "YPort",
    "Vanilla",
    "AAC",
    "HypixelBHop",
    "HypixelLowHop",
    "Spartan",
    "SpartanFastFall",
    "SentinelDamage",
    "Vulcan286",
    "Vulcan288",
    "VulcanGround286",
    "GrimCollide",
    "GrimBHop",
    "GrimLowHop",
    "NCP",
    "NCPBHop",
    "NCPYPort",
    "Intave14",
    "Intave14Fast",
    "HylexLowHop",
    "HylexGround",
    "BlocksMC",
    "BlocksMCHop",
    "Matrix7",
    "MatrixHop",
    "Matrix",
    "VerusHop",
    "VerusFHop",
    "VerusLowHop",
    "Verus",
    "Librecraft",
    "KoksCraft",
    "MiJump",
    "SlowHop",
    "Frame",
    "SpectreHop",
    "SpectreOnGround",
    "Swift",
    "WatchdogTerrain",
];

/// The Speed modes each setting belongs to, by `SPEED_MODES` index.
const SPD_CUSTOM: &[usize] = &[1];
const SPD_YPORT: &[usize] = &[2];
const SPD_VANILLA: &[usize] = &[3];
const SPD_HYPIXEL_BHOP: &[usize] = &[5];
const SPD_HYPIXEL_LOWHOP: &[usize] = &[6];
const SPD_SENTINEL: &[usize] = &[9];
const SPD_GRIM_COLLIDE: &[usize] = &[13];
const SPD_NCP: &[usize] = &[16];
const SPD_INTAVE: &[usize] = &[19];
const SPD_BLOCKSMC: &[usize] = &[23];
const SPD_BLOCKSMC_HOP: &[usize] = &[24];
const SPD_MATRIX_HOP: &[usize] = &[26];
const SPD_SPECTRE_HOP: &[usize] = &[37];

/// Step's modes, one per distinct implementation: Alan's Step, the FDPClient
/// modes it does not have, LiquidBounce's Step and the Celestial step ports.
///
/// The FDPClient AAC and OldAAC modes send exactly NCP's packets, so they are
/// NCP; its Vanilla, NCP, Jump and Vulcan copies and Celestial's Matrix, NCP
/// and Vanilla are the same modes again. LiquidBounce's Legit only permits
/// vanilla auto-jump, which Jump covers. FDPClient's BlocksMC sends a fake
/// stone placement in its C08, which Horizon cannot reproduce, so the 1.9+
/// LiquidBounce BlocksMC is the one ported. The kept modes are distinct
/// techniques, including the two Matrix versions.
pub const STEP_MODES: [&str; 22] = [
    "Vanilla",
    "Jump",
    "NCP",
    "NCPNew",
    "NCPPacketless",
    "MotionNCP",
    "OldNCP",
    "AAC4.4.0",
    "LAAC",
    "AAC3.3.4",
    "Spartan",
    "Rewinside",
    "Vulcan",
    "Vulcan286",
    "Verus",
    "Watchdog",
    "Instant",
    "BlocksMC",
    "BlocksMCTimer",
    "Hypixel",
    "Matrix",
    "Matrix6.7.0",
];

/// The Step modes each setting belongs to, by `STEP_MODES` index.
const STEP_JUMPS: &[usize] = &[1];
const STEP_HEIGHT: &[usize] = &[0, 2, 3, 6, 7, 10, 11, 12, 14, 16, 21];
const STEP_DELAY: &[usize] = &[0, 2, 3, 6, 7, 8, 10, 11, 12, 14, 16, 21];
const STEP_CONFIRM: &[usize] = &[0, 2, 3, 6, 7, 10, 11, 12, 14, 15, 16, 21];
const STEP_TIMER: &[usize] = &[0, 2, 3, 6, 7, 10, 11, 12, 16];
const STEP_MATRIX: &[usize] = &[20];
const STEP_INSTANT: &[usize] = &[16];
const STEP_BLOCKSMC: &[usize] = &[17];
const STEP_HYPIXEL: &[usize] = &[19];

/// Fly's modes, one per distinct implementation: the Alan flight folder, the
/// FDPClient/LiquidBounce fly modes, and the Sigma, Flux, Celestial, Tenacity,
/// Siekera, Prisma and White Floor flight ports.
///
/// The plain key-driven hover (Alan Vanilla, LiquidBounce FlyVanilla, Sigma
/// Motion, Siekera Vanilla, Tenacity Vanilla/HvH, Flux Motion, Rich Motion and
/// Sentinel27thJan) is Vanilla. The ground-spoof hover (LiquidBounce AirWalk and
/// VerusB3869Flat, Flux Float, Sentinel10thMar) is AirWalk. The key-driven
/// glide template of GrimFly/PolarFly/SentinelFly is Grim, and the same
/// motion law with the Glyph-era freeze/timer of Alan's Grim 1.9-1.18.1 is
/// GrimOld. The elytra command mode merges Alan's Grim 3 and LiquidBounce's
/// Grim2373Jan15 without its blink; the sine hover of Siekera's GrimFloat and
/// the boat/shulker exemption of Prisma's GrimEntity are their own modes.
///
/// The NCP family keeps Alan's LatestNCP (Normal and Clip) as NCP, Alan's
/// OldNCP as OldNCP, and the enable-burst of FDPClient's NCP/BugSpartan/
/// KeepAlive plus LiquidBounce's NCP as NCPBurst. LiquidBounce's FlyHypixel and
/// FlyHypixelFlat are Hypixel, FDPClient's BoostHypixel is HypixelBoost.
/// Vulcan absorbs FDPClient's Vulcan/VulcanOld and VerusGlide plus
/// LiquidBounce's Vulcan277 and Alan's Vulcan 2, which all set the same descent
/// rates. Alan's Verus and VerusDamageNew plus LiquidBounce's VulcanDamage are
/// Verus and VerusDamage. FDPClient's Spartan and Spartan2 plus LiquidBounce's
/// Spartan524 are Spartan, and AAC3312Glide plus NeruxVace are AACGlide.
///
/// Matrix is Alan's teleport-cancelling Matrix, Damage merges Alan's Damage and
/// DamageDeprecated with Sigma's ColdPVP and LiquidBounce's DamageFly, and
/// Sentinel is LiquidBounce's four-packet self-damage boost (Sentinel20thApr,
/// Sentinel26thDec and VerusB3896Damage). Jetpack, Explosion and BufferAbuse
/// come from LiquidBounce's FlyGeneric, its explosion amplifier and Alan's
/// BufferAbuse.
///
/// Left out with their containers: the abilities-based Creative/Hypixel/CubeCraft
/// fly modes (Horizon sends PlayerAbilities, which the server rejects while
/// flying), the blink/transaction modes (Grim2, Watchdog, HycraftDamage,
/// LiquidBounce's Grim2373Jan15 queue, FDPClient's BlockDrop/BlocksMC2 and the
/// Sentinel blink), the collision-shape overrides (Collide, FakeGround,
/// VulcanGhost, Vulcan286's ghost block), the item modes (Fireball, Enderpearl,
/// MMCFireball, Bloxd, White Floor's Sunrise Drop), the vehicle modes (Boat,
/// BoatPhase, ElytraGlide is also item-bound), the position-clip modes
/// (MiniBlox, MMC, NcpClip, Clip, ZoneCraft, MineLand, Flag, TeleportRewinside,
/// Minesucht) and Sigma's teleport-ID NCPFly. Their packet queues, collision
/// injection or inventory layer have no equivalent in the ported engine.
/// MatrixHighJump keeps Alan's hurt-triggered height boost, its self-damage
/// start and the motion it restores after a setback. MatrixDamage
/// is Damage's timer and slow hover with
/// a different Y decay, so Damage covers it; AAC305's three-tick pulse is the
/// same Y reset as Grim's hover.
pub const FLY_MODES: [&str; 24] = [
    "Vanilla",
    "AirWalk",
    "Grim",
    "GrimOld",
    "GrimElytra",
    "GrimFloat",
    "GrimEntity",
    "NCP",
    "OldNCP",
    "NCPBurst",
    "Hypixel",
    "HypixelBoost",
    "Vulcan",
    "Verus",
    "VerusDamage",
    "Spartan",
    "AACGlide",
    "Matrix",
    "Damage",
    "Sentinel",
    "Jetpack",
    "Explosion",
    "BufferAbuse",
    "MatrixHighJump",
];

/// The Fly modes each setting belongs to, by `FLY_MODES` index.
const FLY_VANILLA: &[usize] = &[0];
const FLY_AIRWALK: &[usize] = &[1];
const FLY_GRIM: &[usize] = &[2];
const FLY_GRIM_OLD: &[usize] = &[3];
const FLY_GRIM_FLOAT: &[usize] = &[5];
const FLY_GRIM_ENTITY: &[usize] = &[6];
const FLY_NCP: &[usize] = &[7];
const FLY_OLD_NCP: &[usize] = &[8];
const FLY_NCP_BURST: &[usize] = &[9];
const FLY_HYPIXEL: &[usize] = &[10];
const FLY_HYPIXEL_BOOST: &[usize] = &[11];
const FLY_VULCAN: &[usize] = &[12];
const FLY_VERUS: &[usize] = &[13];
const FLY_VERUS_DAMAGE: &[usize] = &[14];
const FLY_SPARTAN: &[usize] = &[15];
const FLY_AAC_GLIDE: &[usize] = &[16];
const FLY_MATRIX: &[usize] = &[17];
const FLY_DAMAGE: &[usize] = &[18];
const FLY_SENTINEL: &[usize] = &[19];
const FLY_JETPACK: &[usize] = &[20];
const FLY_EXPLOSION: &[usize] = &[21];
const FLY_BUFFER: &[usize] = &[22];
const FLY_MATRIX_HIGH: &[usize] = &[23];

/// ChestStealer's slot order, both directions of the FDPClient module.
pub const CHESTSTEALER_ORDERS: [&str; 2] = ["TopToBottom", "BottomToTop"];
/// ChestStealer's behaviour when the inventory no longer holds the next stack.
pub const CHESTSTEALER_ON_FULL: [&str; 2] = ["Stop", "Continue"];
/// ChestStealer's debug destination. Horizon has no notification queue, so
/// the module prints to chat.
pub const CHESTSTEALER_DEBUG: [&str; 2] = ["Off", "Text"];

/// FastBreak's progress models: LiquidBounce's damage threshold, then Alan's
/// percentage speed and per-tick finish point.
pub const FASTBREAK_MODES: [&str; 3] = ["Damage", "Percentage", "Ticks"];
/// Damage's mode index (LiquidBounce's `breakDamage`).
pub const FASTBREAK_DAMAGE: u8 = 0;
/// Ticks' mode index (Alan's hardness times the tick count).
pub const FASTBREAK_TICKS: u8 = 2;

/// BreakDamage belongs to Damage.
const FASTBREAK_BREAK_DAMAGE: &[usize] = &[0];
/// Speed belongs to Percentage.
const FASTBREAK_SPEED: &[usize] = &[1];
/// The Ticks slider belongs to Ticks.
const FASTBREAK_TICKS_ONLY: &[usize] = &[2];
/// Alan's EqualAirGroundDig serves his two modes.
const FASTBREAK_ALAN: &[usize] = &[1, 2];

/// Phase's modes, one per distinct mechanism in the twenty-five references.
/// The repeats were folded away: Alan's Spartan and Normal, the Vulcan and
/// Watchdog semi-block ports and FDP's Vanilla all send the same 0.5 burst, so
/// they are Vanilla; Seppuku's PACKET is Vanilla and its SKIP is Skip; the two
/// BlocksMC ports, Seppuku's SAND, Shoreline's SAND and CLIMB, ThunderHack's
/// Vanilla and Wurst's NoClip are the collision-shape family, which Horizon
/// cannot express (it has no client collision hook) and so are not modes;
/// Celestial's Sunrise New/Old and Lorent Craft, Grim's and LiquidBounce's
/// PhaseBlink and Alan's WatchdogBlink are the packet-hold family the Blink
/// module already owns; and earthhack's, Wurst+'s, Konas's, Phobos's,
/// Celestial's Lorent Packet/New Lorent and Alan's WatchdogPrediction are the
/// teleport-ID desync family, which is PacketFly rather than Phase.
pub const PHASE_MODES: [&str; 9] = [
    "Vanilla",
    "Skip",
    "Mineplex",
    "Clip",
    "Center",
    "Nudge",
    "Break",
    "Sink",
    "Pearl",
];

/// The Phase modes each setting belongs to, by `PHASE_MODES` index. Pearl is
/// not here: its wait is the pearl's own cooldown, not the module's.
const PHASE_DELAY: &[usize] = &[0, 1, 3, 6];
const PHASE_PUSH: &[usize] = &[5];
const PHASE_PITCH: &[usize] = &[8];
const PHASE_SCAN: &[usize] = &[3];

/// NoFall's modes, in LiquidBounce's NoFall order, then the extra modes ported
/// from the LiquidBounce/FDP/other-client implementations.
pub const NOFALL_MODES: [&str; 57] = [
    "SpoofGround",
    "SpoofLanding",
    "NoGround",
    "Packet",
    "PacketJump",
    "MLG",
    "Mount",
    "Rettungsplatform",
    "Spartan524Flag",
    "Vulcan277",
    "VulcanTP288",
    "Verus",
    "ForceJump",
    "Cancel",
    "Blink",
    "HypixelPacket",
    "Hypixel",
    "BlocksMC",
    "Grim2371-1.9+",
    "AlwaysSpoof",
    "Damage",
    "Vanilla",
    "Packet1",
    "Packet2",
    "MotionFlag",
    "CubeCraft",
    "Intave",
    "Polar",
    "Sentinel",
    "LatestVulcan",
    "MatrixCollide",
    "MatrixNew",
    "MatrixNoFall",
    "Matrix6.2.X",
    "Matrix6.2.X-Packet",
    "Matrix6.6.3",
    "AAC",
    "AAC3.3.11",
    "AAC3.3.15",
    "AAC4.4.X-Flag",
    "AAC5.0.4",
    "AAC5.0.14",
    "LAAC",
    "VulcanOld",
    "VulcanFast288",
    "PacketLegacy",
    "RoundNoFall",
    "NoFallDamage",
    "WatchdogPacket",
    "WatchdogPrediction",
    "HypixelTimer",
    "HypixelFlag",
    "Heypixel",
    "ChunkLoad",
    "Phase",
    "Grim72",
    "Grim73",
];

/// The NoFall modes each setting belongs to, by `NOFALL_MODES` index.
const SPOOF_GROUND: &[usize] = &[0];
const SPOOF_LANDING: &[usize] = &[1];
const PACKET: &[usize] = &[3];
const PACKET_JUMP: &[usize] = &[4];
const MLG: &[usize] = &[5];
const MOUNT: &[usize] = &[6];
const VULCAN_TP: &[usize] = &[10];
const FORCE_JUMP: &[usize] = &[12];
const CANCEL: &[usize] = &[13];
const NOFALL_BLINK: &[usize] = &[14];
const HYPIXEL_PACKET: &[usize] = &[15];
const MOTION_FLAG: &[usize] = &[24];
const MATRIX_663: &[usize] = &[35];
/// NoFall's Phase mode, named apart from the Phase module's own id.
const NOFALL_PHASE: &[usize] = &[54];

/// OffHand's item choices, and the two slices that decide which settings they
/// show: the safety gate only exists when the totem is the fallback, and the
/// gapple settings only when a gapple is the item.
pub const OFFHAND_ITEMS: [&str; 3] = ["Totem", "GApple", "Crystal"];
const OFFHAND_SAFETY: &[usize] = &[1, 2];
const OFFHAND_GAPPLE: &[usize] = &[1];

/// AutoArmor's modes: `Best` replaces a worn piece whenever a carried one
/// beats it, `Empty` only fills a slot nothing is worn in.
pub const AUTOARMOR_MODES: [&str; 2] = ["Best", "Empty"];
/// AutoArmor's comparison: what decides which of two pieces for the same slot
/// is the better one.
pub const AUTOARMOR_COMPARE: [&str; 2] = ["Armor", "Protection"];

/// AutoXP's modes. `Silent` selects the bottle slot for the server only and
/// puts the old one back in the same tick; `Normal` leaves the bottle in hand.
pub const AUTOXP_MODES: [&str; 2] = ["Silent", "Normal"];

/// HoleFill's blocks: what it will take from the hotbar to fill a hole with.
pub const HOLEFILL_BLOCKS: [&str; 3] = ["Obsidian", "Ender chest", "Any block"];

/// AutoClicker's modes: a steady CPS, or a drag-click burst.
pub const AUTOCLICKER_MODES: [&str; 2] = ["Normal", "Drag"];
/// The settings each mode shows, by `AUTOCLICKER_MODES` index.
const AUTOCLICKER_NORMAL: &[usize] = &[0];
const AUTOCLICKER_DRAG: &[usize] = &[1];

/// Where AimAssist points on a target's body.
pub const AIMASSIST_POINTS: [&str; 3] = ["Head", "Chest", "Legs"];

/// What Surround will build with. The same three choices HoleFill offers.
pub const SURROUND_BLOCKS: [&str; 3] = ["Obsidian", "Ender chest", "Any block"];

/// Criticals' modes: LiquidBounce's, FDPClient's, then the modes from the
/// Alan/Edit, Verus, Vulcan, Watchdog, Mospixel, NCP, Summer, Tenacity, Flux,
/// Sigma, Deadcode, Celestial and Novoline criticals ports.
pub const CRITICALS_MODES: [&str; 50] = [
    "Off",
    "Packet",
    "NoGround",
    "Jump",
    "Blink",
    "Timer",
    "NCPPacket",
    "BlocksMC",
    "BlocksMC2",
    "Hop",
    "MiniJump",
    "TPHop",
    "LowJump",
    "CustomMotion",
    "Visual",
    "Edit",
    "PacketAttack",
    "Verus",
    "Vulcan",
    "Watchdog",
    "Mospixel",
    "NCP",
    "Summer",
    "TenacityWatchdog",
    "TenacityPacket",
    "TenacityDev",
    "TenacityVerus",
    "FluxPacket",
    "FluxJump",
    "FluxHypixel",
    "SigmaPacket",
    "SigmaHPacket",
    "SigmaHover",
    "SigmaJump",
    "SigmaMinis",
    "DeadcodePacket",
    "DeadcodeJump",
    "DeadcodeFall",
    "DeadcodeMatrixDisabler",
    "DeadcodeMatrix",
    "CelestialPacket",
    "CelestialNCP",
    "CelestialNCPNew",
    "CelestialSunriseGround",
    "CelestialSunriseAir",
    "CelestialSunriseDrop",
    "CelestialOldSunrise",
    "CelestialOldMatrix",
    "CelestialMatrix630",
    "Novoline",
];

/// The Criticals modes each setting belongs to, by `CRITICALS_MODES` index.
const CRIT_ALL: &[usize] = &[
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
];
const CRIT_PACKET: &[usize] = &[1];
const CRIT_FDP: &[usize] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];
const CRIT_JUMP: &[usize] = &[3];
const CRIT_BLINK: &[usize] = &[4];
const CRIT_TIMER: &[usize] = &[5];
const CRIT_CUSTOM: &[usize] = &[13];
const CRIT_EDIT: &[usize] = &[15];
const CRIT_ALAN: &[usize] = &[16, 17];
const CRIT_WATCHDOG: &[usize] = &[19];
const CRIT_NCP: &[usize] = &[21];
const CRIT_SUMMER: &[usize] = &[22];
const CRIT_TENACITY: &[usize] = &[23, 24, 25, 26];
const CRIT_TENACITY_WATCHDOG: &[usize] = &[23];
const CRIT_FLUX: &[usize] = &[27, 28, 29];
const CRIT_SIGMA: &[usize] = &[30, 31, 32, 33, 34];
const CRIT_DEADCODE: &[usize] = &[35, 36, 37, 38, 39];
const CRIT_DEADCODE_FALL: &[usize] = &[37];
const CRIT_DEADCODE_AUTO: &[usize] = &[37];
const CRIT_CELESTIAL_MINI: &[usize] = &[40, 41, 42, 43, 44, 45, 46, 47];
const CRIT_CELESTIAL_NCP: &[usize] = &[41];
const CRIT_CELESTIAL_OLD_MATRIX: &[usize] = &[47];
const CRIT_CELESTIAL_MATRIX: &[usize] = &[48];
const CRIT_NOVOLINE: &[usize] = &[49];

/// The window's starting size, in GUI pixels.
pub const DEFAULT_W: f32 = 480.0;
pub const DEFAULT_H: f32 = 244.0;
/// The smallest the window can be dragged to, so the panes stay usable.
pub const MIN_W: f32 = 360.0;
pub const MIN_H: f32 = 190.0;

/// The tabs across the top of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Render,
    Movement,
    Combat,
    Player,
    World,
    Misc,
    /// The saved-config manager, which has no modules of its own.
    Config,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Category::Render,
        Category::Movement,
        Category::Combat,
        Category::Player,
        Category::World,
        Category::Misc,
        Category::Config,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Render => "Render",
            Category::Movement => "Movement",
            Category::Combat => "Combat",
            Category::Player => "Player",
            Category::World => "World",
            Category::Misc => "Misc",
            Category::Config => "Config",
        }
    }

    pub fn index(self) -> usize {
        Category::ALL
            .iter()
            .position(|c| *c == self)
            .unwrap_or_default()
    }
}

/// What a setting holds and how the settings pane draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingKind {
    Bool,
    Int {
        min: i32,
        max: i32,
        step: i32,
    },
    Float {
        min: f32,
        max: f32,
        step: f32,
    },
    Choice(&'static [&'static str]),
    /// A free line of text, edited in place (block ids, names).
    Text,
}

/// A setting's current value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Float(f32),
    Choice(usize),
    Text(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: SettingKind,
    pub value: Value,
    /// Draw the setting only while another setting of the same module holds
    /// one of these choices: the NoFall modes' own settings.
    pub shown_when: Option<(&'static str, &'static [usize])>,
}

impl Setting {
    pub fn new(id: &'static str, label: &'static str, kind: SettingKind, value: Value) -> Self {
        let mut setting = Self {
            id,
            label,
            kind,
            value,
            shown_when: None,
        };
        setting.clamp();
        setting
    }

    /// Shows the setting only while `id`'s choice is one of `values`.
    pub fn shown_when(mut self, id: &'static str, values: &'static [usize]) -> Self {
        self.shown_when = Some((id, values));
        self
    }

    pub fn bool(id: &'static str, label: &'static str, value: bool) -> Self {
        Self::new(id, label, SettingKind::Bool, Value::Bool(value))
    }

    pub fn int(
        id: &'static str,
        label: &'static str,
        value: i32,
        min: i32,
        max: i32,
        step: i32,
    ) -> Self {
        Self::new(
            id,
            label,
            SettingKind::Int { min, max, step },
            Value::Int(value),
        )
    }

    pub fn float(
        id: &'static str,
        label: &'static str,
        value: f32,
        min: f32,
        max: f32,
        step: f32,
    ) -> Self {
        Self::new(
            id,
            label,
            SettingKind::Float { min, max, step },
            Value::Float(value),
        )
    }

    pub fn choice(
        id: &'static str,
        label: &'static str,
        value: usize,
        options: &'static [&'static str],
    ) -> Self {
        Self::new(
            id,
            label,
            SettingKind::Choice(options),
            Value::Choice(value),
        )
    }

    pub fn text_value(id: &'static str, label: &'static str, value: &str) -> Self {
        Self::new(id, label, SettingKind::Text, Value::Text(value.to_owned()))
    }

    pub fn as_bool(&self) -> bool {
        matches!(&self.value, Value::Bool(true))
    }

    pub fn as_int(&self) -> i32 {
        match &self.value {
            Value::Int(v) => *v,
            Value::Float(v) => v.round() as i32,
            Value::Choice(v) => *v as i32,
            Value::Bool(v) => i32::from(*v),
            Value::Text(_) => 0,
        }
    }

    pub fn as_float(&self) -> f32 {
        match &self.value {
            Value::Float(v) => *v,
            Value::Int(v) => *v as f32,
            Value::Choice(v) => *v as f32,
            Value::Bool(v) => f32::from(u8::from(*v)),
            Value::Text(_) => 0.0,
        }
    }

    pub fn as_choice(&self) -> usize {
        match &self.value {
            Value::Choice(v) => *v,
            Value::Int(v) => (*v).max(0) as usize,
            _ => 0,
        }
    }

    pub fn as_text(&self) -> &str {
        match &self.value {
            Value::Text(v) => v,
            _ => "",
        }
    }

    /// The selected choice's label, or that choice's index.
    pub fn choice_label(&self) -> &'static str {
        match self.kind {
            SettingKind::Choice(options) => options.get(self.as_choice()).copied().unwrap_or("?"),
            _ => "?",
        }
    }

    /// Every choice, for a dropdown a future module may draw.
    #[allow(dead_code)]
    pub fn choice_options(&self) -> &'static [&'static str] {
        match self.kind {
            SettingKind::Choice(options) => options,
            _ => &[],
        }
    }

    /// The value as it is kept in the mod's settings folder.
    pub fn text(&self) -> String {
        match &self.value {
            Value::Bool(v) => (if *v { "true" } else { "false" }).to_owned(),
            Value::Int(v) => v.to_string(),
            Value::Float(v) => format!("{v:.3}"),
            Value::Choice(v) => v.to_string(),
            Value::Text(v) => v.clone(),
        }
    }

    /// Reads a kept value, clamping it to this setting's range.
    pub fn set_text(&mut self, text: &str) {
        let text = text.trim();
        match (&self.kind, &mut self.value) {
            (SettingKind::Bool, Value::Bool(v)) => {
                *v = matches!(text, "true" | "1" | "on" | "yes");
            }
            (SettingKind::Text, Value::Text(v)) => {
                *v = text.to_owned();
            }
            (SettingKind::Int { min, max, .. }, Value::Int(v)) => {
                if let Ok(n) = text.parse::<i32>() {
                    *v = n.clamp(*min, *max);
                }
            }
            (SettingKind::Float { min, max, .. }, Value::Float(v)) => {
                if let Ok(n) = text.parse::<f32>()
                    && n.is_finite()
                {
                    *v = n.clamp(*min, *max);
                }
            }
            (SettingKind::Choice(options), Value::Choice(v)) => {
                if let Ok(n) = text.parse::<usize>() {
                    *v = n.min(options.len().saturating_sub(1));
                } else if let Some(i) = options.iter().position(|o| *o == text) {
                    *v = i;
                }
            }
            _ => {}
        }
        self.clamp();
    }

    /// Snaps and clamps a numeric value (the UI's sliders).
    pub fn set_number(&mut self, n: f32) {
        if !n.is_finite() {
            return;
        }
        match (&self.kind, &mut self.value) {
            (SettingKind::Int { min, max, step }, Value::Int(v)) => {
                let step = (*step).max(1) as f32;
                let n = ((n / step).round() * step).clamp(*min as f32, *max as f32);
                *v = n as i32;
            }
            (SettingKind::Float { min, max, step }, Value::Float(v)) => {
                let step = if *step > 0.0 { *step } else { 0.01 };
                let n = ((n / step).round() * step).clamp(*min, *max);
                // Strip binary noise from the snap (0.30000001 -> 0.3).
                *v = (n * 1000.0).round() / 1000.0;
            }
            _ => {}
        }
    }

    fn clamp(&mut self) {
        match (&self.kind, &mut self.value) {
            (SettingKind::Int { min, max, .. }, Value::Int(v)) => *v = (*v).clamp(*min, *max),
            (SettingKind::Float { min, max, .. }, Value::Float(v)) => *v = v.clamp(*min, *max),
            (SettingKind::Choice(options), Value::Choice(v)) => {
                *v = (*v).min(options.len().saturating_sub(1));
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub category: Category,
    pub enabled: bool,
    /// The key that toggles the module while playing, as a vanilla key name
    /// (`key.keyboard.g`, `key.mouse.right`), or none.
    pub bind: Option<String>,
    pub settings: Vec<Setting>,
}

impl Module {
    pub fn setting(&self, id: &str) -> Option<&Setting> {
        self.settings.iter().find(|s| s.id == id)
    }

    /// A module's setting for modules that edit their own values.
    #[allow(dead_code)]
    pub fn setting_mut(&mut self, id: &str) -> Option<&mut Setting> {
        self.settings.iter_mut().find(|s| s.id == id)
    }

    pub fn bool(&self, id: &str) -> bool {
        self.setting(id).is_some_and(Setting::as_bool)
    }

    pub fn int(&self, id: &str) -> i32 {
        self.setting(id).map_or(0, Setting::as_int)
    }

    pub fn float(&self, id: &str) -> f32 {
        self.setting(id).map_or(0.0, Setting::as_float)
    }

    pub fn choice(&self, id: &str) -> usize {
        self.setting(id).map_or(0, Setting::as_choice)
    }

    /// Whether the settings pane draws `setting`: always, unless it belongs to
    /// a mode (its condition, another setting's choice, does not hold).
    pub fn setting_visible(&self, setting: &Setting) -> bool {
        setting
            .shown_when
            .is_none_or(|(id, values)| values.contains(&self.choice(id)))
    }

    pub fn text(&self, id: &str) -> &str {
        self.setting(id).map_or("", Setting::as_text)
    }
}

fn default_modules() -> Vec<Module> {
    vec![
        Module {
            id: FULLBRIGHT,
            name: "FullBright",
            description: "See in the dark. Gamma brightens the lightmap; NightVision adds the effect.",
            category: Category::Render,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 0, &["Gamma", "NightVision"]),
                Setting::int("brightness", "Brightness", 15, 1, 15, 1),
                Setting::bool("smooth", "Smooth", true),
                Setting::float("speed", "Speed", 0.1, 0.05, 2.0, 0.05),
                Setting::int("amplifier", "Night Vision level", 0, 0, 3, 1),
            ],
        },
        Module {
            id: ARRAYLIST,
            name: "ArrayList",
            description: "Lists the enabled modules in the top-right corner while playing.",
            category: Category::Render,
            enabled: true,
            bind: None,
            settings: vec![
                Setting::bool("background", "Background", true),
                Setting::bool("shadow", "Text shadow", true),
                Setting::bool("sorted", "Sort alphabetically", true),
            ],
        },
        Module {
            id: WATERMARK,
            name: "Watermark",
            description: "Shows your name, FPS and ping in the corner and at the offset you pick.",
            category: Category::Render,
            enabled: true,
            bind: None,
            settings: vec![
                Setting::choice(
                    "corner",
                    "Corner",
                    0,
                    &["Top Left", "Top Right", "Bottom Left", "Bottom Right"],
                ),
                Setting::int("offset_x", "Offset X", 10, 0, 400, 1),
                Setting::int("offset_y", "Offset Y", 10, 0, 400, 1),
                Setting::bool("shadow", "Text shadow", true),
            ],
        },
        Module {
            id: AUTOTOOL,
            name: "AutoTool",
            description: "Switches to the best tool for the block you are mining, and back afterwards.",
            category: Category::Player,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("selector", "ToolSelector", 0, &["Dynamic", "Static"]),
                Setting::bool("ignore_durability", "IgnoreDurability", false),
                Setting::bool("consider_inventory", "ConsiderInventory", false),
                Setting::int("static_slot", "Slot", 0, 0, 8, 1),
                Setting::choice("filter", "Filter", 0, &["Blacklist", "Whitelist"]),
                Setting::text_value("blocks", "Blocks", ""),
                Setting::bool("silk_touch", "SilkTouchHandler", false),
                Setting::choice(
                    "silk_touch_filter",
                    "SilkTouch Filter",
                    0,
                    &["Whitelist", "Blacklist"],
                ),
                Setting::text_value(
                    "silk_touch_blocks",
                    "SilkTouch Blocks",
                    "minecraft:ender_chest, minecraft:glowstone, minecraft:sea_lantern, minecraft:turtle_egg",
                ),
                Setting::int("swap_back_delay", "SwapBackDelay", 20, 1, 100, 1),
                Setting::int("switch_delay", "SwitchDelay", 0, 0, 100, 1),
                Setting::bool("require_sneaking", "RequireSneaking", false),
                Setting::bool("not_during_combat", "NotDuringCombat", false),
                Setting::bool("require_near_bed", "RequireNearBed", false),
                Setting::float("bed_distance", "BedDistance", 10.0, 3.0, 50.0, 1.0),
            ],
        },
        Module {
            id: BLINK,
            name: "Blink",
            description: "Holds your movement packets back, so the server keeps seeing you where you were until the held trail goes out in one burst.",
            category: Category::Player,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::bool("pulse", "Pulse", false),
                Setting::int("delay", "Delay", 20, 2, 200, 1),
                Setting::bool("marker", "Server marker", true),
                Setting::text_value("marker_color", "Marker color", "#60FFAA00"),
                Setting::bool("marker_walls", "Marker through walls", true),
            ],
        },
        Module {
            id: AUTOWEAPON,
            name: "AutoWeapon",
            description: "Switches to the best weapon in your hotbar when you attack or aim at an entity, breaking shields and smashing with a mace.",
            category: Category::Combat,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::text_value("preferred", "Preferred", "Sword"),
                Setting::choice(
                    "priority",
                    "Priority",
                    0,
                    &["Default", "Knockback", "Damage", "AttackSpeed"],
                ),
                Setting::bool("auto_shield_break", "AutoShieldBreak", true),
                Setting::bool("auto_mace", "AutoMace", true),
                Setting::int("switch_back", "SwitchBack", 20, 1, 300, 1),
                Setting::choice(
                    "change_on",
                    "ChangeOn",
                    0,
                    &["OnAttack", "OnTarget", "Both"],
                ),
            ],
        },
        Module {
            id: OFFHAND,
            name: "OffHand",
            description: "Keeps a totem, a golden apple or an end crystal in your off hand, swapping it in when the fight needs it.",
            category: Category::Combat,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("item", "Item", 0, &OFFHAND_ITEMS),
                Setting::float("health", "Health", 10.0, 0.0, 20.0, 0.5)
                    .shown_when("item", OFFHAND_SAFETY),
                Setting::bool("absorption", "Absorption", true)
                    .shown_when("item", OFFHAND_SAFETY),
                Setting::bool("sword_only", "Sword only", false)
                    .shown_when("item", OFFHAND_GAPPLE),
                Setting::bool("eat", "Eat gapple", true).shown_when("item", OFFHAND_GAPPLE),
                Setting::float("eat_at", "Eat at", 15.0, 0.0, 20.0, 0.5)
                    .shown_when("item", OFFHAND_GAPPLE),
                Setting::bool("fall", "Fall", true).shown_when("item", OFFHAND_SAFETY),
                Setting::bool("elytra", "Elytra", true).shown_when("item", OFFHAND_SAFETY),
                Setting::bool("explosion", "Explosion", true).shown_when("item", OFFHAND_SAFETY),
                Setting::bool("hotbar", "Hotbar", false),
                Setting::int("delay", "Delay", 2, 0, 20, 1),
            ],
        },
        Module {
            id: AUTOARMOR,
            name: "AutoArmor",
            description: "Puts the best armour you carry onto your body, replacing a worn piece only when the spare is better.",
            category: Category::Combat,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 0, &AUTOARMOR_MODES),
                Setting::choice("compare", "Compare", 0, &AUTOARMOR_COMPARE),
                Setting::int("min_durability", "Min durability", 5, 0, 100, 1),
                Setting::int("delay", "Delay", 2, 0, 20, 1),
            ],
        },
        Module {
            id: AUTOXP,
            name: "AutoXP",
            description: "Throws experience bottles at your feet to repair Mending gear, stopping when nothing is damaged.",
            category: Category::Combat,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 0, &AUTOXP_MODES),
                Setting::int("delay", "Delay", 2, 0, 20, 1),
                Setting::int("frequency", "Frequency", 1, 1, 16, 1),
                Setting::bool("rotate", "Rotate", false),
                Setting::bool("air", "Air", true),
                Setting::bool("inventory", "Inventory", false),
                Setting::bool("stop", "Stop", true),
                Setting::int("stop_at", "Stop at", 100, 80, 100, 1),
                Setting::bool("auto_disable", "Auto disable", false),
                Setting::bool("tools", "Tools", false),
            ],
        },
        Module {
            id: HOLEFILL,
            name: "HoleFill",
            description: "Fills nearby sealed holes with blocks so players cannot use them for cover.",
            category: Category::Combat,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("block", "Block", 0, &HOLEFILL_BLOCKS),
                Setting::int("range", "Range", 4, 1, 5, 1),
                Setting::int("vertical", "Vertical", 3, 0, 6, 1),
                Setting::bool("doubles", "2x1 holes", false),
                Setting::bool("quads", "2x2 holes", false),
                Setting::int("per_tick", "Blocks per tick", 1, 1, 4, 1),
                Setting::int("delay", "Delay", 2, 0, 20, 1),
                Setting::bool("self_fill", "Self fill", false),
                Setting::float("min_self", "Min self", 0.0, 0.0, 6.0, 0.5),
                Setting::bool("require_target", "Require target", false),
                Setting::float("target_range", "Target range", 8.0, 1.0, 16.0, 0.5),
                Setting::bool("smart", "Smart", false),
                Setting::bool("auto_disable", "Auto disable", false),
                Setting::bool("silent", "Silent", true),
            ],
        },
        Module {
            id: REACH,
            name: "Reach",
            description: "Widens the crosshair's entity reach so you can pick and hit a little further than vanilla.",
            category: Category::Combat,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::float("min", "Range min", 3.0, 3.0, 6.0, 0.05),
                Setting::float("max", "Range max", 4.0, 3.0, 6.0, 0.05),
            ],
        },
        Module {
            id: AUTOCLICKER,
            name: "AutoClicker",
            description: "Clicks for you at a chosen rate, with the reference gates: weapon only, on click, and stop on blocks.",
            category: Category::Combat,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::bool("left", "Left click", true),
                Setting::bool("right", "Right click", false),
                Setting::choice("mode", "Mode", 0, &AUTOCLICKER_MODES),
                Setting::float("min_cps", "Min CPS", 8.0, 1.0, 20.0, 0.5)
                    .shown_when("mode", AUTOCLICKER_NORMAL),
                Setting::float("max_cps", "Max CPS", 14.0, 1.0, 20.0, 0.5)
                    .shown_when("mode", AUTOCLICKER_NORMAL),
                Setting::int("drag_length", "Drag length", 17, 1, 50, 1)
                    .shown_when("mode", AUTOCLICKER_DRAG),
                Setting::int("drag_delay", "Drag delay", 6, 1, 20, 1)
                    .shown_when("mode", AUTOCLICKER_DRAG),
                Setting::bool("on_click", "On click", false),
                Setting::bool("stop_on_block", "Stop on block", true),
                Setting::bool("weapon_only", "Weapon only", false),
            ],
        },
        Module {
            id: AIMASSIST,
            name: "AimAssist",
            description: "Turns your view toward nearby targets inside a cone, one step per tick.",
            category: Category::Combat,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("point", "Aim at", 0, &AIMASSIST_POINTS),
                Setting::float("range", "Range", 4.5, 1.0, 8.0, 0.1),
                Setting::float("fov", "FOV", 90.0, 30.0, 360.0, 5.0),
                Setting::float("speed", "Speed", 4.0, 0.5, 20.0, 0.5),
                Setting::bool("yaw", "Yaw", true),
                Setting::bool("pitch", "Pitch", false),
                Setting::bool("players", "Players", true),
                Setting::bool("mobs", "Mobs", false),
                Setting::bool("invisibles", "Invisibles", false),
                Setting::bool("through_walls", "Through walls", false),
                Setting::bool("weapon_only", "Weapon only", false),
                Setting::bool("click_aim", "Click aim", false),
                Setting::bool("stop_on_target", "Stop on target", false),
                Setting::bool("sticky", "Sticky", false),
            ],
        },
        Module {
            id: SURROUND,
            name: "Surround",
            description: "Walls your feet in with blocks, following the shape of the hole you stand in.",
            category: Category::Combat,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("block", "Block", 0, &SURROUND_BLOCKS),
                Setting::int("per_tick", "Blocks per tick", 2, 1, 4, 1),
                Setting::int("delay", "Delay", 2, 0, 20, 1),
                Setting::bool("center", "Center", false),
                Setting::bool("flatten", "Flatten", false),
                Setting::bool("cover", "Cover", false),
                Setting::bool("on_ground", "Only on ground", true),
                Setting::bool("silent", "Silent", true),
                Setting::bool("auto_disable", "Auto disable", false),
            ],
        },
        Module {
            id: CRYSTALAURA,
            name: "CrystalAura",
            description: "Places and breaks end crystals where the blast hurts a target more than it hurts you.",
            category: Category::Combat,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::bool("place", "Place", true),
                Setting::int("place_delay", "Place delay", 0, 0, 10, 1),
                Setting::float("place_range", "Place range", 4.5, 1.0, 6.0, 0.1),
                Setting::bool("breaking", "Break", true),
                Setting::int("break_delay", "Break delay", 0, 0, 10, 1),
                Setting::float("break_range", "Break range", 4.5, 1.0, 6.0, 0.1),
                Setting::float("min_damage", "Min damage", 15.0, 2.0, 100.0, 1.0),
                Setting::float("max_self_damage", "Max self damage", 40.0, 2.0, 100.0, 1.0),
                Setting::bool("allow_trades", "Allow trades", true),
                Setting::float("target_range", "Target range", 10.0, 4.0, 16.0, 0.5),
                Setting::bool("instant", "Instant", true),
                Setting::bool("rotate", "Rotate", false),
                Setting::bool("silent", "Silent", true),
                Setting::bool("debug", "Debug", false),
            ],
        },
        Module {
            id: CRITICALS,
            name: "Criticals",
            description: "Turns your hits into critical hits. The modes follow LiquidBounce, FDPClient and the Alan, Verus, Vulcan, Watchdog, Mospixel, NCP, Summer, Tenacity, Flux, Sigma, Deadcode, Celestial and Novoline criticals.",
            category: Category::Combat,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 1, &CRITICALS_MODES),
                Setting::choice(
                    "sprint_stop",
                    "Stop sprinting",
                    1,
                    &["None", "Legit", "OnNetwork", "OnAttack"],
                )
                .shown_when("mode", CRIT_ALL),
                Setting::float("sprint_range", "Sprint range", 4.0, 0.0, 10.0, 0.5)
                    .shown_when("mode", CRIT_ALL),
                Setting::bool("only_sprinting", "OnlyWhileSprinting", false)
                    .shown_when("mode", CRIT_ALL),
                Setting::choice(
                    "packet_mode",
                    "Packet Mode",
                    1,
                    &[
                        "Vanilla",
                        "NoCheatPlus",
                        "Falling",
                        "Low",
                        "Down",
                        "Grim",
                        "BlocksMC",
                    ],
                )
                .shown_when("mode", CRIT_PACKET),
                Setting::choice(
                    "packet_type",
                    "Packet Type",
                    0,
                    &["FULL", "POSITION_AND_ON_GROUND"],
                )
                .shown_when("mode", CRIT_PACKET),
                Setting::int("delay", "Delay", 0, 0, 5000, 50).shown_when("mode", CRIT_FDP),
                Setting::int("hurt_time", "Hurt time", 10, 0, 20, 1).shown_when("mode", CRIT_FDP),
                Setting::float("jump_height", "Jump Height", 0.42, 0.1, 0.42, 0.01)
                    .shown_when("mode", CRIT_JUMP),
                Setting::float("jump_range", "Jump Range", 4.0, 1.0, 6.0, 0.5)
                    .shown_when("mode", CRIT_JUMP),
                Setting::bool("jump_optimize", "OptimizeForCooldown", true)
                    .shown_when("mode", CRIT_JUMP),
                Setting::bool("jump_can_be_seen", "CanBeSeen", true).shown_when("mode", CRIT_JUMP),
                Setting::int("blink_delay_min", "Blink Delay Min", 300, 0, 1000, 50)
                    .shown_when("mode", CRIT_BLINK),
                Setting::int("blink_delay_max", "Blink Delay Max", 600, 0, 1000, 50)
                    .shown_when("mode", CRIT_BLINK),
                Setting::float("blink_range", "Blink Range", 4.0, 0.0, 10.0, 0.5)
                    .shown_when("mode", CRIT_BLINK),
                Setting::float("timer_speed", "Timer Speed", 0.8, 0.1, 1.0, 0.05)
                    .shown_when("mode", CRIT_TIMER),
                Setting::float("timer_range", "Timer Range", 4.0, 0.0, 10.0, 0.5)
                    .shown_when("mode", CRIT_TIMER),
                Setting::float("custom_motion_y", "Custom-Y", 0.2, 0.01, 0.42, 0.01)
                    .shown_when("mode", CRIT_CUSTOM),
                Setting::int("edit_delay", "Edit Delay", 500, 0, 1000, 50)
                    .shown_when("mode", CRIT_EDIT),
                Setting::int("alan_delay", "Delay", 500, 0, 1000, 50).shown_when("mode", CRIT_ALAN),
                Setting::int("watchdog_delay", "Watchdog Delay", 500, 0, 1000, 50)
                    .shown_when("mode", CRIT_WATCHDOG),
                Setting::int("ncp_ticks", "NCP Ticks", 6, 0, 20, 1).shown_when("mode", CRIT_NCP),
                Setting::int("summer_delay", "Summer Delay", 500, 0, 2000, 100)
                    .shown_when("mode", CRIT_SUMMER),
                Setting::int("summer_ground_ticks", "Summer Ground Ticks", 1, 0, 10, 1)
                    .shown_when("mode", CRIT_SUMMER),
                Setting::choice(
                    "tenacity_watchdog_mode",
                    "Watchdog Sub-mode",
                    0,
                    &["Packet", "Edit"],
                )
                .shown_when("mode", CRIT_TENACITY_WATCHDOG),
                Setting::int("tenacity_delay", "Tenacity Delay", 1, 0, 20, 1)
                    .shown_when("mode", CRIT_TENACITY),
                Setting::int("flux_hurt_time", "Flux Hurt Time", 15, 0, 20, 1)
                    .shown_when("mode", CRIT_FLUX),
                Setting::int("flux_delay", "Flux Delay", 300, 0, 5000, 100)
                    .shown_when("mode", CRIT_FLUX),
                Setting::bool("flux_post_motion", "Flux Post Motion", true)
                    .shown_when("mode", CRIT_FLUX),
                Setting::int("sigma_hurt_time", "Sigma Hurt Time", 15, 0, 20, 1)
                    .shown_when("mode", CRIT_SIGMA),
                Setting::float(
                    "deadcode_fall_distance",
                    "Fall distance",
                    0.2,
                    0.01,
                    0.41,
                    0.01,
                )
                .shown_when("mode", CRIT_DEADCODE_FALL),
                Setting::int("deadcode_delay", "Deadcode Delay", 0, 0, 500, 10)
                    .shown_when("mode", CRIT_DEADCODE),
                Setting::bool("deadcode_auto_jump", "Fall auto jump", false)
                    .shown_when("mode", CRIT_DEADCODE_AUTO),
                Setting::float(
                    "deadcode_auto_jump_distance",
                    "Auto jump distance",
                    6.0,
                    1.0,
                    20.0,
                    0.5,
                )
                .shown_when("mode", CRIT_DEADCODE_AUTO),
                Setting::bool("deadcode_smart", "Smart criticals", true)
                    .shown_when("mode", CRIT_DEADCODE),
                Setting::bool("celestial_mini_jump", "Mini-Jump", false)
                    .shown_when("mode", CRIT_CELESTIAL_MINI),
                Setting::int(
                    "celestial_ncp_delay",
                    "Celestial NCP Delay",
                    800,
                    0,
                    2000,
                    100,
                )
                .shown_when("mode", CRIT_CELESTIAL_NCP),
                Setting::bool("celestial_ground_false", "Ground False", false)
                    .shown_when("mode", CRIT_CELESTIAL_OLD_MATRIX),
                Setting::bool("celestial_reduce_damage", "Reduce damage on disable", true)
                    .shown_when("mode", CRIT_CELESTIAL_MATRIX),
                Setting::bool("novoline_hypixel", "Hypixel", true)
                    .shown_when("mode", CRIT_NOVOLINE),
                Setting::float("novoline_random", "Random", 0.001, 0.0, 0.01, 0.001)
                    .shown_when("mode", CRIT_NOVOLINE),
                Setting::bool("visuals", "Visuals", false).shown_when("mode", CRIT_ALL),
                Setting::bool("visual_fake", "Fake", false).shown_when("mode", CRIT_ALL),
                Setting::int("visual_critical", "Critical particles", 1, 0, 20, 1)
                    .shown_when("mode", CRIT_ALL),
                Setting::int("visual_magic", "Magic particles", 0, 0, 20, 1)
                    .shown_when("mode", CRIT_ALL),
            ],
        },
        Module {
            id: SCAFFOLD,
            name: "Scaffold",
            description: "Places blocks under your feet while you walk, bridge or tower. The modes follow Flux, Alan, Augustus, Novoline, Sigma, Tenacity, Yuri and LiquidBounce.",
            category: Category::Player,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 1, &SCAFFOLD_MODES),
                Setting::int("place_delay", "Place delay", 90, 0, 500, 10)
                    .shown_when("mode", SCAFFOLD_ALL),
                Setting::bool("rotations", "Rotations", true).shown_when("mode", SCAFFOLD_ALL),
                Setting::bool("silent", "Silent switch", true).shown_when("mode", SCAFFOLD_ALL),
                Setting::choice("sprint", "Sprint", 1, &SCAFFOLD_SPRINTS)
                    .shown_when("mode", SCAFFOLD_ALL),
                Setting::choice("tower", "Tower", 0, &SCAFFOLD_TOWERS)
                    .shown_when("mode", SCAFFOLD_ALL),
                Setting::bool("tower_move", "Tower move", true).shown_when("mode", SCAFFOLD_ALL),
                Setting::bool("keep_y", "Keep Y", false).shown_when("mode", SCAFFOLD_ALL),
                Setting::choice("downwards", "Downwards", 0, &SCAFFOLD_DOWNWARDS)
                    .shown_when("mode", SCAFFOLD_ALL),
                Setting::bool("sneak", "Sneak", false).shown_when("mode", SCAFFOLD_ALL),
                Setting::bool("swing", "Swing", true).shown_when("mode", SCAFFOLD_ALL),
                Setting::bool("auto_jump", "Auto jump", false).shown_when("mode", SCAFFOLD_ALL),
                Setting::float("extend", "Extend", 0.4, 0.0, 2.0, 0.05)
                    .shown_when("mode", SCAFFOLD_EXTEND),
                Setting::int("eagle_length", "Eagle length", 1, 1, 4, 1)
                    .shown_when("mode", SCAFFOLD_EAGLE),
                Setting::bool("eagle_ground", "Only on ground", false)
                    .shown_when("mode", SCAFFOLD_EAGLE),
                Setting::bool("eagle_blocks", "Only holding blocks", false)
                    .shown_when("mode", SCAFFOLD_EAGLE),
                Setting::bool("eagle_backwards", "Only backwards", false)
                    .shown_when("mode", SCAFFOLD_EAGLE),
            ],
        },
        Module {
            id: CHESTSTEALER,
            name: "ChestStealer",
            description: "Empties the open chest or furnace into your inventory, with human-like delays, slot orders and quick hotbar swaps.",
            category: Category::Player,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::bool("smart_delay", "SmartDelay", false),
                Setting::int("delay_multiplier", "DelayMultiplier", 120, 0, 500, 1),
                Setting::bool("smart_order", "SmartOrder", true),
                Setting::bool("simulate_short_stop", "SimulateShortStop", false),
                Setting::bool("auto_close", "AutoClose", true),
                Setting::choice("order", "Order", 0, &CHESTSTEALER_ORDERS),
                Setting::bool("quick_swaps", "QuickSwaps", false),
                Setting::choice("on_full", "OnFull", 0, &CHESTSTEALER_ON_FULL),
                Setting::int("delay", "Delay", 50, 0, 500, 1),
                Setting::int("start_delay", "StartDelay", 75, 0, 500, 1),
                Setting::int("close_delay", "CloseDelay", 75, 0, 500, 1),
                Setting::bool("chest_title", "ChestTitle", true),
                Setting::bool("random_slot", "RandomSlot", true),
                Setting::bool("progress_bar", "ProgressBar", true),
                Setting::bool("furnace", "Furnace", false),
                Setting::bool("highlight_slot", "Highlight-Slot", false),
                Setting::text_value("background_color", "BackgroundColor", "#FF808080"),
                Setting::int("border_strength", "Border-Strength", 3, 1, 5, 1),
                Setting::text_value("border_color", "BorderColor", "#FF808080"),
                Setting::choice("debug", "Debug", 0, &CHESTSTEALER_DEBUG),
                Setting::bool("item_debug", "ItemStolen-Debug", false),
            ],
        },
        Module {
            id: FASTBREAK,
            name: "FastBreak",
            description: "Breaks blocks faster: a damage threshold (LiquidBounce), or a speed percentage or tick count (Alan), with the AbortAnother Grim bypass.",
            category: Category::Player,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 0, &FASTBREAK_MODES),
                Setting::float("break_damage", "BreakDamage", 0.8, 0.1, 1.0, 0.1)
                    .shown_when("mode", FASTBREAK_BREAK_DAMAGE),
                Setting::int("speed", "Speed", 50, 0, 100, 1).shown_when("mode", FASTBREAK_SPEED),
                Setting::int("ticks", "Ticks", 1, 1, 100, 1)
                    .shown_when("mode", FASTBREAK_TICKS_ONLY),
                Setting::bool("equal_air_ground", "EqualAirGroundDig", true)
                    .shown_when("mode", FASTBREAK_ALAN),
                Setting::bool("abort_another", "AbortAnother", false),
                Setting::bool("only_tool", "OnlyTool", false),
                Setting::bool("ignore_mining_fatigue", "IgnoreMiningFatigue", false),
            ],
        },
        Module {
            id: VELOCITY,
            name: "Velocity",
            description: "Cancels, scales or redirects knockback. The modes follow LiquidBounce, FDPClient and the Alan, Sigma, Tenacity, Flux, Expensive, Azura, Augustus and Cel velocity implementations.",
            category: Category::Combat,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 1, &VELOCITY_MODES),
                Setting::float("horizontal", "Horizontal", 0.6, 0.0, 1.0, 0.05)
                    .shown_when("mode", VELO_STANDARD),
                Setting::float("vertical", "Vertical", 1.0, 0.0, 1.0, 0.05)
                    .shown_when("mode", VELO_STANDARD),
                Setting::float(
                    "motion_horizontal",
                    "Motion Horizontal",
                    0.0,
                    0.0,
                    1.0,
                    0.05,
                )
                .shown_when("mode", VELO_STANDARD),
                Setting::float("motion_vertical", "Motion Vertical", 0.0, 0.0, 1.0, 0.05)
                    .shown_when("mode", VELO_STANDARD),
                Setting::choice("filter", "Filter", 0, &["Always", "OnGround", "InAir"])
                    .shown_when("mode", VELO_STANDARD),
                Setting::bool("explosions", "Explosions", true).shown_when("mode", VELO_STANDARD),
                Setting::int("chance", "Chance", 100, 1, 100, 1).shown_when("mode", VELO_CHANCE),
                Setting::bool("only_move", "Only Move", false).shown_when("mode", VELO_ONLY_MOVE),
                Setting::int("motion_hurt_time", "Hurt Time", 8, 0, 10, 1)
                    .shown_when("mode", VELO_MOTION),
                Setting::float("scale_horizontal", "Horizontal", 0.6, 0.0, 1.0, 0.05)
                    .shown_when("mode", VELO_MOTION),
                Setting::float("scale_vertical", "Vertical", 1.0, 0.0, 1.0, 0.05)
                    .shown_when("mode", VELO_MOTION),
                Setting::int("delay_ticks", "Delay", 3, 0, 20, 1).shown_when("mode", VELO_DELAY),
                Setting::float("x_modifier", "X Modifier", -1.0, -2.0, 2.0, 0.05)
                    .shown_when("mode", VELO_REVERSE),
                Setting::float("z_modifier", "Z Modifier", -1.0, -2.0, 2.0, 0.05)
                    .shown_when("mode", VELO_REVERSE),
                Setting::int("wait_ticks", "Wait", 3, 1, 10, 1).shown_when("mode", VELO_TICK),
                Setting::float("reduction", "Reduction", 0.0, 0.0, 1.0, 0.05)
                    .shown_when("mode", VELO_TICK),
                Setting::bool("reset_vertical", "Reset Vertical", true)
                    .shown_when("mode", VELO_TICK),
                Setting::int("jump_delay", "Jump Delay", 0, 0, 10, 1).shown_when("mode", VELO_JUMP),
                Setting::bool("only_sprinting", "Only Sprinting", false)
                    .shown_when("mode", VELO_JUMP),
                Setting::bool("air_only", "Air Only", true).shown_when("mode", VELO_HYPIXEL),
                Setting::float("y_boost", "Y Boost", 0.0001, 0.0, 0.1, 0.0001)
                    .shown_when("mode", VELO_VERUS),
                Setting::int("grim_duplicates", "Duplicates", 4, 0, 10, 1)
                    .shown_when("mode", VELO_GRIM),
                Setting::bool("grim_block_action", "Block Action", true)
                    .shown_when("mode", VELO_GRIM),
                Setting::int("blocksmc_ticks", "Sneak Ticks", 2, 0, 10, 1)
                    .shown_when("mode", VELO_BLOCKSMC),
                Setting::float("phase_height", "Height", 0.1, 0.0, 1.0, 0.01)
                    .shown_when("mode", VELO_PHASE),
                Setting::bool("phase_ground", "On Ground", true).shown_when("mode", VELO_PHASE),
                Setting::int("click_count", "Clicks", 3, 1, 10, 1).shown_when("mode", VELO_CLICK),
                Setting::int("click_hurt_time", "Hurt Time", 9, 0, 10, 1)
                    .shown_when("mode", VELO_CLICK),
                Setting::int("click_interval", "Interval", 0, 0, 5, 1)
                    .shown_when("mode", VELO_CLICK),
                Setting::float("side_strength", "Strength", 0.6, 0.0, 2.0, 0.05)
                    .shown_when("mode", VELO_SIDE),
                Setting::float("aac_factor", "Factor", 0.62, 0.0, 1.0, 0.01)
                    .shown_when("mode", VELO_AAC),
                Setting::bool("aac_only_air", "Only Air", true).shown_when("mode", VELO_AAC),
                Setting::float("push_reducer", "Reducer", 1.2, 1.0, 5.0, 0.01)
                    .shown_when("mode", VELO_AACPUSH),
                Setting::bool("push_reduce_y", "Reduce Y", false).shown_when("mode", VELO_AACPUSH),
                Setting::float("push_y", "Y Reduction", 0.015, 0.0, 0.1, 0.001)
                    .shown_when("mode", VELO_AACPUSH),
                Setting::float("polar_divisor", "Divisor", 1.2224, 1.0, 2.0, 0.0001)
                    .shown_when("mode", VELO_POLAR),
                Setting::int("polar_hurt_min", "Hurt Min", 1, 0, 10, 1)
                    .shown_when("mode", VELO_POLAR),
                Setting::int("polar_hurt_max", "Hurt Max", 6, 0, 10, 1)
                    .shown_when("mode", VELO_POLAR),
                Setting::float("intave_factor", "Factor", 0.6, 0.0, 1.0, 0.05)
                    .shown_when("mode", VELO_INTAVE),
                Setting::int("intave_hurt_min", "Hurt Min", 5, 0, 10, 1)
                    .shown_when("mode", VELO_INTAVE),
                Setting::int("intave_hurt_max", "Hurt Max", 7, 0, 10, 1)
                    .shown_when("mode", VELO_INTAVE),
                Setting::int("intave_window", "Window", 2000, 0, 10000, 100)
                    .shown_when("mode", VELO_INTAVE),
                Setting::int("intave_jump_chance", "Jump Chance", 50, 0, 100, 1)
                    .shown_when("mode", VELO_INTAVE),
                Setting::bool("spoof_modify_timer", "ModifyTimer", true)
                    .shown_when("mode", VELO_SPOOF),
                Setting::float("spoof_timer", "Timer", 0.6, 0.1, 1.0, 0.05)
                    .shown_when("mode", VELO_SPOOF),
            ],
        },
        Module {
            id: NAMETAGS,
            name: "NameTags",
            description: "Replaces the vanilla name tags with player tags (name, health, head, equipment) and item tags.",
            category: Category::Render,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::bool("players", "Players", true),
                Setting::bool("items", "Items", true),
                Setting::bool("player_head", "Player head", true),
                Setting::bool("item_icons", "Item icons", true),
            ],
        },
        Module {
            id: ESP,
            name: "ESP",
            description: "Outlines players, friends, yourself and mobs with a dashed box, coloured by entity and fading with distance.",
            category: Category::Render,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::bool("players", "Players", true),
                Setting::bool("friends", "Friends", true),
                Setting::bool("self", "Self", false),
                Setting::bool("mobs", "Mobs", true),
                Setting::float("max_distance", "Distance", 72.0, 8.0, 200.0, 1.0),
                Setting::bool("outline", "Outline", true),
                Setting::text_value("friend_color", "Friend color", "#FF34FF60"),
                Setting::text_value("friend_names", "Friends", ""),
            ],
        },
        Module {
            id: TRACERS,
            name: "Tracers",
            description: "Draws lines from your view to other players and (optionally) dropped items.",
            category: Category::Render,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::text_value("color", "Color", "#FFFFFFFF"),
                Setting::text_value("friend_color", "Friend color", "#FFFFFFFF"),
                Setting::bool("players", "Players", true),
                Setting::bool("items", "Dropped items", false),
                Setting::text_value("friend_names", "Friends", ""),
            ],
        },
        Module {
            id: HOLEESP,
            name: "HoleESP",
            description: "Highlights the sealed obsidian and bedrock holes around you.",
            category: Category::Render,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 0, &HOLEESP_MODES),
                Setting::int("radius", "Radius", 8, 1, 32, 1),
                Setting::int("vertical", "Vertical radius", 4, 0, 16, 1),
                Setting::bool("doubles", "Doubles", true),
                Setting::float("height", "Height", 0.25, 0.05, 2.0, 0.05),
                Setting::bool("filled", "Filled", true),
                Setting::bool("outline", "Outline", true),
                Setting::bool("through_walls", "Through walls", true),
                Setting::bool("fade", "Distance fade", true),
                Setting::int("fill_alpha", "Fill alpha", 60, 0, 255, 5),
                Setting::int("outline_alpha", "Outline alpha", 200, 0, 255, 5),
                Setting::text_value("obsidian_color", "Obsidian color", "#FFFF5555"),
                Setting::text_value("bedrock_color", "Bedrock color", "#FF55FF55"),
                Setting::text_value("mixed_color", "Mixed color", "#FFFFAA00"),
                Setting::bool("ignore_own", "Ignore own hole", false),
            ],
        },
        Module {
            id: NORENDER,
            name: "NoRender",
            description: "Stops the game from drawing the elements you pick.",
            category: Category::Render,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::bool("crosshair", "Crosshair", false),
                Setting::bool("boss_bar", "Boss bar", false),
                Setting::bool("scoreboard", "Scoreboard", false),
                Setting::bool("title", "Title and subtitle", false),
                Setting::bool("held_item_name", "Held item name", false),
                Setting::bool("potion_icons", "Potion icons", false),
                Setting::bool("toasts", "Toasts", false),
                Setting::choice("fog", "Fog", 0, &NORENDER_FOG_MODES),
                Setting::bool("water", "Water overlay", false),
                Setting::bool("fire", "Fire overlay", false),
                Setting::bool("particles", "Particles", false),
                Setting::bool("nametags", "Name tags", false),
                Setting::bool("glowing", "Glowing", false),
                Setting::bool("hand", "First-person hand", false),
                Setting::bool("item_frames", "Item frame items", false),
                Setting::bool("mobs", "Mobs", false),
            ],
        },
        Module {
            id: CHAMS,
            name: "Chams",
            description: "Draws entity bodies through walls: as filled boxes, or as the game's own glow outline.",
            category: Category::Render,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 0, &CHAMS_MODES),
                Setting::bool("players", "Players", true),
                Setting::bool("self", "Self", true),
                Setting::bool("monsters", "Monsters", true),
                Setting::bool("animals", "Animals", true),
                Setting::bool("crystals", "Crystals", true),
                Setting::bool("invisibles", "Invisibles", true),
                Setting::bool("through_walls", "Through walls", true),
                Setting::int("range", "Range", 64, 8, 256, 8),
                Setting::text_value("color", "Color", "#3CFF0000"),
            ],
        },
        Module {
            id: VIEWMODEL,
            name: "ViewModel",
            description: "Moves, turns and resizes the first-person hands, and the items in them.",
            category: Category::Render,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::float("main_scale", "Main scale", 1.0, 0.1, 3.0, 0.05),
                Setting::float("main_x", "Main X", 0.0, -1.0, 1.0, 0.01),
                Setting::float("main_y", "Main Y", 0.0, -1.0, 1.0, 0.01),
                Setting::float("main_z", "Main Z", 0.0, -1.0, 1.0, 0.01),
                Setting::float("main_pitch", "Main pitch", 0.0, -180.0, 180.0, 1.0),
                Setting::float("main_yaw", "Main yaw", 0.0, -180.0, 180.0, 1.0),
                Setting::float("main_roll", "Main roll", 0.0, -180.0, 180.0, 1.0),
                Setting::float("off_scale", "Off scale", 1.0, 0.1, 3.0, 0.05),
                Setting::float("off_x", "Off X", 0.0, -1.0, 1.0, 0.01),
                Setting::float("off_y", "Off Y", 0.0, -1.0, 1.0, 0.01),
                Setting::float("off_z", "Off Z", 0.0, -1.0, 1.0, 0.01),
                Setting::float("off_pitch", "Off pitch", 0.0, -180.0, 180.0, 1.0),
                Setting::float("off_yaw", "Off yaw", 0.0, -180.0, 180.0, 1.0),
                Setting::float("off_roll", "Off roll", 0.0, -180.0, 180.0, 1.0),
                // The bare arm: a hand with nothing in it, which is what the
                // references' Arm group is. Added to the hand's own pose.
                Setting::float("arm_scale", "Arm scale", 1.0, 0.1, 3.0, 0.05),
                Setting::float("arm_x", "Arm X", 0.0, -1.0, 1.0, 0.01),
                Setting::float("arm_y", "Arm Y", 0.0, -1.0, 1.0, 0.01),
                Setting::float("arm_z", "Arm Z", 0.0, -1.0, 1.0, 0.01),
                Setting::float("arm_pitch", "Arm pitch", 0.0, -180.0, 180.0, 1.0),
                Setting::float("arm_yaw", "Arm yaw", 0.0, -180.0, 180.0, 1.0),
                Setting::float("arm_roll", "Arm roll", 0.0, -180.0, 180.0, 1.0),
            ],
        },
        Module {
            id: ANIMATIONS,
            name: "Animations",
            description: "Changes how fast the hand swings, how far it travels, and whether an item in use comes to the mouth.",
            category: Category::Render,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::float("swing_speed", "Swing speed", 0.0, -200.0, 50.0, 5.0),
                Setting::float("sway", "Swing sway", 0.0, 0.0, 200.0, 5.0),
                Setting::bool("eating", "Eating animation", true),
            ],
        },
        Module {
            id: AMBIENCE,
            name: "Ambience",
            description: "Fixes the time of day and the weather, and recolours and pulls in the open air's fog.",
            category: Category::Render,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("time", "Time", 0, &AMBIENCE_TIME_MODES),
                Setting::choice("weather", "Weather", 0, &AMBIENCE_WEATHER_MODES),
                Setting::choice("fog", "Fog", 0, &AMBIENCE_FOG_MODES),
                Setting::text_value("fog_color", "Fog color", "#6E7FD8"),
                Setting::float("fog_start", "Fog start", 0.5, 0.1, 1.5, 0.05),
                Setting::float("fog_end", "Fog end", 1.0, 0.1, 1.5, 0.05),
            ],
        },
        Module {
            id: ZOOM,
            name: "Zoom",
            description: "Hold the bind key to zoom in; the mouse wheel changes the zoom while it is held.",
            category: Category::Render,
            enabled: false,
            // Horizon's own Zoom binds Z by default; so does this module.
            bind: Some("key.keyboard.z".to_owned()),
            settings: vec![
                Setting::choice("level", "Starting zoom", 2, &ZOOM_LEVELS),
                Setting::bool("smooth", "Smooth zoom", true),
                Setting::bool("sensitivity", "Slower turning", true),
            ],
        },
        Module {
            id: NOFALL,
            name: "NoFall",
            description: "Prevents fall damage. The modes follow LiquidBounce's NoFall, from packet ground spoofs to MLG and mount landings.",
            category: Category::Movement,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 0, &NOFALL_MODES),
                Setting::bool("not_while_gliding", "NotWhileGliding", false),
                Setting::bool("not_with_mace", "NotWithMace", false),
                Setting::choice(
                    "spoof_ground_distance",
                    "SpoofGround FallDistance",
                    0,
                    &["Smart", "Constant"],
                )
                .shown_when("mode", SPOOF_GROUND),
                Setting::float(
                    "spoof_ground_value",
                    "SpoofGround Value",
                    1.7,
                    0.0,
                    5.0,
                    0.05,
                )
                .shown_when("mode", SPOOF_GROUND),
                Setting::bool("spoof_ground_reset", "SpoofGround ResetFallDistance", true)
                    .shown_when("mode", SPOOF_GROUND),
                Setting::float(
                    "offset_x",
                    "SpoofLanding OffsetX",
                    1337.0,
                    -10000.0,
                    10000.0,
                    1.0,
                )
                .shown_when("mode", SPOOF_LANDING),
                Setting::float(
                    "offset_y",
                    "SpoofLanding OffsetY",
                    0.0,
                    -10000.0,
                    10000.0,
                    1.0,
                )
                .shown_when("mode", SPOOF_LANDING),
                Setting::float(
                    "offset_z",
                    "SpoofLanding OffsetZ",
                    1337.0,
                    -10000.0,
                    10000.0,
                    1.0,
                )
                .shown_when("mode", SPOOF_LANDING),
                Setting::choice(
                    "packet_type",
                    "Packet PacketType",
                    0,
                    &["FULL", "POSITION_AND_ON_GROUND"],
                )
                .shown_when("mode", PACKET),
                Setting::choice(
                    "packet_filter",
                    "Packet Filter",
                    0,
                    &["FallDistance", "Always"],
                )
                .shown_when("mode", PACKET),
                Setting::choice(
                    "packet_distance",
                    "Packet Distance",
                    0,
                    &["Smart", "Constant"],
                )
                .shown_when("mode", PACKET),
                Setting::float("packet_value", "Packet Value", 2.0, 0.0, 5.0, 0.05)
                    .shown_when("mode", PACKET),
                Setting::bool("packet_reset", "Packet ResetFallDistance", true)
                    .shown_when("mode", PACKET),
                Setting::choice(
                    "jump_packet_type",
                    "PacketJump PacketType",
                    0,
                    &["FULL", "POSITION_AND_ON_GROUND"],
                )
                .shown_when("mode", PACKET_JUMP),
                Setting::choice(
                    "jump_distance",
                    "PacketJump FallDistance",
                    0,
                    &["Smart", "Constant"],
                )
                .shown_when("mode", PACKET_JUMP),
                Setting::float("jump_value", "PacketJump Value", 3.0, 0.0, 5.0, 0.05)
                    .shown_when("mode", PACKET_JUMP),
                Setting::choice(
                    "jump_timing",
                    "PacketJump Timing",
                    0,
                    &["Landing", "Falling"],
                )
                .shown_when("mode", PACKET_JUMP),
                Setting::bool("jump_reset", "PacketJump ResetFallDistance", true)
                    .shown_when("mode", PACKET_JUMP),
                Setting::float("mlg_min_fall", "MLG MinFallDistance", 5.0, 2.0, 50.0, 0.5)
                    .shown_when("mode", MLG),
                Setting::float(
                    "mount_min_fall",
                    "Mount MinFallDistance",
                    5.0,
                    2.0,
                    50.0,
                    0.5,
                )
                .shown_when("mode", MOUNT),
                Setting::float(
                    "mount_search_range",
                    "Mount SearchRange",
                    4.5,
                    1.0,
                    8.0,
                    0.5,
                )
                .shown_when("mode", MOUNT),
                Setting::int("mount_retry_delay", "Mount RetryDelay", 2, 0, 20, 1)
                    .shown_when("mode", MOUNT),
                Setting::int("void_level", "VulcanTP VoidLevel", 0, -256, 0, 1)
                    .shown_when("mode", VULCAN_TP),
                Setting::float(
                    "block_distance",
                    "ForceJump BlockDistance",
                    1.0,
                    0.1,
                    5.0,
                    0.1,
                )
                .shown_when("mode", FORCE_JUMP),
                Setting::float(
                    "force_fall_distance",
                    "ForceJump FallDistance",
                    3.35,
                    3.35,
                    10.0,
                    0.05,
                )
                .shown_when("mode", FORCE_JUMP),
                Setting::float("jump_height", "ForceJump JumpHeight", 0.42, 0.1, 0.42, 0.01)
                    .shown_when("mode", FORCE_JUMP),
                Setting::choice(
                    "cancel_distance",
                    "Cancel FallDistance",
                    0,
                    &["Smart", "Constant"],
                )
                .shown_when("mode", CANCEL),
                Setting::float("cancel_value", "Cancel Value", 1.7, 0.0, 5.0, 0.05)
                    .shown_when("mode", CANCEL),
                Setting::bool("cancel_reset", "Cancel ResetFallDistance", true)
                    .shown_when("mode", CANCEL),
                Setting::bool("cancel_setback", "CancelSetbackPacket", false)
                    .shown_when("mode", CANCEL),
                Setting::float(
                    "blink_trigger",
                    "Blink TriggerFallDistance",
                    2.5,
                    0.5,
                    3.0,
                    0.1,
                )
                .shown_when("mode", NOFALL_BLINK),
                Setting::float(
                    "blink_maximum",
                    "Blink MaximumFallDistance",
                    20.0,
                    2.0,
                    50.0,
                    0.5,
                )
                .shown_when("mode", NOFALL_BLINK),
                Setting::bool("over_void", "HypixelPacket OverVoid", false)
                    .shown_when("mode", HYPIXEL_PACKET),
                Setting::float(
                    "motion_flag_speed",
                    "MotionFlag Speed",
                    0.1,
                    -1.0,
                    1.0,
                    0.05,
                )
                .shown_when("mode", MOTION_FLAG),
                Setting::bool("matrix_safe", "Matrix6.6.3 Safe", false)
                    .shown_when("mode", MATRIX_663),
                Setting::float(
                    "phase_offset",
                    "Phase FallDistance Offset",
                    0.0,
                    0.0,
                    5.0,
                    0.05,
                )
                .shown_when("mode", NOFALL_PHASE),
            ],
        },
        Module {
            id: SPEED,
            name: "Speed",
            description: "Moves faster. The modes follow LiquidBounce's Speed module, the FDPClient speed modes, and the Alan, Librecraft, KoksCraft, Spectre and Swift speed implementations.",
            category: Category::Movement,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 0, &SPEED_MODES),
                Setting::float(
                    "custom_horizontal_acceleration",
                    "HorizontalAcceleration",
                    0.0,
                    -0.1,
                    0.2,
                    0.005,
                )
                .shown_when("mode", SPD_CUSTOM),
                Setting::float("custom_jump_off", "HorizontalJumpOff", 0.0, -0.5, 1.0, 0.01)
                    .shown_when("mode", SPD_CUSTOM),
                Setting::int("custom_boost_ticks", "TicksToBoostOff", 0, 0, 20, 1)
                    .shown_when("mode", SPD_CUSTOM),
                Setting::float("custom_jump_height", "JumpHeight", 0.42, 0.0, 3.0, 0.01)
                    .shown_when("mode", SPD_CUSTOM),
                Setting::float("custom_pulldown", "Pulldown", 0.0, 0.0, 1.0, 0.01)
                    .shown_when("mode", SPD_CUSTOM),
                Setting::float(
                    "custom_fall_pulldown",
                    "PullDownDuringFall",
                    0.0,
                    0.0,
                    1.0,
                    0.01,
                )
                .shown_when("mode", SPD_CUSTOM),
                Setting::float("custom_strafe_strength", "Strength", 1.0, 0.1, 1.0, 0.01)
                    .shown_when("mode", SPD_CUSTOM),
                Setting::bool("custom_speed_enabled", "CustomSpeed", false)
                    .shown_when("mode", SPD_CUSTOM),
                Setting::float("custom_speed", "Speed", 1.0, 0.1, 10.0, 0.1)
                    .shown_when("mode", SPD_CUSTOM),
                Setting::float("yp_speed", "Speed", 0.4, 0.1, 1.0, 0.01)
                    .shown_when("mode", SPD_YPORT),
                Setting::float("vanilla_speed", "Speed", 1.0, 0.1, 9.5, 0.1)
                    .shown_when("mode", SPD_VANILLA),
                Setting::bool(
                    "hypixel_horizontal_acceleration",
                    "HorizontalAcceleration",
                    true,
                )
                .shown_when("mode", SPD_HYPIXEL_BHOP),
                Setting::bool(
                    "hypixel_vertical_acceleration",
                    "VerticalAcceleration",
                    true,
                )
                .shown_when("mode", SPD_HYPIXEL_BHOP),
                Setting::bool("hypixel_lowhop_glide", "Glide", false)
                    .shown_when("mode", SPD_HYPIXEL_LOWHOP),
                Setting::float("sentinel_speed", "Speed", 0.5, 0.1, 5.0, 0.05)
                    .shown_when("mode", SPD_SENTINEL),
                Setting::int("sentinel_reboost_ticks", "ReboostTicks", 30, 10, 50, 1)
                    .shown_when("mode", SPD_SENTINEL),
                Setting::float("grim_boost", "BoostSpeed", 0.08, 0.01, 0.08, 0.001)
                    .shown_when("mode", SPD_GRIM_COLLIDE),
                Setting::float("grim_shrink", "ShrinkBox", 0.5, 0.1, 2.0, 0.05)
                    .shown_when("mode", SPD_GRIM_COLLIDE),
                Setting::bool("ncp_pull_down", "PullDown", true).shown_when("mode", SPD_NCP),
                Setting::float(
                    "ncp_motion_multiplier",
                    "MotionMultiplier",
                    1.0,
                    0.01,
                    10.0,
                    0.01,
                )
                .shown_when("mode", SPD_NCP),
                Setting::int("ncp_pull_down_tick", "OnTick", 5, 1, 9, 1)
                    .shown_when("mode", SPD_NCP),
                Setting::bool("ncp_on_hurt", "OnHurt", true).shown_when("mode", SPD_NCP),
                Setting::bool("ncp_boost", "Boost", true).shown_when("mode", SPD_NCP),
                Setting::float(
                    "ncp_initial_boost_multiplier",
                    "InitialBoostMultiplier",
                    1.0,
                    0.01,
                    10.0,
                    0.01,
                )
                .shown_when("mode", SPD_NCP),
                Setting::bool("ncp_damage_boost", "DamageBoost", true).shown_when("mode", SPD_NCP),
                Setting::bool("ncp_low_hop", "LowHop", true).shown_when("mode", SPD_NCP),
                Setting::bool("ncp_air_strafe", "AirStrafe", true).shown_when("mode", SPD_NCP),
                Setting::bool("intave_strafe", "Strafe", true).shown_when("mode", SPD_INTAVE),
                Setting::float("intave_strafe_strength", "Strength", 0.27, 0.01, 0.27, 0.01)
                    .shown_when("mode", SPD_INTAVE),
                Setting::bool("intave_air_boost", "AirBoost", true).shown_when("mode", SPD_INTAVE),
                Setting::bool("blocksmc_round_yaw", "RoundStrafeYaw", false)
                    .shown_when("mode", SPD_BLOCKSMC),
                Setting::bool("blocksmc_hop_full_strafe", "FullStrafe", true)
                    .shown_when("mode", SPD_BLOCKSMC_HOP),
                Setting::bool("blocksmc_hop_low_hop", "LowHop", true)
                    .shown_when("mode", SPD_BLOCKSMC_HOP),
                Setting::bool("blocksmc_hop_damage_boost", "DamageBoost", true)
                    .shown_when("mode", SPD_BLOCKSMC_HOP),
                Setting::bool("blocksmc_hop_damage_low_hop", "DamageLowHop", true)
                    .shown_when("mode", SPD_BLOCKSMC_HOP),
                Setting::bool("blocksmc_hop_safe_y", "SafeY", false)
                    .shown_when("mode", SPD_BLOCKSMC_HOP),
                Setting::bool("matrix_low_hop", "LowHop", false).shown_when("mode", SPD_MATRIX_HOP),
                Setting::float(
                    "matrix_extra_boost",
                    "ExtraGroundBoost",
                    0.2,
                    0.0,
                    1.0,
                    0.01,
                )
                .shown_when("mode", SPD_MATRIX_HOP),
                Setting::float("spectre_jump_motion", "JumpMotion", 0.44, 0.0, 1.0, 0.01)
                    .shown_when("mode", SPD_SPECTRE_HOP),
            ],
        },
        Module {
            id: PHASE,
            name: "Phase",
            description: "Walks through blocks. The modes follow Alan's Phase, FDPClient's Phase, LiquidBounce's phase modes, Shoreline's, ThunderHack's and Zio's Phase and the Watchdog ports.",
            category: Category::Movement,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 0, &PHASE_MODES),
                Setting::int("delay", "Delay", 2, 1, 20, 1).shown_when("mode", PHASE_DELAY),
                Setting::float("push", "Push", 0.15, 0.005, 1.0, 0.005)
                    .shown_when("mode", PHASE_PUSH),
                Setting::float("pitch", "Pitch", 85.0, 0.0, 90.0, 1.0)
                    .shown_when("mode", PHASE_PITCH),
                Setting::int("scan", "Scan", 10, 1, 20, 1).shown_when("mode", PHASE_SCAN),
            ],
        },
        Module {
            id: STEP,
            name: "Step",
            description: "Steps up blocks. The modes follow Alan's Step, the FDPClient step modes, LiquidBounce's Step and the Celestial step ports.",
            category: Category::Movement,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 0, &STEP_MODES),
                Setting::float("height", "Height", 1.0, 0.6, 10.0, 0.1)
                    .shown_when("mode", STEP_HEIGHT),
                Setting::float("jump_height", "Jump Height", 0.42, 0.37, 0.42, 0.01)
                    .shown_when("mode", STEP_JUMPS),
                Setting::int("delay", "Delay", 0, 0, 500, 10).shown_when("mode", STEP_DELAY),
                Setting::int("extra_delay", "Extra Delay", 0, 0, 1000, 10)
                    .shown_when("mode", STEP_DELAY),
                Setting::bool("simulate_jump", "Simulate Jump", false)
                    .shown_when("mode", STEP_CONFIRM),
                Setting::float("timer", "Timer", 1.0, 0.05, 1.0, 0.05)
                    .shown_when("mode", STEP_TIMER),
                Setting::bool("dynamic_timer", "UseDynamicTimer", false)
                    .shown_when("mode", STEP_TIMER),
                Setting::bool("matrix_two_block", "2 Block", true).shown_when("mode", STEP_MATRIX),
                Setting::bool("matrix_instant", "Instant", true).shown_when("mode", STEP_MATRIX),
                Setting::bool("instant_trim", "Trim", false).shown_when("mode", STEP_INSTANT),
                Setting::int("instant_order_min", "SimulateJumpOrder Min", 0, 0, 8, 1)
                    .shown_when("mode", STEP_INSTANT),
                Setting::int("instant_order_max", "SimulateJumpOrder Max", 2, 0, 8, 1)
                    .shown_when("mode", STEP_INSTANT),
                Setting::int("instant_wait_min", "Wait Min", 0, 0, 60, 1)
                    .shown_when("mode", STEP_INSTANT),
                Setting::int("instant_wait_max", "Wait Max", 0, 0, 60, 1)
                    .shown_when("mode", STEP_INSTANT),
                Setting::choice(
                    "instant_packet",
                    "PacketType",
                    1,
                    &["PositionAndOnGround", "Full"],
                )
                .shown_when("mode", STEP_INSTANT),
                Setting::float("blocksmc_base_timer", "BaseTimer", 3.0, 0.1, 5.0, 0.05)
                    .shown_when("mode", STEP_BLOCKSMC),
                Setting::float(
                    "blocksmc_recovery_timer",
                    "RecoveryTimer",
                    0.6,
                    0.1,
                    5.0,
                    0.05,
                )
                .shown_when("mode", STEP_BLOCKSMC),
                Setting::bool("hypixel_alternate", "AlternateBypass", false)
                    .shown_when("mode", STEP_HYPIXEL),
                Setting::bool("hypixel_spoof", "Spoof", false).shown_when("mode", STEP_HYPIXEL),
            ],
        },
        Module {
            id: FLY,
            name: "Fly",
            description: "Flies. The modes follow Alan's Flight, the FDPClient and LiquidBounce fly modes, and the Sigma, Flux, Celestial, Tenacity, Siekera, Prisma and White Floor flight ports.",
            category: Category::Movement,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 0, &FLY_MODES),
                Setting::float("vanilla_speed", "Speed", 0.44, 0.05, 10.0, 0.01)
                    .shown_when("mode", FLY_VANILLA),
                Setting::float("vanilla_vertical", "VerticalSpeed", 0.44, 0.0, 10.0, 0.01)
                    .shown_when("mode", FLY_VANILLA),
                Setting::float("vanilla_glide", "Glide", 0.0, -1.0, 1.0, 0.01)
                    .shown_when("mode", FLY_VANILLA),
                Setting::bool("vanilla_bypass", "BypassVanillaCheck", true)
                    .shown_when("mode", FLY_VANILLA),
                Setting::bool("anti_kick", "AntiKick", false).shown_when("mode", FLY_VANILLA),
                Setting::float("airwalk_speed", "Speed", 0.35, 0.05, 10.0, 0.01)
                    .shown_when("mode", FLY_AIRWALK),
                Setting::float("airwalk_vertical", "VerticalSpeed", 0.42, 0.0, 10.0, 0.01)
                    .shown_when("mode", FLY_AIRWALK),
                Setting::int("airwalk_ticks", "Ticks", 11, 1, 20, 1)
                    .shown_when("mode", FLY_AIRWALK),
                Setting::float("airwalk_timer", "Timer", 1.0, 0.1, 15.0, 0.05)
                    .shown_when("mode", FLY_AIRWALK),
                Setting::float("grim_speed", "Speed", 0.28, 0.05, 1.0, 0.01)
                    .shown_when("mode", FLY_GRIM),
                Setting::float("grim_vertical", "VerticalSpeed", 0.18, 0.0, 1.0, 0.01)
                    .shown_when("mode", FLY_GRIM),
                Setting::float("grim_sink", "Glide", 0.0, 0.0, 0.3, 0.01)
                    .shown_when("mode", FLY_GRIM),
                Setting::float("grim_hurt_speed", "HurtSpeed", 0.4, 0.05, 1.0, 0.01)
                    .shown_when("mode", FLY_GRIM),
                Setting::float("grim_old_speed", "Speed", 0.275, 0.0, 0.32, 0.001)
                    .shown_when("mode", FLY_GRIM_OLD),
                Setting::float("grim_old_timer", "Timer", 1.0, 0.1, 10.0, 0.1)
                    .shown_when("mode", FLY_GRIM_OLD),
                Setting::float("grim_float_vertical", "VerticalSpeed", 0.2, 0.05, 1.0, 0.01)
                    .shown_when("mode", FLY_GRIM_FLOAT),
                Setting::float("grim_float_speed", "Speed", 0.1, 0.0, 1.0, 0.01)
                    .shown_when("mode", FLY_GRIM_FLOAT),
                Setting::float("grim_entity_speed", "HorizontalSpeed", 0.6, 0.1, 1.1, 0.05)
                    .shown_when("mode", FLY_GRIM_ENTITY),
                Setting::float(
                    "grim_entity_vertical",
                    "VerticalSpeed",
                    0.3,
                    0.05,
                    1.1,
                    0.05,
                )
                .shown_when("mode", FLY_GRIM_ENTITY),
                Setting::float("grim_entity_distance", "Distance", 1.0, 0.5, 1.5, 0.05)
                    .shown_when("mode", FLY_GRIM_ENTITY),
                Setting::bool("ncp_clip", "Clip", false).shown_when("mode", FLY_NCP),
                Setting::float("ncp_speed", "Speed", 9.0, 0.1, 10.0, 0.1)
                    .shown_when("mode", FLY_NCP),
                Setting::float("ncp_timer", "Timer", 0.4, 0.1, 1.0, 0.05)
                    .shown_when("mode", FLY_NCP),
                Setting::float("old_ncp_speed", "Speed", 0.2, 0.05, 1.0, 0.01)
                    .shown_when("mode", FLY_OLD_NCP),
                Setting::float("ncp_burst_vertical", "VerticalSpeed", 0.5, 0.05, 1.0, 0.01)
                    .shown_when("mode", FLY_NCP_BURST),
                Setting::float("ncp_burst_speed", "Speed", 0.2, 0.05, 2.0, 0.01)
                    .shown_when("mode", FLY_NCP_BURST),
                Setting::float("hypixel_speed", "Speed", 1.9, 0.1, 5.0, 0.05)
                    .shown_when("mode", FLY_HYPIXEL),
                Setting::float("hypixel_vertical", "VerticalSpeed", 0.8, 0.0, 2.0, 0.05)
                    .shown_when("mode", FLY_HYPIXEL),
                Setting::float("hypixel_timer", "Timer", 1.0, 0.1, 1.0, 0.05)
                    .shown_when("mode", FLY_HYPIXEL),
                Setting::bool("hypixel_flat", "Flat", false).shown_when("mode", FLY_HYPIXEL),
                Setting::float("hypixel_boost_speed", "Speed", 0.29, 0.1, 1.0, 0.01)
                    .shown_when("mode", FLY_HYPIXEL_BOOST),
                Setting::float("hypixel_boost_timer", "Timer", 1.0, 0.1, 5.0, 0.05)
                    .shown_when("mode", FLY_HYPIXEL_BOOST),
                Setting::float("vulcan_down", "DownSpeed", 0.155, 0.0, 1.0, 0.001)
                    .shown_when("mode", FLY_VULCAN),
                Setting::float("vulcan_up", "UpSpeed", 0.1, 0.0, 1.0, 0.001)
                    .shown_when("mode", FLY_VULCAN),
                Setting::float("vulcan_speed", "Strafe", 0.2, 0.05, 1.0, 0.01)
                    .shown_when("mode", FLY_VULCAN),
                Setting::float("verus_ground_speed", "GroundSpeed", 1.01, 0.1, 5.0, 0.01)
                    .shown_when("mode", FLY_VERUS),
                Setting::float("verus_air_speed", "AirSpeed", 0.41, 0.1, 5.0, 0.01)
                    .shown_when("mode", FLY_VERUS),
                Setting::float("verus_damage_speed", "Speed", 1.0, 0.1, 9.5, 0.1)
                    .shown_when("mode", FLY_VERUS_DAMAGE),
                Setting::float(
                    "verus_damage_vertical",
                    "VerticalSpeed",
                    1.0,
                    0.0,
                    3.0,
                    0.05,
                )
                .shown_when("mode", FLY_VERUS_DAMAGE),
                Setting::float("verus_damage_cancel", "CancelDistance", 8.5, 0.0, 20.0, 0.5)
                    .shown_when("mode", FLY_VERUS_DAMAGE),
                Setting::int("verus_damage_ticks", "FlyTicks", 20, 1, 100, 1)
                    .shown_when("mode", FLY_VERUS_DAMAGE),
                Setting::float("spartan_speed", "Speed", 0.28, 0.05, 2.0, 0.01)
                    .shown_when("mode", FLY_SPARTAN),
                Setting::int("spartan_interval", "Ticks", 12, 1, 40, 1)
                    .shown_when("mode", FLY_SPARTAN),
                Setting::float("spartan_offset", "Offset", 8.0, 1.0, 20.0, 0.5)
                    .shown_when("mode", FLY_SPARTAN),
                Setting::int("aac_glide_ticks", "Ticks", 12, 1, 40, 1)
                    .shown_when("mode", FLY_AAC_GLIDE),
                Setting::float("aac_glide_lift", "Lift", 0.015, 0.0, 0.2, 0.001)
                    .shown_when("mode", FLY_AAC_GLIDE),
                Setting::float("aac_glide_timer", "Timer", 0.1, 0.05, 1.0, 0.05)
                    .shown_when("mode", FLY_AAC_GLIDE),
                Setting::float("matrix_speed", "Speed", 9.3, 0.1, 10.0, 0.1)
                    .shown_when("mode", FLY_MATRIX),
                Setting::float("matrix_vertical", "VerticalSpeed", 0.42, 0.0, 1.0, 0.01)
                    .shown_when("mode", FLY_MATRIX),
                Setting::float("damage_speed", "Speed", 1.0, 0.1, 9.5, 0.1)
                    .shown_when("mode", FLY_DAMAGE),
                Setting::float("damage_vertical", "VerticalSpeed", 1.0, 0.0, 3.0, 0.05)
                    .shown_when("mode", FLY_DAMAGE),
                Setting::int("damage_ticks", "FlyTicks", 14, 1, 100, 1)
                    .shown_when("mode", FLY_DAMAGE),
                Setting::float("damage_timer", "Timer", 4.0, 0.1, 15.0, 0.1)
                    .shown_when("mode", FLY_DAMAGE),
                Setting::choice("damage_trigger", "Trigger", 0, &["Hurt", "Velocity"])
                    .shown_when("mode", FLY_DAMAGE),
                Setting::float("sentinel_speed", "HorizontalSpeed", 3.5, 0.1, 10.0, 0.1)
                    .shown_when("mode", FLY_SENTINEL),
                Setting::float("sentinel_vertical", "VerticalSpeed", 0.7, 0.05, 5.0, 0.05)
                    .shown_when("mode", FLY_SENTINEL),
                Setting::int("sentinel_ticks", "Ticks", 20, 10, 50, 1)
                    .shown_when("mode", FLY_SENTINEL),
                Setting::float("sentinel_timer", "Timer", 0.5, 0.1, 5.0, 0.05)
                    .shown_when("mode", FLY_SENTINEL),
                Setting::int("sentinel_reboost", "ReboostTicks", 0, 0, 200, 1)
                    .shown_when("mode", FLY_SENTINEL),
                Setting::float("jetpack_impulse", "Impulse", 0.15, 0.01, 1.0, 0.01)
                    .shown_when("mode", FLY_JETPACK),
                Setting::float("jetpack_speed", "Speed", 1.1, 0.1, 2.0, 0.01)
                    .shown_when("mode", FLY_JETPACK),
                Setting::float("explosion_vertical", "Vertical", 4.0, 0.0, 10.0, 0.1)
                    .shown_when("mode", FLY_EXPLOSION),
                Setting::float("explosion_speed", "StartStrafe", 1.0, 0.6, 4.0, 0.05)
                    .shown_when("mode", FLY_EXPLOSION),
                Setting::float(
                    "explosion_decrease",
                    "StrafeDecrease",
                    0.005,
                    0.001,
                    0.1,
                    0.001,
                )
                .shown_when("mode", FLY_EXPLOSION),
                Setting::float("buffer_speed", "Speed", 1.0, 0.1, 9.5, 0.1)
                    .shown_when("mode", FLY_BUFFER),
                Setting::bool("buffer_send_flying", "SendFlying", false)
                    .shown_when("mode", FLY_BUFFER),
                Setting::bool("highjump_self_damage", "Self Damage", true)
                    .shown_when("mode", FLY_MATRIX_HIGH),
                Setting::float("highjump_height", "Height", 1.0, 0.42, 7.0, 0.1)
                    .shown_when("mode", FLY_MATRIX_HIGH),
            ],
        },
        Module {
            id: TIMER,
            name: "Timer",
            description: "Changes how fast the game runs, optionally alternating a boost speed with the normal multiplier.",
            category: Category::Misc,
            enabled: false,
            bind: None,
            settings: vec![
                Setting::choice("mode", "Mode", 0, &TIMER_MODES),
                Setting::float("timer_multiplier", "Multiplier", 1.0, 0.1, 15.0, 0.05),
                Setting::int("boost_ticks", "Boost Ticks", 0, 0, 200, 1),
                Setting::float("boost_speed", "Boost Speed", 2.0, 0.1, 15.0, 0.05),
                Setting::int("normal_ticks", "Normal Ticks", 20, 1, 200, 1),
            ],
        },
    ]
}

/// FullBright's settings as the tick wants them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FullBrightConfig {
    /// 0 Gamma, 1 NightVision.
    pub mode: u8,
    pub brightness: i32,
    pub smooth: bool,
    pub speed: f32,
    pub amplifier: i32,
}

/// The parts of FullBright that outlive a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FullBrightState {
    /// The renderer's gamma before the mod changed it.
    pub saved_gamma: Option<f32>,
    /// The gamma the smooth ramp is at now.
    pub current_gamma: f32,
    /// Whether this session took the renderer's gamma over.
    pub applied: bool,
    /// The mode applied last, 0 Gamma and 1 NightVision.
    pub mode: u8,
    /// Whether our NightVision effect is on the player.
    pub night_vision: bool,
}

/// Zoom's held state: the factor while the bind key is down, none when it is
/// up. The settings live in the module, so only the in-between wheel changes
/// are kept here.
#[derive(Clone, Copy, Debug, Default)]
pub struct ZoomState {
    pub held: Option<f32>,
}

/// The ArrayList's settings as the HUD wants them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrayListConfig {
    pub background: bool,
    pub shadow: bool,
    pub sorted: bool,
}

/// AutoTool's settings as its tick wants them. The block lists are kept as
/// written; `parse_blocks` tidies them.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoToolConfig {
    /// 0 Dynamic, 1 Static.
    pub mode: u8,
    pub ignore_durability: bool,
    pub consider_inventory: bool,
    /// The hotbar slot Static mode always selects (0..8).
    pub static_slot: usize,
    /// 0 Blacklist, 1 Whitelist.
    pub filter: u8,
    pub blocks: Vec<String>,
    pub silk_touch: bool,
    /// 0 Whitelist, 1 Blacklist.
    pub silk_touch_filter: u8,
    pub silk_touch_blocks: Vec<String>,
    pub swap_back_delay: i32,
    pub switch_delay: i32,
    pub require_sneaking: bool,
    pub not_during_combat: bool,
    pub require_near_bed: bool,
    pub bed_distance: f32,
}

/// A weapon type AutoWeapon's Preferred list can name (LiquidBounce's
/// `WeaponType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponKind {
    Any,
    Sword,
    Axe,
    Mace,
    Spear,
    Pickaxe,
    Shovel,
    Hoe,
    Knockback,
    FireAspect,
}

impl WeaponKind {
    /// The setting name other code (or a future multi-select) shows.
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            WeaponKind::Any => "Any",
            WeaponKind::Sword => "Sword",
            WeaponKind::Axe => "Axe",
            WeaponKind::Mace => "Mace",
            WeaponKind::Spear => "Spear",
            WeaponKind::Pickaxe => "Pickaxe",
            WeaponKind::Shovel => "Shovel",
            WeaponKind::Hoe => "Hoe",
            WeaponKind::Knockback => "Knockback",
            WeaponKind::FireAspect => "FireAspect",
        }
    }

    /// The type named `label`, case-insensitively.
    pub fn from_label(label: &str) -> Option<Self> {
        match label.to_ascii_lowercase().as_str() {
            "any" => Some(WeaponKind::Any),
            "sword" => Some(WeaponKind::Sword),
            "axe" => Some(WeaponKind::Axe),
            "mace" => Some(WeaponKind::Mace),
            "spear" => Some(WeaponKind::Spear),
            "pickaxe" => Some(WeaponKind::Pickaxe),
            "shovel" => Some(WeaponKind::Shovel),
            "hoe" => Some(WeaponKind::Hoe),
            "knockback" => Some(WeaponKind::Knockback),
            "fireaspect" | "fire_aspect" | "fire aspect" => Some(WeaponKind::FireAspect),
            _ => None,
        }
    }
}

/// Splits Preferred (comma, semicolon or newline separated) into weapon
/// types, dropping blanks, unknown names and duplicates. An empty list
/// prefers no type at all, as LiquidBounce's empty choice does.
pub fn parse_weapons(text: &str) -> Vec<WeaponKind> {
    let mut kinds = Vec::new();
    for label in text.split([',', ';', '\n']) {
        let Some(kind) = WeaponKind::from_label(label.trim()) else {
            continue;
        };
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    kinds
}

/// AutoWeapon's settings as its tick wants them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoWeaponConfig {
    /// The weapon types to choose from; empty prefers none.
    pub preferred: Vec<WeaponKind>,
    /// 0 Default, 1 Knockback, 2 Damage, 3 AttackSpeed.
    pub priority: u8,
    pub auto_shield_break: bool,
    pub auto_mace: bool,
    /// Ticks without a new selection before the held slot goes back.
    pub switch_back: i32,
    /// 0 OnAttack, 1 OnTarget, 2 Both.
    pub change_on: u8,
}

impl AutoWeaponConfig {
    /// Whether an attack triggers a switch (OnAttack or Both).
    pub fn on_attack(&self) -> bool {
        self.change_on != 1
    }

    /// Whether aiming at an entity triggers a switch (OnTarget or Both).
    pub fn on_target(&self) -> bool {
        self.change_on >= 1
    }
}

/// The Watermark's settings as the HUD wants them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WatermarkConfig {
    /// 0 Top Left, 1 Top Right, 2 Bottom Left, 3 Bottom Right.
    pub corner: usize,
    /// The distance from the corner's edges, in GUI pixels.
    pub offset_x: f32,
    pub offset_y: f32,
    pub shadow: bool,
}

/// NameTags' settings as the HUD wants them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NameTagsConfig {
    /// Tag other players.
    pub players: bool,
    /// Tag dropped items.
    pub items: bool,
    /// Draw the player's face beside the name.
    pub player_head: bool,
    /// Draw equipped items above the name, and a dropped item's icon.
    pub item_icons: bool,
}

/// A NoFall mode, in `NOFALL_MODES` order (LiquidBounce's list).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoFallMode {
    SpoofGround,
    SpoofLanding,
    NoGround,
    Packet,
    PacketJump,
    Mlg,
    Mount,
    Rettungsplatform,
    Spartan524Flag,
    Vulcan277,
    VulcanTp288,
    Verus,
    ForceJump,
    Cancel,
    Blink,
    HypixelPacket,
    Hypixel,
    BlocksMc,
    Grim2371,
    AlwaysSpoof,
    Damage,
    Vanilla,
    Packet1,
    Packet2,
    MotionFlag,
    CubeCraft,
    Intave,
    Polar,
    Sentinel,
    LatestVulcan,
    MatrixCollide,
    MatrixNew,
    MatrixNoFall,
    Matrix62x,
    Matrix62xPacket,
    Matrix663,
    Aac,
    Aac3311,
    Aac3315,
    Aac44xFlag,
    Aac504,
    Aac5014,
    Laac,
    VulcanOld,
    VulcanFast288,
    PacketLegacy,
    RoundNoFall,
    NoFallDamage,
    WatchdogPacket,
    WatchdogPrediction,
    HypixelTimer,
    HypixelFlag,
    Heypixel,
    ChunkLoad,
    Phase,
    Grim72,
    Grim73,
}

impl NoFallMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => NoFallMode::SpoofLanding,
            2 => NoFallMode::NoGround,
            3 => NoFallMode::Packet,
            4 => NoFallMode::PacketJump,
            5 => NoFallMode::Mlg,
            6 => NoFallMode::Mount,
            7 => NoFallMode::Rettungsplatform,
            8 => NoFallMode::Spartan524Flag,
            9 => NoFallMode::Vulcan277,
            10 => NoFallMode::VulcanTp288,
            11 => NoFallMode::Verus,
            12 => NoFallMode::ForceJump,
            13 => NoFallMode::Cancel,
            14 => NoFallMode::Blink,
            15 => NoFallMode::HypixelPacket,
            16 => NoFallMode::Hypixel,
            17 => NoFallMode::BlocksMc,
            18 => NoFallMode::Grim2371,
            19 => NoFallMode::AlwaysSpoof,
            20 => NoFallMode::Damage,
            21 => NoFallMode::Vanilla,
            22 => NoFallMode::Packet1,
            23 => NoFallMode::Packet2,
            24 => NoFallMode::MotionFlag,
            25 => NoFallMode::CubeCraft,
            26 => NoFallMode::Intave,
            27 => NoFallMode::Polar,
            28 => NoFallMode::Sentinel,
            29 => NoFallMode::LatestVulcan,
            30 => NoFallMode::MatrixCollide,
            31 => NoFallMode::MatrixNew,
            32 => NoFallMode::MatrixNoFall,
            33 => NoFallMode::Matrix62x,
            34 => NoFallMode::Matrix62xPacket,
            35 => NoFallMode::Matrix663,
            36 => NoFallMode::Aac,
            37 => NoFallMode::Aac3311,
            38 => NoFallMode::Aac3315,
            39 => NoFallMode::Aac44xFlag,
            40 => NoFallMode::Aac504,
            41 => NoFallMode::Aac5014,
            42 => NoFallMode::Laac,
            43 => NoFallMode::VulcanOld,
            44 => NoFallMode::VulcanFast288,
            45 => NoFallMode::PacketLegacy,
            46 => NoFallMode::RoundNoFall,
            47 => NoFallMode::NoFallDamage,
            48 => NoFallMode::WatchdogPacket,
            49 => NoFallMode::WatchdogPrediction,
            50 => NoFallMode::HypixelTimer,
            51 => NoFallMode::HypixelFlag,
            52 => NoFallMode::Heypixel,
            53 => NoFallMode::ChunkLoad,
            54 => NoFallMode::Phase,
            55 => NoFallMode::Grim72,
            56 => NoFallMode::Grim73,
            _ => NoFallMode::SpoofGround,
        }
    }
}

/// NoFall's settings as its packets want them. The mode-specific settings sit
/// together; only the chosen mode reads its own.
#[derive(Clone, Debug, PartialEq)]
pub struct NoFallConfig {
    pub mode: NoFallMode,
    pub not_while_gliding: bool,
    pub not_with_mace: bool,
    /// SpoofGround: 0 Smart, 1 Constant.
    pub spoof_ground_distance: u8,
    pub spoof_ground_value: f32,
    pub spoof_ground_reset: bool,
    /// SpoofLanding's modification, added to the landing packet.
    pub offset: [f64; 3],
    /// Packet: 0 FULL, 1 POSITION_AND_ON_GROUND (both are position updates here).
    pub packet_type: u8,
    /// Packet: 0 FallDistance, 1 Always.
    pub packet_filter: u8,
    /// Packet: 0 Smart, 1 Constant.
    pub packet_distance: u8,
    pub packet_value: f32,
    pub packet_reset: bool,
    /// PacketJump: 0 FULL, 1 POSITION_AND_ON_GROUND.
    pub jump_packet_type: u8,
    /// PacketJump: 0 Smart, 1 Constant.
    pub jump_distance: u8,
    pub jump_value: f32,
    /// PacketJump: 0 Landing, 1 Falling.
    pub jump_timing: u8,
    pub jump_reset: bool,
    pub mlg_min_fall: f32,
    pub mount_min_fall: f32,
    pub mount_search_range: f32,
    pub mount_retry_delay: i32,
    pub void_level: i32,
    pub block_distance: f32,
    pub force_fall_distance: f32,
    pub jump_height: f32,
    /// Cancel: 0 Smart, 1 Constant.
    pub cancel_distance: u8,
    pub cancel_value: f32,
    pub cancel_reset: bool,
    pub cancel_setback: bool,
    pub blink_trigger: f32,
    pub blink_maximum: f32,
    pub over_void: bool,
    /// MotionFlag: the downward speed it sets while falling.
    pub motion_flag_speed: f32,
    /// Matrix6.6.3: slower the timer and bleed horizontal speed instead of
    /// only zeroing the fall.
    pub matrix_safe: bool,
    /// Phase: added to the fall distance before the collision is looked up.
    pub phase_offset: f32,
}

impl NoFallConfig {
    /// The fall distance SpoofGround, Packet, Cancel and PacketJump compare
    /// against when their mode is Smart: the player's safe fall distance.
    pub fn smart_distance(safe_fall_distance: f64, mode: u8, value: f32) -> f64 {
        if mode == 1 {
            f64::from(value)
        } else {
            safe_fall_distance
        }
    }
}

/// A Criticals mode, in `CRITICALS_MODES` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CriticalsMode {
    Off,
    Packet,
    NoGround,
    Jump,
    Blink,
    Timer,
    NcpPacket,
    BlocksMc,
    BlocksMc2,
    Hop,
    MiniJump,
    TpHop,
    LowJump,
    CustomMotion,
    Visual,
    Edit,
    PacketAttack,
    Verus,
    Vulcan,
    Watchdog,
    Mospixel,
    Ncp,
    Summer,
    TenacityWatchdog,
    TenacityPacket,
    TenacityDev,
    TenacityVerus,
    FluxPacket,
    FluxJump,
    FluxHypixel,
    SigmaPacket,
    SigmaHPacket,
    SigmaHover,
    SigmaJump,
    SigmaMinis,
    DeadcodePacket,
    DeadcodeJump,
    DeadcodeFall,
    DeadcodeMatrixDisabler,
    DeadcodeMatrix,
    CelestialPacket,
    CelestialNcp,
    CelestialNcpNew,
    CelestialSunriseGround,
    CelestialSunriseAir,
    CelestialSunriseDrop,
    CelestialOldSunrise,
    CelestialOldMatrix,
    CelestialMatrix630,
    Novoline,
}

impl CriticalsMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => CriticalsMode::Packet,
            2 => CriticalsMode::NoGround,
            3 => CriticalsMode::Jump,
            4 => CriticalsMode::Blink,
            5 => CriticalsMode::Timer,
            6 => CriticalsMode::NcpPacket,
            7 => CriticalsMode::BlocksMc,
            8 => CriticalsMode::BlocksMc2,
            9 => CriticalsMode::Hop,
            10 => CriticalsMode::MiniJump,
            11 => CriticalsMode::TpHop,
            12 => CriticalsMode::LowJump,
            13 => CriticalsMode::CustomMotion,
            14 => CriticalsMode::Visual,
            15 => CriticalsMode::Edit,
            16 => CriticalsMode::PacketAttack,
            17 => CriticalsMode::Verus,
            18 => CriticalsMode::Vulcan,
            19 => CriticalsMode::Watchdog,
            20 => CriticalsMode::Mospixel,
            21 => CriticalsMode::Ncp,
            22 => CriticalsMode::Summer,
            23 => CriticalsMode::TenacityWatchdog,
            24 => CriticalsMode::TenacityPacket,
            25 => CriticalsMode::TenacityDev,
            26 => CriticalsMode::TenacityVerus,
            27 => CriticalsMode::FluxPacket,
            28 => CriticalsMode::FluxJump,
            29 => CriticalsMode::FluxHypixel,
            30 => CriticalsMode::SigmaPacket,
            31 => CriticalsMode::SigmaHPacket,
            32 => CriticalsMode::SigmaHover,
            33 => CriticalsMode::SigmaJump,
            34 => CriticalsMode::SigmaMinis,
            35 => CriticalsMode::DeadcodePacket,
            36 => CriticalsMode::DeadcodeJump,
            37 => CriticalsMode::DeadcodeFall,
            38 => CriticalsMode::DeadcodeMatrixDisabler,
            39 => CriticalsMode::DeadcodeMatrix,
            40 => CriticalsMode::CelestialPacket,
            41 => CriticalsMode::CelestialNcp,
            42 => CriticalsMode::CelestialNcpNew,
            43 => CriticalsMode::CelestialSunriseGround,
            44 => CriticalsMode::CelestialSunriseAir,
            45 => CriticalsMode::CelestialSunriseDrop,
            46 => CriticalsMode::CelestialOldSunrise,
            47 => CriticalsMode::CelestialOldMatrix,
            48 => CriticalsMode::CelestialMatrix630,
            49 => CriticalsMode::Novoline,
            _ => CriticalsMode::Off,
        }
    }
}

/// Criticals' settings as its packets want them.
#[derive(Clone, Debug, PartialEq)]
pub struct CriticalsConfig {
    pub mode: CriticalsMode,
    /// 0 None, 1 Legit, 2 OnNetwork, 3 OnAttack.
    pub sprint_stop: u8,
    pub sprint_range: f32,
    pub only_sprinting: bool,
    /// 0 Vanilla, 1 NoCheatPlus, 2 Falling, 3 Low, 4 Down, 5 Grim, 6 BlocksMC.
    pub packet_mode: u8,
    /// 0 FULL, 1 POSITION_AND_ON_GROUND.
    pub packet_type: u8,
    /// The FDP-style delay between criticals, in milliseconds.
    pub delay: i32,
    /// The target hurt time the FDP-style modes wait for.
    pub hurt_time: i32,
    pub jump_height: f32,
    pub jump_range: f32,
    pub jump_optimize: bool,
    pub jump_can_be_seen: bool,
    pub blink_delay_min: i32,
    pub blink_delay_max: i32,
    pub blink_range: f32,
    pub timer_speed: f32,
    pub timer_range: f32,
    pub custom_motion_y: f32,
    pub edit_delay: i32,
    pub alan_delay: i32,
    pub watchdog_delay: i32,
    pub ncp_ticks: i32,
    pub summer_delay: i32,
    pub summer_ground_ticks: i32,
    /// Tenacity Watchdog: 0 Packet, 1 Edit.
    pub tenacity_watchdog_mode: u8,
    pub tenacity_delay: i32,
    pub flux_hurt_time: i32,
    pub flux_delay: i32,
    pub flux_post_motion: bool,
    pub sigma_hurt_time: i32,
    pub deadcode_fall_distance: f32,
    pub deadcode_delay: i32,
    pub deadcode_auto_jump: bool,
    pub deadcode_auto_jump_distance: f32,
    pub deadcode_smart: bool,
    pub celestial_mini_jump: bool,
    pub celestial_ncp_delay: i32,
    pub celestial_ground_false: bool,
    pub celestial_reduce_damage: bool,
    pub novoline_hypixel: bool,
    pub novoline_random: f32,
    pub visuals: bool,
    pub visual_fake: bool,
    pub visual_critical: i32,
    pub visual_magic: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drag {
    Window {
        grab: (f32, f32),
    },
    /// Resizing from the bottom-right grip. `offset` is the distance from the
    /// mouse to the window's bottom-right corner when the drag started, so the
    /// corner keeps following the cursor without jumping.
    Resize {
        offset: (f32, f32),
    },
    Slider(u64),
}

/// A text field that takes typed keys. Settings and the Config tab's name
/// field share the same input path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Editing {
    Setting {
        module: &'static str,
        setting: &'static str,
    },
    ConfigName,
}

/// A Config tab button's work, run after the draw because the file helpers
/// borrow the client's registry.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigAction {
    Make(String),
    Save(String),
    Load(String),
    OpenFolder,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuiState {
    pub open: bool,
    pub category: usize,
    pub selected: Option<&'static str>,
    pub pos: Option<(f32, f32)>,
    /// Width and height in GUI pixels (see `DEFAULT_W`/`MIN_W`).
    pub size: (f32, f32),
    pub drag: Option<Drag>,
    /// The module whose bind row is waiting for the next key, if any.
    pub listening: Option<&'static str>,
    /// The text field keys are typed into, if any.
    pub editing: Option<Editing>,
    pub list_scroll: usize,
    pub settings_scroll: usize,
    /// The saved configs shown in the Config tab, the selected one and the
    /// name the buttons use.
    pub configs: Vec<String>,
    pub config_selected: Option<String>,
    pub config_name: String,
    /// The last Config tab result, and whether it was an error.
    pub config_message: String,
    pub config_error: bool,
    pub configs_dirty: bool,
    /// The button work waiting for the draw's registry borrow to end.
    pub config_pending: Option<ConfigAction>,
}

impl Default for GuiState {
    fn default() -> Self {
        Self {
            open: false,
            category: 0,
            selected: Some(FULLBRIGHT),
            pos: None,
            size: (DEFAULT_W, DEFAULT_H),
            drag: None,
            listening: None,
            editing: None,
            list_scroll: 0,
            settings_scroll: 0,
            configs: Vec::new(),
            config_selected: None,
            config_name: "default".to_owned(),
            config_message: String::new(),
            config_error: false,
            configs_dirty: true,
            config_pending: None,
        }
    }
}

impl GuiState {
    /// Keeps the size inside the usable range and returns the result.
    pub fn clamped_size(&self) -> (f32, f32) {
        (
            self.size.0.clamp(MIN_W, 8192.0),
            self.size.1.clamp(MIN_H, 8192.0),
        )
    }
}

/// Mouse state between a window event and the next draw.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub down: bool,
    pub pressed: bool,
    pub scroll: f32,
}

impl Input {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub struct Client {
    pub modules: Vec<Module>,
    pub gui: GuiState,
    pub input: Input,
    pub loaded: bool,
    pub fullbright: FullBrightState,
    pub zoom: ZoomState,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    pub fn new() -> Self {
        Self {
            modules: default_modules(),
            gui: GuiState::default(),
            input: Input::default(),
            loaded: false,
            fullbright: FullBrightState::default(),
            zoom: ZoomState::default(),
        }
    }
}

thread_local! {
    static CLIENT: RefCell<Client> = RefCell::new(Client::new());
}

/// Borrows the GUI state. Keep the closure short: the borrow is exclusive.
pub fn with<R>(f: impl FnOnce(&mut Client) -> R) -> R {
    CLIENT.with(|client| f(&mut client.borrow_mut()))
}

pub fn is_enabled(module: &str) -> bool {
    with(|c| {
        c.modules
            .iter()
            .find(|m| m.id == module)
            .is_some_and(|m| m.enabled)
    })
}

pub fn is_open() -> bool {
    with(|c| c.gui.open)
}

/// A module's display name, or None if it is not registered.
pub fn module_name(id: &str) -> Option<&'static str> {
    with(|c| {
        c.modules
            .iter()
            .find(|module| module.id == id)
            .map(|module| module.name)
    })
}

/// Finds a module by id or display name, case-insensitively (for `.bind`).
pub fn module_id(query: &str) -> Option<&'static str> {
    let query = query.trim();
    with(|c| {
        c.modules
            .iter()
            .find(|module| {
                module.id.eq_ignore_ascii_case(query) || module.name.eq_ignore_ascii_case(query)
            })
            .map(|module| module.id)
    })
}

/// Every module bound to a vanilla key name (`key.keyboard.g`,
/// `key.mouse.right`). A key can drive several modules at once.
pub fn modules_bound_to(key: &str) -> Vec<&'static str> {
    with(|c| {
        c.modules
            .iter()
            .filter(|module| module.bind.as_deref() == Some(key))
            .map(|module| module.id)
            .collect()
    })
}

/// The first module bound to a key; `modules_bound_to` has all of them.
pub fn module_bound_to(key: &str) -> Option<&'static str> {
    modules_bound_to(key).first().copied()
}

/// Whether `key` is the Zoom module's bind, whether it is on or off. The key
/// is held rather than toggling, so the window hook claims it even to arm the
/// module on the first hold.
pub fn zoom_bound_key(key: &str) -> bool {
    with(|c| {
        c.modules
            .iter()
            .find(|module| module.id == ZOOM)
            .is_some_and(|module| module.bind.as_deref() == Some(key))
    })
}

/// Whether the zoom key is down: the wheel claims its steps only then.
pub fn zoom_held() -> bool {
    with(|c| c.zoom.held.is_some())
}

/// The zoom key went down or up. A press while a screen is open does nothing,
/// so typing in chat does not zoom; a release always lets go.
pub fn zoom_key(pressed: bool, playing: bool) {
    if pressed && !playing {
        return;
    }
    with(|c| {
        let factor = if pressed {
            c.modules
                .iter()
                .find(|module| module.id == ZOOM && module.enabled)
                .map(|module| ZOOM_FACTORS[module.choice("level").min(ZOOM_FACTORS.len() - 1)])
        } else {
            None
        };
        c.zoom.held = factor;
    });
}

/// Wheel steps while the zoom key is held: true when they zoom.
pub fn zoom_wheel(steps: f32) -> bool {
    with(|c| {
        let Some(held) = c.zoom.held else {
            return false;
        };
        if !c
            .modules
            .iter()
            .any(|module| module.id == ZOOM && module.enabled)
        {
            return false;
        }
        c.zoom.held = Some((held * ZOOM_STEP.powf(steps)).clamp(ZOOM_MIN, ZOOM_MAX));
        true
    })
}

/// The zoom in effect: (factor, smooth, turning scale), none when the key is
/// up or the module is off.
pub fn zoom_view() -> Option<(f32, bool, f32)> {
    with(|c| {
        let factor = c.zoom.held?;
        let module = c
            .modules
            .iter()
            .find(|module| module.id == ZOOM && module.enabled)?;
        let turn = if module.bool("sensitivity") {
            1.0 / factor
        } else {
            1.0
        };
        Some((factor, module.bool("smooth"), turn))
    })
}

/// Lets go of a held zoom when the window opens, closes or loses focus.
pub fn zoom_release() {
    with(|c| c.zoom.held = None);
}

/// The config file's first line.
pub const CONFIG_HEADER: &str = "# StariyBog config v1";

/// Where the client's configs live: `config/stariy-bog/` beside
/// `options.txt`, made when missing. `options.txt` sits in the working
/// directory (the game's `options_path`), so this matches
/// `Game::native_config_dir()` without a game to borrow.
pub fn config_directory() -> Result<std::path::PathBuf, String> {
    let dir = std::env::current_dir()
        .unwrap_or_default()
        .join("config")
        .join("stariy-bog");
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    Ok(dir)
}

/// A safe config file stem: letters, digits, `-` and `_`, at most 32
/// characters. `None` means the name cannot be used.
pub fn valid_config_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    .then(|| name.to_owned())
}

/// The saved configs, sorted by name. The folder is made when missing.
pub fn config_file_names() -> Result<Vec<String>, String> {
    let dir = config_directory()?;
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|error| format!("{}: {error}", dir.display()))?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !(path.is_file() && path.extension().is_some_and(|ext| ext == "cfg")) {
                return None;
            }
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_owned)
        })
        .collect();
    names.sort();
    Ok(names)
}

/// Every saved key and value, in registry order: `window.size`,
/// `enabled.<module>`, `bind.<module>` and `<module>.<setting>`. The keys are
/// the same ones the mod's per-file settings use, so loading a config also
/// refreshes them for the next launch.
pub fn config_entries() -> Vec<(String, String)> {
    with(|c| {
        let mut entries = vec![(
            "window.size".to_owned(),
            format!("{}x{}", c.gui.size.0, c.gui.size.1),
        )];
        for module in &c.modules {
            entries.push((format!("enabled.{}", module.id), module.enabled.to_string()));
            entries.push((
                format!("bind.{}", module.id),
                module.bind.clone().unwrap_or_default(),
            ));
            for setting in &module.settings {
                entries.push((format!("{}.{}", module.id, setting.id), setting.text()));
            }
        }
        entries
    })
}

/// The whole client as one config file.
pub fn config_document() -> String {
    let mut text = String::from(CONFIG_HEADER);
    text.push('\n');
    for (key, value) in config_entries() {
        text.push_str(&key);
        text.push('=');
        text.push_str(&escape_value(&value));
        text.push('\n');
    }
    text
}

/// Reads a config file's `key=value` lines. The header, blank lines and
/// comments are ignored; unknown keys are left for `apply_config` to drop.
pub fn parse_config(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_owned(), unescape_value(value.trim())))
        .collect()
}

/// Applies parsed config entries to the registry and returns how many keys
/// matched. Unknown modules, settings and window sizes are ignored.
pub fn apply_config(entries: &[(String, String)]) -> usize {
    with(|c| {
        let mut applied = 0;
        for (key, value) in entries {
            if key == "window.size" {
                if let Some((w, h)) = parse_window_size(value) {
                    c.gui.size = (w.clamp(MIN_W, 8192.0), h.clamp(MIN_H, 8192.0));
                    applied += 1;
                }
                continue;
            }
            if let Some(id) = key.strip_prefix("enabled.") {
                if let Some(module) = c.modules.iter_mut().find(|m| m.id == id) {
                    module.enabled = matches!(value.as_str(), "true" | "1" | "on" | "yes");
                    applied += 1;
                }
                continue;
            }
            if let Some(id) = key.strip_prefix("bind.") {
                if let Some(module) = c.modules.iter_mut().find(|m| m.id == id) {
                    module.bind = (!value.is_empty()).then(|| value.clone());
                    applied += 1;
                }
                continue;
            }
            let Some((module_id, setting_id)) = key.split_once('.') else {
                continue;
            };
            let Some(module) = c.modules.iter_mut().find(|m| m.id == module_id) else {
                continue;
            };
            if let Some(setting) = module.settings.iter_mut().find(|s| s.id == setting_id) {
                setting.set_text(value);
                applied += 1;
            }
        }
        c.gui.editing = None;
        c.gui.listening = None;
        applied
    })
}

fn parse_window_size(text: &str) -> Option<(f32, f32)> {
    let (w, h) = text.split_once('x')?;
    let w = w.trim().parse::<f32>().ok()?;
    let h = h.trim().parse::<f32>().ok()?;
    (w.is_finite() && h.is_finite()).then_some((w, h))
}

fn escape_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

fn unescape_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// FullBright's on/off state and settings, or None if the module is missing.
pub fn fullbright() -> Option<(bool, FullBrightConfig)> {
    with(|c| {
        c.modules.iter().find(|m| m.id == FULLBRIGHT).map(|m| {
            (
                m.enabled,
                FullBrightConfig {
                    mode: m.choice("mode").min(1) as u8,
                    brightness: m.int("brightness").clamp(1, 15),
                    smooth: m.bool("smooth"),
                    speed: m.float("speed").max(0.01),
                    amplifier: m.int("amplifier").clamp(0, 3),
                },
            )
        })
    })
}

/// The names the ArrayList shows: enabled modules except itself, sorted if the
/// module's `sorted` setting asks. None when the ArrayList is off.
pub fn arraylist() -> Option<(ArrayListConfig, Vec<&'static str>)> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == ARRAYLIST)?;
        if !module.enabled {
            return None;
        }
        let config = ArrayListConfig {
            background: module.bool("background"),
            shadow: module.bool("shadow"),
            sorted: module.bool("sorted"),
        };
        let mut names: Vec<&'static str> = c
            .modules
            .iter()
            .filter(|m| m.enabled && m.id != ARRAYLIST)
            .map(|m| m.name)
            .collect();
        if config.sorted {
            names.sort_unstable_by_key(|name| name.to_ascii_lowercase());
        }
        Some((config, names))
    })
}

/// The Watermark's settings, or None when the module is off.
pub fn watermark() -> Option<WatermarkConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == WATERMARK)?;
        if !module.enabled {
            return None;
        }
        Some(WatermarkConfig {
            corner: module.choice("corner").min(3),
            offset_x: module.int("offset_x").clamp(0, 400) as f32,
            offset_y: module.int("offset_y").clamp(0, 400) as f32,
            shadow: module.bool("shadow"),
        })
    })
}

/// The AutoTool's settings, or None when the module is off.
pub fn autotool() -> Option<AutoToolConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == AUTOTOOL)?;
        if !module.enabled {
            return None;
        }
        Some(AutoToolConfig {
            mode: module.choice("selector").min(1) as u8,
            ignore_durability: module.bool("ignore_durability"),
            consider_inventory: module.bool("consider_inventory"),
            static_slot: module.int("static_slot").clamp(0, 8) as usize,
            filter: module.choice("filter").min(1) as u8,
            blocks: parse_blocks(module.text("blocks")),
            silk_touch: module.bool("silk_touch"),
            silk_touch_filter: module.choice("silk_touch_filter").min(1) as u8,
            silk_touch_blocks: parse_blocks(module.text("silk_touch_blocks")),
            swap_back_delay: module.int("swap_back_delay").clamp(1, 100),
            switch_delay: module.int("switch_delay").clamp(0, 100),
            require_sneaking: module.bool("require_sneaking"),
            not_during_combat: module.bool("not_during_combat"),
            require_near_bed: module.bool("require_near_bed"),
            bed_distance: module.float("bed_distance").clamp(3.0, 50.0),
        })
    })
}

/// The AutoWeapon's settings, or None when the module is off.
pub fn autoweapon() -> Option<AutoWeaponConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == AUTOWEAPON)?;
        if !module.enabled {
            return None;
        }
        Some(AutoWeaponConfig {
            preferred: parse_weapons(module.text("preferred")),
            priority: module.choice("priority").min(3) as u8,
            auto_shield_break: module.bool("auto_shield_break"),
            auto_mace: module.bool("auto_mace"),
            switch_back: module.int("switch_back").clamp(1, 300),
            change_on: module.choice("change_on").min(2) as u8,
        })
    })
}

/// NameTags' settings, or None when the module is off.
pub fn nametags() -> Option<NameTagsConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == NAMETAGS)?;
        if !module.enabled {
            return None;
        }
        Some(NameTagsConfig {
            players: module.bool("players"),
            items: module.bool("items"),
            player_head: module.bool("player_head"),
            item_icons: module.bool("item_icons"),
        })
    })
}

/// NoFall's settings, or None when the module is off.
pub fn nofall() -> Option<NoFallConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == NOFALL)?;
        if !module.enabled {
            return None;
        }
        Some(NoFallConfig {
            mode: NoFallMode::from_index(module.choice("mode")),
            not_while_gliding: module.bool("not_while_gliding"),
            not_with_mace: module.bool("not_with_mace"),
            spoof_ground_distance: module.choice("spoof_ground_distance").min(1) as u8,
            spoof_ground_value: module.float("spoof_ground_value").clamp(0.0, 5.0),
            spoof_ground_reset: module.bool("spoof_ground_reset"),
            offset: [
                f64::from(module.float("offset_x")),
                f64::from(module.float("offset_y")),
                f64::from(module.float("offset_z")),
            ],
            packet_type: module.choice("packet_type").min(1) as u8,
            packet_filter: module.choice("packet_filter").min(1) as u8,
            packet_distance: module.choice("packet_distance").min(1) as u8,
            packet_value: module.float("packet_value").clamp(0.0, 5.0),
            packet_reset: module.bool("packet_reset"),
            jump_packet_type: module.choice("jump_packet_type").min(1) as u8,
            jump_distance: module.choice("jump_distance").min(1) as u8,
            jump_value: module.float("jump_value").clamp(0.0, 5.0),
            jump_timing: module.choice("jump_timing").min(1) as u8,
            jump_reset: module.bool("jump_reset"),
            mlg_min_fall: module.float("mlg_min_fall").clamp(2.0, 50.0),
            mount_min_fall: module.float("mount_min_fall").clamp(2.0, 50.0),
            mount_search_range: module.float("mount_search_range").clamp(1.0, 8.0),
            mount_retry_delay: module.int("mount_retry_delay").clamp(0, 20),
            void_level: module.int("void_level").clamp(-256, 0),
            block_distance: module.float("block_distance").clamp(0.1, 5.0),
            force_fall_distance: module.float("force_fall_distance").clamp(3.35, 10.0),
            jump_height: module.float("jump_height").clamp(0.1, 0.42),
            cancel_distance: module.choice("cancel_distance").min(1) as u8,
            cancel_value: module.float("cancel_value").clamp(0.0, 5.0),
            cancel_reset: module.bool("cancel_reset"),
            cancel_setback: module.bool("cancel_setback"),
            blink_trigger: module.float("blink_trigger").clamp(0.5, 3.0),
            blink_maximum: module.float("blink_maximum").clamp(2.0, 50.0),
            over_void: module.bool("over_void"),
            motion_flag_speed: module.float("motion_flag_speed").clamp(-1.0, 1.0),
            matrix_safe: module.bool("matrix_safe"),
            phase_offset: module.float("phase_offset").clamp(0.0, 5.0),
        })
    })
}

/// Criticals' settings, or None when the module is off.
pub fn criticals() -> Option<CriticalsConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == CRITICALS)?;
        if !module.enabled {
            return None;
        }
        Some(CriticalsConfig {
            mode: CriticalsMode::from_index(module.choice("mode")),
            sprint_stop: module.choice("sprint_stop").min(3) as u8,
            sprint_range: module.float("sprint_range").clamp(0.0, 10.0),
            only_sprinting: module.bool("only_sprinting"),
            packet_mode: module.choice("packet_mode").min(6) as u8,
            packet_type: module.choice("packet_type").min(1) as u8,
            delay: module.int("delay").clamp(0, 5000),
            hurt_time: module.int("hurt_time").clamp(0, 20),
            jump_height: module.float("jump_height").clamp(0.1, 0.42),
            jump_range: module.float("jump_range").clamp(1.0, 6.0),
            jump_optimize: module.bool("jump_optimize"),
            jump_can_be_seen: module.bool("jump_can_be_seen"),
            blink_delay_min: module.int("blink_delay_min").clamp(0, 1000),
            blink_delay_max: module.int("blink_delay_max").clamp(0, 1000),
            blink_range: module.float("blink_range").clamp(0.0, 10.0),
            timer_speed: module.float("timer_speed").clamp(0.1, 1.0),
            timer_range: module.float("timer_range").clamp(0.0, 10.0),
            custom_motion_y: module.float("custom_motion_y").clamp(0.01, 0.42),
            edit_delay: module.int("edit_delay").clamp(0, 1000),
            alan_delay: module.int("alan_delay").clamp(0, 1000),
            watchdog_delay: module.int("watchdog_delay").clamp(0, 1000),
            ncp_ticks: module.int("ncp_ticks").clamp(0, 20),
            summer_delay: module.int("summer_delay").clamp(0, 2000),
            summer_ground_ticks: module.int("summer_ground_ticks").clamp(0, 10),
            tenacity_watchdog_mode: module.choice("tenacity_watchdog_mode").min(1) as u8,
            tenacity_delay: module.int("tenacity_delay").clamp(0, 20),
            flux_hurt_time: module.int("flux_hurt_time").clamp(0, 20),
            flux_delay: module.int("flux_delay").clamp(0, 5000),
            flux_post_motion: module.bool("flux_post_motion"),
            sigma_hurt_time: module.int("sigma_hurt_time").clamp(0, 20),
            deadcode_fall_distance: module.float("deadcode_fall_distance").clamp(0.01, 0.41),
            deadcode_delay: module.int("deadcode_delay").clamp(0, 500),
            deadcode_auto_jump: module.bool("deadcode_auto_jump"),
            deadcode_auto_jump_distance: module
                .float("deadcode_auto_jump_distance")
                .clamp(1.0, 20.0),
            deadcode_smart: module.bool("deadcode_smart"),
            celestial_mini_jump: module.bool("celestial_mini_jump"),
            celestial_ncp_delay: module.int("celestial_ncp_delay").clamp(0, 2000),
            celestial_ground_false: module.bool("celestial_ground_false"),
            celestial_reduce_damage: module.bool("celestial_reduce_damage"),
            novoline_hypixel: module.bool("novoline_hypixel"),
            novoline_random: module.float("novoline_random").clamp(0.0, 0.01),
            visuals: module.bool("visuals"),
            visual_fake: module.bool("visual_fake"),
            visual_critical: module.int("visual_critical").clamp(0, 20),
            visual_magic: module.int("visual_magic").clamp(0, 20),
        })
    })
}

/// A Scaffold placement mode, in `SCAFFOLD_MODES` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaffoldMode {
    Off,
    Normal,
    Legit,
    New,
    Aac,
    Hypixel,
    Alan,
    NewScaffold,
    Novoline,
    Sigma,
    Tenacity,
    Yuri,
    Expand,
    GodBridge,
    Breezily,
    /// Alan's LegitScaffold: sneak at the edge and place nothing.
    Eagle,
}

impl ScaffoldMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => ScaffoldMode::Normal,
            2 => ScaffoldMode::Legit,
            3 => ScaffoldMode::New,
            4 => ScaffoldMode::Aac,
            5 => ScaffoldMode::Hypixel,
            6 => ScaffoldMode::Alan,
            7 => ScaffoldMode::NewScaffold,
            8 => ScaffoldMode::Novoline,
            9 => ScaffoldMode::Sigma,
            10 => ScaffoldMode::Tenacity,
            11 => ScaffoldMode::Yuri,
            12 => ScaffoldMode::Expand,
            13 => ScaffoldMode::GodBridge,
            14 => ScaffoldMode::Breezily,
            15 => ScaffoldMode::Eagle,
            _ => ScaffoldMode::Off,
        }
    }
}

/// A Scaffold sprint mode, in `SCAFFOLD_SPRINTS` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaffoldSprint {
    Off,
    Disabled,
    Legit,
    Matrix,
    Verus,
    Vulcan,
    Watchdog,
}

impl ScaffoldSprint {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => ScaffoldSprint::Disabled,
            2 => ScaffoldSprint::Legit,
            3 => ScaffoldSprint::Matrix,
            4 => ScaffoldSprint::Verus,
            5 => ScaffoldSprint::Vulcan,
            6 => ScaffoldSprint::Watchdog,
            _ => ScaffoldSprint::Off,
        }
    }
}

/// A Scaffold tower mode, in `SCAFFOLD_TOWERS` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaffoldTower {
    Off,
    Vanilla,
    Legit,
    Vulcan,
    Snap,
    Watchdog,
}

impl ScaffoldTower {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => ScaffoldTower::Vanilla,
            2 => ScaffoldTower::Legit,
            3 => ScaffoldTower::Vulcan,
            4 => ScaffoldTower::Snap,
            5 => ScaffoldTower::Watchdog,
            _ => ScaffoldTower::Off,
        }
    }
}

/// A Scaffold downwards mode, in `SCAFFOLD_DOWNWARDS` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaffoldDownwards {
    Off,
    Normal,
    Verus,
}

impl ScaffoldDownwards {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => ScaffoldDownwards::Normal,
            2 => ScaffoldDownwards::Verus,
            _ => ScaffoldDownwards::Off,
        }
    }
}

/// Scaffold's settings as its placement tick wants them.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaffoldConfig {
    pub mode: ScaffoldMode,
    pub place_delay: i32,
    pub rotations: bool,
    pub silent: bool,
    pub sprint: ScaffoldSprint,
    pub tower: ScaffoldTower,
    pub tower_move: bool,
    pub keep_y: bool,
    pub downwards: ScaffoldDownwards,
    pub sneak: bool,
    pub swing: bool,
    pub auto_jump: bool,
    pub extend: f32,
    /// Eagle's sneak hold, in ticks.
    pub eagle_length: i32,
    /// Eagle only engages while the player is on the ground.
    pub eagle_ground: bool,
    /// Eagle only engages while a block is held.
    pub eagle_blocks: bool,
    /// Eagle only engages while the forward key is up.
    pub eagle_backwards: bool,
}

/// Scaffold's settings, or None when the module is off.
pub fn scaffold() -> Option<ScaffoldConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == SCAFFOLD)?;
        if !module.enabled {
            return None;
        }
        Some(ScaffoldConfig {
            mode: ScaffoldMode::from_index(module.choice("mode")),
            place_delay: module.int("place_delay").clamp(0, 500),
            rotations: module.bool("rotations"),
            silent: module.bool("silent"),
            sprint: ScaffoldSprint::from_index(module.choice("sprint")),
            tower: ScaffoldTower::from_index(module.choice("tower")),
            tower_move: module.bool("tower_move"),
            keep_y: module.bool("keep_y"),
            downwards: ScaffoldDownwards::from_index(module.choice("downwards")),
            sneak: module.bool("sneak"),
            swing: module.bool("swing"),
            auto_jump: module.bool("auto_jump"),
            extend: module.float("extend").clamp(0.0, 2.0),
            eagle_length: module.int("eagle_length").clamp(1, 4),
            eagle_ground: module.bool("eagle_ground"),
            eagle_blocks: module.bool("eagle_blocks"),
            eagle_backwards: module.bool("eagle_backwards"),
        })
    })
}

/// ChestStealer's settings as its tick and its overlay want them.
#[derive(Clone, Debug, PartialEq)]
pub struct ChestStealerConfig {
    /// Delay by the distance to the next slot instead of the plain delay.
    pub smart_delay: bool,
    /// SmartDelay's multiplier: milliseconds per slot of distance.
    pub delay_multiplier: i32,
    /// Steal along the shortest path from the last slot.
    pub smart_order: bool,
    /// Sometimes pause longer, so the stealing looks less robotic.
    pub simulate_short_stop: bool,
    /// Close the chest once it holds nothing useful.
    pub auto_close: bool,
    /// 0 TopToBottom, 1 BottomToTop.
    pub order: u8,
    /// Move single stacks by swapping them into an empty hotbar slot.
    pub quick_swaps: bool,
    /// 0 Stop, 1 Continue.
    pub on_full: u8,
    /// The delay between steals in milliseconds, without SmartDelay.
    pub delay: i32,
    /// The delay before the first steal.
    pub start_delay: i32,
    /// The delay before the chest is closed.
    pub close_delay: i32,
    /// Only steal from menus titled a chest, not custom GUIs.
    pub chest_title: bool,
    /// Shuffle the slots, so the clicks are not in a straight line.
    pub random_slot: bool,
    /// Draw the progress bar while stealing.
    pub progress_bar: bool,
    /// Also steal from furnace menus.
    pub furnace: bool,
    /// Highlight the slot being stolen.
    pub highlight_slot: bool,
    pub background_color: u32,
    pub border_strength: i32,
    pub border_color: u32,
    /// 0 Off, 1 Text.
    pub debug: u8,
    /// Print each stolen stack's slot and delay.
    pub item_debug: bool,
}

/// ChestStealer's settings, or None when the module is off.
pub fn cheststealer() -> Option<ChestStealerConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == CHESTSTEALER)?;
        if !module.enabled {
            return None;
        }
        Some(ChestStealerConfig {
            smart_delay: module.bool("smart_delay"),
            delay_multiplier: module.int("delay_multiplier").clamp(0, 500),
            smart_order: module.bool("smart_order"),
            simulate_short_stop: module.bool("simulate_short_stop"),
            auto_close: module.bool("auto_close"),
            order: module.choice("order").min(1) as u8,
            quick_swaps: module.bool("quick_swaps"),
            on_full: module.choice("on_full").min(1) as u8,
            delay: module.int("delay").clamp(0, 500),
            start_delay: module.int("start_delay").clamp(0, 500),
            close_delay: module.int("close_delay").clamp(0, 500),
            chest_title: module.bool("chest_title"),
            random_slot: module.bool("random_slot"),
            progress_bar: module.bool("progress_bar"),
            furnace: module.bool("furnace"),
            highlight_slot: module.bool("highlight_slot"),
            background_color: parse_color(module.text("background_color"), 0xFF80_8080),
            border_strength: module.int("border_strength").clamp(1, 5),
            border_color: parse_color(module.text("border_color"), 0xFF80_8080),
            debug: module.choice("debug").min(1) as u8,
            item_debug: module.bool("item_debug"),
        })
    })
}

/// ChestStealer's delay setting as whole game ticks (20 a second). At least
/// one tick, so a steal never shares its tick with the next.
pub fn steal_delay_ticks(ms: i32) -> i32 {
    ((ms.max(0) + 49) / 50).max(1)
}

/// SmartDelay's time to the next slot: the slot-grid distance times the
/// multiplier, plus the 0..=20 ms jitter the FDPClient module adds.
pub fn smart_delay_ms(distance_squared: i32, multiplier: i32, jitter: i32) -> i32 {
    let distance = (distance_squared.max(0) as f32).sqrt();
    (distance * multiplier.max(0) as f32) as i32 + jitter.clamp(0, 20)
}

/// The squared distance between two slots of a container panel (9 a row).
/// Panel slots start at 300 (see `Game::slot_position`, which draws a slot's
/// row and column from `slot - 300`), so the base has to come off first or
/// the columns wrap in the middle of a row.
pub fn slot_distance_squared(from: usize, to: usize) -> i32 {
    let coords = |slot: usize| {
        let slot = slot.saturating_sub(300);
        ((slot % 9) as i32, (slot / 9) as i32)
    };
    let (ax, ay) = coords(from);
    let (bx, by) = coords(to);
    (ax - bx) * (ax - bx) + (ay - by) * (ay - by)
}

/// FastBreak's settings, as the two references' fields.
pub struct FastBreakConfig {
    /// A `FASTBREAK_MODES` index.
    pub mode: u8,
    /// Damage: finish once the stored progress passes this fraction.
    pub break_damage: f32,
    /// Percentage: how much earlier than the end to finish, in percent.
    pub speed: i32,
    /// Ticks: finish within this many ticks of the computed end.
    pub ticks: i32,
    /// Percentage/Ticks: Alan's EqualAirGroundDig.
    pub equal_air_ground: bool,
    /// LiquidBounce's AbortAnother: abort the block above with each break.
    pub abort_another: bool,
    /// Only while the main hand holds a mining tool.
    pub only_tool: bool,
    /// Drop mining fatigue from the client's effect list.
    pub ignore_mining_fatigue: bool,
}

/// FastBreak's settings, or None when the module is off.
pub fn fastbreak() -> Option<FastBreakConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == FASTBREAK)?;
        if !module.enabled {
            return None;
        }
        Some(FastBreakConfig {
            mode: module.choice("mode").min(2) as u8,
            break_damage: module.float("break_damage").clamp(0.1, 1.0),
            speed: module.int("speed").clamp(0, 100),
            ticks: module.int("ticks").clamp(1, 100),
            equal_air_ground: module.bool("equal_air_ground"),
            abort_another: module.bool("abort_another"),
            only_tool: module.bool("only_tool"),
            ignore_mining_fatigue: module.bool("ignore_mining_fatigue"),
        })
    })
}

/// Alan's EqualAirGroundDig: the stored progress the reference's mode sees
/// and the fraction of the block that still has to be dug before it finishes.
///
/// In water the client's progress is divided by the off-ground penalty's
/// factor and the finish point is fixed at `margin`; moving through a liquid
/// off the ground credits it back and moves the finish point by the same
/// margin, so the two cancel in the common swimming case.
pub fn fastbreak_air_ground(
    mut progress: f32,
    mut threshold: f32,
    margin: f32,
    in_water: bool,
    ahead_air: bool,
    on_ground: bool,
) -> (f32, f32) {
    if in_water {
        progress /= 5.0;
        threshold = margin;
    }
    if !ahead_air && !on_ground {
        progress *= 5.0;
        threshold -= margin;
    }
    (progress, threshold)
}

/// The early finish a percentage/tick threshold grants, or None: Alan sets
/// the stored progress just under one when it already passed the finish point
/// but has not reached it yet.
pub fn fastbreak_finish(progress: f32, threshold: f32) -> Option<f32> {
    (progress > threshold && progress < 0.99).then_some(0.99)
}

/// A Velocity mode, in `VELOCITY_MODES` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VelocityMode {
    Off,
    Standard,
    Motion,
    Delayed,
    Reverse,
    Tick,
    JumpReset,
    Hypixel,
    Verus,
    Grim,
    BlocksMc,
    Phase,
    Click,
    SideStrafe,
    Aac,
    AacPush,
    Polar,
    Intave,
    NewGrim,
    Spoof,
    GrimC03,
}

impl VelocityMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => VelocityMode::Standard,
            2 => VelocityMode::Motion,
            3 => VelocityMode::Delayed,
            4 => VelocityMode::Reverse,
            5 => VelocityMode::Tick,
            6 => VelocityMode::JumpReset,
            7 => VelocityMode::Hypixel,
            8 => VelocityMode::Verus,
            9 => VelocityMode::Grim,
            10 => VelocityMode::BlocksMc,
            11 => VelocityMode::Phase,
            12 => VelocityMode::Click,
            13 => VelocityMode::SideStrafe,
            14 => VelocityMode::Aac,
            15 => VelocityMode::AacPush,
            16 => VelocityMode::Polar,
            17 => VelocityMode::Intave,
            18 => VelocityMode::NewGrim,
            19 => VelocityMode::Spoof,
            20 => VelocityMode::GrimC03,
            _ => VelocityMode::Off,
        }
    }
}

/// Velocity's settings as its packet and tick hooks want them.
#[derive(Clone, Debug, PartialEq)]
pub struct VelocityConfig {
    pub mode: VelocityMode,
    pub horizontal: f32,
    pub vertical: f32,
    pub motion_horizontal: f32,
    pub motion_vertical: f32,
    pub filter: u8,
    pub explosions: bool,
    pub chance: i32,
    pub only_move: bool,
    pub motion_hurt_time: i32,
    pub scale_horizontal: f32,
    pub scale_vertical: f32,
    pub delay_ticks: i32,
    pub x_modifier: f32,
    pub z_modifier: f32,
    pub wait_ticks: i32,
    pub reduction: f32,
    pub reset_vertical: bool,
    pub jump_delay: i32,
    pub only_sprinting: bool,
    pub air_only: bool,
    pub y_boost: f32,
    pub grim_duplicates: i32,
    pub grim_block_action: bool,
    pub blocksmc_ticks: i32,
    pub phase_height: f32,
    pub phase_ground: bool,
    pub click_count: i32,
    pub click_hurt_time: i32,
    pub click_interval: i32,
    pub side_strength: f32,
    pub aac_factor: f32,
    pub aac_only_air: bool,
    pub push_reducer: f32,
    pub push_reduce_y: bool,
    pub push_y: f32,
    pub polar_divisor: f32,
    pub polar_hurt_min: i32,
    pub polar_hurt_max: i32,
    pub intave_factor: f32,
    pub intave_hurt_min: i32,
    pub intave_hurt_max: i32,
    pub intave_window: i32,
    pub intave_jump_chance: i32,
    pub spoof_modify_timer: bool,
    pub spoof_timer: f32,
}

/// Velocity's settings, or None when the module is off.
pub fn velocity() -> Option<VelocityConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == VELOCITY)?;
        if !module.enabled {
            return None;
        }
        Some(VelocityConfig {
            mode: VelocityMode::from_index(module.choice("mode")),
            horizontal: module.float("horizontal").clamp(0.0, 1.0),
            vertical: module.float("vertical").clamp(0.0, 1.0),
            motion_horizontal: module.float("motion_horizontal").clamp(0.0, 1.0),
            motion_vertical: module.float("motion_vertical").clamp(0.0, 1.0),
            filter: module.choice("filter").min(2) as u8,
            explosions: module.bool("explosions"),
            chance: module.int("chance").clamp(1, 100),
            only_move: module.bool("only_move"),
            motion_hurt_time: module.int("motion_hurt_time").clamp(0, 10),
            scale_horizontal: module.float("scale_horizontal").clamp(0.0, 1.0),
            scale_vertical: module.float("scale_vertical").clamp(0.0, 1.0),
            delay_ticks: module.int("delay_ticks").clamp(0, 20),
            x_modifier: module.float("x_modifier").clamp(-2.0, 2.0),
            z_modifier: module.float("z_modifier").clamp(-2.0, 2.0),
            wait_ticks: module.int("wait_ticks").clamp(1, 10),
            reduction: module.float("reduction").clamp(0.0, 1.0),
            reset_vertical: module.bool("reset_vertical"),
            jump_delay: module.int("jump_delay").clamp(0, 10),
            only_sprinting: module.bool("only_sprinting"),
            air_only: module.bool("air_only"),
            y_boost: module.float("y_boost").clamp(0.0, 0.1),
            grim_duplicates: module.int("grim_duplicates").clamp(0, 10),
            grim_block_action: module.bool("grim_block_action"),
            blocksmc_ticks: module.int("blocksmc_ticks").clamp(0, 10),
            phase_height: module.float("phase_height").clamp(0.0, 1.0),
            phase_ground: module.bool("phase_ground"),
            click_count: module.int("click_count").clamp(1, 10),
            click_hurt_time: module.int("click_hurt_time").clamp(0, 10),
            click_interval: module.int("click_interval").clamp(0, 5),
            side_strength: module.float("side_strength").clamp(0.0, 2.0),
            aac_factor: module.float("aac_factor").clamp(0.0, 1.0),
            aac_only_air: module.bool("aac_only_air"),
            push_reducer: module.float("push_reducer").clamp(1.0, 5.0),
            push_reduce_y: module.bool("push_reduce_y"),
            push_y: module.float("push_y").clamp(0.0, 0.1),
            polar_divisor: module.float("polar_divisor").clamp(1.0, 2.0),
            polar_hurt_min: module.int("polar_hurt_min").clamp(0, 10),
            polar_hurt_max: module.int("polar_hurt_max").clamp(0, 10),
            intave_factor: module.float("intave_factor").clamp(0.0, 1.0),
            intave_hurt_min: module.int("intave_hurt_min").clamp(0, 10),
            intave_hurt_max: module.int("intave_hurt_max").clamp(0, 10),
            intave_window: module.int("intave_window").clamp(0, 10000),
            intave_jump_chance: module.int("intave_jump_chance").clamp(0, 100),
            spoof_modify_timer: module.bool("spoof_modify_timer"),
            spoof_timer: module.float("spoof_timer").clamp(0.1, 1.0),
        })
    })
}

/// A Speed mode, in `SPEED_MODES` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedMode {
    LegitHop,
    Custom,
    YPort,
    Vanilla,
    Aac,
    HypixelBHop,
    HypixelLowHop,
    Spartan,
    SpartanFastFall,
    SentinelDamage,
    Vulcan286,
    Vulcan288,
    VulcanGround286,
    GrimCollide,
    GrimBHop,
    GrimLowHop,
    Ncp,
    NcpBHop,
    NcpYPort,
    Intave14,
    Intave14Fast,
    HylexLowHop,
    HylexGround,
    BlocksMc,
    BlocksMcHop,
    Matrix7,
    MatrixHop,
    Matrix,
    VerusHop,
    VerusFHop,
    VerusLowHop,
    Verus,
    Librecraft,
    KoksCraft,
    MiJump,
    SlowHop,
    Frame,
    SpectreHop,
    SpectreOnGround,
    Swift,
    WatchdogTerrain,
}

impl SpeedMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => SpeedMode::Custom,
            2 => SpeedMode::YPort,
            3 => SpeedMode::Vanilla,
            4 => SpeedMode::Aac,
            5 => SpeedMode::HypixelBHop,
            6 => SpeedMode::HypixelLowHop,
            7 => SpeedMode::Spartan,
            8 => SpeedMode::SpartanFastFall,
            9 => SpeedMode::SentinelDamage,
            10 => SpeedMode::Vulcan286,
            11 => SpeedMode::Vulcan288,
            12 => SpeedMode::VulcanGround286,
            13 => SpeedMode::GrimCollide,
            14 => SpeedMode::GrimBHop,
            15 => SpeedMode::GrimLowHop,
            16 => SpeedMode::Ncp,
            17 => SpeedMode::NcpBHop,
            18 => SpeedMode::NcpYPort,
            19 => SpeedMode::Intave14,
            20 => SpeedMode::Intave14Fast,
            21 => SpeedMode::HylexLowHop,
            22 => SpeedMode::HylexGround,
            23 => SpeedMode::BlocksMc,
            24 => SpeedMode::BlocksMcHop,
            25 => SpeedMode::Matrix7,
            26 => SpeedMode::MatrixHop,
            27 => SpeedMode::Matrix,
            28 => SpeedMode::VerusHop,
            29 => SpeedMode::VerusFHop,
            30 => SpeedMode::VerusLowHop,
            31 => SpeedMode::Verus,
            32 => SpeedMode::Librecraft,
            33 => SpeedMode::KoksCraft,
            34 => SpeedMode::MiJump,
            35 => SpeedMode::SlowHop,
            36 => SpeedMode::Frame,
            37 => SpeedMode::SpectreHop,
            38 => SpeedMode::SpectreOnGround,
            39 => SpeedMode::Swift,
            40 => SpeedMode::WatchdogTerrain,
            _ => SpeedMode::LegitHop,
        }
    }
}

/// Speed's settings as its tick hook wants them.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeedConfig {
    pub mode: SpeedMode,
    pub custom_horizontal_acceleration: f32,
    pub custom_jump_off: f32,
    pub custom_boost_ticks: i32,
    pub custom_jump_height: f32,
    pub custom_pulldown: f32,
    pub custom_fall_pulldown: f32,
    pub custom_strafe_strength: f32,
    pub custom_speed_enabled: bool,
    pub custom_speed: f32,
    pub yp_speed: f32,
    pub vanilla_speed: f32,
    pub hypixel_horizontal_acceleration: bool,
    pub hypixel_vertical_acceleration: bool,
    pub hypixel_lowhop_glide: bool,
    pub sentinel_speed: f32,
    pub sentinel_reboost_ticks: i32,
    pub grim_boost: f32,
    pub grim_shrink: f32,
    pub ncp_pull_down: bool,
    pub ncp_motion_multiplier: f32,
    pub ncp_pull_down_tick: i32,
    pub ncp_on_hurt: bool,
    pub ncp_boost: bool,
    pub ncp_initial_boost_multiplier: f32,
    pub ncp_damage_boost: bool,
    pub ncp_low_hop: bool,
    pub ncp_air_strafe: bool,
    pub intave_strafe: bool,
    pub intave_strafe_strength: f32,
    pub intave_air_boost: bool,
    pub blocksmc_round_yaw: bool,
    pub blocksmc_hop_full_strafe: bool,
    pub blocksmc_hop_low_hop: bool,
    pub blocksmc_hop_damage_boost: bool,
    pub blocksmc_hop_damage_low_hop: bool,
    pub blocksmc_hop_safe_y: bool,
    pub matrix_low_hop: bool,
    pub matrix_extra_boost: f32,
    pub spectre_jump_motion: f32,
}

/// Speed's settings, or None when the module is off.
pub fn speed() -> Option<SpeedConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == SPEED)?;
        if !module.enabled {
            return None;
        }
        Some(SpeedConfig {
            mode: SpeedMode::from_index(module.choice("mode")),
            custom_horizontal_acceleration: module
                .float("custom_horizontal_acceleration")
                .clamp(-0.1, 0.2),
            custom_jump_off: module.float("custom_jump_off").clamp(-0.5, 1.0),
            custom_boost_ticks: module.int("custom_boost_ticks").clamp(0, 20),
            custom_jump_height: module.float("custom_jump_height").clamp(0.0, 3.0),
            custom_pulldown: module.float("custom_pulldown").clamp(0.0, 1.0),
            custom_fall_pulldown: module.float("custom_fall_pulldown").clamp(0.0, 1.0),
            custom_strafe_strength: module.float("custom_strafe_strength").clamp(0.1, 1.0),
            custom_speed_enabled: module.bool("custom_speed_enabled"),
            custom_speed: module.float("custom_speed").clamp(0.1, 10.0),
            yp_speed: module.float("yp_speed").clamp(0.1, 1.0),
            vanilla_speed: module.float("vanilla_speed").clamp(0.1, 9.5),
            hypixel_horizontal_acceleration: module.bool("hypixel_horizontal_acceleration"),
            hypixel_vertical_acceleration: module.bool("hypixel_vertical_acceleration"),
            hypixel_lowhop_glide: module.bool("hypixel_lowhop_glide"),
            sentinel_speed: module.float("sentinel_speed").clamp(0.1, 5.0),
            sentinel_reboost_ticks: module.int("sentinel_reboost_ticks").clamp(10, 50),
            grim_boost: module.float("grim_boost").clamp(0.01, 0.08),
            grim_shrink: module.float("grim_shrink").clamp(0.1, 2.0),
            ncp_pull_down: module.bool("ncp_pull_down"),
            ncp_motion_multiplier: module.float("ncp_motion_multiplier").clamp(0.01, 10.0),
            ncp_pull_down_tick: module.int("ncp_pull_down_tick").clamp(1, 9),
            ncp_on_hurt: module.bool("ncp_on_hurt"),
            ncp_boost: module.bool("ncp_boost"),
            ncp_initial_boost_multiplier: module
                .float("ncp_initial_boost_multiplier")
                .clamp(0.01, 10.0),
            ncp_damage_boost: module.bool("ncp_damage_boost"),
            ncp_low_hop: module.bool("ncp_low_hop"),
            ncp_air_strafe: module.bool("ncp_air_strafe"),
            intave_strafe: module.bool("intave_strafe"),
            intave_strafe_strength: module.float("intave_strafe_strength").clamp(0.01, 0.27),
            intave_air_boost: module.bool("intave_air_boost"),
            blocksmc_round_yaw: module.bool("blocksmc_round_yaw"),
            blocksmc_hop_full_strafe: module.bool("blocksmc_hop_full_strafe"),
            blocksmc_hop_low_hop: module.bool("blocksmc_hop_low_hop"),
            blocksmc_hop_damage_boost: module.bool("blocksmc_hop_damage_boost"),
            blocksmc_hop_damage_low_hop: module.bool("blocksmc_hop_damage_low_hop"),
            blocksmc_hop_safe_y: module.bool("blocksmc_hop_safe_y"),
            matrix_low_hop: module.bool("matrix_low_hop"),
            matrix_extra_boost: module.float("matrix_extra_boost").clamp(0.0, 1.0),
            spectre_jump_motion: module.float("spectre_jump_motion").clamp(0.0, 1.0),
        })
    })
}

/// The Timer module's settings as its tick wants them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimerConfig {
    /// 0 Always, 1 OnMove, 2 NoMove.
    pub mode: u8,
    pub multiplier: f32,
    pub boost_ticks: u32,
    pub boost_speed: f32,
    pub normal_ticks: u32,
}

/// The Timer module's settings, or None when the module is off.
pub fn timer() -> Option<TimerConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == TIMER)?;
        if !module.enabled {
            return None;
        }
        Some(TimerConfig {
            mode: module.choice("mode").min(2) as u8,
            multiplier: module.float("timer_multiplier").clamp(0.1, 15.0),
            boost_ticks: module.int("boost_ticks").clamp(0, 200) as u32,
            boost_speed: module.float("boost_speed").clamp(0.1, 15.0),
            normal_ticks: module.int("normal_ticks").clamp(1, 200) as u32,
        })
    })
}

/// A Phase mode, in `PHASE_MODES` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseMode {
    /// The 0.5 teleport burst, then a small local push.
    Vanilla,
    /// The 0.3 burst, in three rising steps.
    Skip,
    /// The three-tick 0.06 then 1.6 offset.
    Mineplex,
    /// Scan forward for a column of two air blocks and stand in it.
    Clip,
    /// Settle into the centre of the block below.
    Center,
    /// Push into the block every tick while it is hit.
    Nudge,
    /// Break the block in the way, then step into its place.
    Break,
    /// Sink through the floor while sneaking.
    Sink,
    /// Throw an ender pearl into the block.
    Pearl,
}

impl PhaseMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => PhaseMode::Skip,
            2 => PhaseMode::Mineplex,
            3 => PhaseMode::Clip,
            4 => PhaseMode::Center,
            5 => PhaseMode::Nudge,
            6 => PhaseMode::Break,
            7 => PhaseMode::Sink,
            8 => PhaseMode::Pearl,
            _ => PhaseMode::Vanilla,
        }
    }
}

/// Phase's settings as its hook wants them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseConfig {
    pub mode: PhaseMode,
    /// Ticks between two attempts.
    pub delay: u32,
    /// How far forward Nudge moves the player each tick.
    pub push: f64,
    /// The pitch a pearl is thrown at.
    pub pitch: f32,
    /// How far ahead Clip scans, in blocks.
    pub scan: i32,
}

/// Phase's settings, or None when the module is off.
pub fn phase() -> Option<PhaseConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == PHASE)?;
        if !module.enabled {
            return None;
        }
        Some(PhaseConfig {
            mode: PhaseMode::from_index(module.choice("mode")),
            delay: module.int("delay").clamp(1, 20) as u32,
            push: f64::from(module.float("push")).clamp(0.005, 1.0),
            pitch: module.float("pitch").clamp(0.0, 90.0),
            scan: module.int("scan").clamp(1, 20),
        })
    })
}

/// A Step mode, in `STEP_MODES` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepMode {
    Vanilla,
    Jump,
    Ncp,
    NcpNew,
    NcpPacketless,
    MotionNcp,
    OldNcp,
    Aac440,
    Laac,
    Aac334,
    Spartan,
    Rewinside,
    Vulcan,
    Vulcan286,
    Verus,
    Watchdog,
    Instant,
    BlocksMc,
    BlocksMcTimer,
    Hypixel,
    Matrix,
    Matrix670,
}

impl StepMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => StepMode::Jump,
            2 => StepMode::Ncp,
            3 => StepMode::NcpNew,
            4 => StepMode::NcpPacketless,
            5 => StepMode::MotionNcp,
            6 => StepMode::OldNcp,
            7 => StepMode::Aac440,
            8 => StepMode::Laac,
            9 => StepMode::Aac334,
            10 => StepMode::Spartan,
            11 => StepMode::Rewinside,
            12 => StepMode::Vulcan,
            13 => StepMode::Vulcan286,
            14 => StepMode::Verus,
            15 => StepMode::Watchdog,
            16 => StepMode::Instant,
            17 => StepMode::BlocksMc,
            18 => StepMode::BlocksMcTimer,
            19 => StepMode::Hypixel,
            20 => StepMode::Matrix,
            21 => StepMode::Matrix670,
            _ => StepMode::Vanilla,
        }
    }
}

/// Step's settings as its tick and packet hooks want them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepConfig {
    pub mode: StepMode,
    pub height: f32,
    pub jump_height: f32,
    /// Milliseconds between steps, and the extra delay before them.
    pub delay: u32,
    pub extra_delay: u32,
    pub simulate_jump: bool,
    /// The step timer, and whether it grows with the step's height.
    pub timer: f32,
    pub dynamic_timer: bool,
    pub matrix_two_block: bool,
    pub matrix_instant: bool,
    /// LiquidBounce Instant's trim, jump-order range and wait, and packet.
    pub instant_trim: bool,
    pub instant_order_min: u32,
    pub instant_order_max: u32,
    pub instant_wait_min: u32,
    pub instant_wait_max: u32,
    pub instant_full: bool,
    /// LiquidBounce BlocksMC's two timers.
    pub blocksmc_base_timer: f32,
    pub blocksmc_recovery_timer: f32,
    /// LiquidBounce Hypixel's bypass choices.
    pub hypixel_alternate: bool,
    pub hypixel_spoof: bool,
}

/// Step's settings, or None when the module is off.
pub fn step() -> Option<StepConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == STEP)?;
        if !module.enabled {
            return None;
        }
        Some(StepConfig {
            mode: StepMode::from_index(module.choice("mode")),
            height: module.float("height").clamp(0.6, 10.0),
            jump_height: module.float("jump_height").clamp(0.37, 0.42),
            delay: module.int("delay").clamp(0, 500) as u32,
            extra_delay: module.int("extra_delay").clamp(0, 1000) as u32,
            simulate_jump: module.bool("simulate_jump"),
            timer: module.float("timer").clamp(0.05, 1.0),
            dynamic_timer: module.bool("dynamic_timer"),
            matrix_two_block: module.bool("matrix_two_block"),
            matrix_instant: module.bool("matrix_instant"),
            instant_trim: module.bool("instant_trim"),
            instant_order_min: module.int("instant_order_min").clamp(0, 8) as u32,
            instant_order_max: module.int("instant_order_max").clamp(0, 8) as u32,
            instant_wait_min: module.int("instant_wait_min").clamp(0, 60) as u32,
            instant_wait_max: module.int("instant_wait_max").clamp(0, 60) as u32,
            instant_full: module.choice("instant_packet") == 1,
            blocksmc_base_timer: module.float("blocksmc_base_timer").clamp(0.1, 5.0),
            blocksmc_recovery_timer: module.float("blocksmc_recovery_timer").clamp(0.1, 5.0),
            hypixel_alternate: module.bool("hypixel_alternate"),
            hypixel_spoof: module.bool("hypixel_spoof"),
        })
    })
}

/// A Fly mode, in `FLY_MODES` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlyMode {
    Vanilla,
    AirWalk,
    Grim,
    GrimOld,
    GrimElytra,
    GrimFloat,
    GrimEntity,
    Ncp,
    OldNcp,
    NcpBurst,
    Hypixel,
    HypixelBoost,
    Vulcan,
    Verus,
    VerusDamage,
    Spartan,
    AacGlide,
    Matrix,
    Damage,
    Sentinel,
    Jetpack,
    Explosion,
    BufferAbuse,
    MatrixHighJump,
}

impl FlyMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => FlyMode::AirWalk,
            2 => FlyMode::Grim,
            3 => FlyMode::GrimOld,
            4 => FlyMode::GrimElytra,
            5 => FlyMode::GrimFloat,
            6 => FlyMode::GrimEntity,
            7 => FlyMode::Ncp,
            8 => FlyMode::OldNcp,
            9 => FlyMode::NcpBurst,
            10 => FlyMode::Hypixel,
            11 => FlyMode::HypixelBoost,
            12 => FlyMode::Vulcan,
            13 => FlyMode::Verus,
            14 => FlyMode::VerusDamage,
            15 => FlyMode::Spartan,
            16 => FlyMode::AacGlide,
            17 => FlyMode::Matrix,
            18 => FlyMode::Damage,
            19 => FlyMode::Sentinel,
            20 => FlyMode::Jetpack,
            21 => FlyMode::Explosion,
            22 => FlyMode::BufferAbuse,
            23 => FlyMode::MatrixHighJump,
            _ => FlyMode::Vanilla,
        }
    }
}

/// Fly's settings as its tick and packet hooks want them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlyConfig {
    pub mode: FlyMode,
    /// Vanilla's hover, glide and vanilla-check bypass.
    pub vanilla_speed: f32,
    pub vanilla_vertical: f32,
    pub vanilla_glide: f32,
    pub vanilla_bypass: bool,
    pub anti_kick: bool,
    /// AirWalk's ground-spoof hover and timer.
    pub airwalk_speed: f32,
    pub airwalk_vertical: f32,
    pub airwalk_ticks: u32,
    pub airwalk_timer: f32,
    /// Grim's key-driven glide and its hurt speed.
    pub grim_speed: f32,
    pub grim_vertical: f32,
    pub grim_sink: f32,
    pub grim_hurt_speed: f32,
    /// GrimOld's knockback-frozen fly and timer.
    pub grim_old_speed: f32,
    pub grim_old_timer: f32,
    /// GrimFloat's sine hover.
    pub grim_float_vertical: f32,
    pub grim_float_speed: f32,
    /// GrimEntity's vector fly and the exemption's reach.
    pub grim_entity_speed: f32,
    pub grim_entity_vertical: f32,
    pub grim_entity_distance: f32,
    /// NCP's clip toggle, speed and timer.
    pub ncp_clip: bool,
    pub ncp_speed: f32,
    pub ncp_timer: f32,
    /// OldNCP's preserved horizontal speed.
    pub old_ncp_speed: f32,
    /// NCPBurst's descending hover.
    pub ncp_burst_vertical: f32,
    pub ncp_burst_speed: f32,
    /// Hypixel's explosion curve.
    pub hypixel_speed: f32,
    pub hypixel_vertical: f32,
    pub hypixel_timer: f32,
    pub hypixel_flat: bool,
    /// HypixelBoost's speed ramp and timer.
    pub hypixel_boost_speed: f32,
    pub hypixel_boost_timer: f32,
    /// Vulcan's two descent rates and strafe.
    pub vulcan_down: f32,
    pub vulcan_up: f32,
    pub vulcan_speed: f32,
    /// Verus' ground and air speeds.
    pub verus_ground_speed: f32,
    pub verus_air_speed: f32,
    /// VerusDamage's hover, packet-cancel distance and lifetime.
    pub verus_damage_speed: f32,
    pub verus_damage_vertical: f32,
    pub verus_damage_cancel: f32,
    pub verus_damage_ticks: u32,
    /// Spartan's hover and its vertical packet cadence.
    pub spartan_speed: f32,
    pub spartan_interval: u32,
    pub spartan_offset: f32,
    /// AACGlide's relift period and timer.
    pub aac_glide_ticks: u32,
    pub aac_glide_lift: f32,
    pub aac_glide_timer: f32,
    /// Matrix's setback fly.
    pub matrix_speed: f32,
    pub matrix_vertical: f32,
    /// Damage's hurt-window hover.
    pub damage_speed: f32,
    pub damage_vertical: f32,
    pub damage_ticks: u32,
    pub damage_timer: f32,
    /// 0 is a server hurt, 1 a SetEntityMotion.
    pub damage_trigger: u8,
    /// Sentinel's self-damage boost and hurt-gated speed.
    pub sentinel_speed: f32,
    pub sentinel_vertical: f32,
    pub sentinel_ticks: u32,
    pub sentinel_timer: f32,
    pub sentinel_reboost: u32,
    /// Jetpack's impulse and horizontal acceleration.
    pub jetpack_impulse: f32,
    pub jetpack_speed: f32,
    /// Explosion's amplification and strafe decay.
    pub explosion_vertical: f32,
    pub explosion_speed: f32,
    pub explosion_decrease: f32,
    /// BufferAbuse's speed and status-only packet toggle.
    pub buffer_speed: f32,
    pub buffer_send_flying: bool,
    /// MatrixHighJump's hurt-triggered height and its self-damage start.
    pub highjump_height: f32,
    pub highjump_self_damage: bool,
}

/// Fly's settings, or None when the module is off.
pub fn fly() -> Option<FlyConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == FLY)?;
        if !module.enabled {
            return None;
        }
        Some(FlyConfig {
            mode: FlyMode::from_index(module.choice("mode")),
            vanilla_speed: module.float("vanilla_speed").clamp(0.05, 10.0),
            vanilla_vertical: module.float("vanilla_vertical").clamp(0.0, 10.0),
            vanilla_glide: module.float("vanilla_glide").clamp(-1.0, 1.0),
            vanilla_bypass: module.bool("vanilla_bypass"),
            anti_kick: module.bool("anti_kick"),
            airwalk_speed: module.float("airwalk_speed").clamp(0.05, 10.0),
            airwalk_vertical: module.float("airwalk_vertical").clamp(0.0, 10.0),
            airwalk_ticks: module.int("airwalk_ticks").clamp(1, 20) as u32,
            airwalk_timer: module.float("airwalk_timer").clamp(0.1, 15.0),
            grim_speed: module.float("grim_speed").clamp(0.05, 1.0),
            grim_vertical: module.float("grim_vertical").clamp(0.0, 1.0),
            grim_sink: module.float("grim_sink").clamp(0.0, 0.3),
            grim_hurt_speed: module.float("grim_hurt_speed").clamp(0.05, 1.0),
            grim_old_speed: module.float("grim_old_speed").clamp(0.0, 0.32),
            grim_old_timer: module.float("grim_old_timer").clamp(0.1, 10.0),
            grim_float_vertical: module.float("grim_float_vertical").clamp(0.05, 1.0),
            grim_float_speed: module.float("grim_float_speed").clamp(0.0, 1.0),
            grim_entity_speed: module.float("grim_entity_speed").clamp(0.1, 1.1),
            grim_entity_vertical: module.float("grim_entity_vertical").clamp(0.05, 1.1),
            grim_entity_distance: module.float("grim_entity_distance").clamp(0.5, 1.5),
            ncp_clip: module.bool("ncp_clip"),
            ncp_speed: module.float("ncp_speed").clamp(0.1, 10.0),
            ncp_timer: module.float("ncp_timer").clamp(0.1, 1.0),
            old_ncp_speed: module.float("old_ncp_speed").clamp(0.05, 1.0),
            ncp_burst_vertical: module.float("ncp_burst_vertical").clamp(0.05, 1.0),
            ncp_burst_speed: module.float("ncp_burst_speed").clamp(0.05, 2.0),
            hypixel_speed: module.float("hypixel_speed").clamp(0.1, 5.0),
            hypixel_vertical: module.float("hypixel_vertical").clamp(0.0, 2.0),
            hypixel_timer: module.float("hypixel_timer").clamp(0.1, 1.0),
            hypixel_flat: module.bool("hypixel_flat"),
            hypixel_boost_speed: module.float("hypixel_boost_speed").clamp(0.1, 1.0),
            hypixel_boost_timer: module.float("hypixel_boost_timer").clamp(0.1, 5.0),
            vulcan_down: module.float("vulcan_down").clamp(0.0, 1.0),
            vulcan_up: module.float("vulcan_up").clamp(0.0, 1.0),
            vulcan_speed: module.float("vulcan_speed").clamp(0.05, 1.0),
            verus_ground_speed: module.float("verus_ground_speed").clamp(0.1, 5.0),
            verus_air_speed: module.float("verus_air_speed").clamp(0.1, 5.0),
            verus_damage_speed: module.float("verus_damage_speed").clamp(0.1, 9.5),
            verus_damage_vertical: module.float("verus_damage_vertical").clamp(0.0, 3.0),
            verus_damage_cancel: module.float("verus_damage_cancel").clamp(0.0, 20.0),
            verus_damage_ticks: module.int("verus_damage_ticks").clamp(1, 100) as u32,
            spartan_speed: module.float("spartan_speed").clamp(0.05, 2.0),
            spartan_interval: module.int("spartan_interval").clamp(1, 40) as u32,
            spartan_offset: module.float("spartan_offset").clamp(1.0, 20.0),
            aac_glide_ticks: module.int("aac_glide_ticks").clamp(1, 40) as u32,
            aac_glide_lift: module.float("aac_glide_lift").clamp(0.0, 0.2),
            aac_glide_timer: module.float("aac_glide_timer").clamp(0.05, 1.0),
            matrix_speed: module.float("matrix_speed").clamp(0.1, 10.0),
            matrix_vertical: module.float("matrix_vertical").clamp(0.0, 1.0),
            damage_speed: module.float("damage_speed").clamp(0.1, 9.5),
            damage_vertical: module.float("damage_vertical").clamp(0.0, 3.0),
            damage_ticks: module.int("damage_ticks").clamp(1, 100) as u32,
            damage_timer: module.float("damage_timer").clamp(0.1, 15.0),
            damage_trigger: module.choice("damage_trigger").min(1) as u8,
            sentinel_speed: module.float("sentinel_speed").clamp(0.1, 10.0),
            sentinel_vertical: module.float("sentinel_vertical").clamp(0.05, 5.0),
            sentinel_ticks: module.int("sentinel_ticks").clamp(10, 50) as u32,
            sentinel_timer: module.float("sentinel_timer").clamp(0.1, 5.0),
            sentinel_reboost: module.int("sentinel_reboost").clamp(0, 200) as u32,
            jetpack_impulse: module.float("jetpack_impulse").clamp(0.01, 1.0),
            jetpack_speed: module.float("jetpack_speed").clamp(0.1, 2.0),
            explosion_vertical: module.float("explosion_vertical").clamp(0.0, 10.0),
            explosion_speed: module.float("explosion_speed").clamp(0.6, 4.0),
            explosion_decrease: module.float("explosion_decrease").clamp(0.001, 0.1),
            buffer_speed: module.float("buffer_speed").clamp(0.1, 9.5),
            buffer_send_flying: module.bool("buffer_send_flying"),
            highjump_height: module.float("highjump_height").clamp(0.42, 7.0),
            highjump_self_damage: module.bool("highjump_self_damage"),
        })
    })
}

/// ESP's settings as its box drawing wants them.
#[derive(Clone, Debug, PartialEq)]
pub struct EspConfig {
    pub players: bool,
    pub friends: bool,
    pub self_body: bool,
    pub mobs: bool,
    pub max_distance: f32,
    pub outline: bool,
    /// The colour friends' boxes and lines use, ARGB.
    pub friend_color: u32,
    /// The names `friends` matches, case-insensitively.
    pub friend_names: Vec<String>,
}

/// Tracers' settings as its lines want them.
#[derive(Clone, Debug, PartialEq)]
pub struct TracersConfig {
    pub players: bool,
    pub items: bool,
    /// The line colour, ARGB.
    pub color: u32,
    /// The colour a friend's line uses, ARGB.
    pub friend_color: u32,
    pub friend_names: Vec<String>,
}

/// HoleESP's modes: which wall materials count as a hole.
pub const HOLEESP_MODES: [&str; 3] = ["Both", "Obsidian", "Bedrock"];

/// What a block is to HoleESP's scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoleMaterial {
    /// An empty block the player can stand in.
    Air,
    /// Bedrock: the unbreakable floor and wall.
    Bedrock,
    /// A blast-resistant block: obsidian, crying obsidian, ancient debris...
    Obsidian,
    /// Any other block with a full collision box.
    Solid,
    /// Anything else (fluids, plants, torches...).
    Other,
}

/// What a hole's five blocks are made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoleKind {
    Obsidian,
    Bedrock,
    /// Bedrock and obsidian together.
    Mixed,
}

/// One hole: the feet block of the empty cell, and its material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hole {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// 1 for a single hole, 2 for a double hole along x.
    pub width: i32,
    /// 1 for a single hole, 2 for a double hole along z.
    pub depth: i32,
    pub kind: HoleKind,
}

/// The material each kind is drawn with, in ARGB after the alpha settings.
pub const HOLE_OBSIDIAN_COLOR: u32 = 0xFFFF_5555;
pub const HOLE_BEDROCK_COLOR: u32 = 0xFF55_FF55;
pub const HOLE_MIXED_COLOR: u32 = 0xFFFF_AA00;

/// HoleESP's settings as its scan and drawing want them.
#[derive(Clone, Debug, PartialEq)]
pub struct HoleEspConfig {
    pub mode: HoleMode,
    pub radius: i32,
    pub vertical: i32,
    /// Highlight 1x2 holes as one box, not just 1x1 holes.
    pub doubles: bool,
    pub height: f32,
    pub filled: bool,
    pub outline: bool,
    pub through_walls: bool,
    pub fade: bool,
    pub fill_alpha: u8,
    pub outline_alpha: u8,
    /// The RGB to tint an obsidian, bedrock or mixed hole with. The alpha
    /// comes from `fill_alpha` and `outline_alpha` (and `fade`).
    pub obsidian_color: u32,
    pub bedrock_color: u32,
    pub mixed_color: u32,
    pub ignore_own: bool,
}

impl Default for HoleEspConfig {
    fn default() -> Self {
        Self {
            mode: HoleMode::Both,
            radius: 8,
            vertical: 4,
            doubles: true,
            height: 0.25,
            filled: true,
            outline: true,
            through_walls: true,
            fade: true,
            fill_alpha: 60,
            outline_alpha: 200,
            obsidian_color: HOLE_OBSIDIAN_COLOR,
            bedrock_color: HOLE_BEDROCK_COLOR,
            mixed_color: HOLE_MIXED_COLOR,
            ignore_own: false,
        }
    }
}

/// Which hole materials the module shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HoleMode {
    /// Every hole, mixed ones included.
    #[default]
    Both,
    /// Only holes whose five blocks are all obsidian (or blast-resistant).
    Obsidian,
    /// Only holes whose five blocks are all bedrock.
    Bedrock,
}

impl HoleMode {
    fn keeps(self, kind: HoleKind) -> bool {
        match self {
            Self::Both => true,
            Self::Obsidian => kind == HoleKind::Obsidian,
            Self::Bedrock => kind == HoleKind::Bedrock,
        }
    }
}

/// The holes around `feet`, sealed on all four sides and below. `material`
/// reads the block at a position; the scan only looks at the cells within
/// the configured radii.
pub fn scan_holes(
    feet: [i32; 3],
    config: &HoleEspConfig,
    material: impl Fn(i32, i32, i32) -> HoleMaterial,
) -> Vec<Hole> {
    let radius = config.radius.clamp(1, 32);
    let vertical = config.vertical.clamp(0, 16);
    let mut holes = Vec::new();
    for x in (feet[0] - radius)..=(feet[0] + radius) {
        for z in (feet[2] - radius)..=(feet[2] + radius) {
            for y in (feet[1] - vertical)..=(feet[1] + vertical) {
                if config.ignore_own && [x, y, z] == feet {
                    continue;
                }
                // The player needs the cell and its head space.
                if material(x, y, z) != HoleMaterial::Air
                    || material(x, y + 1, z) != HoleMaterial::Air
                {
                    continue;
                }
                if let Some(kind) = single_hole_kind(&material, x, y, z) {
                    if config.mode.keeps(kind) {
                        holes.push(Hole {
                            x,
                            y,
                            z,
                            width: 1,
                            depth: 1,
                            kind,
                        });
                    }
                    continue;
                }
                if !config.doubles {
                    continue;
                }
                // A 1x2 trench. Only the lower-x or lower-z cell reports it, so
                // each double is found once and never overlaps another.
                for (dx, dz, width, depth) in [(1, 0, 2, 1), (0, 1, 1, 2)] {
                    let Some(kind) = double_hole_kind(&material, x, y, z, dx, dz) else {
                        continue;
                    };
                    if config.ignore_own && [x + dx, y, z + dz] == feet {
                        continue;
                    }
                    if config.mode.keeps(kind) {
                        holes.push(Hole {
                            x,
                            y,
                            z,
                            width,
                            depth,
                            kind,
                        });
                    }
                    break;
                }
            }
        }
    }
    holes
}

/// The 2x2 sealed holes around `feet`, which `scan_holes` deliberately leaves
/// out (its box is a 1x1 or a 1x2). A quad is four empty cells with head space,
/// four sealed floors and eight sealed outer walls. Only the lower-x/lower-z
/// cell can qualify, so each quad is reported once and never overlaps another.
pub fn scan_quads(
    feet: [i32; 3],
    radius: i32,
    vertical: i32,
    ignore_own: bool,
    material: impl Fn(i32, i32, i32) -> HoleMaterial,
) -> Vec<Hole> {
    let radius = radius.clamp(1, 32);
    let vertical = vertical.clamp(0, 16);
    let mut holes = Vec::new();
    for x in (feet[0] - radius)..=(feet[0] + radius) {
        for z in (feet[2] - radius)..=(feet[2] + radius) {
            for y in (feet[1] - vertical)..=(feet[1] + vertical) {
                let cells = [(x, z), (x + 1, z), (x, z + 1), (x + 1, z + 1)];
                if ignore_own && cells.iter().any(|&(cx, cz)| [cx, y, cz] == feet) {
                    continue;
                }
                if cells.iter().any(|&(cx, cz)| {
                    material(cx, y, cz) != HoleMaterial::Air
                        || material(cx, y + 1, cz) != HoleMaterial::Air
                }) {
                    continue;
                }
                let mut walls: Vec<(i32, i32)> = Vec::with_capacity(8);
                for (cx, cz) in cells {
                    for (nx, nz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let wall = (cx + nx, cz + nz);
                        if cells.contains(&wall) || walls.contains(&wall) {
                            continue;
                        }
                        walls.push(wall);
                    }
                }
                let floors = cells.map(|(cx, cz)| material(cx, y - 1, cz));
                let Some(kind) = hole_kind(
                    floors
                        .into_iter()
                        .chain(walls.into_iter().map(|(wx, wz)| material(wx, y, wz))),
                ) else {
                    continue;
                };
                holes.push(Hole {
                    x,
                    y,
                    z,
                    width: 2,
                    depth: 2,
                    kind,
                });
            }
        }
    }
    holes
}

/// A 1x1 hole's kind: the floor and all four sides are bedrock or obsidian.
fn single_hole_kind(
    material: &impl Fn(i32, i32, i32) -> HoleMaterial,
    x: i32,
    y: i32,
    z: i32,
) -> Option<HoleKind> {
    let sides = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    hole_kind(
        std::iter::once(material(x, y - 1, z)).chain(
            sides
                .into_iter()
                .map(|(dx, dz)| material(x + dx, y, z + dz)),
        ),
    )
}

/// A 1x2 hole's kind when `(x, y, z)` and `(x + dx, y, z + dz)` are both
/// empty cells: their two floors and the six blocks around the pair are all
/// bedrock or obsidian.
fn double_hole_kind(
    material: &impl Fn(i32, i32, i32) -> HoleMaterial,
    x: i32,
    y: i32,
    z: i32,
    dx: i32,
    dz: i32,
) -> Option<HoleKind> {
    if material(x + dx, y, z + dz) != HoleMaterial::Air
        || material(x + dx, y + 1, z + dz) != HoleMaterial::Air
    {
        return None;
    }
    let cells = [(x, z), (x + dx, z + dz)];
    let mut walls = Vec::with_capacity(6);
    for (cx, cz) in cells {
        for (nx, nz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let wall = (cx + nx, cz + nz);
            if cells.contains(&wall) || walls.contains(&wall) {
                continue;
            }
            walls.push(wall);
        }
    }
    hole_kind(
        [material(x, y - 1, z), material(x + dx, y - 1, z + dz)]
            .into_iter()
            .chain(walls.into_iter().map(|(wx, wz)| material(wx, y, wz))),
    )
}

/// The kind of a sealed hole whose floor and walls are `materials`: mixed
/// when both materials appear, None if anything else does.
fn hole_kind(materials: impl Iterator<Item = HoleMaterial>) -> Option<HoleKind> {
    let (mut bedrock, mut obsidian) = (false, false);
    for material in materials {
        match material {
            HoleMaterial::Bedrock => bedrock = true,
            HoleMaterial::Obsidian => obsidian = true,
            _ => return None,
        }
    }
    match (bedrock, obsidian) {
        (true, true) => Some(HoleKind::Mixed),
        (true, false) => Some(HoleKind::Bedrock),
        (false, true) => Some(HoleKind::Obsidian),
        (false, false) => None,
    }
}

/// How opaque a hole at `distance` blocks from the camera is: 1 without
/// `fade`, otherwise it falls from 1 at the camera to a fifth at `max`.
pub fn hole_fade(distance: f64, max: f64) -> f32 {
    if max <= 0.0 {
        return 1.0;
    }
    let factor = 1.0 - (distance / max).clamp(0.0, 1.0);
    (0.2 + 0.8 * factor) as f32
}

/// The furthest a hole the settings allow can be from the camera, for the
/// distance fade.
pub fn hole_max_distance(config: &HoleEspConfig) -> f64 {
    let radius = f64::from(config.radius.clamp(1, 32)) + 0.5;
    let vertical = f64::from(config.vertical.clamp(0, 16)) + 1.5;
    (radius * radius * 2.0 + vertical * vertical).sqrt()
}

/// Everything a HoleESP scan depends on: a new tick, player cell or scan
/// setting means the cached holes are stale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HoleScan {
    pub time: i64,
    pub feet: [i32; 3],
    pub mode: HoleMode,
    pub radius: i32,
    pub vertical: i32,
    pub doubles: bool,
    pub ignore_own: bool,
}

impl HoleScan {
    pub fn of(time: i64, feet: [i32; 3], config: &HoleEspConfig) -> Self {
        Self {
            time,
            feet,
            mode: config.mode,
            radius: config.radius,
            vertical: config.vertical,
            doubles: config.doubles,
            ignore_own: config.ignore_own,
        }
    }
}

thread_local! {
    static HOLE_CACHE: RefCell<(Option<HoleScan>, Vec<Hole>)> =
        RefCell::new((None, Vec::new()));
}

/// The last scan's holes, recomputed by `compute` only when `scan` changed.
/// `draw` sees the holes either way.
pub fn with_holes<R>(
    scan: HoleScan,
    compute: impl FnOnce() -> Vec<Hole>,
    draw: impl FnOnce(&[Hole]) -> R,
) -> R {
    HOLE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.0 != Some(scan) {
            *cache = (Some(scan), compute());
        }
        draw(&cache.1)
    })
}

/// The ARGB colour a hole kind is drawn with, from the configured RGB.
pub fn hole_color(config: &HoleEspConfig, kind: HoleKind) -> u32 {
    let rgb = match kind {
        HoleKind::Obsidian => config.obsidian_color,
        HoleKind::Bedrock => config.bedrock_color,
        HoleKind::Mixed => config.mixed_color,
    };
    rgb & 0x00FF_FFFF
}

/// HoleESP's settings, or None when the module is off.
pub fn holeesp() -> Option<HoleEspConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == HOLEESP)?;
        if !module.enabled {
            return None;
        }
        let mode = match module.choice("mode") {
            1 => HoleMode::Obsidian,
            2 => HoleMode::Bedrock,
            _ => HoleMode::Both,
        };
        Some(HoleEspConfig {
            mode,
            radius: module.int("radius").clamp(1, 32),
            vertical: module.int("vertical").clamp(0, 16),
            doubles: module.bool("doubles"),
            height: module.float("height").clamp(0.05, 2.0),
            filled: module.bool("filled"),
            outline: module.bool("outline"),
            through_walls: module.bool("through_walls"),
            fade: module.bool("fade"),
            fill_alpha: module.int("fill_alpha").clamp(0, 255) as u8,
            outline_alpha: module.int("outline_alpha").clamp(0, 255) as u8,
            obsidian_color: parse_color(module.text("obsidian_color"), HOLE_OBSIDIAN_COLOR),
            bedrock_color: parse_color(module.text("bedrock_color"), HOLE_BEDROCK_COLOR),
            mixed_color: parse_color(module.text("mixed_color"), HOLE_MIXED_COLOR),
            ignore_own: module.bool("ignore_own"),
        })
    })
}

/// NoRender's fog modes: leave it alone, clear it everywhere, clear it only
/// while the camera is in lava, or clear it only while the camera is in water.
/// The last one is appended so the numbers of saved configs keep their meaning.
pub const NORENDER_FOG_MODES: [&str; 4] = ["Off", "Clear", "Lava", "Water"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoRenderFogMode {
    #[default]
    Off,
    Clear,
    Lava,
    Water,
}

/// NoRender's settings as its hooks want them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NoRenderConfig {
    pub crosshair: bool,
    pub boss_bar: bool,
    pub scoreboard: bool,
    pub title: bool,
    pub held_item_name: bool,
    pub potion_icons: bool,
    pub toasts: bool,
    pub fog: NoRenderFogMode,
    pub water: bool,
    pub fire: bool,
    pub particles: bool,
    pub nametags: bool,
    pub glowing: bool,
    pub hand: bool,
    pub item_frames: bool,
    pub mobs: bool,
}

impl NoRenderConfig {
    /// Whether the vanilla HUD layer `layer` (the ids of
    /// `native_gui_layer_pre`) is one this configuration hides.
    pub fn hides_layer(&self, layer: &str) -> bool {
        match layer {
            "minecraft:crosshair" => self.crosshair,
            "minecraft:boss_overlay" => self.boss_bar,
            "minecraft:scoreboard_sidebar" => self.scoreboard,
            "minecraft:title" | "minecraft:subtitle_overlay" => self.title,
            "minecraft:selected_item_name" => self.held_item_name,
            "minecraft:effects" => self.potion_icons,
            _ => false,
        }
    }
}

/// NoRender's settings, or None when the module is off.
pub fn norender() -> Option<NoRenderConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == NORENDER)?;
        if !module.enabled {
            return None;
        }
        let fog = match module.choice("fog") {
            1 => NoRenderFogMode::Clear,
            2 => NoRenderFogMode::Lava,
            3 => NoRenderFogMode::Water,
            _ => NoRenderFogMode::Off,
        };
        Some(NoRenderConfig {
            crosshair: module.bool("crosshair"),
            boss_bar: module.bool("boss_bar"),
            scoreboard: module.bool("scoreboard"),
            title: module.bool("title"),
            held_item_name: module.bool("held_item_name"),
            potion_icons: module.bool("potion_icons"),
            toasts: module.bool("toasts"),
            fog,
            water: module.bool("water"),
            fire: module.bool("fire"),
            particles: module.bool("particles"),
            nametags: module.bool("nametags"),
            glowing: module.bool("glowing"),
            hand: module.bool("hand"),
            item_frames: module.bool("item_frames"),
            mobs: module.bool("mobs"),
        })
    })
}

/// Chams' modes: the body as filled boxes, the game's own glow outline in the
/// colour, or both. `Outline` is the model's own silhouette, which is what the
/// game draws for a glowing entity; `Filled` is a figure of boxes, which is the
/// only filled shape the world-primitive hook can draw.
pub const CHAMS_MODES: [&str; 3] = ["Filled", "Outline", "Both"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChamsMode {
    #[default]
    Filled,
    Outline,
    Both,
}

impl ChamsMode {
    /// Whether the mode draws the filled boxes.
    pub fn fills(self) -> bool {
        matches!(self, Self::Filled | Self::Both)
    }
    /// Whether the mode asks the game for its glow outline.
    pub fn outlines(self) -> bool {
        matches!(self, Self::Outline | Self::Both)
    }
}

/// Chams' settings as its hooks want them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChamsConfig {
    pub mode: ChamsMode,
    pub players: bool,
    pub self_body: bool,
    pub monsters: bool,
    pub animals: bool,
    /// End crystals, which are not mobs and so are drawn as a box of their own.
    pub crystals: bool,
    pub invisibles: bool,
    pub through_walls: bool,
    pub range: f64,
    /// The ARGB colour of both the boxes and the outline.
    pub color: u32,
}

/// Chams' settings, or None when the module is off.
pub fn chams() -> Option<ChamsConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == CHAMS)?;
        if !module.enabled {
            return None;
        }
        let mode = match module.choice("mode") {
            1 => ChamsMode::Outline,
            2 => ChamsMode::Both,
            _ => ChamsMode::Filled,
        };
        Some(ChamsConfig {
            mode,
            players: module.bool("players"),
            self_body: module.bool("self"),
            monsters: module.bool("monsters"),
            animals: module.bool("animals"),
            crystals: module.bool("crystals"),
            invisibles: module.bool("invisibles"),
            through_walls: module.bool("through_walls"),
            range: f64::from(module.int("range")).clamp(8.0, 256.0),
            color: parse_color(module.text("color"), 0x3CFF_0000),
        })
    })
}

/// One hand's transform, as `native_render_hand_pose` takes it: an offset in
/// blocks, pitch/yaw/roll in degrees, and one scale for the whole hand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandPose {
    pub scale: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

impl HandPose {
    /// What the game draws with no help: no offset, no turn, scale one.
    pub const IDENTITY: Self = Self {
        scale: 1.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
        pitch: 0.0,
        yaw: 0.0,
        roll: 0.0,
    };

    /// Whether this leaves the hand exactly where the game put it.
    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }
}

impl Default for HandPose {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// ViewModel's settings: one pose for each hand, and one for a bare arm.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewModelConfig {
    pub main: HandPose,
    pub off: HandPose,
    /// Added to the hand's own pose when that hand holds nothing: the arm the
    /// references give its own group.
    pub arm: HandPose,
}

/// ViewModel's settings, or None when the module is off.
pub fn viewmodel() -> Option<ViewModelConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == VIEWMODEL)?;
        if !module.enabled {
            return None;
        }
        Some(ViewModelConfig {
            main: HandPose {
                scale: module.float("main_scale").clamp(0.1, 3.0),
                x: module.float("main_x").clamp(-1.0, 1.0),
                y: module.float("main_y").clamp(-1.0, 1.0),
                z: module.float("main_z").clamp(-1.0, 1.0),
                pitch: module.float("main_pitch").clamp(-180.0, 180.0),
                yaw: module.float("main_yaw").clamp(-180.0, 180.0),
                roll: module.float("main_roll").clamp(-180.0, 180.0),
            },
            off: HandPose {
                scale: module.float("off_scale").clamp(0.1, 3.0),
                x: module.float("off_x").clamp(-1.0, 1.0),
                y: module.float("off_y").clamp(-1.0, 1.0),
                z: module.float("off_z").clamp(-1.0, 1.0),
                pitch: module.float("off_pitch").clamp(-180.0, 180.0),
                yaw: module.float("off_yaw").clamp(-180.0, 180.0),
                roll: module.float("off_roll").clamp(-180.0, 180.0),
            },
            arm: HandPose {
                scale: module.float("arm_scale").clamp(0.1, 3.0),
                x: module.float("arm_x").clamp(-1.0, 1.0),
                y: module.float("arm_y").clamp(-1.0, 1.0),
                z: module.float("arm_z").clamp(-1.0, 1.0),
                pitch: module.float("arm_pitch").clamp(-180.0, 180.0),
                yaw: module.float("arm_yaw").clamp(-180.0, 180.0),
                roll: module.float("arm_roll").clamp(-180.0, 180.0),
            },
        })
    })
}

/// Animations' settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationsConfig {
    /// The swing's duration in percent, the reference client's `Swing Speed`:
    /// 50 halves it, -200 triples it, and 0 leaves it as the game has it.
    pub swing_speed: f32,
    /// How much further the hand travels than the vanilla swing, in percent
    /// of the extra arc: 0 is the vanilla swing on its own.
    pub sway: f32,
    /// Whether an item in use is brought to the mouth.
    pub eating: bool,
}

impl Default for AnimationsConfig {
    fn default() -> Self {
        Self {
            swing_speed: 0.0,
            sway: 0.0,
            eating: true,
        }
    }
}

impl AnimationsConfig {
    /// Whether every setting is what the game does by itself.
    pub fn is_vanilla(&self) -> bool {
        *self == Self::default()
    }
}

/// Animations' settings, or None when the module is off.
pub fn animations() -> Option<AnimationsConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == ANIMATIONS)?;
        if !module.enabled {
            return None;
        }
        Some(AnimationsConfig {
            swing_speed: module.float("swing_speed").clamp(-200.0, 50.0),
            sway: module.float("sway").clamp(0.0, 200.0),
            eating: module.bool("eating"),
        })
    })
}

/// Blink's settings: whether the held packets go out on a timer, how many
/// ticks that timer runs, and the marker that shows where the server still
/// has the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlinkConfig {
    pub pulse: bool,
    pub delay: i32,
    pub marker: bool,
    pub marker_color: u32,
    pub marker_walls: bool,
}

impl Default for BlinkConfig {
    fn default() -> Self {
        Self {
            pulse: false,
            delay: 20,
            marker: true,
            marker_color: 0x60FF_AA00,
            marker_walls: true,
        }
    }
}

/// Blink's settings, or None when the module is off.
pub fn blink() -> Option<BlinkConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == BLINK)?;
        if !module.enabled {
            return None;
        }
        Some(BlinkConfig {
            pulse: module.bool("pulse"),
            delay: module.int("delay").clamp(2, 200),
            marker: module.bool("marker"),
            marker_color: parse_color(module.text("marker_color"), 0x60FF_AA00),
            marker_walls: module.bool("marker_walls"),
        })
    })
}

/// The item OffHand keeps in the off hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffHandItem {
    Totem,
    GApple,
    Crystal,
}

impl OffHandItem {
    /// The item the `Item` choice names. An index past the list is the totem,
    /// the safe default.
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::GApple,
            2 => Self::Crystal,
            _ => Self::Totem,
        }
    }
}

/// OffHand's settings, flattened out of the registry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OffHandConfig {
    pub item: OffHandItem,
    /// Below this health the totem replaces the preferred item.
    pub health: f32,
    /// Count absorption towards the health the gates read.
    pub absorption: bool,
    /// Only keep a gapple while the main hand holds a sword.
    pub sword_only: bool,
    /// Eat a gapple that is in the off hand while the health is low.
    pub eat: bool,
    /// The health at or below which the gapple is eaten.
    pub eat_at: f32,
    /// A fall that would deal this much damage calls for a totem.
    pub fall: bool,
    /// Gliding calls for a totem.
    pub elytra: bool,
    /// A nearby end crystal or primed TNT calls for a totem.
    pub explosion: bool,
    /// Search the hotbar as well as the main inventory.
    pub hotbar: bool,
    /// Ticks between clicks.
    pub delay: u32,
}

impl Default for OffHandConfig {
    /// The registry's own defaults, in one place for the tests to check.
    fn default() -> Self {
        Self {
            item: OffHandItem::Totem,
            health: 10.0,
            absorption: true,
            sword_only: false,
            eat: true,
            eat_at: 15.0,
            fall: true,
            elytra: true,
            explosion: true,
            hotbar: false,
            delay: 2,
        }
    }
}

/// OffHand's settings, or None when the module is off.
pub fn offhand() -> Option<OffHandConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == OFFHAND)?;
        if !module.enabled {
            return None;
        }
        Some(OffHandConfig {
            item: OffHandItem::from_index(module.choice("item")),
            health: module.float("health").clamp(0.0, 20.0),
            absorption: module.bool("absorption"),
            sword_only: module.bool("sword_only"),
            eat: module.bool("eat"),
            eat_at: module.float("eat_at").clamp(0.0, 20.0),
            fall: module.bool("fall"),
            elytra: module.bool("elytra"),
            explosion: module.bool("explosion"),
            hotbar: module.bool("hotbar"),
            delay: module.int("delay").clamp(0, 20) as u32,
        })
    })
}

/// One of the four armour slots AutoArmor fills, in the order `Game::armor`
/// keeps them (head first).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmorSlot {
    Helmet,
    Chest,
    Legs,
    Boots,
}

impl ArmorSlot {
    /// The index in `Game::armor`.
    pub fn index(self) -> usize {
        match self {
            Self::Helmet => 0,
            Self::Chest => 1,
            Self::Legs => 2,
            Self::Boots => 3,
        }
    }

    /// The slot's id in the game's numbering (`slot_view`, `menu_slot`):
    /// `InventoryMenu`'s armour slots are 5..9, and 500..504 here.
    pub fn slot(self) -> usize {
        500 + self.index()
    }

    /// The `equippable.slot` name the item data uses.
    pub fn name(self) -> &'static str {
        match self {
            Self::Helmet => "head",
            Self::Chest => "chest",
            Self::Legs => "legs",
            Self::Boots => "feet",
        }
    }

    /// The slot an `equippable.slot` name means, or None for a name that is
    /// not one of the four (`offhand`, `mainhand`, `body`, `saddle`).
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "head" => Some(Self::Helmet),
            "chest" => Some(Self::Chest),
            "legs" => Some(Self::Legs),
            "feet" => Some(Self::Boots),
            _ => None,
        }
    }
}

/// AutoArmor's mode: whether a worn piece can be replaced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AutoArmorMode {
    /// A carried piece replaces a worn one whenever it is better.
    #[default]
    Best,
    /// Only a slot nothing is worn in is filled.
    Empty,
}

impl AutoArmorMode {
    /// The mode the `Mode` choice names. An index past the list is `Best`.
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Empty,
            _ => Self::Best,
        }
    }
}

/// What AutoArmor compares two pieces for the same slot by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AutoArmorCompare {
    /// Armour points first, then the protection enchantment.
    #[default]
    Armor,
    /// The protection enchantment first, then armour points.
    Protection,
}

impl AutoArmorCompare {
    /// The comparison the `Compare` choice names. An index past the list is
    /// `Armor`, the one every reference client starts on.
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Protection,
            _ => Self::Armor,
        }
    }
}

/// AutoArmor's settings, flattened out of the registry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoArmorConfig {
    pub mode: AutoArmorMode,
    pub compare: AutoArmorCompare,
    /// A carried piece with less than this much durability left is left
    /// alone, as a percentage of its maximum.
    pub min_durability: i32,
    /// Ticks between clicks.
    pub delay: u32,
}

impl Default for AutoArmorConfig {
    /// The registry's own defaults, in one place for the tests to check.
    fn default() -> Self {
        Self {
            mode: AutoArmorMode::Best,
            compare: AutoArmorCompare::Armor,
            min_durability: 5,
            delay: 2,
        }
    }
}

/// AutoArmor's settings, or None when the module is off.
pub fn autoarmor() -> Option<AutoArmorConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == AUTOARMOR)?;
        if !module.enabled {
            return None;
        }
        Some(AutoArmorConfig {
            mode: AutoArmorMode::from_index(module.choice("mode")),
            compare: AutoArmorCompare::from_index(module.choice("compare")),
            min_durability: module.int("min_durability").clamp(0, 100),
            delay: module.int("delay").clamp(0, 20) as u32,
        })
    })
}

/// AutoXP's hotbar behaviour: `Silent` restores the selected slot in the same
/// tick, `Normal` leaves the bottles in hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoXpMode {
    Silent,
    Normal,
}

impl AutoXpMode {
    /// The mode the `Mode` choice names. An index past the list is `Silent`.
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Normal,
            _ => Self::Silent,
        }
    }

    /// Whether the old hotbar slot is sent back after the throws.
    pub fn silent(self) -> bool {
        self == Self::Silent
    }
}

/// AutoXP's settings, flattened out of the registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoXpConfig {
    pub mode: AutoXpMode,
    /// Ticks between throw cycles.
    pub delay: u32,
    /// `UseItem` packets per cycle.
    pub frequency: u32,
    /// Aim the throw straight down instead of along the player's view.
    pub rotate: bool,
    /// With this off, only throw while the crosshair is on a solid block.
    pub air: bool,
    /// Restock the hotbar from the main inventory with one menu swap.
    pub inventory: bool,
    /// Stop when nothing has damage that Mending could repair.
    pub stop: bool,
    /// Repair while durability is below this percentage.
    pub stop_at: i32,
    /// Turn the module off when `Stop` says there is nothing left to do.
    pub auto_disable: bool,
    /// Include the main and off hand in the `Stop` check.
    pub tools: bool,
}

impl Default for AutoXpConfig {
    /// The registry's own defaults, in one place for the tests to check.
    fn default() -> Self {
        Self {
            mode: AutoXpMode::Silent,
            delay: 2,
            frequency: 1,
            rotate: false,
            air: true,
            inventory: false,
            stop: true,
            stop_at: 100,
            auto_disable: false,
            tools: false,
        }
    }
}

/// AutoXP's settings, or None when the module is off.
pub fn autoxp() -> Option<AutoXpConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == AUTOXP)?;
        if !module.enabled {
            return None;
        }
        Some(AutoXpConfig {
            mode: AutoXpMode::from_index(module.choice("mode")),
            delay: module.int("delay").clamp(0, 20) as u32,
            frequency: module.int("frequency").clamp(1, 16) as u32,
            rotate: module.bool("rotate"),
            air: module.bool("air"),
            inventory: module.bool("inventory"),
            stop: module.bool("stop"),
            stop_at: module.int("stop_at").clamp(80, 100),
            auto_disable: module.bool("auto_disable"),
            tools: module.bool("tools"),
        })
    })
}

/// What HoleFill will take from the hotbar. `Any` accepts any placeable block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoleFillBlock {
    Obsidian,
    EnderChest,
    Any,
}

impl HoleFillBlock {
    /// The block the `Block` choice names. An index past the list is obsidian.
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::EnderChest,
            2 => Self::Any,
            _ => Self::Obsidian,
        }
    }
}

/// HoleFill's settings, flattened out of the registry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoleFillConfig {
    pub block: HoleFillBlock,
    /// Horizontal scan and placement radius, in blocks.
    pub range: i32,
    /// How far above and below the player's feet the scan looks.
    pub vertical: i32,
    pub doubles: bool,
    pub quads: bool,
    /// `UseItemOn` packets in one cycle.
    pub per_tick: u32,
    /// Ticks between cycles.
    pub delay: u32,
    /// Whether the cell the player stands in may be filled.
    pub self_fill: bool,
    /// Skip holes closer than this to the player, unless the player is in a
    /// hole. Zero disables the check.
    pub min_self: f32,
    pub require_target: bool,
    pub target_range: f32,
    /// Only fill a hole a nearby player is moving toward.
    pub smart: bool,
    pub auto_disable: bool,
    /// Put the old hotbar slot back in the same tick.
    pub silent: bool,
}

impl Default for HoleFillConfig {
    /// The registry's own defaults, in one place for the tests to check.
    fn default() -> Self {
        Self {
            block: HoleFillBlock::Obsidian,
            range: 4,
            vertical: 3,
            doubles: false,
            quads: false,
            per_tick: 1,
            delay: 2,
            self_fill: false,
            min_self: 0.0,
            require_target: false,
            target_range: 8.0,
            smart: false,
            auto_disable: false,
            silent: true,
        }
    }
}

/// HoleFill's settings, or None when the module is off.
pub fn holefill() -> Option<HoleFillConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == HOLEFILL)?;
        if !module.enabled {
            return None;
        }
        Some(HoleFillConfig {
            block: HoleFillBlock::from_index(module.choice("block")),
            range: module.int("range").clamp(1, 5),
            vertical: module.int("vertical").clamp(0, 6),
            doubles: module.bool("doubles"),
            quads: module.bool("quads"),
            per_tick: module.int("per_tick").clamp(1, 4) as u32,
            delay: module.int("delay").clamp(0, 20) as u32,
            self_fill: module.bool("self_fill"),
            min_self: module.float("min_self").clamp(0.0, 6.0),
            require_target: module.bool("require_target"),
            target_range: module.float("target_range").clamp(1.0, 16.0),
            smart: module.bool("smart"),
            auto_disable: module.bool("auto_disable"),
            silent: module.bool("silent"),
        })
    })
}

/// Reach's settings, flattened out of the registry.
///
/// The reference randomises its reach between two bounds rather than holding
/// one value, so the pick never sits at a constant maximum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReachConfig {
    pub min: f32,
    pub max: f32,
}

impl Default for ReachConfig {
    /// The registry's own defaults, in one place for the tests to check.
    fn default() -> Self {
        Self {
            min: 3.0,
            max: 4.0,
        }
    }
}

impl ReachConfig {
    /// The reach for one tick: uniform between the bounds, in either order.
    pub fn pick(&self, roll: f32) -> f32 {
        let (low, high) = if self.min <= self.max {
            (self.min, self.max)
        } else {
            (self.max, self.min)
        };
        low + (high - low) * roll.clamp(0.0, 1.0)
    }
}

/// Reach's settings, or None when the module is off.
pub fn reach() -> Option<ReachConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == REACH)?;
        if !module.enabled {
            return None;
        }
        Some(ReachConfig {
            min: module.float("min").clamp(3.0, 6.0),
            max: module.float("max").clamp(3.0, 6.0),
        })
    })
}

/// AutoClicker's two timing shapes: a steady click rate, or the drag-click
/// burst the references simulate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoClickerMode {
    Normal,
    Drag,
}

impl AutoClickerMode {
    /// The mode the `Mode` choice names. An index past the list is `Normal`.
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Drag,
            _ => Self::Normal,
        }
    }
}

/// AutoClicker's settings, flattened out of the registry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoClickerConfig {
    pub mode: AutoClickerMode,
    pub left: bool,
    pub right: bool,
    /// Clicks per second, uniform between the two while in `Normal`.
    pub min_cps: f32,
    pub max_cps: f32,
    /// Clicks in one drag burst.
    pub drag_length: i32,
    /// Ticks between bursts.
    pub drag_delay: i32,
    /// Only click while the button is actually held.
    pub on_click: bool,
    /// Skip a left click while the crosshair is on a block within reach.
    pub stop_on_block: bool,
    /// Skip a left click unless a sword or an axe is held.
    pub weapon_only: bool,
}

impl Default for AutoClickerConfig {
    /// The registry's own defaults, in one place for the tests to check.
    fn default() -> Self {
        Self {
            mode: AutoClickerMode::Normal,
            left: true,
            right: false,
            min_cps: 8.0,
            max_cps: 14.0,
            drag_length: 17,
            drag_delay: 6,
            on_click: false,
            stop_on_block: true,
            weapon_only: false,
        }
    }
}

impl AutoClickerConfig {
    /// The clicks per second for one click: uniform between the bounds, in
    /// either order, never below one.
    pub fn cps(&self, roll: f32) -> f32 {
        let (low, high) = if self.min_cps <= self.max_cps {
            (self.min_cps, self.max_cps)
        } else {
            (self.max_cps, self.min_cps)
        };
        (low + (high - low) * roll.clamp(0.0, 1.0)).max(1.0)
    }

    /// Milliseconds between clicks at `cps`.
    pub fn interval_ms(cps: f32) -> i64 {
        (1000.0 / cps.max(1.0)).round().max(1.0) as i64
    }
}

/// AutoClicker's settings, or None when the module is off.
pub fn autoclicker() -> Option<AutoClickerConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == AUTOCLICKER)?;
        if !module.enabled {
            return None;
        }
        Some(AutoClickerConfig {
            mode: AutoClickerMode::from_index(module.choice("mode")),
            left: module.bool("left"),
            right: module.bool("right"),
            min_cps: module.float("min_cps").clamp(1.0, 20.0),
            max_cps: module.float("max_cps").clamp(1.0, 20.0),
            drag_length: module.int("drag_length").clamp(1, 50),
            drag_delay: module.int("drag_delay").clamp(1, 20),
            on_click: module.bool("on_click"),
            stop_on_block: module.bool("stop_on_block"),
            weapon_only: module.bool("weapon_only"),
        })
    })
}

/// Where AimAssist points on a target: the eye line, the chest, or the legs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AimPoint {
    Head,
    Chest,
    Legs,
}

impl AimPoint {
    /// The point the `Aim at` choice names. An index past the list is `Head`.
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Chest,
            2 => Self::Legs,
            _ => Self::Head,
        }
    }
}

/// AimAssist's settings, flattened out of the registry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimAssistConfig {
    pub point: AimPoint,
    pub range: f32,
    /// The full cone, in degrees: half of it either side of the view.
    pub fov: f32,
    /// Maximum degrees the view turns toward the target in one tick.
    pub speed: f32,
    pub yaw: bool,
    pub pitch: bool,
    pub players: bool,
    pub mobs: bool,
    pub invisibles: bool,
    pub through_walls: bool,
    pub weapon_only: bool,
    /// Only turn while the attack button is held.
    pub click_aim: bool,
    /// Stop turning while the crosshair is already on the target.
    pub stop_on_target: bool,
    /// Keep the chosen target while it stays valid.
    pub sticky: bool,
}

impl Default for AimAssistConfig {
    /// The registry's own defaults, in one place for the tests to check.
    fn default() -> Self {
        Self {
            point: AimPoint::Head,
            range: 4.5,
            fov: 90.0,
            speed: 4.0,
            yaw: true,
            pitch: false,
            players: true,
            mobs: false,
            invisibles: false,
            through_walls: false,
            weapon_only: false,
            click_aim: false,
            stop_on_target: false,
            sticky: false,
        }
    }
}

/// AimAssist's settings, or None when the module is off.
pub fn aimassist() -> Option<AimAssistConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == AIMASSIST)?;
        if !module.enabled {
            return None;
        }
        Some(AimAssistConfig {
            point: AimPoint::from_index(module.choice("point")),
            range: module.float("range").clamp(1.0, 8.0),
            fov: module.float("fov").clamp(30.0, 360.0),
            speed: module.float("speed").clamp(0.5, 20.0),
            yaw: module.bool("yaw"),
            pitch: module.bool("pitch"),
            players: module.bool("players"),
            mobs: module.bool("mobs"),
            invisibles: module.bool("invisibles"),
            through_walls: module.bool("through_walls"),
            weapon_only: module.bool("weapon_only"),
            click_aim: module.bool("click_aim"),
            stop_on_target: module.bool("stop_on_target"),
            sticky: module.bool("sticky"),
        })
    })
}

/// The friend names ESP and Tracers share, read straight from the registry so
/// AimAssist can honour them even while ESP is off.
pub fn friend_names() -> Vec<String> {
    with(|c| {
        c.modules
            .iter()
            .find(|m| m.id == ESP)
            .map(|m| parse_names(m.text("friend_names")))
            .unwrap_or_default()
    })
}

/// What Surround takes from the hotbar. The names match HoleFill's choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurroundBlock {
    Obsidian,
    EnderChest,
    Any,
}

impl SurroundBlock {
    /// The block the `Block` choice names. An index past the list is obsidian.
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::EnderChest,
            2 => Self::Any,
            _ => Self::Obsidian,
        }
    }
}

/// Surround's settings, flattened out of the registry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurroundConfig {
    pub block: SurroundBlock,
    /// `UseItemOn` packets in one cycle.
    pub per_tick: u32,
    /// Ticks between cycles.
    pub delay: u32,
    /// Clamp the player toward the middle of their cell before placing.
    pub center: bool,
    /// Also place a block under the feet.
    pub flatten: bool,
    /// Also place a block above the head.
    pub cover: bool,
    /// Do nothing while airborne.
    pub on_ground: bool,
    /// Put the old hotbar slot back in the same tick.
    pub silent: bool,
    /// Turn off once every surrounding cell is solid.
    pub auto_disable: bool,
}

impl Default for SurroundConfig {
    /// The registry's own defaults, in one place for the tests to check.
    fn default() -> Self {
        Self {
            block: SurroundBlock::Obsidian,
            per_tick: 2,
            delay: 2,
            center: false,
            flatten: false,
            cover: false,
            on_ground: true,
            silent: true,
            auto_disable: false,
        }
    }
}

/// Surround's settings, or None when the module is off.
pub fn surround() -> Option<SurroundConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == SURROUND)?;
        if !module.enabled {
            return None;
        }
        Some(SurroundConfig {
            block: SurroundBlock::from_index(module.choice("block")),
            per_tick: module.int("per_tick").clamp(1, 4) as u32,
            delay: module.int("delay").clamp(0, 20) as u32,
            center: module.bool("center"),
            flatten: module.bool("flatten"),
            cover: module.bool("cover"),
            on_ground: module.bool("on_ground"),
            silent: module.bool("silent"),
            auto_disable: module.bool("auto_disable"),
        })
    })
}

/// CrystalAura's settings, flattened out of the registry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrystalAuraConfig {
    pub place: bool,
    pub place_delay: u32,
    pub place_range: f32,
    pub breaking: bool,
    pub break_delay: u32,
    pub break_range: f32,
    /// Pre-armor explosion damage the placement or break must deal to the
    /// target. The references' sliders are in post-armor units because they
    /// compute armor-aware damage; this module does not, so the numbers here
    /// are the raw blast value and the range has to reach it.
    pub min_damage: f32,
    /// Pre-armor explosion damage it may deal to the player.
    pub max_self_damage: f32,
    /// Place even when the blast hurts the player as much as the target, as long
    /// as it would not kill them. On flat ground both stand the same distance
    /// from the crystal, so the absolute cap alone rejects every placement.
    pub allow_trades: bool,
    /// How far a target may be.
    pub target_range: f32,
    /// Attack a crystal the tick it first appears, ignoring the break delay.
    pub instant: bool,
    /// Turn the view toward the placement base or the crystal.
    pub rotate: bool,
    /// Put the old hotbar slot back in the same tick.
    pub silent: bool,
    /// Print the scan result to chat once a second.
    pub debug: bool,
}

impl Default for CrystalAuraConfig {
    /// The registry's own defaults, in one place for the tests to check.
    fn default() -> Self {
        Self {
            place: true,
            place_delay: 0,
            place_range: 4.5,
            breaking: true,
            break_delay: 0,
            break_range: 4.5,
            min_damage: 15.0,
            max_self_damage: 40.0,
            allow_trades: true,
            target_range: 10.0,
            instant: true,
            rotate: false,
            silent: true,
            debug: false,
        }
    }
}

/// CrystalAura's settings, or None when the module is off.
pub fn crystal_aura() -> Option<CrystalAuraConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == CRYSTALAURA)?;
        if !module.enabled {
            return None;
        }
        Some(CrystalAuraConfig {
            place: module.bool("place"),
            place_delay: module.int("place_delay").clamp(0, 10) as u32,
            place_range: module.float("place_range").clamp(1.0, 6.0),
            breaking: module.bool("breaking"),
            break_delay: module.int("break_delay").clamp(0, 10) as u32,
            break_range: module.float("break_range").clamp(1.0, 6.0),
            min_damage: module.float("min_damage").clamp(2.0, 100.0),
            max_self_damage: module.float("max_self_damage").clamp(2.0, 100.0),
            allow_trades: module.bool("allow_trades"),
            target_range: module.float("target_range").clamp(4.0, 16.0),
            instant: module.bool("instant"),
            rotate: module.bool("rotate"),
            silent: module.bool("silent"),
            debug: module.bool("debug"),
        })
    })
}

/// The times of day Ambience fixes the world at. `Off` leaves the server's
/// time alone; `Real time` reads the wall clock where the player is.
pub const AMBIENCE_TIME_MODES: [&str; 9] = [
    "Off",
    "Dawn",
    "Morning",
    "Noon",
    "Sunset",
    "Evening",
    "Night",
    "Midnight",
    "Real time",
];

/// Ambience's time of day.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AmbienceTime {
    #[default]
    Off,
    Dawn,
    Morning,
    Noon,
    Sunset,
    Evening,
    Night,
    Midnight,
    RealTime,
}

impl AmbienceTime {
    /// The ticks of the day this mode fixes, or None for the two modes that
    /// fix none: `Off`, and `Real time`, which reads the clock instead.
    pub fn ticks(self) -> Option<i64> {
        match self {
            Self::Off | Self::RealTime => None,
            Self::Dawn => Some(23_000),
            Self::Morning => Some(1_000),
            Self::Noon => Some(6_000),
            Self::Sunset => Some(12_000),
            Self::Evening => Some(13_000),
            Self::Night => Some(15_000),
            Self::Midnight => Some(18_000),
        }
    }

    /// Whether this mode is the clock's time rather than a fixed one.
    pub fn real_time(self) -> bool {
        self == Self::RealTime
    }
}

/// The weathers Ambience forces. `Off` leaves the server's weather alone.
pub const AMBIENCE_WEATHER_MODES: [&str; 4] = ["Off", "Clear", "Rain", "Thunder"];

/// Ambience's weather.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AmbienceWeather {
    #[default]
    Off,
    Clear,
    Rain,
    Thunder,
}

impl AmbienceWeather {
    /// The rain and thunder flags this mode forces, or None for `Off`.
    pub fn levels(self) -> Option<(bool, bool)> {
        match self {
            Self::Off => None,
            Self::Clear => Some((false, false)),
            Self::Rain => Some((true, false)),
            Self::Thunder => Some((true, true)),
        }
    }
}

/// Ambience's fog modes.
pub const AMBIENCE_FOG_MODES: [&str; 2] = ["Off", "Override"];

/// Ambience's fog. `Off` leaves the frame's own fog alone; `Override` replaces
/// its colour and where it starts and ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AmbienceFog {
    #[default]
    Off,
    Override,
}

impl AmbienceFog {
    /// Whether this mode replaces the frame's fog.
    pub fn overrides(self) -> bool {
        self == Self::Override
    }
}

/// Ambience's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmbienceConfig {
    pub time: AmbienceTime,
    pub weather: AmbienceWeather,
    pub fog: AmbienceFog,
    /// The fog's colour, 0xAARRGGBB; the alpha is unused.
    pub fog_color: u32,
    /// Where the fog starts, as a fraction of the view distance.
    pub fog_start: f32,
    /// Where it ends, the same way.
    pub fog_end: f32,
}

impl Default for AmbienceConfig {
    fn default() -> Self {
        Self {
            time: AmbienceTime::Off,
            weather: AmbienceWeather::Off,
            fog: AmbienceFog::Off,
            fog_color: 0xFF6E_7FD8,
            fog_start: 0.5,
            fog_end: 1.0,
        }
    }
}

impl AmbienceConfig {
    /// Whether every setting is what the game does by itself.
    pub fn is_vanilla(&self) -> bool {
        *self == Self::default()
    }
}

/// The three components of an 0xAARRGGBB colour, 0..=1.
pub fn rgb(color: u32) -> [f32; 3] {
    [
        ((color >> 16) & 0xFF) as f32 / 255.0,
        ((color >> 8) & 0xFF) as f32 / 255.0,
        (color & 0xFF) as f32 / 255.0,
    ]
}

/// Ambience's settings, or None when the module is off.
pub fn ambience() -> Option<AmbienceConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == AMBIENCE)?;
        if !module.enabled {
            return None;
        }
        Some(AmbienceConfig {
            time: match module.choice("time") {
                1 => AmbienceTime::Dawn,
                2 => AmbienceTime::Morning,
                3 => AmbienceTime::Noon,
                4 => AmbienceTime::Sunset,
                5 => AmbienceTime::Evening,
                6 => AmbienceTime::Night,
                7 => AmbienceTime::Midnight,
                8 => AmbienceTime::RealTime,
                _ => AmbienceTime::Off,
            },
            weather: match module.choice("weather") {
                1 => AmbienceWeather::Clear,
                2 => AmbienceWeather::Rain,
                3 => AmbienceWeather::Thunder,
                _ => AmbienceWeather::Off,
            },
            fog: match module.choice("fog") {
                1 => AmbienceFog::Override,
                _ => AmbienceFog::Off,
            },
            fog_color: parse_color(module.text("fog_color"), 0xFF6E_7FD8),
            fog_start: module.float("fog_start").clamp(0.1, 1.5),
            fog_end: module.float("fog_end").clamp(0.1, 1.5),
        })
    })
}

/// ESP's settings, or None when the module is off.
pub fn esp() -> Option<EspConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == ESP)?;
        if !module.enabled {
            return None;
        }
        Some(EspConfig {
            players: module.bool("players"),
            friends: module.bool("friends"),
            self_body: module.bool("self"),
            mobs: module.bool("mobs"),
            max_distance: module.float("max_distance").clamp(8.0, 200.0),
            outline: module.bool("outline"),
            friend_color: parse_color(module.text("friend_color"), DEFAULT_FRIEND_COLOR),
            friend_names: parse_names(module.text("friend_names")),
        })
    })
}

/// Tracers' settings, or None when the module is off.
pub fn tracers() -> Option<TracersConfig> {
    with(|c| {
        let module = c.modules.iter().find(|m| m.id == TRACERS)?;
        if !module.enabled {
            return None;
        }
        Some(TracersConfig {
            players: module.bool("players"),
            items: module.bool("items"),
            color: parse_color(module.text("color"), 0xFFFF_FFFF),
            friend_color: parse_color(module.text("friend_color"), 0xFFFF_FFFF),
            friend_names: parse_names(module.text("friend_names")),
        })
    })
}

/// The ESP friend colour the port starts with (the reference's -13303968).
pub const DEFAULT_FRIEND_COLOR: u32 = 0xFF34_FF60;

/// Whether `name` is on a module's friend list (case-insensitive).
pub fn is_friend(names: &[String], name: &str) -> bool {
    names.iter().any(|n| n.eq_ignore_ascii_case(name))
}

/// Splits a comma-separated player-name list, dropping blanks.
pub fn parse_names(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Reads a colour setting: `#RRGGBB`, `#AARRGGBB`, the same without the `#`,
/// or a decimal ARGB integer (negative Java values included). Anything else
/// falls back to `fallback`.
pub fn parse_color(text: &str, fallback: u32) -> u32 {
    let text = text.trim();
    let hex = text.strip_prefix('#').unwrap_or(text);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(rgb) = u32::from_str_radix(hex, 16) {
            return 0xFF00_0000 | rgb;
        }
    }
    if hex.len() == 8 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(argb) = u32::from_str_radix(hex, 16) {
            return argb;
        }
    }
    if let Ok(number) = text.parse::<i64>()
        && (i64::from(i32::MIN)..=i64::from(u32::MAX)).contains(&number)
    {
        return number as u32;
    }
    fallback
}

/// Splits a block list (comma, semicolon or newline separated) into
/// namespaced ids (`oak_log` becomes `minecraft:oak_log`), dropping blanks.
pub fn parse_blocks(text: &str) -> Vec<String> {
    text.split([',', ';', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            let s = s.to_ascii_lowercase();
            if s.contains(':') {
                s
            } else {
                format!("minecraft:{s}")
            }
        })
        .collect()
}

/// Filter (0 Blacklist, 1 Whitelist) on a block list: whether `block` passes,
/// i.e. whether AutoTool should look for a tool at all.
pub fn filter_allows(filter: u8, listed: &[String], block: &str) -> bool {
    let listed = listed.iter().any(|b| b == block);
    if filter == 1 { listed } else { !listed }
}

/// SilkTouchHandler: when on, a block on the list only accepts a tool whose
/// silk touch matches the filter (Whitelist: silk touch wanted, Blacklist:
/// silk touch not wanted). Off, or a block not on the list, always passes.
pub fn silk_touch_allows(
    enabled: bool,
    filter: u8,
    listed: &[String],
    block: &str,
    silk: i32,
) -> bool {
    if !enabled || !listed.iter().any(|b| b == block) {
        return true;
    }
    (filter == 0) == (silk > 0)
}

/// Whether a stack has enough durability left to be picked as a tool. A stack
/// with no durability (`max_damage <= 0`) always passes; `ignore` skips it.
pub fn durability_ok(ignore: bool, max_damage: i32, damage: i32) -> bool {
    ignore || max_damage <= 0 || max_damage - damage > 2
}

/// Picks the fastest candidate as `(slot, speed)`; on ties prefers the held
/// slot, then the nearest by hotbar distance, then the lowest slot
/// (LiquidBounce's `PREFER_NEARBY`).
pub fn best_slot(candidates: &[(usize, f32)], selected: usize) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for &(slot, speed) in candidates {
        if !speed.is_finite() {
            continue;
        }
        let better = match best {
            None => true,
            Some((held, best_speed)) => {
                speed > best_speed
                    || (speed == best_speed
                        && nearby_rank(slot, selected) < nearby_rank(held, selected))
            }
        };
        if better {
            best = Some((slot, speed));
        }
    }
    best.map(|(slot, _)| slot)
}

/// Distance order: the held slot, then the nearest hotbar slot; equal
/// distances fall back to the lower index.
fn nearby_rank(slot: usize, selected: usize) -> (u8, usize, usize) {
    if slot == selected {
        (0, 0, slot)
    } else {
        (1, slot.abs_diff(selected), slot)
    }
}

/// A stable id for a slider: the same module and setting always hash the same.
pub fn widget_id(module: &str, setting: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in module
        .bytes()
        .chain(std::iter::once(b'/'))
        .chain(setting.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Moves `current` toward `target` by at most `step`. Symmetric, so lowering
/// the brightness setting dims back down instead of only ever brightening.
pub fn gamma_step(current: f32, target: f32, step: f32) -> f32 {
    if !current.is_finite() {
        return target;
    }
    let step = step.max(0.0);
    if current < target {
        (current + step).min(target)
    } else if current > target {
        (current - step).max(target)
    } else {
        target
    }
}

/// A keyboard's movement as `[strafe, forward]`, normalized like
/// `minecraft:keyboard_direction`: opposite keys cancel and diagonals are
/// unit length.
pub fn input_direction(forward: bool, backward: bool, left: bool, right: bool) -> [f32; 2] {
    let strafe = i8::from(left) - i8::from(right);
    let forward = i8::from(forward) - i8::from(backward);
    let v = [f32::from(strafe), f32::from(forward)];
    let length = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if length < 1.0e-4 {
        [0.0, 0.0]
    } else {
        [v[0] / length, v[1] / length]
    }
}

/// The yaw a movement input faces: the player's yaw turned by the input's own
/// angle, the reference clients' `getMovementDirectionOfInput`.
pub fn movement_yaw(yaw: f32, direction: [f32; 2]) -> f32 {
    yaw + (-direction[0]).atan2(direction[1]).to_degrees()
}

/// LiquidBounce's `withStrafe`: turn the horizontal velocity toward the input
/// at `speed` (the current horizontal speed when None) along `yaw`, blending
/// by `strength`. No input clears the horizontal velocity.
pub fn with_strafe(
    current: [f32; 2],
    direction: [f32; 2],
    yaw: f32,
    speed: Option<f32>,
    strength: f32,
) -> [f32; 2] {
    if direction[0] == 0.0 && direction[1] == 0.0 {
        return [0.0, 0.0];
    }
    let speed = speed.unwrap_or((current[0] * current[0] + current[1] * current[1]).sqrt());
    if speed == 0.0 {
        return [0.0, 0.0];
    }
    let rad = movement_yaw(yaw, direction).to_radians();
    let desired = [-rad.sin() * speed, rad.cos() * speed];
    let k = strength.clamp(0.0, 1.0);
    [
        current[0] + (desired[0] - current[0]) * k,
        current[1] + (desired[1] - current[1]) * k,
    ]
}

/// HypixelBHop's jump floor: vanilla's sprint speed plus the Speed effect's
/// per-level gain (0.008003278196411223).
pub fn hypixel_at_least(amplifier: i32) -> f32 {
    0.281 + 0.008_003_278 * amplifier as f32
}

/// The NCP/Verus allowed horizontal distance: vanilla's 0.2873 boosted by the
/// Speed effect (+20% per level) and slowed by Slowness (-15% per level).
pub fn allowed_horizontal_distance(speed_amplifier: i32, slowness_amplifier: i32) -> f32 {
    let speed = 1.0 + 0.2 * (speed_amplifier + 1) as f32;
    let slowness = 1.0 - 0.15 * (slowness_amplifier + 1) as f32;
    0.2873 * speed * slowness
}

/// Alan's `predictedMotion`: the vanilla air velocity after `ticks` more
/// gravity and drag steps, from a current vertical motion.
pub fn predicted_motion(motion_y: f32, ticks: i32) -> f32 {
    let mut motion = motion_y;
    for _ in 0..ticks.max(0) {
        motion = (motion - 0.08) * 0.98;
    }
    motion
}

/// BlocksMC's strafe yaw: rounded to the nearest eighth of a turn.
pub fn round_yaw(yaw: f32) -> f32 {
    (yaw / 45.0).round() * 45.0
}

/// BlocksMC's air speed: a base plus the ground/air term, a fifth of the
/// vertical motion, +0.1 at Speed I, and 0.007 less per server-setback tick.
pub fn blocksmc_speed(on_ground: bool, motion_y: f32, amplifier: i32, flag_delay: i32) -> f32 {
    let mut speed = 0.06 + if on_ground { 0.12 } else { 0.21 } + motion_y / 20.0;
    if amplifier == 1 {
        speed += 0.1;
    }
    speed - 0.007 * flag_delay.max(0) as f32
}

#[cfg(test)]
mod tests {
    use super::{
        AMBIENCE, AMBIENCE_TIME_MODES, ANIMATIONS, ARRAYLIST, AUTOARMOR, AUTOARMOR_COMPARE,
        AUTOARMOR_MODES, AUTOTOOL, AUTOWEAPON, AUTOXP, AUTOXP_MODES, BLINK,
        HOLEFILL, HOLEFILL_BLOCKS, REACH, AUTOCLICKER, AUTOCLICKER_MODES,
        AIMASSIST, AIMASSIST_POINTS, SURROUND, SURROUND_BLOCKS, CRYSTALAURA,
        AmbienceConfig, AmbienceFog, AmbienceTime, AmbienceWeather, AnimationsConfig,
        ArmorSlot, AutoArmorCompare, AutoArmorConfig, AutoArmorMode,
        AutoWeaponConfig, AutoXpConfig, AutoXpMode, BlinkConfig, CHAMS,
        HoleFillBlock, HoleFillConfig, ReachConfig, AutoClickerConfig, AutoClickerMode,
        AimAssistConfig, AimPoint, SurroundBlock, SurroundConfig, CrystalAuraConfig,
        CHAMS_MODES, CHESTSTEALER,
        CONFIG_HEADER, CRITICALS, CRITICALS_MODES, Category, ChamsMode, CriticalsMode, ESP,
        FASTBREAK, FASTBREAK_DAMAGE, FASTBREAK_MODES, FASTBREAK_TICKS, FLY, FLY_MODES, FULLBRIGHT,
        FlyMode, GuiState, HOLEESP, HOLEESP_MODES, HandPose, Hole, HoleEspConfig, HoleKind,
        HoleMaterial, HoleMode,
        MIN_H, MIN_W, NAMETAGS, NOFALL, NOFALL_MODES, NORENDER, NORENDER_FOG_MODES, NoFallConfig,
        NoFallMode, NoRenderConfig, NoRenderFogMode, OFFHAND, OFFHAND_ITEMS, OffHandConfig,
        OffHandItem, PHASE, PHASE_MODES, PhaseMode,
        SCAFFOLD, SCAFFOLD_DOWNWARDS, SCAFFOLD_MODES,
        SCAFFOLD_SPRINTS, SCAFFOLD_TOWERS, SPEED, SPEED_MODES, STEP, STEP_MODES, ScaffoldDownwards,
        ScaffoldMode, ScaffoldSprint, ScaffoldTower, Setting, SpeedMode, StepMode, TIMER,
        TIMER_MODES, TRACERS, VELOCITY, VELOCITY_MODES, VIEWMODEL, Value, VelocityMode,
        ViewModelConfig, WeaponKind, ZOOM,
        ZOOM_LEVELS, allowed_horizontal_distance, ambience, animations, apply_config, best_slot,
        blocksmc_speed, blink, chams,
        cheststealer, config_document, config_entries, durability_ok, fastbreak,
        fastbreak_air_ground, fastbreak_finish, filter_allows, fly, gamma_step, hole_fade, holeesp,
        hypixel_at_least, input_direction, is_friend, movement_yaw, norender, parse_blocks,
        parse_color, parse_config, parse_names, parse_weapons, phase, predicted_motion, rgb,
        round_yaw,
        scan_holes, silk_touch_allows, slot_distance_squared, smart_delay_ms, speed,
        steal_delay_ticks, step, timer, valid_config_name, viewmodel, widget_id, with, with_strafe,
        autoarmor, aimassist, autoclicker, autoxp, holefill, offhand, reach, scaffold, scan_quads,
        surround, crystal_aura,
    };

    #[test]
    fn fullbright_starts_off_with_both_modes() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == FULLBRIGHT).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Render);
            assert_eq!(
                m.setting("mode").unwrap().choice_options(),
                &["Gamma", "NightVision"][..]
            );
            assert_eq!(m.setting("brightness").unwrap().as_int(), 15);
        });
    }

    #[test]
    fn arraylist_starts_on_and_lists_enabled_modules() {
        with(|c| {
            let list = c.modules.iter().find(|m| m.id == ARRAYLIST).unwrap();
            assert!(list.enabled);
            assert_eq!(list.category, Category::Render);
            c.modules
                .iter_mut()
                .find(|m| m.id == FULLBRIGHT)
                .unwrap()
                .enabled = true;
        });
        let (config, names) = super::arraylist().unwrap();
        assert!(config.background && config.shadow && config.sorted);
        assert_eq!(names, vec!["FullBright", "Watermark"]);

        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == ARRAYLIST)
                .unwrap()
                .enabled = false;
        });
        assert!(super::arraylist().is_none());
    }

    #[test]
    fn watermark_starts_on_in_the_top_left() {
        let config = super::watermark().unwrap();
        assert_eq!(
            config,
            super::WatermarkConfig {
                corner: 0,
                offset_x: 10.0,
                offset_y: 10.0,
                shadow: true,
            }
        );
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == super::WATERMARK)
                .unwrap()
                .enabled = false;
        });
        assert!(super::watermark().is_none());
    }

    #[test]
    fn autotool_starts_off_in_player_with_liquidbounce_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == AUTOTOOL).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Player);
            assert_eq!(m.setting("selector").unwrap().as_choice(), 0);
            assert!(!m.setting("ignore_durability").unwrap().as_bool());
            assert!(!m.setting("consider_inventory").unwrap().as_bool());
            assert_eq!(m.setting("static_slot").unwrap().as_int(), 0);
            assert_eq!(m.setting("filter").unwrap().as_choice(), 0);
            assert_eq!(m.text("blocks"), "");
            assert!(!m.setting("silk_touch").unwrap().as_bool());
            assert_eq!(m.setting("silk_touch_filter").unwrap().as_choice(), 0);
            assert_eq!(m.setting("swap_back_delay").unwrap().as_int(), 20);
            assert_eq!(m.setting("switch_delay").unwrap().as_int(), 0);
            assert!(!m.setting("require_sneaking").unwrap().as_bool());
            assert!(!m.setting("not_during_combat").unwrap().as_bool());
            assert!(!m.setting("require_near_bed").unwrap().as_bool());
            assert_eq!(m.setting("bed_distance").unwrap().as_float(), 10.0);
            assert_eq!(
                parse_blocks(m.text("silk_touch_blocks")),
                vec![
                    "minecraft:ender_chest",
                    "minecraft:glowstone",
                    "minecraft:sea_lantern",
                    "minecraft:turtle_egg"
                ]
            );
        });
        assert!(super::autotool().is_none());
    }

    #[test]
    fn autoweapon_starts_off_in_combat_with_liquidbounce_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == AUTOWEAPON).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Combat);
            assert_eq!(m.text("preferred"), "Sword");
            assert_eq!(parse_weapons(m.text("preferred")), vec![WeaponKind::Sword]);
            assert_eq!(
                m.setting("priority").unwrap().choice_options(),
                &["Default", "Knockback", "Damage", "AttackSpeed"][..]
            );
            assert_eq!(m.setting("priority").unwrap().as_choice(), 0);
            assert!(m.setting("auto_shield_break").unwrap().as_bool());
            assert!(m.setting("auto_mace").unwrap().as_bool());
            assert_eq!(m.setting("switch_back").unwrap().as_int(), 20);
            assert_eq!(
                m.setting("change_on").unwrap().choice_options(),
                &["OnAttack", "OnTarget", "Both"][..]
            );
            assert_eq!(m.setting("change_on").unwrap().as_choice(), 0);
        });
        assert!(super::autoweapon().is_none());

        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == AUTOWEAPON)
                .unwrap()
                .enabled = true;
        });
        let config = super::autoweapon().unwrap();
        assert_eq!(
            config,
            AutoWeaponConfig {
                preferred: vec![WeaponKind::Sword],
                priority: 0,
                auto_shield_break: true,
                auto_mace: true,
                switch_back: 20,
                change_on: 0,
            }
        );
        assert!(config.on_attack() && !config.on_target());
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == AUTOWEAPON)
                .unwrap()
                .enabled = false;
        });
    }

    #[test]
    fn weapon_preferences_are_parsed_by_label() {
        assert_eq!(
            parse_weapons("Sword, Axe"),
            vec![WeaponKind::Sword, WeaponKind::Axe]
        );
        assert_eq!(
            parse_weapons(" sword ; MACE\nFireAspect "),
            vec![WeaponKind::Sword, WeaponKind::Mace, WeaponKind::FireAspect]
        );
        // Pickaxe is its own type (and not an axe), unknown names are dropped.
        assert_eq!(
            parse_weapons("Pickaxe, Nonsense, pickaxe"),
            vec![WeaponKind::Pickaxe]
        );
        assert_eq!(parse_weapons("fire aspect"), vec![WeaponKind::FireAspect]);
        assert!(parse_weapons(" , ").is_empty());
    }

    #[test]
    fn change_on_selects_attack_target_or_both() {
        let config = |change_on| AutoWeaponConfig {
            preferred: Vec::new(),
            priority: 0,
            auto_shield_break: false,
            auto_mace: false,
            switch_back: 20,
            change_on,
        };
        assert!(config(0).on_attack() && !config(0).on_target());
        assert!(!config(1).on_attack() && config(1).on_target());
        assert!(config(2).on_attack() && config(2).on_target());
    }

    #[test]
    fn nametags_starts_off_in_render_with_every_toggle_on() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == NAMETAGS).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Render);
            assert!(m.setting("players").unwrap().as_bool());
            assert!(m.setting("items").unwrap().as_bool());
            assert!(m.setting("player_head").unwrap().as_bool());
            assert!(m.setting("item_icons").unwrap().as_bool());
        });
        assert!(super::nametags().is_none());

        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == NAMETAGS)
                .unwrap()
                .enabled = true;
        });
        assert_eq!(
            super::nametags(),
            Some(super::NameTagsConfig {
                players: true,
                items: true,
                player_head: true,
                item_icons: true,
            })
        );
    }

    #[test]
    fn nofall_starts_off_in_movement_with_liquidbounce_modes() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == NOFALL).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Movement);
            assert_eq!(
                m.setting("mode").unwrap().choice_options(),
                &NOFALL_MODES[..]
            );
            assert_eq!(m.setting("mode").unwrap().as_choice(), 0);
            assert!(!m.setting("not_while_gliding").unwrap().as_bool());
            assert!(!m.setting("not_with_mace").unwrap().as_bool());
            assert_eq!(m.setting("spoof_ground_distance").unwrap().as_choice(), 0);
            assert_eq!(m.setting("spoof_ground_value").unwrap().as_float(), 1.7);
            assert!(m.setting("spoof_ground_reset").unwrap().as_bool());
            assert_eq!(m.setting("offset_x").unwrap().as_float(), 1337.0);
            assert_eq!(m.setting("offset_y").unwrap().as_float(), 0.0);
            assert_eq!(m.setting("offset_z").unwrap().as_float(), 1337.0);
            assert_eq!(m.setting("packet_type").unwrap().as_choice(), 0);
            assert_eq!(m.setting("packet_filter").unwrap().as_choice(), 0);
            assert_eq!(m.setting("packet_value").unwrap().as_float(), 2.0);
            assert!(m.setting("packet_reset").unwrap().as_bool());
            assert_eq!(m.setting("jump_timing").unwrap().as_choice(), 0);
            assert_eq!(m.setting("jump_value").unwrap().as_float(), 3.0);
            assert!(m.setting("jump_reset").unwrap().as_bool());
            assert_eq!(m.setting("mlg_min_fall").unwrap().as_float(), 5.0);
            assert_eq!(m.setting("mount_min_fall").unwrap().as_float(), 5.0);
            assert_eq!(m.setting("mount_search_range").unwrap().as_float(), 4.5);
            assert_eq!(m.setting("mount_retry_delay").unwrap().as_int(), 2);
            assert_eq!(m.setting("void_level").unwrap().as_int(), 0);
            assert_eq!(m.setting("block_distance").unwrap().as_float(), 1.0);
            assert_eq!(m.setting("force_fall_distance").unwrap().as_float(), 3.35);
            assert_eq!(m.setting("jump_height").unwrap().as_float(), 0.42);
            assert_eq!(m.setting("cancel_value").unwrap().as_float(), 1.7);
            assert!(m.setting("cancel_reset").unwrap().as_bool());
            assert!(!m.setting("cancel_setback").unwrap().as_bool());
            assert_eq!(m.setting("blink_trigger").unwrap().as_float(), 2.5);
            assert_eq!(m.setting("blink_maximum").unwrap().as_float(), 20.0);
            assert!(!m.setting("over_void").unwrap().as_bool());
        });
        assert!(super::nofall().is_none());

        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == NOFALL)
                .unwrap()
                .enabled = true;
        });
        let config = super::nofall().unwrap();
        assert_eq!(
            config,
            NoFallConfig {
                mode: NoFallMode::SpoofGround,
                not_while_gliding: false,
                not_with_mace: false,
                spoof_ground_distance: 0,
                spoof_ground_value: 1.7,
                spoof_ground_reset: true,
                offset: [1337.0, 0.0, 1337.0],
                packet_type: 0,
                packet_filter: 0,
                packet_distance: 0,
                packet_value: 2.0,
                packet_reset: true,
                jump_packet_type: 0,
                jump_distance: 0,
                jump_value: 3.0,
                jump_timing: 0,
                jump_reset: true,
                mlg_min_fall: 5.0,
                mount_min_fall: 5.0,
                mount_search_range: 4.5,
                mount_retry_delay: 2,
                void_level: 0,
                block_distance: 1.0,
                force_fall_distance: 3.35,
                jump_height: 0.42,
                cancel_distance: 0,
                cancel_value: 1.7,
                cancel_reset: true,
                cancel_setback: false,
                blink_trigger: 2.5,
                blink_maximum: 20.0,
                over_void: false,
                motion_flag_speed: 0.1,
                matrix_safe: false,
                phase_offset: 0.0,
            }
        );
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == NOFALL)
                .unwrap()
                .enabled = false;
        });
    }

    #[test]
    fn nofall_mode_indices_follow_the_label_list() {
        for (index, label) in NOFALL_MODES.iter().enumerate() {
            let mode = NoFallMode::from_index(index);
            assert_eq!(NOFALL_MODES[mode as usize], *label);
        }
        assert_eq!(NoFallMode::from_index(99), NoFallMode::SpoofGround);
        assert_eq!(NoFallMode::from_index(18), NoFallMode::Grim2371);
        assert_eq!(NoFallMode::from_index(54), NoFallMode::Phase);
        assert_eq!(NoFallMode::from_index(55), NoFallMode::Grim72);
        assert_eq!(NoFallMode::from_index(56), NoFallMode::Grim73);
    }

    #[test]
    fn nofall_settings_follow_the_selected_mode() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == NOFALL).unwrap();
            let shown = |mode: usize| {
                let mut m = m.clone();
                m.setting_mut("mode").unwrap().value = Value::Choice(mode);
                m.settings
                    .iter()
                    .filter(|s| m.setting_visible(s))
                    .map(|s| s.id)
                    .collect::<Vec<_>>()
            };
            // The module-wide guard and the mode picker are always listed.
            assert_eq!(shown(2), ["mode", "not_while_gliding", "not_with_mace"]);
            assert!(shown(0).contains(&"spoof_ground_reset"));
            assert!(!shown(0).contains(&"packet_reset"));
            assert!(shown(3).contains(&"packet_reset"));
            assert!(shown(4).contains(&"jump_reset"));
            assert_eq!(
                shown(5),
                ["mode", "not_while_gliding", "not_with_mace", "mlg_min_fall"]
            );
            assert!(shown(6).contains(&"mount_retry_delay"));
            assert!(shown(10).contains(&"void_level"));
            assert!(shown(12).contains(&"jump_height"));
            assert!(shown(13).contains(&"cancel_setback"));
            assert!(shown(14).contains(&"blink_trigger"));
            assert!(shown(15).contains(&"over_void"));
            assert!(shown(24).contains(&"motion_flag_speed"));
            assert!(!shown(24).contains(&"over_void"));
            assert!(shown(35).contains(&"matrix_safe"));
            assert!(shown(54).contains(&"phase_offset"));
            assert_eq!(shown(55), ["mode", "not_while_gliding", "not_with_mace"]);
            assert_eq!(shown(56), ["mode", "not_while_gliding", "not_with_mace"]);
        });
    }

    #[test]
    fn block_lists_are_tidied_into_namespaced_ids() {
        assert_eq!(
            parse_blocks(" oak_log, minecraft:stone ;  ,\n Deepslate\n"),
            vec![
                "minecraft:oak_log",
                "minecraft:stone",
                "minecraft:deepslate"
            ]
        );
        assert!(parse_blocks(" , ").is_empty());
    }

    #[test]
    fn filters_blacklist_by_default_and_whitelist_on_request() {
        let listed = parse_blocks("ender_chest");
        assert!(!filter_allows(0, &listed, "minecraft:ender_chest"));
        assert!(filter_allows(0, &listed, "minecraft:stone"));
        assert!(filter_allows(1, &listed, "minecraft:ender_chest"));
        assert!(!filter_allows(1, &listed, "minecraft:stone"));
        // An empty blacklist allows everything, an empty whitelist nothing.
        assert!(filter_allows(0, &[], "minecraft:stone"));
        assert!(!filter_allows(1, &[], "minecraft:stone"));
    }

    #[test]
    fn silk_touch_handler_matches_the_filter() {
        let listed = parse_blocks("glowstone");
        // Off, or a block not on the list, always passes.
        assert!(silk_touch_allows(
            false,
            0,
            &listed,
            "minecraft:glowstone",
            0
        ));
        assert!(silk_touch_allows(true, 0, &listed, "minecraft:stone", 0));
        // Whitelist: only silk touch; Blacklist: only without.
        assert!(!silk_touch_allows(
            true,
            0,
            &listed,
            "minecraft:glowstone",
            0
        ));
        assert!(silk_touch_allows(
            true,
            0,
            &listed,
            "minecraft:glowstone",
            1
        ));
        assert!(silk_touch_allows(
            true,
            1,
            &listed,
            "minecraft:glowstone",
            0
        ));
        assert!(!silk_touch_allows(
            true,
            1,
            &listed,
            "minecraft:glowstone",
            1
        ));
    }

    #[test]
    fn nearly_broken_tools_are_skipped_unless_ignored() {
        assert!(!durability_ok(false, 100, 98));
        assert!(durability_ok(false, 100, 97));
        assert!(durability_ok(false, 0, 0));
        assert!(durability_ok(true, 100, 100));
    }

    #[test]
    fn best_slot_takes_the_fastest_then_the_nearer_tool() {
        assert_eq!(best_slot(&[], 3), None);
        assert_eq!(best_slot(&[(4, 3.0)], 3), Some(4));
        // Speed wins over closeness.
        assert_eq!(best_slot(&[(2, 2.0), (7, 6.0)], 2), Some(7));
        // Ties go to the held slot, then the nearest, then the lowest.
        assert_eq!(best_slot(&[(1, 4.0), (5, 4.0)], 5), Some(5));
        assert_eq!(best_slot(&[(1, 4.0), (5, 4.0)], 3), Some(1));
        assert_eq!(best_slot(&[(1, 4.0), (5, 4.0)], 9), Some(5));
        // Non-finite speeds are skipped.
        assert_eq!(best_slot(&[(2, f32::NAN), (4, 1.0)], 4), Some(4));
    }

    #[test]
    fn gamma_ramps_in_both_directions() {
        assert!((gamma_step(0.5, 15.0, 0.1) - 0.6).abs() < 1e-6);
        assert!((gamma_step(15.0, 1.0, 0.1) - 14.9).abs() < 1e-5);
        assert_eq!(gamma_step(0.5, 0.5, 0.1), 0.5);
        assert_eq!(gamma_step(f32::NAN, 3.0, 0.1), 3.0);
    }

    #[test]
    fn settings_read_kept_values_and_clamp_them() {
        let mut brightness = Setting::int("brightness", "Brightness", 15, 1, 15, 1);
        brightness.set_text("99");
        assert_eq!(brightness.as_int(), 15);
        brightness.set_number(7.4);
        assert_eq!(brightness.as_int(), 7);

        let mut mode = Setting::choice("mode", "Mode", 0, &["Gamma", "NightVision"]);
        mode.set_text("NightVision");
        assert_eq!(mode.as_choice(), 1);
        mode.set_text("9");
        assert_eq!(mode.as_choice(), 1);

        let mut speed = Setting::float("speed", "Speed", 0.1, 0.05, 2.0, 0.05);
        // The next f32 above 0.3: snapping must drop the binary noise.
        speed.set_number(f32::from_bits(0.3_f32.to_bits() + 1));
        assert_eq!(speed.as_float(), 0.3);
    }

    #[test]
    fn slider_ids_are_stable_and_distinct() {
        assert_eq!(widget_id("a", "b"), widget_id("a", "b"));
        assert_ne!(widget_id("a", "b"), widget_id("b", "a"));
    }

    #[test]
    fn the_window_size_has_a_floor() {
        let gui = GuiState {
            size: (10.0, 10.0),
            ..GuiState::default()
        };
        assert_eq!(gui.clamped_size(), (MIN_W, MIN_H));
        let gui = GuiState {
            size: (600.0, 300.0),
            ..GuiState::default()
        };
        assert_eq!(gui.clamped_size(), (600.0, 300.0));
    }

    #[test]
    fn config_is_the_last_tab() {
        assert_eq!(Category::ALL.len(), 7);
        assert_eq!(Category::ALL.last(), Some(&Category::Config));
        assert_eq!(Category::Config.index(), 6);
        assert_eq!(Category::Config.label(), "Config");
        // The module tabs keep their old indices.
        assert_eq!(Category::Render.index(), 0);
        assert_eq!(Category::Misc.index(), 5);
    }

    #[test]
    fn config_names_are_safe_file_stems() {
        assert_eq!(valid_config_name("default"), Some("default".to_owned()));
        assert_eq!(
            valid_config_name("  pvp-1_test "),
            Some("pvp-1_test".to_owned())
        );
        assert_eq!(valid_config_name(""), None);
        assert_eq!(valid_config_name("   "), None);
        assert_eq!(valid_config_name("a/b"), None);
        assert_eq!(valid_config_name("a.b"), None);
        assert_eq!(valid_config_name("a b"), None);
        assert_eq!(valid_config_name(&"a".repeat(33)), None);
        assert_eq!(valid_config_name(&"a".repeat(32)), Some("a".repeat(32)));
    }

    #[test]
    fn esp_starts_off_in_render_with_the_reference_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == ESP).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Render);
            assert!(m.bool("players") && m.bool("friends") && m.bool("mobs"));
            assert!(!m.bool("self"));
            assert_eq!(m.float("max_distance"), 72.0);
            assert!(m.bool("outline"));
            assert_eq!(m.text("friend_color"), "#FF34FF60");
            assert_eq!(m.text("friend_names"), "");
        });
        assert!(super::esp().is_none());
    }

    #[test]
    fn tracers_starts_off_in_render_with_the_reference_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == TRACERS).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Render);
            assert!(m.bool("players"));
            assert!(!m.bool("items"));
            assert_eq!(m.text("color"), "#FFFFFFFF");
            assert_eq!(m.text("friend_color"), "#FFFFFFFF");
        });
        assert!(super::tracers().is_none());
    }

    #[test]
    fn holeesp_starts_off_in_render_with_the_reference_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == HOLEESP).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Render);
            assert_eq!(
                m.setting("mode").unwrap().choice_options(),
                &HOLEESP_MODES[..]
            );
            assert_eq!(m.choice("mode"), 0);
            assert_eq!(m.int("radius"), 8);
            assert_eq!(m.int("vertical"), 4);
            assert!(m.bool("doubles"));
            assert!((m.float("height") - 0.25).abs() < f32::EPSILON);
            assert!(m.bool("filled"));
            assert!(m.bool("outline"));
            assert!(m.bool("through_walls"));
            assert!(m.bool("fade"));
            assert_eq!(m.int("fill_alpha"), 60);
            assert_eq!(m.int("outline_alpha"), 200);
            assert_eq!(m.text("obsidian_color"), "#FFFF5555");
            assert_eq!(m.text("bedrock_color"), "#FF55FF55");
            assert_eq!(m.text("mixed_color"), "#FFFFAA00");
            assert!(!m.bool("ignore_own"));
        });
        assert!(holeesp().is_none());
    }

    #[test]
    fn holeesp_finds_sealed_holes_and_skips_open_ones() {
        use std::collections::HashMap;
        // A bedrock hole at (0, 0, 0), an obsidian hole at (4, 0, 0), a mixed
        // one at (-4, 0, 0), and an open obsidian floor at (0, 0, 4) that is
        // not a hole.
        let mut world: HashMap<[i32; 3], HoleMaterial> = HashMap::new();
        let mut put = |pos: [i32; 3], mat: HoleMaterial| {
            world.insert(pos, mat);
        };
        put([0, -1, 0], HoleMaterial::Bedrock);
        for side in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            put([0 + side[0], 0, 0 + side[2]], HoleMaterial::Bedrock);
        }
        put([4, -1, 0], HoleMaterial::Obsidian);
        for side in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            put([4 + side[0], 0, 0 + side[2]], HoleMaterial::Obsidian);
        }
        put([-4, -1, 0], HoleMaterial::Bedrock);
        put([-5, 0, 0], HoleMaterial::Bedrock);
        put([-3, 0, 0], HoleMaterial::Obsidian);
        put([-4, 0, 1], HoleMaterial::Bedrock);
        put([-4, 0, -1], HoleMaterial::Bedrock);
        put([0, -1, 4], HoleMaterial::Obsidian);

        let config = HoleEspConfig::default();
        let block =
            |x: i32, y: i32, z: i32| world.get(&[x, y, z]).copied().unwrap_or(HoleMaterial::Air);
        let holes = scan_holes([0, 0, 0], &config, block);
        assert_eq!(holes.len(), 3);
        assert!(
            holes
                .iter()
                .any(|h| h.kind == HoleKind::Bedrock && (h.x, h.y, h.z) == (0, 0, 0))
        );
        assert!(
            holes
                .iter()
                .any(|h| h.kind == HoleKind::Obsidian && (h.x, h.y, h.z) == (4, 0, 0))
        );
        assert!(
            holes
                .iter()
                .any(|h| h.kind == HoleKind::Mixed && (h.x, h.y, h.z) == (-4, 0, 0))
        );
        assert!(!holes.iter().any(|h| (h.x, h.z) == (0, 4)));

        // The mode filters to one material, and the own hole can be hidden.
        let obsidian = HoleEspConfig {
            mode: HoleMode::Obsidian,
            ..HoleEspConfig::default()
        };
        let kinds: Vec<HoleKind> = scan_holes([0, 0, 0], &obsidian, block)
            .into_iter()
            .map(|h| h.kind)
            .collect();
        assert_eq!(kinds, vec![HoleKind::Obsidian]);
        let own = HoleEspConfig {
            ignore_own: true,
            ..HoleEspConfig::default()
        };
        assert!(
            !scan_holes([0, 0, 0], &own, block)
                .iter()
                .any(|h| (h.x, h.y, h.z) == (0, 0, 0))
        );
    }

    #[test]
    fn holeesp_detects_1x2_doubles_only_when_enabled() {
        use std::collections::HashMap;
        // An obsidian trench along x at (10, 0, 0)..(11, 0, 0) and a bedrock
        // trench along z at (20, 0, 10)..(20, 0, 11). Neither cell is a 1x1
        // hole, since each opens onto its partner.
        let mut world: HashMap<[i32; 3], HoleMaterial> = HashMap::new();
        for pos in [
            [10, -1, 0],
            [11, -1, 0],
            [9, 0, 0],
            [12, 0, 0],
            [10, 0, 1],
            [11, 0, 1],
            [10, 0, -1],
            [11, 0, -1],
        ] {
            world.insert(pos, HoleMaterial::Obsidian);
        }
        for pos in [
            [20, -1, 10],
            [20, -1, 11],
            [19, 0, 10],
            [21, 0, 10],
            [20, 0, 9],
            [19, 0, 11],
            [21, 0, 11],
            [20, 0, 12],
        ] {
            world.insert(pos, HoleMaterial::Bedrock);
        }
        let block =
            |x: i32, y: i32, z: i32| world.get(&[x, y, z]).copied().unwrap_or(HoleMaterial::Air);

        let wide = HoleEspConfig {
            radius: 32,
            ..HoleEspConfig::default()
        };
        let holes = scan_holes([0, 0, 0], &wide, block);
        let x_double = holes
            .iter()
            .find(|h| (h.x, h.y, h.z) == (10, 0, 0))
            .expect("the x trench");
        assert_eq!((x_double.width, x_double.depth), (2, 1));
        assert_eq!(x_double.kind, HoleKind::Obsidian);
        let z_double = holes
            .iter()
            .find(|h| (h.x, h.y, h.z) == (20, 0, 10))
            .expect("the z trench");
        assert_eq!((z_double.width, z_double.depth), (1, 2));
        assert_eq!(z_double.kind, HoleKind::Bedrock);
        // The upper cells do not report the same double again.
        assert!(!holes.iter().any(|h| (h.x, h.z) == (11, 0)));
        assert!(!holes.iter().any(|h| (h.x, h.z) == (20, 11)));

        let singles_only = HoleEspConfig {
            doubles: false,
            ..wide
        };
        assert!(scan_holes([0, 0, 0], &singles_only, block).is_empty());
    }

    #[test]
    fn holeesp_fade_falls_from_the_camera_to_a_fifth() {
        assert_eq!(hole_fade(0.0, 16.0), 1.0);
        assert!((hole_fade(16.0, 16.0) - 0.2).abs() < 1e-6);
        assert!((hole_fade(8.0, 16.0) - 0.6).abs() < 1e-6);
        assert_eq!(hole_fade(4.0, 0.0), 1.0);
    }

    #[test]
    fn norender_starts_off_in_render_with_every_hide_off() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == NORENDER).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Render);
            assert_eq!(
                m.setting("fog").unwrap().choice_options(),
                &NORENDER_FOG_MODES[..]
            );
            assert_eq!(m.choice("fog"), 0);
            for id in [
                "crosshair",
                "boss_bar",
                "scoreboard",
                "title",
                "held_item_name",
                "potion_icons",
                "toasts",
                "water",
                "fire",
                "particles",
                "nametags",
                "glowing",
                "hand",
                "item_frames",
                "mobs",
            ] {
                assert!(!m.bool(id), "{id} starts off");
            }
        });
        assert!(norender().is_none());
    }

    #[test]
    fn norender_hides_only_the_selected_hud_layers() {
        let config = NoRenderConfig {
            crosshair: true,
            boss_bar: true,
            scoreboard: true,
            title: true,
            held_item_name: true,
            potion_icons: true,
            ..NoRenderConfig::default()
        };
        for layer in [
            "minecraft:crosshair",
            "minecraft:boss_overlay",
            "minecraft:scoreboard_sidebar",
            "minecraft:title",
            "minecraft:subtitle_overlay",
            "minecraft:selected_item_name",
            "minecraft:effects",
        ] {
            assert!(config.hides_layer(layer), "{layer}");
        }
        for layer in [
            "minecraft:hotbar",
            "minecraft:player_health",
            "minecraft:chat",
            "minecraft:tab_list",
            "minecraft:overlay_message",
        ] {
            assert!(!config.hides_layer(layer), "{layer}");
        }
    }

    #[test]
    fn norender_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == NORENDER).unwrap();
            m.enabled = true;
            m.setting_mut("crosshair").unwrap().value = Value::Bool(true);
            m.setting_mut("fog").unwrap().value = Value::Choice(2);
            m.setting_mut("water").unwrap().value = Value::Bool(true);
            m.setting_mut("fire").unwrap().value = Value::Bool(true);
            m.setting_mut("particles").unwrap().value = Value::Bool(true);
            m.setting_mut("mobs").unwrap().value = Value::Bool(true);
        });
        let config = norender().unwrap();
        assert!(config.crosshair);
        assert_eq!(config.fog, NoRenderFogMode::Lava);
        assert!(config.water);
        assert!(config.fire);
        assert!(config.particles);
        assert!(config.mobs);
        assert!(!config.boss_bar);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == NORENDER).unwrap();
            m.setting_mut("fog").unwrap().value = Value::Choice(3);
        });
        assert_eq!(norender().unwrap().fog, NoRenderFogMode::Water);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == NORENDER).unwrap();
            m.enabled = false;
            m.setting_mut("crosshair").unwrap().value = Value::Bool(false);
            m.setting_mut("fog").unwrap().value = Value::Choice(0);
            m.setting_mut("water").unwrap().value = Value::Bool(false);
            m.setting_mut("fire").unwrap().value = Value::Bool(false);
            m.setting_mut("particles").unwrap().value = Value::Bool(false);
            m.setting_mut("mobs").unwrap().value = Value::Bool(false);
        });
        assert!(norender().is_none());
    }

    #[test]
    fn chams_starts_off_in_render_with_the_reference_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == CHAMS).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Render);
            assert_eq!(m.setting("mode").unwrap().choice_options(), &CHAMS_MODES[..]);
            assert_eq!(m.choice("mode"), 0);
            for id in [
                "players",
                "self",
                "monsters",
                "animals",
                "crystals",
                "invisibles",
                "through_walls",
            ] {
                assert!(m.bool(id), "{id} starts on");
            }
            assert_eq!(m.int("range"), 64);
            assert_eq!(m.text("color"), "#3CFF0000");
        });
        assert!(chams().is_none());
    }

    #[test]
    fn chams_modes_own_the_half_they_draw() {
        assert!(ChamsMode::Filled.fills());
        assert!(!ChamsMode::Filled.outlines());
        assert!(!ChamsMode::Outline.fills());
        assert!(ChamsMode::Outline.outlines());
        assert!(ChamsMode::Both.fills());
        assert!(ChamsMode::Both.outlines());
    }

    #[test]
    fn chams_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == CHAMS).unwrap();
            m.enabled = true;
            m.setting_mut("mode").unwrap().value = Value::Choice(2);
            m.setting_mut("players").unwrap().value = Value::Bool(false);
            m.setting_mut("crystals").unwrap().value = Value::Bool(false);
            m.setting_mut("through_walls").unwrap().value = Value::Bool(false);
            m.setting_mut("range").unwrap().value = Value::Int(32);
            m.setting_mut("color").unwrap().value = Value::Text("#80FF8000".to_owned());
        });
        let config = chams().unwrap();
        assert_eq!(config.mode, ChamsMode::Both);
        assert!(!config.players);
        assert!(config.monsters);
        assert!(!config.crystals);
        assert!(!config.through_walls);
        assert_eq!(config.range, 32.0);
        assert_eq!(config.color, 0x80FF_8000);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == CHAMS).unwrap();
            m.enabled = false;
            m.setting_mut("mode").unwrap().value = Value::Choice(0);
            m.setting_mut("players").unwrap().value = Value::Bool(true);
            m.setting_mut("crystals").unwrap().value = Value::Bool(true);
            m.setting_mut("through_walls").unwrap().value = Value::Bool(true);
            m.setting_mut("range").unwrap().value = Value::Int(64);
            m.setting_mut("color").unwrap().value = Value::Text("#3CFF0000".to_owned());
        });
        assert!(chams().is_none());
    }

    #[test]
    fn viewmodel_starts_off_in_render_and_leaves_the_hands_alone() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == VIEWMODEL).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Render);
            // Scale one and every offset zero: switching the module on without
            // touching a slider must draw exactly what the game drew.
            for hand in ["main", "off", "arm"] {
                assert_eq!(m.float(&[hand, "_scale"].concat()), 1.0);
                for axis in ["_x", "_y", "_z", "_pitch", "_yaw", "_roll"] {
                    assert_eq!(m.float(&[hand, axis].concat()), 0.0);
                }
            }
        });
        assert!(viewmodel().is_none());
    }

    #[test]
    fn viewmodel_identity_is_what_the_game_draws() {
        let identity = HandPose::IDENTITY;
        assert!(identity.is_identity());
        assert_eq!(HandPose::default(), identity);
        // Any single value is already a change, a turn as much as an offset.
        assert!(!HandPose { scale: 0.9, ..identity }.is_identity());
        assert!(!HandPose { yaw: 1.0, ..identity }.is_identity());
        assert!(!HandPose { z: 0.01, ..identity }.is_identity());
    }

    #[test]
    fn viewmodel_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == VIEWMODEL).unwrap();
            m.enabled = true;
            m.setting_mut("main_scale").unwrap().value = Value::Float(0.5);
            m.setting_mut("main_x").unwrap().value = Value::Float(0.25);
            m.setting_mut("main_pitch").unwrap().value = Value::Float(-30.0);
            m.setting_mut("off_roll").unwrap().value = Value::Float(15.0);
            m.setting_mut("arm_y").unwrap().value = Value::Float(-0.1);
        });
        let ViewModelConfig { main, off, arm } = viewmodel().unwrap();
        assert_eq!(main.scale, 0.5);
        assert_eq!(main.x, 0.25);
        assert_eq!(main.pitch, -30.0);
        assert_eq!(off.roll, 15.0);
        assert_eq!(arm.y, -0.1);
        // The values that were not touched stay at the identity.
        assert_eq!(main.y, 0.0);
        assert_eq!(off.scale, 1.0);
        assert_eq!(off.x, 0.0);
        assert_eq!(arm.scale, 1.0);
        assert_eq!(arm.roll, 0.0);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == VIEWMODEL).unwrap();
            m.enabled = false;
            for (id, value) in [
                ("main_scale", 1.0),
                ("main_x", 0.0),
                ("main_pitch", 0.0),
                ("off_roll", 0.0),
            ] {
                m.setting_mut(id).unwrap().value = Value::Float(value);
            }
        });
        assert!(viewmodel().is_none());
    }

    #[test]
    fn animations_starts_off_in_render_and_leaves_the_swing_alone() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == ANIMATIONS).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Render);
            // Vanilla: the swing keeps its own duration, no extra arc, and the
            // item still comes to the mouth.
            assert_eq!(m.float("swing_speed"), 0.0);
            assert_eq!(m.float("sway"), 0.0);
            assert!(m.bool("eating"));
        });
        assert!(animations().is_none());
        assert!(AnimationsConfig::default().is_vanilla());
    }

    #[test]
    fn blink_starts_off_in_player_and_holds_nothing() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == BLINK).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Player);
            // Vanilla: nothing is held, and the timer is only a pulse concern.
            assert!(!m.bool("pulse"));
            assert_eq!(m.int("delay"), 20);
            // The marker is on, because a blink you cannot see is a blink you
            // cannot trust.
            assert!(m.bool("marker"));
            assert!(m.bool("marker_walls"));
            assert_eq!(m.text("marker_color"), "#60FFAA00");
        });
        assert!(blink().is_none());
        assert_eq!(BlinkConfig::default().delay, 20);
        assert_eq!(BlinkConfig::default().marker_color, 0x60FF_AA00);
    }

    #[test]
    fn blink_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == BLINK).unwrap();
            m.enabled = true;
            m.setting_mut("pulse").unwrap().value = Value::Bool(true);
            m.setting_mut("delay").unwrap().value = Value::Int(40);
            m.setting_mut("marker").unwrap().value = Value::Bool(false);
            m.setting_mut("marker_color").unwrap().value = Value::Text("#80FF0000".into());
            m.setting_mut("marker_walls").unwrap().value = Value::Bool(false);
        });
        let config = blink().unwrap();
        assert!(config.pulse);
        assert_eq!(config.delay, 40);
        assert!(!config.marker);
        assert_eq!(config.marker_color, 0x80FF_0000);
        assert!(!config.marker_walls);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == BLINK).unwrap();
            m.enabled = false;
        });
        assert!(blink().is_none());
    }

    #[test]
    fn a_marker_colour_falls_back_when_the_text_is_not_one() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == BLINK).unwrap();
            m.enabled = true;
            m.setting_mut("marker_color").unwrap().value = Value::Text("orange".into());
        });
        assert_eq!(blink().unwrap().marker_color, 0x60FF_AA00);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == BLINK).unwrap();
            m.enabled = false;
            m.setting_mut("marker_color")
                .unwrap()
                .value = Value::Text("#60FFAA00".into());
        });
    }

    #[test]
    fn blink_clamps_a_delay_the_config_could_not_have_written() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == BLINK).unwrap();
            m.enabled = true;
            m.setting_mut("delay").unwrap().value = Value::Int(0);
        });
        assert_eq!(blink().unwrap().delay, 2);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == BLINK).unwrap();
            m.enabled = false;
            m.setting_mut("delay").unwrap().value = Value::Int(20);
        });
    }

    #[test]
    fn offhand_starts_off_in_combat_on_a_totem() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == OFFHAND).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.name, "OffHand");
            assert_eq!(m.category, Category::Combat);
            assert_eq!(m.setting("item").unwrap().choice_options(), &OFFHAND_ITEMS[..]);
            assert_eq!(m.choice("item"), 0);
            assert_eq!(m.float("health"), 10.0);
            assert!(m.bool("absorption"));
            assert!(!m.bool("sword_only"));
            assert!(m.bool("eat"));
            assert_eq!(m.float("eat_at"), 15.0);
            assert!(m.bool("fall"));
            assert!(m.bool("elytra"));
            assert!(m.bool("explosion"));
            assert!(!m.bool("hotbar"));
            assert_eq!(m.int("delay"), 2);
        });
        assert!(offhand().is_none());
    }

    #[test]
    fn offhand_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == OFFHAND).unwrap();
            m.enabled = true;
            m.setting_mut("item").unwrap().value = Value::Choice(1);
            m.setting_mut("health").unwrap().value = Value::Float(6.5);
            m.setting_mut("absorption").unwrap().value = Value::Bool(false);
            m.setting_mut("sword_only").unwrap().value = Value::Bool(true);
            m.setting_mut("eat").unwrap().value = Value::Bool(false);
            m.setting_mut("eat_at").unwrap().value = Value::Float(17.5);
            m.setting_mut("fall").unwrap().value = Value::Bool(false);
            m.setting_mut("elytra").unwrap().value = Value::Bool(false);
            m.setting_mut("explosion").unwrap().value = Value::Bool(false);
            m.setting_mut("hotbar").unwrap().value = Value::Bool(true);
            m.setting_mut("delay").unwrap().value = Value::Int(7);
        });
        let config = offhand().unwrap();
        assert_eq!(config.item, OffHandItem::GApple);
        assert_eq!(config.health, 6.5);
        assert!(!config.absorption);
        assert!(config.sword_only);
        assert!(!config.eat);
        assert_eq!(config.eat_at, 17.5);
        assert!(!config.fall);
        assert!(!config.elytra);
        assert!(!config.explosion);
        assert!(config.hotbar);
        assert_eq!(config.delay, 7);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == OFFHAND).unwrap();
            m.enabled = false;
        });
        assert!(offhand().is_none());
    }

    #[test]
    fn offhand_clamps_a_threshold_the_config_could_not_have_written() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == OFFHAND).unwrap();
            m.enabled = true;
            m.setting_mut("health").unwrap().value = Value::Float(99.0);
            m.setting_mut("eat_at").unwrap().value = Value::Float(-4.0);
            m.setting_mut("delay").unwrap().value = Value::Int(99);
        });
        let config = offhand().unwrap();
        assert_eq!(config.health, 20.0);
        assert_eq!(config.eat_at, 0.0);
        assert_eq!(config.delay, 20);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == OFFHAND).unwrap();
            m.enabled = false;
            m.setting_mut("health").unwrap().value = Value::Float(10.0);
            m.setting_mut("eat_at").unwrap().value = Value::Float(15.0);
            m.setting_mut("delay").unwrap().value = Value::Int(2);
        });
    }

    #[test]
    fn offhand_item_indices_map_to_the_choices() {
        assert_eq!(OffHandItem::from_index(0), OffHandItem::Totem);
        assert_eq!(OffHandItem::from_index(1), OffHandItem::GApple);
        assert_eq!(OffHandItem::from_index(2), OffHandItem::Crystal);
        // A saved config cannot name an item the list has not got; the totem
        // is the safe answer.
        assert_eq!(OffHandItem::from_index(3), OffHandItem::Totem);
    }

    #[test]
    fn offhand_defaults_match_the_registry() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == OFFHAND).unwrap();
            m.enabled = true;
        });
        assert_eq!(offhand().unwrap(), OffHandConfig::default());
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == OFFHAND).unwrap();
            m.enabled = false;
        });
    }

    #[test]
    fn autoarmor_starts_off_in_combat_on_the_best_piece() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == AUTOARMOR).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.name, "AutoArmor");
            assert_eq!(m.category, Category::Combat);
            assert_eq!(m.setting("mode").unwrap().choice_options(), &AUTOARMOR_MODES[..]);
            assert_eq!(
                m.setting("compare").unwrap().choice_options(),
                &AUTOARMOR_COMPARE[..]
            );
            assert_eq!(m.choice("mode"), 0);
            assert_eq!(m.choice("compare"), 0);
            assert_eq!(m.int("min_durability"), 5);
            assert_eq!(m.int("delay"), 2);
        });
        assert!(autoarmor().is_none());
    }

    #[test]
    fn autoarmor_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOARMOR).unwrap();
            m.enabled = true;
            m.setting_mut("mode").unwrap().value = Value::Choice(1);
            m.setting_mut("compare").unwrap().value = Value::Choice(1);
            m.setting_mut("min_durability").unwrap().value = Value::Int(25);
            m.setting_mut("delay").unwrap().value = Value::Int(4);
        });
        let config = autoarmor().unwrap();
        assert_eq!(config.mode, AutoArmorMode::Empty);
        assert_eq!(config.compare, AutoArmorCompare::Protection);
        assert_eq!(config.min_durability, 25);
        assert_eq!(config.delay, 4);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOARMOR).unwrap();
            m.enabled = false;
            m.setting_mut("mode").unwrap().value = Value::Choice(0);
            m.setting_mut("compare").unwrap().value = Value::Choice(0);
            m.setting_mut("min_durability").unwrap().value = Value::Int(5);
            m.setting_mut("delay").unwrap().value = Value::Int(2);
        });
        assert!(autoarmor().is_none());
    }

    #[test]
    fn autoarmor_clamps_a_threshold_the_config_could_not_have_written() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOARMOR).unwrap();
            m.enabled = true;
            m.setting_mut("min_durability").unwrap().value = Value::Int(400);
            m.setting_mut("delay").unwrap().value = Value::Int(-3);
        });
        let config = autoarmor().unwrap();
        assert_eq!(config.min_durability, 100);
        assert_eq!(config.delay, 0);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOARMOR).unwrap();
            m.enabled = false;
            m.setting_mut("min_durability").unwrap().value = Value::Int(5);
            m.setting_mut("delay").unwrap().value = Value::Int(2);
        });
    }

    #[test]
    fn autoarmor_indices_map_to_the_choices() {
        assert_eq!(AutoArmorMode::from_index(0), AutoArmorMode::Best);
        assert_eq!(AutoArmorMode::from_index(1), AutoArmorMode::Empty);
        // A saved config cannot name a mode the list has not got.
        assert_eq!(AutoArmorMode::from_index(2), AutoArmorMode::Best);
        assert_eq!(AutoArmorCompare::from_index(0), AutoArmorCompare::Armor);
        assert_eq!(AutoArmorCompare::from_index(1), AutoArmorCompare::Protection);
        assert_eq!(AutoArmorCompare::from_index(9), AutoArmorCompare::Armor);
    }

    #[test]
    fn autoarmor_slots_are_the_games_own() {
        assert_eq!(ArmorSlot::Helmet.index(), 0);
        assert_eq!(ArmorSlot::Helmet.slot(), 500);
        assert_eq!(ArmorSlot::Boots.index(), 3);
        assert_eq!(ArmorSlot::Boots.slot(), 503);
        for slot in [
            ArmorSlot::Helmet,
            ArmorSlot::Chest,
            ArmorSlot::Legs,
            ArmorSlot::Boots,
        ] {
            assert_eq!(ArmorSlot::from_name(slot.name()), Some(slot));
        }
        assert_eq!(ArmorSlot::from_name("offhand"), None);
        assert_eq!(ArmorSlot::from_name("body"), None);
    }

    #[test]
    fn autoarmor_defaults_match_the_registry() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOARMOR).unwrap();
            m.enabled = true;
        });
        assert_eq!(autoarmor().unwrap(), AutoArmorConfig::default());
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOARMOR).unwrap();
            m.enabled = false;
        });
    }

    #[test]
    fn autoxp_starts_off_in_combat_on_silent() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == AUTOXP).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.name, "AutoXP");
            assert_eq!(m.category, Category::Combat);
            assert_eq!(m.setting("mode").unwrap().choice_options(), &AUTOXP_MODES[..]);
            assert_eq!(m.choice("mode"), 0);
            assert_eq!(m.int("frequency"), 1);
            assert!(m.bool("stop"));
            assert_eq!(m.int("stop_at"), 100);
        });
        assert!(autoxp().is_none());
    }

    #[test]
    fn autoxp_defaults_match_the_registry() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOXP).unwrap();
            m.enabled = true;
        });
        assert_eq!(autoxp().unwrap(), AutoXpConfig::default());
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOXP).unwrap();
            m.enabled = false;
        });
    }

    #[test]
    fn autoxp_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOXP).unwrap();
            m.enabled = true;
            m.setting_mut("mode").unwrap().value = Value::Choice(1);
            m.setting_mut("frequency").unwrap().value = Value::Int(16);
            m.setting_mut("rotate").unwrap().value = Value::Bool(true);
        });
        let config = autoxp().unwrap();
        assert_eq!(config.mode, AutoXpMode::Normal);
        assert!(!config.mode.silent());
        assert_eq!(config.frequency, 16);
        assert!(config.rotate);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOXP).unwrap();
            m.enabled = false;
            m.setting_mut("mode").unwrap().value = Value::Choice(0);
            m.setting_mut("frequency").unwrap().value = Value::Int(1);
            m.setting_mut("rotate").unwrap().value = Value::Bool(false);
        });
        assert!(autoxp().is_none());
    }

    #[test]
    fn autoxp_indices_map_to_the_choices() {
        assert_eq!(AutoXpMode::from_index(0), AutoXpMode::Silent);
        assert_eq!(AutoXpMode::from_index(1), AutoXpMode::Normal);
        assert_eq!(AutoXpMode::from_index(9), AutoXpMode::Silent);
        assert!(AutoXpMode::from_index(0).silent());
    }

    #[test]
    fn holefill_starts_off_in_combat_on_obsidian() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == HOLEFILL).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.name, "HoleFill");
            assert_eq!(m.category, Category::Combat);
            assert_eq!(
                m.setting("block").unwrap().choice_options(),
                &HOLEFILL_BLOCKS[..]
            );
            assert_eq!(m.int("range"), 4);
            assert_eq!(m.int("per_tick"), 1);
            assert!(!m.bool("quads"));
            assert!(m.bool("silent"));
        });
        assert!(holefill().is_none());
    }

    #[test]
    fn holefill_defaults_match_the_registry() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == HOLEFILL).unwrap();
            m.enabled = true;
        });
        assert_eq!(holefill().unwrap(), HoleFillConfig::default());
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == HOLEFILL).unwrap();
            m.enabled = false;
        });
    }

    #[test]
    fn holefill_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == HOLEFILL).unwrap();
            m.enabled = true;
            m.setting_mut("block").unwrap().value = Value::Choice(1);
            m.setting_mut("quads").unwrap().value = Value::Bool(true);
            m.setting_mut("per_tick").unwrap().value = Value::Int(4);
        });
        let config = holefill().unwrap();
        assert_eq!(config.block, HoleFillBlock::EnderChest);
        assert!(config.quads);
        assert_eq!(config.per_tick, 4);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == HOLEFILL).unwrap();
            m.enabled = false;
            m.setting_mut("block").unwrap().value = Value::Choice(0);
            m.setting_mut("quads").unwrap().value = Value::Bool(false);
            m.setting_mut("per_tick").unwrap().value = Value::Int(1);
        });
    }

    #[test]
    fn holefill_block_indices_map_to_the_choices() {
        assert_eq!(HoleFillBlock::from_index(0), HoleFillBlock::Obsidian);
        assert_eq!(HoleFillBlock::from_index(1), HoleFillBlock::EnderChest);
        assert_eq!(HoleFillBlock::from_index(2), HoleFillBlock::Any);
        assert_eq!(HoleFillBlock::from_index(9), HoleFillBlock::Obsidian);
    }

    #[test]
    fn reach_starts_off_in_combat_between_three_and_four() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == REACH).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.name, "Reach");
            assert_eq!(m.category, Category::Combat);
            assert!((m.float("min") - 3.0).abs() < 1e-6);
            assert!((m.float("max") - 4.0).abs() < 1e-6);
        });
        assert!(reach().is_none());
    }

    #[test]
    fn reach_defaults_match_the_registry() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == REACH).unwrap();
            m.enabled = true;
        });
        assert_eq!(reach().unwrap(), ReachConfig::default());
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == REACH).unwrap();
            m.enabled = false;
        });
    }

    #[test]
    fn reach_reads_the_bounds() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == REACH).unwrap();
            m.enabled = true;
            m.setting_mut("min").unwrap().value = Value::Float(4.5);
            m.setting_mut("max").unwrap().value = Value::Float(5.5);
        });
        let config = reach().unwrap();
        assert!((config.min - 4.5).abs() < 1e-6);
        assert!((config.max - 5.5).abs() < 1e-6);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == REACH).unwrap();
            m.enabled = false;
            m.setting_mut("min").unwrap().value = Value::Float(3.0);
            m.setting_mut("max").unwrap().value = Value::Float(4.0);
        });
    }

    #[test]
    fn reach_pick_stays_inside_the_bounds() {
        let config = ReachConfig {
            min: 3.0,
            max: 4.0,
        };
        assert!((config.pick(0.0) - 3.0).abs() < 1e-6);
        assert!((config.pick(1.0) - 4.0).abs() < 1e-6);
        assert!((config.pick(0.5) - 3.5).abs() < 1e-6);
        // A roll outside 0..1 is clamped, not extrapolated.
        assert!((config.pick(-1.0) - 3.0).abs() < 1e-6);
        assert!((config.pick(2.0) - 4.0).abs() < 1e-6);
        // The sliders can be crossed; the pick still lands between them.
        let crossed = ReachConfig {
            min: 4.0,
            max: 3.0,
        };
        assert!((crossed.pick(0.0) - 3.0).abs() < 1e-6);
        assert!((crossed.pick(1.0) - 4.0).abs() < 1e-6);
    }

    #[test]
    fn autoclicker_starts_off_in_combat_on_eight_to_fourteen() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == AUTOCLICKER).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.name, "AutoClicker");
            assert_eq!(m.category, Category::Combat);
            assert_eq!(
                m.setting("mode").unwrap().choice_options(),
                &AUTOCLICKER_MODES[..]
            );
            assert!(m.bool("left"));
            assert!(!m.bool("right"));
            assert!((m.float("min_cps") - 8.0).abs() < 1e-6);
            assert!((m.float("max_cps") - 14.0).abs() < 1e-6);
            assert!(m.bool("stop_on_block"));
        });
        assert!(autoclicker().is_none());
    }

    #[test]
    fn autoclicker_defaults_match_the_registry() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOCLICKER).unwrap();
            m.enabled = true;
        });
        assert_eq!(autoclicker().unwrap(), AutoClickerConfig::default());
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOCLICKER).unwrap();
            m.enabled = false;
        });
    }

    #[test]
    fn autoclicker_shows_the_selected_modes_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == AUTOCLICKER).unwrap();
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("min_cps"));
            assert!(visible("max_cps"));
            assert!(!visible("drag_length"));
            assert!(!visible("drag_delay"));
        });
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == AUTOCLICKER)
                .unwrap()
                .setting_mut("mode")
                .unwrap()
                .set_text("Drag");
        });
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == AUTOCLICKER).unwrap();
            assert_eq!(m.choice("mode"), 1);
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("drag_length"));
            assert!(visible("drag_delay"));
            assert!(!visible("min_cps"));
        });
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AUTOCLICKER).unwrap();
            m.setting_mut("mode").unwrap().set_text("Normal");
        });
    }

    #[test]
    fn autoclicker_cps_and_interval_stay_sane() {
        let config = AutoClickerConfig::default();
        assert!((config.cps(0.0) - 8.0).abs() < 1e-6);
        assert!((config.cps(1.0) - 14.0).abs() < 1e-6);
        // A roll outside 0..1 is clamped, and crossed sliders still work.
        assert!((config.cps(-1.0) - 8.0).abs() < 1e-6);
        let crossed = AutoClickerConfig {
            min_cps: 14.0,
            max_cps: 8.0,
            ..AutoClickerConfig::default()
        };
        assert!((crossed.cps(0.0) - 8.0).abs() < 1e-6);
        assert_eq!(AutoClickerConfig::interval_ms(20.0), 50);
        assert_eq!(AutoClickerConfig::interval_ms(8.0), 125);
        // A rate at or below zero can never divide by zero.
        assert_eq!(AutoClickerConfig::interval_ms(0.0), 1000);
    }

    #[test]
    fn autoclicker_mode_indices_map_to_the_choices() {
        assert_eq!(AutoClickerMode::from_index(0), AutoClickerMode::Normal);
        assert_eq!(AutoClickerMode::from_index(1), AutoClickerMode::Drag);
        assert_eq!(AutoClickerMode::from_index(9), AutoClickerMode::Normal);
    }

    #[test]
    fn aimassist_starts_off_in_combat_on_a_ninety_degree_cone() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == AIMASSIST).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.name, "AimAssist");
            assert_eq!(m.category, Category::Combat);
            assert_eq!(
                m.setting("point").unwrap().choice_options(),
                &AIMASSIST_POINTS[..]
            );
            assert!((m.float("range") - 4.5).abs() < 1e-6);
            assert!((m.float("fov") - 90.0).abs() < 1e-6);
            assert!((m.float("speed") - 4.0).abs() < 1e-6);
            assert!(m.bool("yaw"));
            assert!(!m.bool("pitch"));
            assert!(m.bool("players"));
            assert!(!m.bool("click_aim"));
        });
        assert!(aimassist().is_none());
    }

    #[test]
    fn aimassist_defaults_match_the_registry() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AIMASSIST).unwrap();
            m.enabled = true;
        });
        assert_eq!(aimassist().unwrap(), AimAssistConfig::default());
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AIMASSIST).unwrap();
            m.enabled = false;
        });
    }

    #[test]
    fn aimassist_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AIMASSIST).unwrap();
            m.enabled = true;
            m.setting_mut("point").unwrap().value = Value::Choice(2);
            m.setting_mut("pitch").unwrap().value = Value::Bool(true);
            m.setting_mut("speed").unwrap().value = Value::Float(9.0);
        });
        let config = aimassist().unwrap();
        assert_eq!(config.point, AimPoint::Legs);
        assert!(config.pitch);
        assert!((config.speed - 9.0).abs() < 1e-6);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AIMASSIST).unwrap();
            m.enabled = false;
            m.setting_mut("point").unwrap().value = Value::Choice(0);
            m.setting_mut("pitch").unwrap().value = Value::Bool(false);
            m.setting_mut("speed").unwrap().value = Value::Float(4.0);
        });
    }

    #[test]
    fn aimassist_point_indices_map_to_the_choices() {
        assert_eq!(AimPoint::from_index(0), AimPoint::Head);
        assert_eq!(AimPoint::from_index(1), AimPoint::Chest);
        assert_eq!(AimPoint::from_index(2), AimPoint::Legs);
        assert_eq!(AimPoint::from_index(9), AimPoint::Head);
    }

    #[test]
    fn surround_starts_off_in_combat_on_obsidian() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == SURROUND).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.name, "Surround");
            assert_eq!(m.category, Category::Combat);
            assert_eq!(
                m.setting("block").unwrap().choice_options(),
                &SURROUND_BLOCKS[..]
            );
            assert_eq!(m.int("per_tick"), 2);
            assert_eq!(m.int("delay"), 2);
            assert!(m.bool("on_ground"));
            assert!(m.bool("silent"));
            assert!(!m.bool("cover"));
        });
        assert!(surround().is_none());
    }

    #[test]
    fn surround_defaults_match_the_registry() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == SURROUND).unwrap();
            m.enabled = true;
        });
        assert_eq!(surround().unwrap(), SurroundConfig::default());
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == SURROUND).unwrap();
            m.enabled = false;
        });
    }

    #[test]
    fn surround_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == SURROUND).unwrap();
            m.enabled = true;
            m.setting_mut("block").unwrap().value = Value::Choice(1);
            m.setting_mut("cover").unwrap().value = Value::Bool(true);
            m.setting_mut("per_tick").unwrap().value = Value::Int(4);
        });
        let config = surround().unwrap();
        assert_eq!(config.block, SurroundBlock::EnderChest);
        assert!(config.cover);
        assert_eq!(config.per_tick, 4);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == SURROUND).unwrap();
            m.enabled = false;
            m.setting_mut("block").unwrap().value = Value::Choice(0);
            m.setting_mut("cover").unwrap().value = Value::Bool(false);
            m.setting_mut("per_tick").unwrap().value = Value::Int(2);
        });
    }

    #[test]
    fn surround_block_indices_map_to_the_choices() {
        assert_eq!(SurroundBlock::from_index(0), SurroundBlock::Obsidian);
        assert_eq!(SurroundBlock::from_index(1), SurroundBlock::EnderChest);
        assert_eq!(SurroundBlock::from_index(2), SurroundBlock::Any);
        assert_eq!(SurroundBlock::from_index(9), SurroundBlock::Obsidian);
    }

    #[test]
    fn crystalaur_starts_off_in_combat_with_both_halves_on() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == CRYSTALAURA).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.name, "CrystalAura");
            assert_eq!(m.category, Category::Combat);
            assert!(m.bool("place"));
            assert!(m.bool("breaking"));
            assert!(m.bool("instant"));
            assert!(!m.bool("rotate"));
            assert!((m.float("min_damage") - 15.0).abs() < 1e-6);
            assert!((m.float("max_self_damage") - 40.0).abs() < 1e-6);
        });
        assert!(crystal_aura().is_none());
    }

    #[test]
    fn crystalaur_defaults_match_the_registry() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == CRYSTALAURA).unwrap();
            m.enabled = true;
        });
        assert_eq!(crystal_aura().unwrap(), CrystalAuraConfig::default());
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == CRYSTALAURA).unwrap();
            m.enabled = false;
        });
    }

    #[test]
    fn crystalaur_reads_and_clamps_the_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == CRYSTALAURA).unwrap();
            m.enabled = true;
            m.setting_mut("place_range").unwrap().value = Value::Float(6.0);
            m.setting_mut("rotate").unwrap().value = Value::Bool(true);
            m.setting_mut("breaking").unwrap().value = Value::Bool(false);
        });
        let config = crystal_aura().unwrap();
        assert!((config.place_range - 6.0).abs() < 1e-6);
        assert!(config.rotate);
        assert!(!config.breaking);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == CRYSTALAURA).unwrap();
            m.enabled = false;
            m.setting_mut("place_range").unwrap().value = Value::Float(4.5);
            m.setting_mut("rotate").unwrap().value = Value::Bool(false);
            m.setting_mut("breaking").unwrap().value = Value::Bool(true);
        });
    }

    #[test]
    fn a_quad_needs_four_sealed_floors_and_eight_sealed_walls() {
        use std::collections::HashMap;
        let mut world: HashMap<[i32; 3], HoleMaterial> = HashMap::new();
        let cells = [(0, 0), (1, 0), (0, 1), (1, 1)];
        for (cx, cz) in cells {
            world.insert([cx, -1, cz], HoleMaterial::Bedrock);
        }
        for (cx, cz) in cells {
            for (nx, nz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let wall = (cx + nx, cz + nz);
                if !cells.contains(&wall) {
                    world.insert([wall.0, 0, wall.1], HoleMaterial::Bedrock);
                }
            }
        }
        let block =
            |x: i32, y: i32, z: i32| world.get(&[x, y, z]).copied().unwrap_or(HoleMaterial::Air);
        let holes = scan_quads([0, 0, 0], 4, 2, false, &block);
        assert_eq!(holes.len(), 1, "{holes:?}");
        assert_eq!((holes[0].x, holes[0].y, holes[0].z), (0, 0, 0));
        assert_eq!((holes[0].width, holes[0].depth), (2, 2));
        assert_eq!(holes[0].kind, HoleKind::Bedrock);
        // A quad is not a 1x1 or 1x2 hole: the scanners do not overlap.
        let mut config = HoleEspConfig::default();
        config.radius = 4;
        config.doubles = true;
        assert!(scan_holes([0, 0, 0], &config, &block).is_empty());
    }

    #[test]
    fn a_broken_wall_or_a_blocked_cell_is_not_a_quad() {
        use std::collections::HashMap;

        fn quads(world: &HashMap<[i32; 3], HoleMaterial>, ignore_own: bool) -> Vec<Hole> {
            let block = |x: i32, y: i32, z: i32| {
                world.get(&[x, y, z]).copied().unwrap_or(HoleMaterial::Air)
            };
            scan_quads([0, 0, 0], 4, 2, ignore_own, block)
        }

        let mut world: HashMap<[i32; 3], HoleMaterial> = HashMap::new();
        let cells = [(0, 0), (1, 0), (0, 1), (1, 1)];
        for (cx, cz) in cells {
            world.insert([cx, -1, cz], HoleMaterial::Bedrock);
        }
        for (cx, cz) in cells {
            for (nx, nz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let wall = (cx + nx, cz + nz);
                if !cells.contains(&wall) {
                    world.insert([wall.0, 0, wall.1], HoleMaterial::Bedrock);
                }
            }
        }
        assert_eq!(quads(&world, false).len(), 1);
        // One wall replaced by ordinary stone: the quad is no longer sealed.
        world.insert([2, 0, 0], HoleMaterial::Solid);
        assert!(quads(&world, false).is_empty());
        world.insert([2, 0, 0], HoleMaterial::Bedrock);
        // Something standing in a cell takes the head space away.
        world.insert([1, 1, 1], HoleMaterial::Solid);
        assert!(quads(&world, false).is_empty());
        // The player's own cell is skipped when asked.
        world.remove(&[1, 1, 1]);
        assert!(quads(&world, true).is_empty());
    }

    #[test]
    fn phase_starts_off_in_movement_on_vanilla() {        with(|c| {
            let m = c.modules.iter().find(|m| m.id == PHASE).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Movement);
            assert_eq!(m.name, "Phase");
            // The references all start on their plain mode, and Alan's and
            // FDP's plain mode is the 0.5 burst.
            assert_eq!(m.choice("mode"), 0);
            assert_eq!(m.int("delay"), 2);
            // The push has to clear vanilla's 0.1 push-out, or Nudge loses as
            // much as it gains.
            assert!(m.float("push") > 0.1);
            assert_eq!(m.float("pitch"), 85.0);
            assert_eq!(m.int("scan"), 10);
        });
        assert!(phase().is_none());
    }

    #[test]
    fn phase_modes_are_the_deduplicated_mechanisms() {
        assert_eq!(PHASE_MODES.len(), 9);
        // One entry per distinct mechanism, and the repeats are gone: no second
        // BlocksMC, no second NCP variant, no packet-hold mode.
        assert_eq!(
            PHASE_MODES,
            [
                "Vanilla", "Skip", "Mineplex", "Clip", "Center", "Nudge", "Break", "Sink", "Pearl",
            ]
        );
        for (index, mode) in [
            PhaseMode::Vanilla,
            PhaseMode::Skip,
            PhaseMode::Mineplex,
            PhaseMode::Clip,
            PhaseMode::Center,
            PhaseMode::Nudge,
            PhaseMode::Break,
            PhaseMode::Sink,
            PhaseMode::Pearl,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(PhaseMode::from_index(index), mode);
        }
        // Out of range falls back to the first mode rather than panicking.
        assert_eq!(PhaseMode::from_index(99), PhaseMode::Vanilla);
    }

    #[test]
    fn phase_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == PHASE).unwrap();
            m.enabled = true;
            m.setting_mut("mode").unwrap().value = Value::Choice(8);
            m.setting_mut("delay").unwrap().value = Value::Int(6);
            m.setting_mut("push").unwrap().value = Value::Float(0.4);
            m.setting_mut("pitch").unwrap().value = Value::Float(70.0);
            m.setting_mut("scan").unwrap().value = Value::Int(4);
        });
        let config = phase().unwrap();
        assert_eq!(config.mode, PhaseMode::Pearl);
        assert_eq!(config.delay, 6);
        // The setting is an `f32`, so the widened `f64` is only close to it.
        assert!((config.push - 0.4).abs() < 1.0e-6, "push {}", config.push);
        assert_eq!(config.pitch, 70.0);
        assert_eq!(config.scan, 4);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == PHASE).unwrap();
            m.enabled = false;
            m.setting_mut("mode").unwrap().value = Value::Choice(0);
            m.setting_mut("delay").unwrap().value = Value::Int(2);
            m.setting_mut("push").unwrap().value = Value::Float(0.15);
            m.setting_mut("pitch").unwrap().value = Value::Float(85.0);
            m.setting_mut("scan").unwrap().value = Value::Int(10);
        });
        assert!(phase().is_none());
    }

    #[test]
    fn phase_clamps_settings_a_config_could_not_have_written() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == PHASE).unwrap();
            m.enabled = true;
            m.setting_mut("delay").unwrap().value = Value::Int(0);
            m.setting_mut("push").unwrap().value = Value::Float(0.0);
            m.setting_mut("pitch").unwrap().value = Value::Float(-5.0);
            m.setting_mut("scan").unwrap().value = Value::Int(0);
        });
        let config = phase().unwrap();
        assert_eq!(config.delay, 1);
        assert!((config.push - 0.005).abs() < 1.0e-9, "push {}", config.push);
        assert_eq!(config.pitch, 0.0);
        assert_eq!(config.scan, 1);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == PHASE).unwrap();
            m.enabled = false;
            m.setting_mut("delay").unwrap().value = Value::Int(2);
            m.setting_mut("push").unwrap().value = Value::Float(0.15);
            m.setting_mut("pitch").unwrap().value = Value::Float(85.0);
            m.setting_mut("scan").unwrap().value = Value::Int(10);
        });
    }

    #[test]
    fn animations_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == ANIMATIONS).unwrap();
            m.enabled = true;
            m.setting_mut("swing_speed").unwrap().value = Value::Float(50.0);
            m.setting_mut("sway").unwrap().value = Value::Float(100.0);
            m.setting_mut("eating").unwrap().value = Value::Bool(false);
        });
        let config = animations().unwrap();
        assert_eq!(config.swing_speed, 50.0);
        assert_eq!(config.sway, 100.0);
        assert!(!config.eating);
        assert!(!config.is_vanilla());
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == ANIMATIONS).unwrap();
            m.enabled = false;
            m.setting_mut("swing_speed").unwrap().value = Value::Float(0.0);
            m.setting_mut("sway").unwrap().value = Value::Float(0.0);
            m.setting_mut("eating").unwrap().value = Value::Bool(true);
        });
        assert!(animations().is_none());
    }

    #[test]
    fn ambience_starts_off_in_render_and_leaves_the_world_alone() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == AMBIENCE).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Render);
            assert_eq!(m.setting("time").unwrap().as_choice(), 0);
            assert_eq!(m.setting("weather").unwrap().as_choice(), 0);
            assert_eq!(m.setting("fog").unwrap().as_choice(), 0);
            assert_eq!(
                m.setting("time").unwrap().choice_options(),
                &AMBIENCE_TIME_MODES[..]
            );
        });
        assert!(ambience().is_none());
        assert!(AmbienceConfig::default().is_vanilla());
    }

    #[test]
    fn ambience_reads_the_selected_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AMBIENCE).unwrap();
            m.enabled = true;
            m.setting_mut("time").unwrap().value = Value::Choice(7);
            m.setting_mut("weather").unwrap().value = Value::Choice(3);
            m.setting_mut("fog").unwrap().value = Value::Choice(1);
            m.setting_mut("fog_color").unwrap().value = Value::Text("#102030".to_owned());
            m.setting_mut("fog_start").unwrap().value = Value::Float(0.2);
            m.setting_mut("fog_end").unwrap().value = Value::Float(0.9);
        });
        let config = ambience().unwrap();
        assert_eq!(config.time, AmbienceTime::Midnight);
        assert_eq!(config.weather, AmbienceWeather::Thunder);
        assert_eq!(config.fog, AmbienceFog::Override);
        assert_eq!(config.fog_color, 0xFF10_2030);
        assert_eq!(config.fog_start, 0.2);
        assert_eq!(config.fog_end, 0.9);
        assert!(!config.is_vanilla());
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == AMBIENCE).unwrap();
            m.enabled = false;
            for id in ["time", "weather", "fog"] {
                m.setting_mut(id).unwrap().value = Value::Choice(0);
            }
            m.setting_mut("fog_color").unwrap().value = Value::Text("#6E7FD8".to_owned());
            m.setting_mut("fog_start").unwrap().value = Value::Float(0.5);
            m.setting_mut("fog_end").unwrap().value = Value::Float(1.0);
        });
        assert!(ambience().is_none());
    }

    #[test]
    fn every_time_of_day_is_a_time_the_sky_knows() {
        // Off and Real time fix nothing; every other mode is a tick of the day.
        assert!(AmbienceTime::Off.ticks().is_none());
        assert!(AmbienceTime::RealTime.ticks().is_none());
        assert!(AmbienceTime::RealTime.real_time());
        let mut seen = std::collections::BTreeSet::new();
        for mode in [
            AmbienceTime::Dawn,
            AmbienceTime::Morning,
            AmbienceTime::Noon,
            AmbienceTime::Sunset,
            AmbienceTime::Evening,
            AmbienceTime::Night,
            AmbienceTime::Midnight,
        ] {
            let ticks = mode.ticks().unwrap();
            assert!((0..24_000).contains(&ticks), "{mode:?} is {ticks}");
            assert!(seen.insert(ticks), "{mode:?} repeats {ticks}");
        }
        // The names mean what the sky does: noon is noon, midnight is midnight.
        assert_eq!(AmbienceTime::Noon.ticks(), Some(6_000));
        assert_eq!(AmbienceTime::Midnight.ticks(), Some(18_000));
        // Dawn is the last tick before sunrise, not the first of the day.
        assert_eq!(AmbienceTime::Dawn.ticks(), Some(23_000));
    }

    #[test]
    fn the_weather_modes_are_the_four_the_world_has() {
        assert_eq!(AmbienceWeather::Off.levels(), None);
        assert_eq!(AmbienceWeather::Clear.levels(), Some((false, false)));
        assert_eq!(AmbienceWeather::Rain.levels(), Some((true, false)));
        // Thunder is rain that also thunders, as the game counts it.
        assert_eq!(AmbienceWeather::Thunder.levels(), Some((true, true)));
        assert!(!AmbienceFog::Off.overrides());
        assert!(AmbienceFog::Override.overrides());
    }

    #[test]
    fn a_colour_splits_into_the_three_components_fog_takes() {
        assert_eq!(rgb(0xFFFF_FFFF), [1.0, 1.0, 1.0]);
        assert_eq!(rgb(0xFF00_0000), [0.0, 0.0, 0.0]);
        let [r, g, b] = rgb(0xFF10_2030);
        assert!((r - 16.0 / 255.0).abs() < 1e-6);
        assert!((g - 32.0 / 255.0).abs() < 1e-6);
        assert!((b - 48.0 / 255.0).abs() < 1e-6);
        // The alpha is not part of the fog.
        assert_eq!(rgb(0x0010_2030), rgb(0xFF10_2030));
    }

    #[test]
    fn zoom_starts_off_in_render_on_z_with_the_reference_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == ZOOM).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Render);
            assert_eq!(m.bind.as_deref(), Some("key.keyboard.z"));
            assert_eq!(
                m.setting("level").unwrap().choice_options(),
                &ZOOM_LEVELS[..]
            );
            assert_eq!(m.setting("level").unwrap().as_choice(), 2);
            assert!(m.bool("smooth"));
            assert!(m.bool("sensitivity"));
        });
        assert_eq!(super::zoom_view(), None);
    }

    #[test]
    fn zoom_holds_the_starting_level_and_the_wheel_moves_it() {
        with(|c| {
            c.modules.iter_mut().find(|m| m.id == ZOOM).unwrap().enabled = false;
        });
        assert!(super::zoom_bound_key("key.keyboard.z"));
        with(|c| {
            c.modules.iter_mut().find(|m| m.id == ZOOM).unwrap().enabled = true;
        });
        super::zoom_key(true, false);
        assert_eq!(super::zoom_view(), None, "not while a screen is open");
        super::zoom_key(true, true);
        assert_eq!(super::zoom_view(), Some((4.0, true, 0.25)));
        assert!(super::zoom_wheel(1.0));
        assert_eq!(super::zoom_view().map(|v| v.0), Some(5.0));
        assert!(super::zoom_wheel(40.0));
        assert_eq!(super::zoom_view().map(|v| v.0), Some(10.0));
        assert!(super::zoom_held());
        super::zoom_key(false, false);
        assert!(!super::zoom_held());
        assert_eq!(super::zoom_view(), None);
        assert!(!super::zoom_wheel(1.0));
        super::zoom_release();
        with(|c| {
            c.modules.iter_mut().find(|m| m.id == ZOOM).unwrap().enabled = false;
        });
    }

    #[test]
    fn a_key_can_drive_several_modules() {
        with(|c| {
            for id in [FULLBRIGHT, ARRAYLIST] {
                c.modules.iter_mut().find(|m| m.id == id).unwrap().bind =
                    Some("key.keyboard.g".to_owned());
            }
        });
        let ids = super::modules_bound_to("key.keyboard.g");
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&FULLBRIGHT));
        assert!(ids.contains(&ARRAYLIST));
        // The first bound module still answers the old single-module lookup.
        assert_eq!(super::module_bound_to("key.keyboard.g"), Some(FULLBRIGHT));
        with(|c| {
            for id in [FULLBRIGHT, ARRAYLIST] {
                c.modules.iter_mut().find(|m| m.id == id).unwrap().bind = None;
            }
        });
        assert!(super::modules_bound_to("key.keyboard.g").is_empty());
    }

    #[test]
    fn colours_accept_hex_and_java_integers() {
        assert_eq!(parse_color("#FF34FF60", 0), 0xFF34_FF60);
        assert_eq!(parse_color("ff34ff60", 0), 0xFF34_FF60);
        assert_eq!(parse_color("#34FF60", 0), 0xFF34_FF60);
        assert_eq!(parse_color("-13303968", 0), 0xFF34_FF60);
        assert_eq!(parse_color("4294967295", 0), 0xFFFF_FFFF);
        assert_eq!(parse_color("", 0x1234_5678), 0x1234_5678);
        assert_eq!(parse_color("nope", 0x1234_5678), 0x1234_5678);
    }

    #[test]
    fn criticals_starts_off_in_combat_with_the_reference_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == CRITICALS).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Combat);
            assert_eq!(
                m.setting("mode").unwrap().choice_options(),
                &CRITICALS_MODES[..]
            );
            assert_eq!(m.setting("mode").unwrap().as_choice(), 1);
            assert_eq!(m.setting("sprint_stop").unwrap().as_choice(), 1);
            assert_eq!(m.setting("sprint_range").unwrap().as_float(), 4.0);
            assert!(!m.setting("only_sprinting").unwrap().as_bool());
            assert_eq!(m.setting("packet_mode").unwrap().as_choice(), 1);
            assert_eq!(m.setting("packet_type").unwrap().as_choice(), 0);
            assert_eq!(m.setting("delay").unwrap().as_int(), 0);
            assert_eq!(m.setting("hurt_time").unwrap().as_int(), 10);
            assert_eq!(m.setting("jump_height").unwrap().as_float(), 0.42);
            assert_eq!(m.setting("jump_range").unwrap().as_float(), 4.0);
            assert!(m.setting("jump_optimize").unwrap().as_bool());
            assert!(m.setting("jump_can_be_seen").unwrap().as_bool());
            assert_eq!(m.setting("blink_delay_min").unwrap().as_int(), 300);
            assert_eq!(m.setting("blink_delay_max").unwrap().as_int(), 600);
            assert_eq!(m.setting("blink_range").unwrap().as_float(), 4.0);
            assert_eq!(m.setting("timer_speed").unwrap().as_float(), 0.8);
            assert_eq!(m.setting("custom_motion_y").unwrap().as_float(), 0.2);
            assert_eq!(m.setting("edit_delay").unwrap().as_int(), 500);
            assert_eq!(m.setting("alan_delay").unwrap().as_int(), 500);
            assert_eq!(m.setting("watchdog_delay").unwrap().as_int(), 500);
            assert_eq!(m.setting("ncp_ticks").unwrap().as_int(), 6);
            assert_eq!(m.setting("summer_delay").unwrap().as_int(), 500);
            assert_eq!(m.setting("summer_ground_ticks").unwrap().as_int(), 1);
            assert_eq!(m.setting("tenacity_watchdog_mode").unwrap().as_choice(), 0);
            assert_eq!(m.setting("tenacity_delay").unwrap().as_int(), 1);
            assert_eq!(m.setting("flux_hurt_time").unwrap().as_int(), 15);
            assert_eq!(m.setting("flux_delay").unwrap().as_int(), 300);
            assert!(m.setting("flux_post_motion").unwrap().as_bool());
            assert_eq!(m.setting("sigma_hurt_time").unwrap().as_int(), 15);
            assert_eq!(m.setting("deadcode_fall_distance").unwrap().as_float(), 0.2);
            assert_eq!(m.setting("deadcode_delay").unwrap().as_int(), 0);
            assert!(m.setting("deadcode_smart").unwrap().as_bool());
            assert!(!m.setting("celestial_mini_jump").unwrap().as_bool());
            assert_eq!(m.setting("celestial_ncp_delay").unwrap().as_int(), 800);
            assert!(!m.setting("celestial_ground_false").unwrap().as_bool());
            assert!(m.setting("celestial_reduce_damage").unwrap().as_bool());
            assert!(m.setting("novoline_hypixel").unwrap().as_bool());
            assert_eq!(m.setting("novoline_random").unwrap().as_float(), 0.001);
            assert!(!m.setting("visuals").unwrap().as_bool());
        });
        assert!(super::criticals().is_none());

        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == CRITICALS)
                .unwrap()
                .enabled = true;
        });
        let config = super::criticals().unwrap();
        assert_eq!(config.mode, CriticalsMode::Packet);
        assert_eq!(config.sprint_stop, 1);
        assert_eq!(config.packet_mode, 1);
        assert_eq!(config.hurt_time, 10);
        assert_eq!(config.edit_delay, 500);
        assert_eq!(config.celestial_ncp_delay, 800);
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == CRITICALS)
                .unwrap()
                .enabled = false;
        });
    }

    #[test]
    fn criticals_mode_indices_match_the_label_list() {
        let mut seen = Vec::new();
        for index in 0..CRITICALS_MODES.len() {
            let mode = CriticalsMode::from_index(index);
            assert!(!seen.contains(&mode), "{} reused", CRITICALS_MODES[index]);
            seen.push(mode);
        }
        assert_eq!(seen.len(), CRITICALS_MODES.len());
        assert_eq!(CriticalsMode::from_index(0), CriticalsMode::Off);
        assert_eq!(CriticalsMode::from_index(1), CriticalsMode::Packet);
        assert_eq!(CriticalsMode::from_index(19), CriticalsMode::Watchdog);
        assert_eq!(
            CriticalsMode::from_index(48),
            CriticalsMode::CelestialMatrix630
        );
        assert_eq!(CriticalsMode::from_index(49), CriticalsMode::Novoline);
        assert_eq!(CriticalsMode::from_index(99), CriticalsMode::Off);
    }

    #[test]
    fn criticals_shows_only_the_selected_modes_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == CRITICALS).unwrap();
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("sprint_stop"));
            assert!(visible("packet_mode"));
            assert!(visible("hurt_time"));
            assert!(!visible("jump_height"));
            assert!(!visible("blink_delay_min"));
            assert!(!visible("tenacity_delay"));
            assert!(!visible("celestial_ncp_delay"));
        });
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == CRITICALS)
                .unwrap()
                .setting_mut("mode")
                .unwrap()
                .set_text("Jump");
        });
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == CRITICALS).unwrap();
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("jump_height"));
            assert!(visible("jump_range"));
            assert!(!visible("packet_mode"));
            assert!(!visible("blink_delay_min"));
        });
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == CRITICALS)
                .unwrap()
                .setting_mut("mode")
                .unwrap()
                .set_text("CelestialNCP");
        });
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == CRITICALS).unwrap();
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("celestial_ncp_delay"));
            assert!(visible("celestial_mini_jump"));
            assert!(!visible("celestial_ground_false"));
            assert!(!visible("celestial_reduce_damage"));
            assert!(!visible("jump_height"));
        });
    }

    #[test]
    fn scaffold_starts_off_in_player_with_the_reference_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == SCAFFOLD).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Player);
            assert_eq!(
                m.setting("mode").unwrap().choice_options(),
                &SCAFFOLD_MODES[..]
            );
            assert_eq!(m.setting("mode").unwrap().as_choice(), 1);
            assert_eq!(m.setting("place_delay").unwrap().as_int(), 90);
            assert!(m.setting("rotations").unwrap().as_bool());
            assert!(m.setting("silent").unwrap().as_bool());
            assert_eq!(
                m.setting("sprint").unwrap().choice_options(),
                &SCAFFOLD_SPRINTS[..]
            );
            assert_eq!(m.setting("sprint").unwrap().as_choice(), 1);
            assert_eq!(
                m.setting("tower").unwrap().choice_options(),
                &SCAFFOLD_TOWERS[..]
            );
            assert_eq!(m.setting("tower").unwrap().as_choice(), 0);
            assert!(m.setting("tower_move").unwrap().as_bool());
            assert!(!m.setting("keep_y").unwrap().as_bool());
            assert_eq!(
                m.setting("downwards").unwrap().choice_options(),
                &SCAFFOLD_DOWNWARDS[..]
            );
            assert_eq!(m.setting("downwards").unwrap().as_choice(), 0);
            assert!(!m.setting("sneak").unwrap().as_bool());
            assert!(m.setting("swing").unwrap().as_bool());
            assert!(!m.setting("auto_jump").unwrap().as_bool());
            assert_eq!(m.setting("extend").unwrap().as_float(), 0.4);
        });
    }

    #[test]
    fn scaffold_mode_indices_match_the_label_lists() {
        let mut seen = Vec::new();
        for index in 0..SCAFFOLD_MODES.len() {
            let mode = ScaffoldMode::from_index(index);
            assert!(!seen.contains(&mode), "{} reused", SCAFFOLD_MODES[index]);
            seen.push(mode);
        }
        assert_eq!(seen.len(), SCAFFOLD_MODES.len());
        assert_eq!(ScaffoldMode::from_index(0), ScaffoldMode::Off);
        assert_eq!(ScaffoldMode::from_index(1), ScaffoldMode::Normal);
        assert_eq!(ScaffoldMode::from_index(4), ScaffoldMode::Aac);
        assert_eq!(ScaffoldMode::from_index(8), ScaffoldMode::Novoline);
        assert_eq!(ScaffoldMode::from_index(14), ScaffoldMode::Breezily);
        assert_eq!(ScaffoldMode::from_index(15), ScaffoldMode::Eagle);
        assert_eq!(ScaffoldMode::from_index(99), ScaffoldMode::Off);

        let sprints: Vec<_> = (0..SCAFFOLD_SPRINTS.len())
            .map(ScaffoldSprint::from_index)
            .collect();
        assert_eq!(sprints.len(), SCAFFOLD_SPRINTS.len());
        assert_eq!(sprints[0], ScaffoldSprint::Off);
        assert_eq!(sprints[1], ScaffoldSprint::Disabled);
        assert_eq!(sprints[6], ScaffoldSprint::Watchdog);

        let towers: Vec<_> = (0..SCAFFOLD_TOWERS.len())
            .map(ScaffoldTower::from_index)
            .collect();
        assert_eq!(towers.len(), SCAFFOLD_TOWERS.len());
        assert_eq!(towers[0], ScaffoldTower::Off);
        assert_eq!(towers[5], ScaffoldTower::Watchdog);

        let downwards: Vec<_> = (0..SCAFFOLD_DOWNWARDS.len())
            .map(ScaffoldDownwards::from_index)
            .collect();
        assert_eq!(downwards.len(), SCAFFOLD_DOWNWARDS.len());
        assert_eq!(downwards[0], ScaffoldDownwards::Off);
        assert_eq!(downwards[2], ScaffoldDownwards::Verus);
    }

    #[test]
    fn scaffold_shows_only_the_selected_modes_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == SCAFFOLD).unwrap();
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("place_delay"));
            assert!(visible("rotations"));
            assert!(visible("silent"));
            assert!(visible("sprint"));
            assert!(visible("tower"));
            assert!(visible("downwards"));
            assert!(!visible("extend"));
        });
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == SCAFFOLD)
                .unwrap()
                .setting_mut("mode")
                .unwrap()
                .set_text("Expand");
        });
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == SCAFFOLD).unwrap();
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("extend"));
            assert!(visible("place_delay"));
        });
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == SCAFFOLD).unwrap();
            m.setting_mut("mode").unwrap().set_text("Off");
            m.enabled = false;
        });
    }

    #[test]
    fn scaffold_eagle_shows_only_its_own_settings() {
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == SCAFFOLD)
                .unwrap()
                .setting_mut("mode")
                .unwrap()
                .set_text("Eagle");
        });
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == SCAFFOLD).unwrap();
            assert_eq!(m.choice("mode"), 15);
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("eagle_length"));
            assert!(visible("eagle_ground"));
            assert!(visible("eagle_blocks"));
            assert!(visible("eagle_backwards"));
            // Eagle places nothing, so the placement settings stay hidden.
            assert!(!visible("place_delay"));
            assert!(!visible("rotations"));
            assert!(!visible("tower"));
            assert!(!visible("extend"));
        });
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == SCAFFOLD).unwrap();
            m.setting_mut("mode").unwrap().set_text("Normal");
            m.enabled = false;
        });
    }

    #[test]
    fn scaffold_reads_the_eagle_settings() {
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == SCAFFOLD).unwrap();
            m.enabled = true;
            m.setting_mut("mode").unwrap().set_text("Eagle");
            m.setting_mut("eagle_length").unwrap().value = Value::Int(3);
            m.setting_mut("eagle_ground").unwrap().value = Value::Bool(true);
        });
        let config = scaffold().unwrap();
        assert_eq!(config.mode, ScaffoldMode::Eagle);
        assert_eq!(config.eagle_length, 3);
        assert!(config.eagle_ground);
        assert!(!config.eagle_blocks);
        assert!(!config.eagle_backwards);
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == SCAFFOLD).unwrap();
            m.enabled = false;
            m.setting_mut("mode").unwrap().set_text("Normal");
            m.setting_mut("eagle_length").unwrap().value = Value::Int(1);
            m.setting_mut("eagle_ground").unwrap().value = Value::Bool(false);
        });
    }

    #[test]
    fn velocity_starts_off_in_combat_with_the_reference_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == VELOCITY).unwrap();
            assert!(!m.enabled);
            assert_eq!(m.category, Category::Combat);
            assert_eq!(
                m.setting("mode").unwrap().choice_options(),
                &VELOCITY_MODES[..]
            );
            assert_eq!(m.setting("mode").unwrap().as_choice(), 1);
            assert_eq!(m.setting("horizontal").unwrap().as_float(), 0.6);
            assert_eq!(m.setting("vertical").unwrap().as_float(), 1.0);
            assert_eq!(m.setting("motion_horizontal").unwrap().as_float(), 0.0);
            assert_eq!(m.setting("motion_vertical").unwrap().as_float(), 0.0);
            assert_eq!(m.setting("filter").unwrap().as_choice(), 0);
            assert!(m.setting("explosions").unwrap().as_bool());
            assert_eq!(m.setting("chance").unwrap().as_int(), 100);
            assert!(!m.setting("only_move").unwrap().as_bool());
            assert_eq!(m.setting("motion_hurt_time").unwrap().as_int(), 8);
            assert_eq!(m.setting("scale_horizontal").unwrap().as_float(), 0.6);
            assert_eq!(m.setting("scale_vertical").unwrap().as_float(), 1.0);
            assert_eq!(m.setting("delay_ticks").unwrap().as_int(), 3);
            assert_eq!(m.setting("x_modifier").unwrap().as_float(), -1.0);
            assert_eq!(m.setting("z_modifier").unwrap().as_float(), -1.0);
            assert_eq!(m.setting("wait_ticks").unwrap().as_int(), 3);
            assert_eq!(m.setting("reduction").unwrap().as_float(), 0.0);
            assert!(m.setting("reset_vertical").unwrap().as_bool());
            assert_eq!(m.setting("jump_delay").unwrap().as_int(), 0);
            assert!(!m.setting("only_sprinting").unwrap().as_bool());
            assert!(m.setting("air_only").unwrap().as_bool());
            assert_eq!(m.setting("y_boost").unwrap().as_float(), 0.0001);
            assert_eq!(m.setting("grim_duplicates").unwrap().as_int(), 4);
            assert!(m.setting("grim_block_action").unwrap().as_bool());
            assert_eq!(m.setting("blocksmc_ticks").unwrap().as_int(), 2);
            assert_eq!(m.setting("phase_height").unwrap().as_float(), 0.1);
            assert!(m.setting("phase_ground").unwrap().as_bool());
            assert_eq!(m.setting("click_count").unwrap().as_int(), 3);
            assert_eq!(m.setting("click_hurt_time").unwrap().as_int(), 9);
            assert_eq!(m.setting("click_interval").unwrap().as_int(), 0);
            assert_eq!(m.setting("side_strength").unwrap().as_float(), 0.6);
            assert_eq!(m.setting("aac_factor").unwrap().as_float(), 0.62);
            assert!(m.setting("aac_only_air").unwrap().as_bool());
            assert_eq!(m.setting("push_reducer").unwrap().as_float(), 1.2);
            assert!(!m.setting("push_reduce_y").unwrap().as_bool());
            assert_eq!(m.setting("push_y").unwrap().as_float(), 0.015);
            assert_eq!(m.setting("polar_divisor").unwrap().as_float(), 1.2224);
            assert_eq!(m.setting("polar_hurt_min").unwrap().as_int(), 1);
            assert_eq!(m.setting("polar_hurt_max").unwrap().as_int(), 6);
            assert_eq!(m.setting("intave_factor").unwrap().as_float(), 0.6);
            assert_eq!(m.setting("intave_hurt_min").unwrap().as_int(), 5);
            assert_eq!(m.setting("intave_hurt_max").unwrap().as_int(), 7);
            assert_eq!(m.setting("intave_window").unwrap().as_int(), 2000);
            assert_eq!(m.setting("intave_jump_chance").unwrap().as_int(), 50);
            assert!(m.setting("spoof_modify_timer").unwrap().as_bool());
            assert_eq!(m.setting("spoof_timer").unwrap().as_float(), 0.6);
        });
        assert!(super::velocity().is_none());

        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == VELOCITY)
                .unwrap()
                .enabled = true;
        });
        let config = super::velocity().unwrap();
        assert_eq!(config.mode, VelocityMode::Standard);
        assert_eq!(config.horizontal, 0.6);
        assert_eq!(config.vertical, 1.0);
        assert_eq!(config.filter, 0);
        assert_eq!(config.chance, 100);
        assert_eq!(config.delay_ticks, 3);
        assert_eq!(config.polar_divisor, 1.2224);
        assert_eq!(config.intave_window, 2000);
        assert!(config.spoof_modify_timer);
        assert_eq!(config.spoof_timer, 0.6);
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == VELOCITY)
                .unwrap()
                .enabled = false;
        });
    }

    #[test]
    fn velocity_mode_indices_match_the_label_list() {
        let mut seen = Vec::new();
        for index in 0..VELOCITY_MODES.len() {
            let mode = VelocityMode::from_index(index);
            assert!(!seen.contains(&mode), "{} reused", VELOCITY_MODES[index]);
            seen.push(mode);
        }
        assert_eq!(seen.len(), VELOCITY_MODES.len());
        assert_eq!(VelocityMode::from_index(0), VelocityMode::Off);
        assert_eq!(VelocityMode::from_index(1), VelocityMode::Standard);
        assert_eq!(VelocityMode::from_index(9), VelocityMode::Grim);
        assert_eq!(VelocityMode::from_index(10), VelocityMode::BlocksMc);
        assert_eq!(VelocityMode::from_index(13), VelocityMode::SideStrafe);
        assert_eq!(VelocityMode::from_index(17), VelocityMode::Intave);
        assert_eq!(VelocityMode::from_index(18), VelocityMode::NewGrim);
        assert_eq!(VelocityMode::from_index(19), VelocityMode::Spoof);
        assert_eq!(VelocityMode::from_index(20), VelocityMode::GrimC03);
        assert_eq!(VelocityMode::from_index(99), VelocityMode::Off);
    }

    #[test]
    fn velocity_shows_only_the_selected_modes_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == VELOCITY).unwrap();
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("horizontal"));
            assert!(visible("vertical"));
            assert!(visible("chance"));
            assert!(visible("only_move"));
            assert!(visible("explosions"));
            assert!(!visible("motion_hurt_time"));
            assert!(!visible("delay_ticks"));
            assert!(!visible("x_modifier"));
            assert!(!visible("jump_delay"));
            assert!(!visible("grim_duplicates"));
            assert!(!visible("polar_divisor"));
        });
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == VELOCITY)
                .unwrap()
                .setting_mut("mode")
                .unwrap()
                .set_text("Reverse");
        });
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == VELOCITY).unwrap();
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("x_modifier"));
            assert!(visible("z_modifier"));
            assert!(visible("delay_ticks"));
            assert!(visible("chance"));
            assert!(visible("only_move"));
            assert!(!visible("horizontal"));
            assert!(!visible("explosions"));
        });
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == VELOCITY)
                .unwrap()
                .setting_mut("mode")
                .unwrap()
                .set_text("Intave");
        });
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == VELOCITY).unwrap();
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("intave_factor"));
            assert!(visible("intave_jump_chance"));
            assert!(!visible("horizontal"));
            assert!(!visible("x_modifier"));
            assert!(!visible("spoof_timer"));
        });
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == VELOCITY)
                .unwrap()
                .setting_mut("mode")
                .unwrap()
                .set_text("Spoof");
        });
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == VELOCITY).unwrap();
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("spoof_modify_timer"));
            assert!(visible("spoof_timer"));
            assert!(!visible("horizontal"));
            assert!(!visible("intave_factor"));
        });
        with(|c| {
            let m = c.modules.iter_mut().find(|m| m.id == VELOCITY).unwrap();
            m.setting_mut("mode").unwrap().set_text("Off");
            m.enabled = false;
        });
    }

    #[test]
    fn friend_lists_are_comma_separated_and_case_insensitive() {
        let names = parse_names("Alice, bob ,, Carol");
        assert_eq!(names, vec!["Alice", "bob", "Carol"]);
        assert!(is_friend(&names, "alice"));
        assert!(is_friend(&names, "BOB"));
        assert!(!is_friend(&names, "dave"));
        assert!(!is_friend(&[], "alice"));
    }

    #[test]
    fn cheststealer_delays_are_whole_ticks() {
        assert_eq!(steal_delay_ticks(0), 1);
        assert_eq!(steal_delay_ticks(49), 1);
        assert_eq!(steal_delay_ticks(50), 1);
        assert_eq!(steal_delay_ticks(51), 2);
        assert_eq!(steal_delay_ticks(500), 10);
        assert_eq!(smart_delay_ms(1, 120, 0), 120);
        assert_eq!(smart_delay_ms(4, 120, 0), 240);
        assert_eq!(smart_delay_ms(2, 100, 20), 161);
        assert_eq!(smart_delay_ms(0, 120, 99), 20);
    }

    #[test]
    fn cheststealer_slot_distance_is_the_grid_distance() {
        assert_eq!(slot_distance_squared(300, 301), 1);
        assert_eq!(slot_distance_squared(300, 309), 1);
        assert_eq!(slot_distance_squared(300, 310), 2);
        assert_eq!(slot_distance_squared(300, 300), 0);
        // The last and first columns of a row are still neighbours.
        assert_eq!(slot_distance_squared(305, 306), 1);
        assert_eq!(slot_distance_squared(308, 309), 8 * 8 + 1);
    }

    #[test]
    fn cheststealer_defaults_are_the_fdpclient_ones() {
        with(|c| {
            let module = c.modules.iter_mut().find(|m| m.id == CHESTSTEALER).unwrap();
            assert_eq!(module.category, Category::Player);
            module.enabled = true;
        });
        let config = cheststealer().unwrap();
        assert!(!config.smart_delay);
        assert_eq!(config.delay_multiplier, 120);
        assert!(config.smart_order);
        assert!(config.auto_close);
        assert_eq!(config.order, 0);
        assert_eq!(config.on_full, 0);
        assert!(config.chest_title);
        assert!(config.random_slot);
        assert!(config.progress_bar);
        assert!(!config.furnace);
        assert_eq!(config.border_strength, 3);
        assert_eq!(config.background_color, 0xFF80_8080);
        assert_eq!(config.debug, 0);
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == CHESTSTEALER)
                .unwrap()
                .enabled = false;
        });
    }

    #[test]
    fn fastbreak_starts_off_in_player_with_the_reference_settings() {
        with(|c| {
            let module = c.modules.iter_mut().find(|m| m.id == FASTBREAK).unwrap();
            assert_eq!(module.category, Category::Player);
            assert_eq!(
                module.setting("mode").unwrap().choice_options(),
                &FASTBREAK_MODES[..]
            );
            module.enabled = true;
        });
        let config = fastbreak().unwrap();
        assert_eq!(config.mode, FASTBREAK_DAMAGE);
        assert_eq!(config.break_damage, 0.8);
        assert_eq!(config.speed, 50);
        assert_eq!(config.ticks, 1);
        assert!(config.equal_air_ground);
        assert!(!config.abort_another);
        assert!(!config.only_tool);
        assert!(!config.ignore_mining_fatigue);
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == FASTBREAK)
                .unwrap()
                .enabled = false;
        });
    }

    #[test]
    fn fastbreak_shows_only_the_selected_modes_settings() {
        with(|c| {
            let m = c.modules.iter().find(|m| m.id == FASTBREAK).unwrap();
            let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
            assert!(visible("break_damage"));
            assert!(!visible("speed"));
            assert!(!visible("ticks"));
            assert!(!visible("equal_air_ground"));
            assert!(visible("abort_another"));
        });
        for (label, visible_ids) in [
            ("Percentage", ["speed", "equal_air_ground"]),
            ("Ticks", ["ticks", "equal_air_ground"]),
        ] {
            with(|c| {
                c.modules
                    .iter_mut()
                    .find(|m| m.id == FASTBREAK)
                    .unwrap()
                    .setting_mut("mode")
                    .unwrap()
                    .set_text(label);
            });
            with(|c| {
                let m = c.modules.iter().find(|m| m.id == FASTBREAK).unwrap();
                let visible = |id: &str| m.setting_visible(m.setting(id).unwrap());
                for id in ["break_damage", "speed", "ticks", "equal_air_ground"] {
                    assert_eq!(visible(id), visible_ids.contains(&id), "{label}: {id}");
                }
            });
        }
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == FASTBREAK)
                .unwrap()
                .setting_mut("mode")
                .unwrap()
                .set_text("Damage");
        });
    }

    #[test]
    fn fastbreak_air_ground_matches_alans_two_edits() {
        let close =
            |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6;
        // In water the progress is rewound and the finish point fixed.
        let got = fastbreak_air_ground(0.6, 0.5, 0.8, true, true, true);
        assert!(close(got, (0.12, 0.8)), "{got:?}");
        // Moving through a liquid off the ground credits it back.
        let got = fastbreak_air_ground(0.6, 0.5, 0.8, false, false, false);
        assert!(close(got, (3.0, -0.3)), "{got:?}");
        // Both together cancel, as in the common swimming case.
        let got = fastbreak_air_ground(0.6, 0.5, 0.81, true, false, false);
        assert!(close(got, (0.6, 0.0)), "{got:?}");
        // Alan finishes only when inside the threshold and not already there.
        assert_eq!(fastbreak_finish(0.6, 0.5), Some(0.99));
        assert_eq!(fastbreak_finish(0.4, 0.5), None);
        assert_eq!(fastbreak_finish(0.99, 0.5), None);
    }

    #[test]
    fn fastbreak_mode_indices_follow_the_label_list() {
        assert_eq!(FASTBREAK_MODES, ["Damage", "Percentage", "Ticks"]);
        assert_eq!(FASTBREAK_MODES[FASTBREAK_DAMAGE as usize], "Damage");
        assert_eq!(FASTBREAK_MODES[FASTBREAK_TICKS as usize], "Ticks");
    }

    #[test]
    fn speed_starts_off_in_movement_with_the_reference_settings() {
        with(|c| {
            let module = c.modules.iter_mut().find(|m| m.id == SPEED).unwrap();
            assert!(!module.enabled);
            assert_eq!(module.category, Category::Movement);
            assert_eq!(
                module.setting("mode").unwrap().choice_options(),
                &SPEED_MODES[..]
            );
            module.enabled = true;
        });
        let config = speed().unwrap();
        assert_eq!(config.mode, SpeedMode::LegitHop);
        assert_eq!(config.custom_horizontal_acceleration, 0.0);
        assert_eq!(config.custom_jump_off, 0.0);
        assert_eq!(config.custom_boost_ticks, 0);
        assert_eq!(config.custom_jump_height, 0.42);
        assert_eq!(config.custom_pulldown, 0.0);
        assert_eq!(config.custom_fall_pulldown, 0.0);
        assert_eq!(config.custom_strafe_strength, 1.0);
        assert!(!config.custom_speed_enabled);
        assert_eq!(config.custom_speed, 1.0);
        assert_eq!(config.yp_speed, 0.4);
        assert_eq!(config.vanilla_speed, 1.0);
        assert!(config.hypixel_horizontal_acceleration);
        assert!(config.hypixel_vertical_acceleration);
        assert!(!config.hypixel_lowhop_glide);
        assert_eq!(config.sentinel_speed, 0.5);
        assert_eq!(config.sentinel_reboost_ticks, 30);
        assert_eq!(config.grim_boost, 0.08);
        assert_eq!(config.grim_shrink, 0.5);
        assert!(config.ncp_pull_down);
        assert_eq!(config.ncp_motion_multiplier, 1.0);
        assert_eq!(config.ncp_pull_down_tick, 5);
        assert!(config.ncp_on_hurt);
        assert!(config.ncp_boost);
        assert_eq!(config.ncp_initial_boost_multiplier, 1.0);
        assert!(config.ncp_damage_boost);
        assert!(config.ncp_low_hop);
        assert!(config.ncp_air_strafe);
        assert!(config.intave_strafe);
        assert_eq!(config.intave_strafe_strength, 0.27);
        assert!(config.intave_air_boost);
        assert!(!config.blocksmc_round_yaw);
        assert!(config.blocksmc_hop_full_strafe);
        assert!(config.blocksmc_hop_low_hop);
        assert!(config.blocksmc_hop_damage_boost);
        assert!(config.blocksmc_hop_damage_low_hop);
        assert!(!config.blocksmc_hop_safe_y);
        assert!(!config.matrix_low_hop);
        assert_eq!(config.matrix_extra_boost, 0.2);
        assert_eq!(config.spectre_jump_motion, 0.44);
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == SPEED)
                .unwrap()
                .enabled = false;
        });
        assert!(speed().is_none());
    }

    #[test]
    fn timer_starts_off_in_misc_with_the_reference_settings() {
        with(|c| {
            let module = c.modules.iter_mut().find(|m| m.id == TIMER).unwrap();
            assert!(!module.enabled);
            assert_eq!(module.category, Category::Misc);
            assert_eq!(
                module.setting("mode").unwrap().choice_options(),
                &TIMER_MODES[..]
            );
            module.enabled = true;
        });
        let config = timer().unwrap();
        assert_eq!(config.mode, 0);
        assert_eq!(config.multiplier, 1.0);
        assert_eq!(config.boost_ticks, 0);
        assert_eq!(config.boost_speed, 2.0);
        assert_eq!(config.normal_ticks, 20);
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == TIMER)
                .unwrap()
                .enabled = false;
        });
        assert!(timer().is_none());
    }

    #[test]
    fn step_starts_off_in_movement_with_the_reference_settings() {
        with(|c| {
            let module = c.modules.iter_mut().find(|m| m.id == STEP).unwrap();
            assert!(!module.enabled);
            assert_eq!(module.category, Category::Movement);
            assert_eq!(
                module.setting("mode").unwrap().choice_options(),
                &STEP_MODES[..]
            );
            module.enabled = true;
        });
        let config = step().unwrap();
        assert_eq!(config.mode, StepMode::Vanilla);
        assert_eq!(config.height, 1.0);
        assert_eq!(config.jump_height, 0.42);
        assert_eq!(config.delay, 0);
        assert_eq!(config.extra_delay, 0);
        assert!(!config.simulate_jump);
        assert_eq!(config.timer, 1.0);
        assert!(!config.dynamic_timer);
        assert!(config.matrix_two_block);
        assert!(config.matrix_instant);
        assert!(!config.instant_trim);
        assert_eq!(config.instant_order_min, 0);
        assert_eq!(config.instant_order_max, 2);
        assert_eq!(config.instant_wait_min, 0);
        assert_eq!(config.instant_wait_max, 0);
        assert!(config.instant_full);
        assert_eq!(config.blocksmc_base_timer, 3.0);
        assert_eq!(config.blocksmc_recovery_timer, 0.6);
        assert!(!config.hypixel_alternate);
        assert!(!config.hypixel_spoof);
        with(|c| {
            c.modules.iter_mut().find(|m| m.id == STEP).unwrap().enabled = false;
        });
        assert!(step().is_none());
    }

    #[test]
    fn step_mode_indices_match_the_label_list() {
        let mut seen = Vec::new();
        for index in 0..STEP_MODES.len() {
            let mode = StepMode::from_index(index);
            assert!(!seen.contains(&mode), "{} reused", STEP_MODES[index]);
            seen.push(mode);
        }
        assert_eq!(seen.len(), STEP_MODES.len());
        assert_eq!(StepMode::from_index(0), StepMode::Vanilla);
        assert_eq!(StepMode::from_index(1), StepMode::Jump);
        assert_eq!(StepMode::from_index(7), StepMode::Aac440);
        assert_eq!(StepMode::from_index(13), StepMode::Vulcan286);
        assert_eq!(StepMode::from_index(17), StepMode::BlocksMc);
        assert_eq!(StepMode::from_index(19), StepMode::Hypixel);
        assert_eq!(StepMode::from_index(21), StepMode::Matrix670);
        assert_eq!(StepMode::from_index(99), StepMode::Vanilla);
    }

    #[test]
    fn fly_starts_off_in_movement_with_the_reference_settings() {
        with(|c| {
            let module = c.modules.iter_mut().find(|m| m.id == FLY).unwrap();
            assert!(!module.enabled);
            assert_eq!(module.category, Category::Movement);
            assert_eq!(
                module.setting("mode").unwrap().choice_options(),
                &FLY_MODES[..]
            );
            module.enabled = true;
        });
        let config = fly().unwrap();
        assert_eq!(config.mode, FlyMode::Vanilla);
        assert_eq!(config.vanilla_speed, 0.44);
        assert_eq!(config.vanilla_vertical, 0.44);
        assert_eq!(config.vanilla_glide, 0.0);
        assert!(config.vanilla_bypass);
        assert!(!config.anti_kick);
        assert_eq!(config.airwalk_speed, 0.35);
        assert_eq!(config.airwalk_vertical, 0.42);
        assert_eq!(config.airwalk_ticks, 11);
        assert_eq!(config.airwalk_timer, 1.0);
        assert_eq!(config.grim_speed, 0.28);
        assert_eq!(config.grim_vertical, 0.18);
        assert_eq!(config.grim_sink, 0.0);
        assert_eq!(config.grim_hurt_speed, 0.4);
        assert_eq!(config.grim_old_speed, 0.275);
        assert_eq!(config.grim_old_timer, 1.0);
        assert_eq!(config.grim_float_vertical, 0.2);
        assert_eq!(config.grim_float_speed, 0.1);
        assert_eq!(config.grim_entity_speed, 0.6);
        assert_eq!(config.grim_entity_vertical, 0.3);
        assert_eq!(config.grim_entity_distance, 1.0);
        assert!(!config.ncp_clip);
        assert_eq!(config.ncp_speed, 9.0);
        assert_eq!(config.ncp_timer, 0.4);
        assert_eq!(config.old_ncp_speed, 0.2);
        assert_eq!(config.ncp_burst_vertical, 0.5);
        assert_eq!(config.ncp_burst_speed, 0.2);
        assert_eq!(config.hypixel_speed, 1.9);
        assert_eq!(config.hypixel_vertical, 0.8);
        assert_eq!(config.hypixel_timer, 1.0);
        assert!(!config.hypixel_flat);
        assert_eq!(config.hypixel_boost_speed, 0.29);
        assert_eq!(config.hypixel_boost_timer, 1.0);
        assert_eq!(config.vulcan_down, 0.155);
        assert_eq!(config.vulcan_up, 0.1);
        assert_eq!(config.vulcan_speed, 0.2);
        assert_eq!(config.verus_ground_speed, 1.01);
        assert_eq!(config.verus_air_speed, 0.41);
        assert_eq!(config.verus_damage_speed, 1.0);
        assert_eq!(config.verus_damage_vertical, 1.0);
        assert_eq!(config.verus_damage_cancel, 8.5);
        assert_eq!(config.verus_damage_ticks, 20);
        assert_eq!(config.spartan_speed, 0.28);
        assert_eq!(config.spartan_interval, 12);
        assert_eq!(config.spartan_offset, 8.0);
        assert_eq!(config.aac_glide_ticks, 12);
        assert_eq!(config.aac_glide_lift, 0.015);
        assert_eq!(config.aac_glide_timer, 0.1);
        assert_eq!(config.matrix_speed, 9.3);
        assert_eq!(config.matrix_vertical, 0.42);
        assert_eq!(config.damage_speed, 1.0);
        assert_eq!(config.damage_vertical, 1.0);
        assert_eq!(config.damage_ticks, 14);
        assert_eq!(config.damage_timer, 4.0);
        assert_eq!(config.damage_trigger, 0);
        assert_eq!(config.sentinel_speed, 3.5);
        assert_eq!(config.sentinel_vertical, 0.7);
        assert_eq!(config.sentinel_ticks, 20);
        assert_eq!(config.sentinel_timer, 0.5);
        assert_eq!(config.sentinel_reboost, 0);
        assert_eq!(config.jetpack_impulse, 0.15);
        assert_eq!(config.jetpack_speed, 1.1);
        assert_eq!(config.explosion_vertical, 4.0);
        assert_eq!(config.explosion_speed, 1.0);
        assert_eq!(config.explosion_decrease, 0.005);
        assert_eq!(config.buffer_speed, 1.0);
        assert!(!config.buffer_send_flying);
        assert_eq!(config.highjump_height, 1.0);
        assert!(config.highjump_self_damage);
        with(|c| {
            c.modules.iter_mut().find(|m| m.id == FLY).unwrap().enabled = false;
        });
        assert!(fly().is_none());
    }

    #[test]
    fn fly_mode_indices_match_the_label_list() {
        let mut seen = Vec::new();
        for index in 0..FLY_MODES.len() {
            let mode = FlyMode::from_index(index);
            assert!(!seen.contains(&mode), "{} reused", FLY_MODES[index]);
            seen.push(mode);
        }
        assert_eq!(seen.len(), FLY_MODES.len());
        assert_eq!(FlyMode::from_index(0), FlyMode::Vanilla);
        assert_eq!(FlyMode::from_index(1), FlyMode::AirWalk);
        assert_eq!(FlyMode::from_index(6), FlyMode::GrimEntity);
        assert_eq!(FlyMode::from_index(7), FlyMode::Ncp);
        assert_eq!(FlyMode::from_index(15), FlyMode::Spartan);
        assert_eq!(FlyMode::from_index(17), FlyMode::Matrix);
        assert_eq!(FlyMode::from_index(19), FlyMode::Sentinel);
        assert_eq!(FlyMode::from_index(22), FlyMode::BufferAbuse);
        assert_eq!(FlyMode::from_index(23), FlyMode::MatrixHighJump);
        assert_eq!(FlyMode::from_index(99), FlyMode::Vanilla);
    }

    #[test]
    fn config_document_round_trips_modules_and_escaped_text() {
        with(|c| {
            let tracers = c.modules.iter_mut().find(|m| m.id == TRACERS).unwrap();
            tracers.enabled = true;
            tracers.bind = Some("key.keyboard.g".to_owned());
            tracers
                .setting_mut("friend_names")
                .unwrap()
                .set_text("a=b\\c\nd");
            let fly = c.modules.iter_mut().find(|m| m.id == FLY).unwrap();
            fly.enabled = true;
            fly.setting_mut("mode").unwrap().set_text("GrimEntity");
            fly.setting_mut("grim_entity_distance")
                .unwrap()
                .set_text("1.25");
            c.gui.size = (700.0, 500.0);
        });
        let document = config_document();
        assert!(document.starts_with(CONFIG_HEADER));
        let entries = parse_config(&document);
        with(|c| {
            let tracers = c.modules.iter_mut().find(|m| m.id == TRACERS).unwrap();
            tracers.enabled = false;
            tracers.bind = None;
            tracers
                .setting_mut("friend_names")
                .unwrap()
                .set_text("other");
            let fly = c.modules.iter_mut().find(|m| m.id == FLY).unwrap();
            fly.enabled = false;
            fly.setting_mut("mode").unwrap().set_text("Vanilla");
            fly.setting_mut("grim_entity_distance")
                .unwrap()
                .set_text("0.5");
            c.gui.size = (600.0, 400.0);
        });
        assert!(apply_config(&entries) > 100);
        with(|c| {
            let tracers = c.modules.iter().find(|m| m.id == TRACERS).unwrap();
            assert!(tracers.enabled);
            assert_eq!(tracers.bind.as_deref(), Some("key.keyboard.g"));
            assert_eq!(tracers.text("friend_names"), "a=b\\c\nd");
            let fly = c.modules.iter().find(|m| m.id == FLY).unwrap();
            assert!(fly.enabled);
            assert_eq!(fly.choice("mode"), 6);
            assert_eq!(fly.float("grim_entity_distance"), 1.25);
            assert_eq!(c.gui.size, (700.0, 500.0));
        });
    }

    #[test]
    fn config_keys_are_unique_and_cover_every_module() {
        let entries = config_entries();
        let mut seen = std::collections::HashSet::new();
        for (key, _) in &entries {
            assert!(seen.insert(key.clone()), "duplicate {key}");
        }
        assert_eq!(entries[0].0, "window.size");
        let modules = with(|c| c.modules.len());
        let settings = with(|c| {
            c.modules
                .iter()
                .map(|module| module.settings.len())
                .sum::<usize>()
        });
        assert_eq!(entries.len(), 1 + modules * 2 + settings);
    }

    #[test]
    fn config_load_ignores_unknown_keys_and_bad_sizes() {
        let entries = parse_config(
            "# StariyBog config v1\n\nunknown=1\nnope.speed=2\nenabled.nope=true\n\
             window.size=wide\nspeed.mode=Vanilla\n",
        );
        assert_eq!(apply_config(&entries), 1);
        with(|c| {
            assert_eq!(
                c.modules
                    .iter()
                    .find(|m| m.id == SPEED)
                    .unwrap()
                    .choice("mode"),
                3
            );
        });
    }

    #[test]
    fn speed_mode_indices_match_the_label_list() {
        let mut seen = Vec::new();
        for index in 0..SPEED_MODES.len() {
            let mode = SpeedMode::from_index(index);
            assert!(!seen.contains(&mode), "{} reused", SPEED_MODES[index]);
            seen.push(mode);
        }
        assert_eq!(seen.len(), SPEED_MODES.len());
        assert_eq!(SpeedMode::from_index(0), SpeedMode::LegitHop);
        assert_eq!(SpeedMode::from_index(1), SpeedMode::Custom);
        assert_eq!(SpeedMode::from_index(4), SpeedMode::Aac);
        assert_eq!(SpeedMode::from_index(16), SpeedMode::Ncp);
        assert_eq!(SpeedMode::from_index(23), SpeedMode::BlocksMc);
        assert_eq!(SpeedMode::from_index(24), SpeedMode::BlocksMcHop);
        assert_eq!(SpeedMode::from_index(26), SpeedMode::MatrixHop);
        assert_eq!(SpeedMode::from_index(31), SpeedMode::Verus);
        assert_eq!(SpeedMode::from_index(39), SpeedMode::Swift);
        assert_eq!(SpeedMode::from_index(40), SpeedMode::WatchdogTerrain);
        assert_eq!(SpeedMode::from_index(99), SpeedMode::LegitHop);
    }

    #[test]
    fn speed_shows_only_the_selected_modes_settings() {
        let visible = |m: &super::Module, ids: &[&str], all: &[&str]| {
            for id in all {
                assert_eq!(
                    m.setting_visible(m.setting(id).unwrap()),
                    ids.contains(id),
                    "{}: {id}",
                    m.setting("mode").unwrap().choice_label()
                );
            }
        };
        let all = [
            "custom_horizontal_acceleration",
            "custom_jump_off",
            "custom_speed",
            "yp_speed",
            "vanilla_speed",
            "hypixel_horizontal_acceleration",
            "hypixel_vertical_acceleration",
            "hypixel_lowhop_glide",
            "sentinel_speed",
            "sentinel_reboost_ticks",
            "grim_boost",
            "grim_shrink",
            "ncp_pull_down",
            "ncp_air_strafe",
            "intave_strafe",
            "blocksmc_round_yaw",
            "blocksmc_hop_full_strafe",
            "matrix_low_hop",
            "matrix_extra_boost",
            "spectre_jump_motion",
        ];
        for (label, ids) in [
            ("LegitHop", vec![]),
            (
                "Custom",
                vec![
                    "custom_horizontal_acceleration",
                    "custom_jump_off",
                    "custom_speed",
                ],
            ),
            ("YPort", vec!["yp_speed"]),
            ("Vanilla", vec!["vanilla_speed"]),
            (
                "HypixelBHop",
                vec![
                    "hypixel_horizontal_acceleration",
                    "hypixel_vertical_acceleration",
                ],
            ),
            ("HypixelLowHop", vec!["hypixel_lowhop_glide"]),
            (
                "SentinelDamage",
                vec!["sentinel_speed", "sentinel_reboost_ticks"],
            ),
            ("GrimCollide", vec!["grim_boost", "grim_shrink"]),
            ("NCP", vec!["ncp_pull_down", "ncp_air_strafe"]),
            ("Intave14", vec!["intave_strafe"]),
            ("BlocksMC", vec!["blocksmc_round_yaw"]),
            ("BlocksMCHop", vec!["blocksmc_hop_full_strafe"]),
            ("MatrixHop", vec!["matrix_low_hop", "matrix_extra_boost"]),
            ("SpectreHop", vec!["spectre_jump_motion"]),
        ] {
            with(|c| {
                c.modules
                    .iter_mut()
                    .find(|m| m.id == SPEED)
                    .unwrap()
                    .setting_mut("mode")
                    .unwrap()
                    .set_text(label);
            });
            with(|c| {
                let m = c.modules.iter().find(|m| m.id == SPEED).unwrap();
                visible(m, &ids, &all);
            });
        }
        with(|c| {
            c.modules
                .iter_mut()
                .find(|m| m.id == SPEED)
                .unwrap()
                .setting_mut("mode")
                .unwrap()
                .set_text("LegitHop");
        });
    }

    #[test]
    fn speed_helpers_match_the_reference_math() {
        let close =
            |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5;
        let unit = std::f32::consts::FRAC_1_SQRT_2;
        assert_eq!(input_direction(true, false, false, false), [0.0, 1.0]);
        assert_eq!(input_direction(false, false, true, false), [1.0, 0.0]);
        assert_eq!(input_direction(true, false, true, false), [unit, unit]);
        assert_eq!(input_direction(true, true, false, false), [0.0, 0.0]);
        assert_eq!(input_direction(true, true, true, false), [1.0, 0.0]);

        assert_eq!(movement_yaw(0.0, [0.0, 1.0]), 0.0);
        assert_eq!(movement_yaw(0.0, [1.0, 0.0]), -90.0);
        assert_eq!(movement_yaw(0.0, [-1.0, 0.0]), 90.0);
        assert_eq!(movement_yaw(90.0, [0.0, 1.0]), 90.0);

        // No input clears the horizontal velocity.
        assert_eq!(
            with_strafe([0.3, 0.2], [0.0, 0.0], 0.0, None, 1.0),
            [0.0, 0.0]
        );
        // Full strength follows the input along the yaw.
        assert!(close(
            with_strafe([0.2, 0.0], [0.0, 1.0], 0.0, Some(0.3), 1.0),
            [0.0, 0.3]
        ));
        assert!(close(
            with_strafe([0.2, 0.0], [1.0, 0.0], 0.0, Some(0.3), 1.0),
            [0.3, 0.0]
        ));
        // Default speed keeps the current horizontal magnitude.
        assert!(close(
            with_strafe([0.3, 0.4], [0.0, 1.0], 0.0, None, 1.0),
            [0.0, 0.5]
        ));
        // Strength blends toward the desired velocity.
        assert!(close(
            with_strafe([0.2, 0.0], [0.0, 1.0], 0.0, Some(0.4), 0.5),
            [0.1, 0.2]
        ));

        assert!((hypixel_at_least(0) - 0.281).abs() < 1e-6);
        assert!((hypixel_at_least(1) - 0.289_003_28).abs() < 1e-6);
        assert!((hypixel_at_least(2) - 0.297_006_56).abs() < 1e-6);
        assert!((allowed_horizontal_distance(0, 0) - 0.293_046).abs() < 1e-6);
        assert!((allowed_horizontal_distance(1, 0) - 0.341_887).abs() < 1e-6);
        assert!((allowed_horizontal_distance(0, 1) - 0.241_332).abs() < 1e-6);
        assert!((predicted_motion(0.42, 0) - 0.42).abs() < 1e-6);
        assert!((predicted_motion(0.42, 2) - 0.248_136).abs() < 1e-5);
        assert_eq!(round_yaw(22.0), 0.0);
        assert_eq!(round_yaw(23.0), 45.0);
        assert_eq!(round_yaw(67.0), 45.0);
        assert_eq!(round_yaw(68.0), 90.0);
        assert_eq!(round_yaw(-30.0), -45.0);
        assert!((blocksmc_speed(true, 0.0, 0, 0) - 0.18).abs() < 1e-6);
        assert!((blocksmc_speed(false, 0.0, 0, 0) - 0.27).abs() < 1e-6);
        assert!((blocksmc_speed(false, 0.2, 0, 0) - 0.28).abs() < 1e-6);
        assert!((blocksmc_speed(false, 0.0, 1, 0) - 0.37).abs() < 1e-6);
        assert!((blocksmc_speed(false, 0.0, 0, 20) - 0.13).abs() < 1e-6);
    }
}
