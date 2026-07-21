use serde::{Deserialize, Serialize};

// ── Diameter ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipeDiameter {
    Half,
    ThreeQuarter,
    One,
}

impl PipeDiameter {
    pub fn label(self) -> &'static str {
        match self {
            Self::Half => "1/2\"",
            Self::ThreeQuarter => "3/4\"",
            Self::One => "1\"",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::Half => Self::ThreeQuarter,
            Self::ThreeQuarter => Self::One,
            Self::One => Self::Half,
        }
    }

    /// Schedule 40 inner diameter in inches (used in all flow calculations)
    pub fn inner_diameter_in(self) -> f32 {
        match self {
            Self::Half => 0.622,
            Self::ThreeQuarter => 0.824,
            Self::One => 1.049,
        }
    }
}

impl Default for PipeDiameter {
    fn default() -> Self {
        Self::ThreeQuarter
    }
}

// ── Material ──────────────────────────────────────────────────────────────────

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PipeMaterial {
    #[default]
    Copper,
    PEX,
    PE,
    GalvanizedIron,
    BlackPlastic, // ABS / Schedule 40 PVC
    CastIron,
}

impl PipeMaterial {
    /// Hazen-Williams C coefficient — higher = smoother = less friction
    pub fn c_value(self) -> f32 {
        match self {
            Self::Copper => 130.0,
            Self::PEX => 150.0,
            Self::PE => 150.0,
            Self::GalvanizedIron => 120.0,
            Self::BlackPlastic => 150.0,
            Self::CastIron => 100.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Copper => "Copper",
            Self::PEX => "PEX",
            Self::PE => "PE",
            Self::GalvanizedIron => "Galv. Iron",
            Self::BlackPlastic => "Black Plastic",
            Self::CastIron => "Cast Iron",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::Copper => Self::PEX,
            Self::PEX => Self::PE,
            Self::PE => Self::GalvanizedIron,
            Self::GalvanizedIron => Self::BlackPlastic,
            Self::BlackPlastic => Self::CastIron,
            Self::CastIron => Self::Copper,
        }
    }

    /// Maximum recommended flow velocity in ft/s for this material.
    pub fn max_velocity_fps(self) -> f32 {
        match self {
            Self::Copper         => 8.0,
            Self::PEX            => 8.0,
            Self::PE             => 5.0,
            Self::GalvanizedIron => 8.0,
            Self::BlackPlastic   => 5.0,
            Self::CastIron       => 6.0,
        }
    }
}

// ── Drain pipe diameter (DWV) ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrainDiameter {
    OneAndHalf, // 1.5" — lavatory, tub, shower
    Two,        // 2"   — toilet in some codes, shower, kitchen sink
    Three,      // 3"   — toilet main run, bathroom group
    Four,       // 4"   — building drain / main stack
}

impl Default for DrainDiameter {
    fn default() -> Self { Self::Two }
}

impl DrainDiameter {
    pub fn label(self) -> &'static str {
        match self {
            Self::OneAndHalf => "1½\"",
            Self::Two        => "2\"",
            Self::Three      => "3\"",
            Self::Four       => "4\"",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::OneAndHalf => Self::Two,
            Self::Two        => Self::Three,
            Self::Three      => Self::Four,
            Self::Four       => Self::OneAndHalf,
        }
    }

    pub fn is_default(&self) -> bool { *self == Self::Two }

    /// Numeric rank for size comparison (higher = larger pipe).
    pub fn rank(self) -> u8 {
        match self {
            Self::OneAndHalf => 0,
            Self::Two        => 1,
            Self::Three      => 2,
            Self::Four       => 3,
        }
    }

    /// Minimum required diameter for a given DFU load (IPC Table 710.1).
    pub fn min_for_dfu(dfu: u32) -> Self {
        if dfu <= 3      { Self::OneAndHalf }
        else if dfu <= 6 { Self::Two }
        else if dfu <= 20 { Self::Three }
        else { Self::Four }
    }
}

// ── Drain type ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DrainType {
    #[default]
    Generic,
    ToiletFlush,  // 1.28 GPM (HET)
    Shower,       // 2.0 GPM
    KitchenSink,  // 2.2 GPM
    BathroomSink, // 1.5 GPM
    WasherFill,   // 3.0 GPM
    OutdoorHose,  // 5.0 GPM
    FloorDrain,   // Passive gravity — no pressure demand
}

