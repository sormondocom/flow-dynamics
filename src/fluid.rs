use serde::{Deserialize, Serialize};

/// The type of fluid flowing through the system.
///
/// Drives both the visual appearance (cavity color, packet characters) and
/// the physics (viscosity scaling applied to Hazen-Williams resistance).
/// Water at 60 °F is the baseline; other fluids scale resistance accordingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FluidType {
    #[default]
    Water,
    Oil,
    NaturalGas,
    Steam,
    Glycol,
    HydraulicOil,
}

impl FluidType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Water        => "Water",
            Self::Oil          => "Light Oil",
            Self::NaturalGas   => "Natural Gas",
            Self::Steam        => "Steam",
            Self::Glycol       => "Glycol",
            Self::HydraulicOil => "Hydraulic Oil",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Self::Water        => Self::Oil,
            Self::Oil          => Self::NaturalGas,
            Self::NaturalGas   => Self::Steam,
            Self::Steam        => Self::Glycol,
            Self::Glycol       => Self::HydraulicOil,
            Self::HydraulicOil => Self::Water,
        }
    }

    /// RGB for the fluid cavity background (the space between the pipe-wall strokes).
    pub fn bg_color(self) -> (u8, u8, u8) {
        match self {
            Self::Water        => (  0,  25,  50),  // dark navy
            Self::Oil          => ( 35,  20,   0),  // dark amber
            Self::NaturalGas   => (  0,  20,  20),  // dark teal
            Self::Steam        => ( 22,  22,  28),  // dark slate
            Self::Glycol       => (  0,  22,   5),  // dark green
            Self::HydraulicOil => ( 28,   0,   0),  // dark red
        }
    }

    /// RGB for the bright travelling-packet characters.
    pub fn fg_color(self) -> (u8, u8, u8) {
        match self {
            Self::Water        => ( 80, 185, 255),  // sky blue
            Self::Oil          => (210, 145,  40),  // amber
            Self::NaturalGas   => (100, 240, 200),  // bright teal
            Self::Steam        => (210, 210, 225),  // pale blue-white
            Self::Glycol       => ( 80, 220, 110),  // bright green
            Self::HydraulicOil => (230,  70,  70),  // red
        }
    }

    /// Cycling character pool for horizontal flow packets.
    ///
    /// The animation indexes into this slice using a slow tick so the packet
    /// shimmers in-place between frames.
    pub fn h_chars(self) -> &'static [char] {
        // Arrays must be 'static — declared as statics inside the function.
        static WATER:    [char; 3] = ['≈', '≋', '∿'];
        static OIL:      [char; 2] = ['≀', '∼'];
        static GAS:      [char; 3] = ['·', '∙', '○'];
        static STEAM:    [char; 3] = ['∿', '≈', '∼'];
        static GLYCOL:   [char; 2] = ['≈', '≋'];
        static HYDRO:    [char; 2] = ['≋', '≈'];

        match self {
            Self::Water        => &WATER,
            Self::Oil          => &OIL,
            Self::NaturalGas   => &GAS,
            Self::Steam        => &STEAM,
            Self::Glycol       => &GLYCOL,
            Self::HydraulicOil => &HYDRO,
        }
    }

    /// Cycling character pool for vertical flow packets.
    pub fn v_chars(self) -> &'static [char] {
        static WATER:    [char; 3] = ['⋮', '⁞', '⋱'];
        static OIL:      [char; 2] = ['⁞', '⁚'];
        static GAS:      [char; 3] = ['·', '∙', '·'];
        static STEAM:    [char; 3] = ['⁞', '⁚', '∙'];
        static GLYCOL:   [char; 2] = ['⋮', '⁞'];
        static HYDRO:    [char; 2] = ['⋮', '⁞'];

        match self {
            Self::Water        => &WATER,
            Self::Oil          => &OIL,
            Self::NaturalGas   => &GAS,
            Self::Steam        => &STEAM,
            Self::Glycol       => &GLYCOL,
            Self::HydraulicOil => &HYDRO,
        }
    }

    /// Cycling character pool for fitting / junction packets.
    pub fn fit_chars(self) -> &'static [char] {
        static WATER:    [char; 2] = ['○', '◌'];
        static OIL:      [char; 2] = ['●', '◉'];
        static GAS:      [char; 2] = ['◌', '○'];
        static STEAM:    [char; 2] = ['◌', '○'];
        static GLYCOL:   [char; 2] = ['○', '◌'];
        static HYDRO:    [char; 2] = ['●', '◉'];

        match self {
            Self::Water        => &WATER,
            Self::Oil          => &OIL,
            Self::NaturalGas   => &GAS,
            Self::Steam        => &STEAM,
            Self::Glycol       => &GLYCOL,
            Self::HydraulicOil => &HYDRO,
        }
    }

    /// Multiplicative scaling applied to Hazen-Williams pipe resistance.
    ///
    /// Water at 60 °F = 1.0 (baseline).  Higher values represent more viscous
    /// fluids (higher pressure drop for the same flow rate).
    pub fn viscosity_scale(self) -> f32 {
        match self {
            Self::Water        =>  1.0,
            Self::Oil          =>  8.0,
            Self::NaturalGas   =>  0.05,
            Self::Steam        =>  0.08,
            Self::Glycol       =>  3.0,
            Self::HydraulicOil => 15.0,
        }
    }
}
