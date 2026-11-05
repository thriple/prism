//! The units lengths are shown in. Models are always stored in metres; this
//! is only how numbers are written for people.

use serde::{Deserialize, Serialize};

/// A unit to show lengths in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LengthUnit {
    Millimetres,
    Centimetres,
    #[default]
    Metres,
    Inches,
    /// Feet and inches: 2' 6.5".
    FeetInches,
}

impl LengthUnit {
    pub const ALL: [LengthUnit; 5] = [
        LengthUnit::Millimetres,
        LengthUnit::Centimetres,
        LengthUnit::Metres,
        LengthUnit::Inches,
        LengthUnit::FeetInches,
    ];

    /// The name in menus and settings.
    pub fn name(self) -> &'static str {
        match self {
            LengthUnit::Millimetres => "Millimetres",
            LengthUnit::Centimetres => "Centimetres",
            LengthUnit::Metres => "Metres",
            LengthUnit::Inches => "Inches",
            LengthUnit::FeetInches => "Feet and inches",
        }
    }

    /// Metres per unit (feet and inches count in inches).
    pub fn metres(self) -> f64 {
        match self {
            LengthUnit::Millimetres => 0.001,
            LengthUnit::Centimetres => 0.01,
            LengthUnit::Metres => 1.0,
            LengthUnit::Inches | LengthUnit::FeetInches => 0.0254,
        }
    }
}

/// How lengths are shown: the unit and the number of decimals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayUnits {
    pub length: LengthUnit,
    pub decimals: u8,
}

impl Default for DisplayUnits {
    fn default() -> Self {
        Self {
            length: LengthUnit::Metres,
            decimals: 3,
        }
    }
}

/// The most decimals offered.
pub const MAX_DECIMALS: u8 = 6;