impl DrainType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Generic => "Generic Outlet",
            Self::ToiletFlush => "Toilet  1.28 GPM",
            Self::Shower => "Shower  2.0 GPM",
            Self::KitchenSink => "Kitchen 2.2 GPM",
            Self::BathroomSink => "Bath    1.5 GPM",
            Self::WasherFill => "Washer  3.0 GPM",
            Self::OutdoorHose => "Hose    5.0 GPM",
            Self::FloorDrain => "Floor Drain (passive)",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::Generic => Self::ToiletFlush,
            Self::ToiletFlush => Self::Shower,
            Self::Shower => Self::KitchenSink,
            Self::KitchenSink => Self::BathroomSink,
            Self::BathroomSink => Self::WasherFill,
            Self::WasherFill => Self::OutdoorHose,
            Self::OutdoorHose => Self::FloorDrain,
            Self::FloorDrain => Self::Generic,
        }
    }
}

// ── Line temperature ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LineTemp {
    #[default]
    Unset,
    Cold,
    Hot,
    Recirc,
}

impl LineTemp {
    pub fn cycle(self) -> Self {
        match self {
            Self::Unset  => Self::Cold,
            Self::Cold   => Self::Hot,
            Self::Hot    => Self::Recirc,
            Self::Recirc => Self::Unset,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Unset  => "",
            Self::Cold   => "COLD",
            Self::Hot    => "HOT",
            Self::Recirc => "RECIRC",
        }
    }

    pub fn is_unset(&self) -> bool { *self == Self::Unset }
}

// ── Valve state ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValveState {
    Open,
    Closed,
}

// ── Component kind ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComponentKind {
    Source,
    Sink,
    PipeH,
    PipeV,
    ElbowNE, // Connects North and East  ╚
    ElbowNW, // Connects North and West  ╝
    ElbowSE, // Connects South and East  ╔
    ElbowSW, // Connects South and West  ╗
    TeeNSE,         // Connects N-S-E           ╠
    TeeNSW,         // Connects N-S-W           ╣
    TeeNEW,         // Connects N-E-W           ╩
    TeeSEW,         // Connects S-E-W           ╦
    ReducerTeeNSE,  // N-S run (full), E branch (reduced)  ╟
    ReducerTeeNSW,  // N-S run (full), W branch (reduced)  ╢
    ReducerTeeNEW,  // E-W run (full), N branch (reduced)  ╧
    ReducerTeeSEW,  // E-W run (full), S branch (reduced)  ╤
    Cross,          // Connects N-S-E-W         ╬
    BallValveH,
    BallValveV,
    CheckValveH, // One-way W→E
    CheckValveV, // One-way N→S
    EndCap,
    Reducer,       // Horizontal diameter transition
    PressureGauge, // Inline gauge — reads local pressure, zero friction loss
    FlowMeterH,    // Inline flow meter, E/W ports — reads GPM, minimal friction  ⊗
    FlowMeterV,    // Inline flow meter, N/S ports — reads GPM, minimal friction  ⊗
    WaterSoftener,    // Inline treatment: ion exchange, E/W ports  ◎
    WholeHouseFilter, // Inline treatment: carbon/media filter, E/W ports  ⊞
    SedimentFilter,   // Inline treatment: particle pre-filter, E/W ports  ⊟
    UvFilter,         // Inline treatment: UV disinfection, E/W ports  ⊕
    Toilet,           // Fixture terminal: toilet bowl, E/W supply port  ○
    WaterHeater,      // Inline treatment: water heater, E/W ports (cold→hot)  ▲
    Faucet,           // Fixture terminal: sink/faucet, E/W supply port  ≈
    BasinSink,        // Fixture: basin sink, E/W supply port + south drain port  ⊔
    SolidBlock,       // Structural element: wall/floor/ceiling, no plumbing  █
    PressureReducingValve, // PRV — inline, clamps downstream pressure to setpoint  ⊵
    ExpansionTank,         // Expansion tank — dead-end branch, absorbs pressure  ⊟
    Label,         // Canvas annotation: single-line text spanning empty cells to the right
    Note,          // Canvas annotation: multi-line boxed note (lines separated by \n)
    Link,          // Canvas annotation: boxed link to another diagram file
    Custom,        // User-defined — connections & glyph stored in Component / GlyphRegistry
    // ── DWV (Drain-Waste-Vent) ────────────────────────────────────────────
    DrainH,        // Horizontal drain pipe, E/W ports  ─
    DrainV,        // Vertical drain pipe / stack, N/S ports  │
    PTrap,         // P-trap — inline W→E, traps sewer gas  ⊓
    Vent,          // Vent riser — N/S, carries to open air  ↑
    DrainWye,      // Wye fitting — N/S run + W branch  ╁
    Cleanout,      // Cleanout access fitting — E/W inline  ⊠
}

impl ComponentKind {
    /// Which faces this component exposes a port on: (north, south, east, west)
    pub fn connections(self) -> (bool, bool, bool, bool) {
        match self {
            Self::Source | Self::Sink | Self::Cross | Self::PressureGauge => (true, true, true, true),
            Self::PipeH | Self::BallValveH | Self::CheckValveH | Self::Reducer
            | Self::FlowMeterH
            | Self::WaterSoftener | Self::WholeHouseFilter | Self::SedimentFilter | Self::UvFilter
            | Self::Toilet | Self::WaterHeater | Self::Faucet
            | Self::Custom => {
                (false, false, true, true)
            }
            // BasinSink: water enters from north inlet only (top-center), no E/W side ports.
            // North connectivity is handled via composite_north_inlet_offset().
            // Label/Note/Link: annotation-only, zero plumbing connections.
            Self::BasinSink | Self::SolidBlock | Self::Label | Self::Note | Self::Link => (false, false, false, false),
            Self::PressureReducingValve => (false, false, true, true), // E/W inline
            Self::ExpansionTank => (false, true, false, false),         // south port only (branches off tee)
            Self::DrainH | Self::PTrap | Self::Cleanout => (false, false, true, true), // E/W
            Self::DrainV | Self::Vent => (true, true, false, false),    // N/S
            Self::DrainWye => (true, true, false, true),                // N/S run + W branch
            Self::PipeV | Self::BallValveV | Self::CheckValveV | Self::FlowMeterV => (true, true, false, false),
            Self::ElbowNE => (true, false, true, false),
            Self::ElbowNW => (true, false, false, true),
            Self::ElbowSE => (false, true, true, false),
            Self::ElbowSW => (false, true, false, true),
            Self::TeeNSE | Self::ReducerTeeNSE => (true, true, true, false),
            Self::TeeNSW | Self::ReducerTeeNSW => (true, true, false, true),
            Self::TeeNEW | Self::ReducerTeeNEW => (true, false, true, true),
            Self::TeeSEW | Self::ReducerTeeSEW => (false, true, true, true),
            Self::EndCap => (true, true, true, true),
        }
    }

    /// Equivalent length in pipe diameters for fitting friction losses.
    /// Pipe cells (PipeH/V) return 0 — their length is set explicitly by the user.
    /// Whether this kind supports per-port arm stub lengths.
    /// Terminal components (source, sink, end cap) don't need stubs.
    pub fn has_arm_stubs(self) -> bool {
        !matches!(self,
            Self::Source | Self::Sink | Self::EndCap
            | Self::PipeH | Self::PipeV | Self::Custom
            | Self::Toilet | Self::WaterHeater | Self::Faucet | Self::BasinSink
            | Self::SolidBlock | Self::Label | Self::Note | Self::Link
            | Self::PressureReducingValve | Self::ExpansionTank
            | Self::DrainH | Self::DrainV | Self::PTrap | Self::Vent
            | Self::DrainWye | Self::Cleanout
        )
    }

    /// Returns true if this is a DWV (drain-waste-vent) component.
    pub fn is_dwv(self) -> bool {
        matches!(self, Self::DrainH | Self::DrainV | Self::PTrap
            | Self::Vent | Self::DrainWye | Self::Cleanout)
    }

    /// Drain Fixture Units for this fixture kind (IPC defaults).
    pub fn dfu(self) -> u32 {
        match self {
            Self::Toilet    => 6,
            Self::Faucet    => 1,
            Self::BasinSink => 1,
            Self::Sink      => 2, // kitchen sink
            _ => 0,
        }
    }

    pub fn is_annotation(self) -> bool {
        matches!(self, Self::Label | Self::Note | Self::Link)
    }

    /// Returns true for sealed passive terminals that should never generate flood
    /// animation, even when Pressurized.  These components cap or monitor the pipe
    /// but don't have an open orifice that water would spray from.
    pub fn is_sealed_terminal(self) -> bool {
        matches!(self, Self::PressureGauge | Self::EndCap | Self::FlowMeterH | Self::FlowMeterV
            | Self::ExpansionTank | Self::Cleanout)
    }

    pub fn equiv_length_diameters(self) -> f32 {
        match self {
            Self::Source | Self::Sink | Self::EndCap | Self::PressureGauge => 0.0,
            Self::FlowMeterH | Self::FlowMeterV => 2.0, // paddle/turbine sensor — near-zero friction
            Self::PipeH | Self::PipeV => 0.0, // uses pipe_length field
            Self::ElbowNE | Self::ElbowNW | Self::ElbowSE | Self::ElbowSW => 30.0,
            Self::TeeNSE | Self::TeeNSW | Self::TeeNEW | Self::TeeSEW => 40.0,
            Self::ReducerTeeNSE | Self::ReducerTeeNSW
            | Self::ReducerTeeNEW | Self::ReducerTeeSEW => 50.0, // extra turbulence at size change
            Self::Cross => 50.0,
            Self::BallValveH | Self::BallValveV => 3.0, // wide-open ball valve
            Self::CheckValveH | Self::CheckValveV => 50.0,
            Self::Reducer => 5.0,
            Self::WaterSoftener => 80.0,
            Self::WholeHouseFilter => 50.0,
            Self::SedimentFilter => 30.0,
            Self::UvFilter => 10.0,
            Self::Toilet => 0.0,       // terminal drain — no friction loss counted
            Self::WaterHeater => 60.0, // tank plus fittings — similar to water softener
            Self::Faucet => 0.0,       // terminal drain — no friction loss counted
            Self::BasinSink => 5.0,    // slight friction through basin fittings
            Self::SolidBlock | Self::Label | Self::Note | Self::Link => 0.0,
            Self::PressureReducingValve => 20.0, // typical PRV friction equiv.
            Self::ExpansionTank => 0.0,           // dead-end branch, no through-flow
            Self::Custom => 0.0, // set per-instance via equiv_length_d in CustomCompDef
            Self::DrainH | Self::DrainV => 0.0,  // uses pipe_length
            Self::PTrap => 25.0,    // DWV equiv. length for P-trap
            Self::Vent  => 0.0,     // vent — no drain flow
            Self::DrainWye => 15.0, // wye fitting
            Self::Cleanout => 0.0,  // cleanout cap
        }
    }

    pub fn symbol(self) -> char {
        match self {
            Self::Source => 'S',
            Self::Sink => 'D',
            Self::PipeH => '═',
            Self::PipeV => '║',
            Self::ElbowNE => '╚',
            Self::ElbowNW => '╝',
            Self::ElbowSE => '╔',
            Self::ElbowSW => '╗',
            Self::TeeNSE => '╠',
            Self::TeeNSW => '╣',
            Self::TeeNEW => '╩',
            Self::TeeSEW => '╦',
            Self::ReducerTeeNSE => '╟',
            Self::ReducerTeeNSW => '╢',
            Self::ReducerTeeNEW => '╧',
            Self::ReducerTeeSEW => '╤',
            Self::Cross => '╬',
            Self::BallValveH | Self::BallValveV => '●',
            Self::CheckValveH => '→',
            Self::CheckValveV => '↓',
            Self::EndCap => '■',
            Self::Reducer => '◄',
            Self::PressureGauge => '⊙',
            Self::FlowMeterH | Self::FlowMeterV => '⊗',
            Self::WaterSoftener => '◎',
            Self::WholeHouseFilter => '⊞',
            Self::SedimentFilter => '⊟',
            Self::UvFilter => '⊕',
            Self::Toilet => '○',
            Self::WaterHeater => '▲',
            Self::Faucet => '≈',
            Self::BasinSink => '⊔',
            Self::SolidBlock => '█',
            Self::PressureReducingValve => '⊵',
            Self::ExpansionTank => '⊡',
            Self::Label => '"',
            Self::Note => '†',
            Self::Link => '⇒',
            Self::Custom => '?',
            Self::DrainH  => '─',
            Self::DrainV  => '│',
            Self::PTrap   => '⊓',
            Self::Vent    => '↑',
            Self::DrainWye => '╁',
            Self::Cleanout => '⊠',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "Source (Inlet)",
            Self::Sink => "Drain (Outlet)",
            Self::PipeH => "Pipe Horiz  ═",
            Self::PipeV => "Pipe Vert   ║",
            Self::ElbowNE => "Elbow NE    ╚",
            Self::ElbowNW => "Elbow NW    ╝",
            Self::ElbowSE => "Elbow SE    ╔",
            Self::ElbowSW => "Elbow SW    ╗",
            Self::TeeNSE => "Tee NSE     ╠",
            Self::TeeNSW => "Tee NSW     ╣",
            Self::TeeNEW => "Tee NEW     ╩",
            Self::TeeSEW => "Tee SEW     ╦",
            Self::ReducerTeeNSE => "RTee NSE    ╟",
            Self::ReducerTeeNSW => "RTee NSW    ╢",
            Self::ReducerTeeNEW => "RTee NEW    ╧",
            Self::ReducerTeeSEW => "RTee SEW    ╤",
            Self::Cross => "Cross 4-way ╬",
            Self::BallValveH => "Ball Valve  ═",
            Self::BallValveV => "Ball Valve  ║",
            Self::CheckValveH => "Check →  W-E",
            Self::CheckValveV => "Check ↓  N-S",
            Self::EndCap => "End Cap     ■",
            Self::Reducer => "Reducer  ═◄═",
            Self::PressureGauge => "Press Gauge ⊙",
            Self::FlowMeterH => "Flow Meter  ═",
            Self::FlowMeterV => "Flow Meter  ║",
            Self::WaterSoftener => "Water Softener◎",
            Self::WholeHouseFilter => "House Filter  ⊞",
            Self::SedimentFilter => "Sediment Fltr ⊟",
            Self::UvFilter => "UV Filter     ⊕",
            Self::Toilet => "Toilet       ○",
            Self::WaterHeater => "Water Heater  ▲",
            Self::Faucet => "Faucet        ≈",
            Self::BasinSink => "Basin/Sink   ⊔",
            Self::SolidBlock => "Solid Block  █",
            Self::PressureReducingValve => "PRV          ⊵",
            Self::ExpansionTank => "Exp Tank     ⊡",
            Self::Label => "Label        \"",
            Self::Note => "Note         †",
            Self::Link => "Link         ⇒",
            Self::Custom => "Custom Comp  ?",
            Self::DrainH   => "Drain Horiz  ─",
            Self::DrainV   => "Drain Vert   │",
            Self::PTrap    => "P-Trap       ⊓",
            Self::Vent     => "Vent Riser   ↑",
            Self::DrainWye => "Drain Wye    ╁",
            Self::Cleanout => "Cleanout     ⊠",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Source => "Fluid inlet — set pressure with [I]. [M]aterial applies system-wide.",
            Self::Sink => "Fixture outlet — set type with [T]. Shows arriving flow & pressure.",
            Self::PipeH | Self::PipeV => "Pipe segment — set length [+/-] and material [M].",
            Self::ElbowNE | Self::ElbowNW | Self::ElbowSE | Self::ElbowSW => {
                "90 deg elbow — adds ~30 pipe-diameters equiv. friction length."
            }
            Self::TeeNSE | Self::TeeNSW | Self::TeeNEW | Self::TeeSEW => {
                "Tee (3-way) — splits or combines flow. Branch: ~60D, run: ~20D."
            }
            Self::ReducerTeeNSE | Self::ReducerTeeNSW
            | Self::ReducerTeeNEW | Self::ReducerTeeSEW => {
                "Reducer tee — double-line ports are the full-size run; single-line is the reduced branch (~50D)."
            }
            Self::Cross => "Cross (4-way) — full intersection fitting (~50D equiv.).",
            Self::BallValveH | Self::BallValveV => {
                "Ball valve — toggle [V]. Open: ~3D equiv. Closed: blocks all flow."
            }
            Self::CheckValveH => "Check valve — one-way W→E only (~50D equiv.).",
            Self::CheckValveV => "Check valve — one-way N→S only (~50D equiv.).",
            Self::EndCap => "End cap — terminates pipe. Creates pressurized dead-end.",
            Self::Reducer => "Reducer — diameter transition fitting (~5D equiv.).",
            Self::PressureGauge => "Pressure gauge — reads local PSI. Zero friction loss.",
            Self::FlowMeterH => "Flow meter (E/W) — reads GPM inline. Minimal friction (~2D equiv.).",
            Self::FlowMeterV => "Flow meter (N/S) — reads GPM inline. Minimal friction (~2D equiv.).",
            Self::WaterSoftener => "Water softener — ion exchange resin tank. 1 inlet (W), 1 outlet (E). ~80D equiv.",
            Self::WholeHouseFilter => "Whole-house filter — carbon/media canister. 1 inlet (W), 1 outlet (E). ~50D equiv.",
            Self::SedimentFilter => "Sediment pre-filter — removes particulates. 1 inlet (W), 1 outlet (E). ~30D equiv.",
            Self::UvFilter => "UV disinfection unit — ultraviolet sterilizer. 1 inlet (W), 1 outlet (E). ~10D equiv.",
            Self::Toilet => "Toilet — cold-water supply fixture. Connects E or W. Acts as drain terminal. [T] cycle flush type.",
            Self::WaterHeater => "Water heater — tank heater. Cold water in (W), hot water out (E). ~60D equiv.",
            Self::Faucet => "Faucet/sink — supply fixture. Connects E or W. Acts as drain terminal. [T] cycle flow type.",
            Self::BasinSink => "Basin sink — E or W supply, south drain port. Connect drain pipe down to a drain outlet. Overflows (animated) if no drain connected.",
            Self::SolidBlock => "Structural element — wall, floor, or ceiling. No plumbing connections. Use to outline rooms.",
            Self::PressureReducingValve => "PRV — reduces inlet pressure to setpoint. [I]/[I] adjust, [P] exact. E/W inline.",
            Self::ExpansionTank => "Expansion tank — dead-end branch absorbs pressure surges. Required in closed systems.",
            Self::Label => "Canvas label — inline text annotation spanning empty cells. Press Enter to type. [E] to edit.",
            Self::Note => "Canvas note — multi-line note box. Use | to separate lines. Press Enter to type. [E] to edit.",
            Self::Link => "Diagram link — stores path to another .json diagram. [Enter] to follow, [E] to edit path.",
            Self::Custom => "Custom component — defined in the glyph editor [G].",
            Self::DrainH   => "DWV horizontal drain pipe — E/W. Set diameter [D]. Shows DFU load in DWV mode.",
            Self::DrainV   => "DWV vertical drain pipe / stack — N/S. Carries waste down to building drain.",
            Self::PTrap    => "P-trap — prevents sewer gas backflow. Required within 5 ft of each fixture. E/W inline.",
            Self::Vent     => "Vent riser — N/S. Carries sewer gas to open air above roofline. Must connect to vent stack.",
            Self::DrainWye => "Drain wye (Y) fitting — N/S stack run, W horizontal tie-in branch (~15D equiv.).",
            Self::Cleanout => "Cleanout — provides rodding access for blockage clearing. E/W inline.",
        }
    }

    /// Returns (width, height) in grid cells for this component's footprint.
    /// Single-cell components return (1, 1).
    pub fn footprint(self) -> (usize, usize) {
        match self {
            Self::WaterSoftener => (17, 5),
            Self::WholeHouseFilter | Self::SedimentFilter | Self::UvFilter => (17, 3),
            Self::Toilet => (11, 5),
            Self::Faucet => (5, 3),
            Self::WaterHeater => (15, 5),
            Self::BasinSink => (13, 5),
            _ => (1, 1),
        }
    }

    pub fn is_composite(self) -> bool {
        self.footprint().0 > 1
    }

    /// Row within the footprint (0-indexed from top) that holds the E/W ports and anchor.
    pub fn port_row(self) -> usize {
        if self.is_composite() { self.footprint().1 / 2 } else { 0 }
    }

    /// Label text rendered inside the composite box.
    /// Must be exactly (footprint_width - 2) chars wide.
    pub fn composite_label(self) -> &'static str {
        match self {
            Self::WaterSoftener    => "Water Softener\u{25CE}",  // ◎  fw=17 → 15 chars
            Self::WholeHouseFilter => "House Filter  \u{229E}",  // ⊞  fw=17 → 15 chars
            Self::SedimentFilter   => "Sediment Fltr \u{229F}",  // ⊟  fw=17 → 15 chars
            Self::UvFilter         => "UV Filter     \u{2295}",  // ⊕  fw=17 → 15 chars
            Self::Toilet           => "Toilet  \u{25CB}",           // ○  fw=11 →  9 chars
            Self::WaterHeater      => "\u{2550}\u{2502}W.Heater\u{25B2}\u{2502}\u{2550}", // ═│W.Heater▲│═  fw=15 → 13 chars
            Self::Faucet           => " \u{2248} ",                  // ≈  fw=5  →  3 chars
            Self::BasinSink        => " Sink Bsn\u{2294} ",              //  Sink Bsn⊔   fw=13 → 11 chars
            _ => "",
        }
    }

    pub fn is_valve(self) -> bool {
        matches!(self, Self::BallValveH | Self::BallValveV)
    }

    /// Column within the composite's top border that holds the north-inlet connector (╦),
    /// if this kind has a north inlet port.
    pub fn composite_north_inlet_dc(self, fw: usize) -> Option<usize> {
        match self {
            Self::BasinSink => Some(fw / 2),
            _ => None,
        }
    }

    /// Whether this kind uses a per-instance color override (independent of material).
    pub fn supports_color_override(self) -> bool {
        matches!(self, Self::SolidBlock)
    }

    pub fn all_palette() -> &'static [ComponentKind] {
        &[
            Self::Source,
            Self::Sink,
            Self::PipeH,
            Self::PipeV,
            Self::ElbowNE,
            Self::ElbowNW,
            Self::ElbowSE,
            Self::ElbowSW,
            Self::TeeNSE,
            Self::TeeNSW,
            Self::TeeNEW,
            Self::TeeSEW,
            Self::ReducerTeeNSE,
            Self::ReducerTeeNSW,
            Self::ReducerTeeNEW,
            Self::ReducerTeeSEW,
            Self::Cross,
            Self::BallValveH,
            Self::BallValveV,
            Self::CheckValveH,
            Self::CheckValveV,
            Self::EndCap,
            Self::Reducer,
            Self::PressureGauge,
            Self::FlowMeterH,
            Self::FlowMeterV,
            Self::WaterSoftener,
            Self::WholeHouseFilter,
            Self::SedimentFilter,
            Self::UvFilter,
            Self::Toilet,
            Self::WaterHeater,
            Self::Faucet,
            Self::BasinSink,
            Self::PressureReducingValve,
            Self::ExpansionTank,
            Self::SolidBlock,
            Self::Label,
            Self::Note,
            Self::Link,
            // DWV section
            Self::DrainH,
            Self::DrainV,
            Self::PTrap,
            Self::Vent,
            Self::DrainWye,
            Self::Cleanout,
        ]
    }
}

// ── Component ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub kind: ComponentKind,
    pub diameter: PipeDiameter,
    pub valve_state: Option<ValveState>,
    pub material: PipeMaterial,
    /// Length in feet — used for PipeH/V; ignored for fittings (they use equiv_length_diameters)
    pub pipe_length: f32,
    /// Sub-type for Sink components
    pub drain_type: DrainType,
    /// Inlet pressure in PSI — only used by Source
    pub source_pressure_psi: f32,
    /// ID into GlyphRegistry::custom_components — only set when kind == Custom
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<String>,
    /// Port connections for Custom kind: [N, S, E, W].  Overrides kind.connections().
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_connections: Option<[bool; 4]>,
    /// Footprint (width, height) for composite Custom components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_footprint: Option<(usize, usize)>,
    /// Label text rendered inside the composite box for composite Custom components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_label: Option<String>,
    /// Stub pipe length (ft) at each port: [N=0, S=1, E=2, W=3].
    /// Used for fittings; PipeH/PipeV use pipe_length instead.
    #[serde(default, skip_serializing_if = "arm_lengths_are_zero")]
    pub arm_lengths: [f32; 4],
    /// Optional RGB color override — used by SolidBlock (and future color-capable kinds)
    /// to store a per-instance foreground color independent of material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_override: Option<[u8; 3]>,
    /// Annotation text for Label/Note kinds; ignored by all other kinds.
    /// Note uses '\n' as line separator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Hot/cold/recirc line designation — purely visual, no sim effect.
    #[serde(default, skip_serializing_if = "LineTemp::is_unset")]
    pub line_temp: LineTemp,

    /// Target pressure for PressureReducingValve; reuses the PSI concept from source_pressure_psi.
    /// Default 60 PSI.  Ignored for all other kinds.
    #[serde(default = "default_prv_setpoint", skip_serializing_if = "prv_setpoint_is_default")]
    pub prv_setpoint_psi: f32,

    /// Pipe diameter for DWV components (DrainH/V, PTrap, Vent, DrainWye, Cleanout).
    /// Ignored for supply-side components.
    #[serde(default, skip_serializing_if = "DrainDiameter::is_default")]
    pub drain_diameter: DrainDiameter,
}

fn default_prv_setpoint() -> f32 { 60.0 }
fn prv_setpoint_is_default(v: &f32) -> bool { (*v - 60.0).abs() < 0.01 }

fn arm_lengths_are_zero(v: &[f32; 4]) -> bool {
    v.iter().all(|&x| x == 0.0)
}

impl Component {
    pub fn new(kind: ComponentKind, diameter: PipeDiameter, material: PipeMaterial) -> Self {
        Self {
            kind,
            diameter,
            valve_state: if kind.is_valve() {
                Some(ValveState::Open)
            } else {
                None
            },
            material,
            pipe_length: 1.0,
            drain_type: DrainType::Generic,
            source_pressure_psi: 60.0,
            custom_id: None,
            custom_connections: None,
            custom_footprint: None,
            custom_label: None,
            arm_lengths: [0.0; 4],
            color_override: None,
            text: None,
            line_temp: LineTemp::Unset,
            prv_setpoint_psi: 60.0,
            drain_diameter: DrainDiameter::default(),
        }
    }


    /// Resolved port connections — respects custom_connections override for Custom kind.
    pub fn connections(&self) -> (bool, bool, bool, bool) {
        if self.kind == ComponentKind::Custom {
            if let Some(c) = &self.custom_connections {
                return (c[0], c[1], c[2], c[3]);
            }
        }
        self.kind.connections()
    }

    /// Footprint (width, height) — respects custom_footprint for composite Custom components.
    /// custom_footprint stores the canvas dimensions directly (no implicit buffer ring).
    pub fn effective_footprint(&self) -> (usize, usize) {
        if self.kind == ComponentKind::Custom {
            if let Some((w, h)) = self.custom_footprint {
                return (w, h);
            }
        }
        self.kind.footprint()
    }

    pub fn effective_is_composite(&self) -> bool {
        self.effective_footprint().0 > 1
    }

    pub fn effective_port_row(&self) -> usize {
        if self.effective_is_composite() { self.effective_footprint().1 / 2 } else { 0 }
    }

    pub fn effective_composite_label(&self) -> &str {
        if self.kind == ComponentKind::Custom {
            return self.custom_label.as_deref().unwrap_or("");
        }
        self.kind.composite_label()
    }

    pub fn is_passable(&self) -> bool {
        self.valve_state != Some(ValveState::Closed)
    }

    pub fn toggle_valve(&mut self) {
        if let Some(ref mut state) = self.valve_state {
            *state = match state {
                ValveState::Open => ValveState::Closed,
                ValveState::Closed => ValveState::Open,
            };
        }
    }

    /// For composites with a north inlet port (e.g. BasinSink), returns the (row, col)
    /// offset from the anchor to the external inlet connection cell — one row above the
    /// top of the composite box, at its horizontal center.
    pub fn composite_north_inlet_offset(&self) -> Option<(isize, isize)> {
        match self.kind {
            ComponentKind::BasinSink => {
                let (fw, _fh) = self.effective_footprint();
                let pr = self.effective_port_row();
                Some((-((pr + 1) as isize), (fw / 2) as isize))
            }
            _ => None,
        }
    }

    /// For components with a south drain port (e.g. BasinSink), returns the (row, col)
    /// offset from the anchor to the external drain connection cell (1 row below the
    /// bottom of the composite box, at its horizontal center).
    pub fn composite_south_drain_offset(&self) -> Option<(isize, isize)> {
        match self.kind {
            ComponentKind::BasinSink => {
                let (fw, fh) = self.effective_footprint();
                let pr = self.effective_port_row();
                // anchor is at port-row; box bottom is (fh-1-pr) rows below anchor;
                // drain pipe sits one more row below that.
                Some(((fh - pr) as isize, (fw / 2) as isize))
            }
            _ => None,
        }
    }

    /// Equivalent length in feet for resistance calculation
    pub fn equiv_length_ft(&self) -> f32 {
        let d_in = self.diameter.inner_diameter_in();
        match self.kind {
            ComponentKind::PipeH | ComponentKind::PipeV => self.pipe_length,
            _ => self.kind.equiv_length_diameters() * d_in / 12.0,
        }
    }

}
