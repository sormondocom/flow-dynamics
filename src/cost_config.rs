use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::components::{ComponentKind, PipeDiameter, PipeMaterial};
use crate::glyphs::{diam_key, mat_key};

/// Per-unit pricing for cost estimation.
/// Persisted alongside glyph config so prices survive sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostConfig {
    /// Cost per linear foot of pipe: key = "Material/Diameter" (e.g. "Copper/ThreeQuarter")
    #[serde(default = "default_pipe_costs")]
    pub pipe_per_ft: HashMap<String, f32>,

    /// Cost per installed unit: key = ComponentKind key string (e.g. "ElbowNE")
    /// Elbow orientations share one price; caller groups them.
    #[serde(default = "default_fitting_costs")]
    pub fitting_per_unit: HashMap<String, f32>,
}

impl Default for CostConfig {
    fn default() -> Self {
        Self {
            pipe_per_ft: default_pipe_costs(),
            fitting_per_unit: default_fitting_costs(),
        }
    }
}

impl CostConfig {
    pub fn pipe_key(mat: PipeMaterial, diam: PipeDiameter) -> String {
        format!("{}/{}", mat_key(mat), diam_key(diam))
    }

    pub fn pipe_price(&self, mat: PipeMaterial, diam: PipeDiameter) -> f32 {
        *self.pipe_per_ft.get(&Self::pipe_key(mat, diam)).unwrap_or(&0.0)
    }

    pub fn set_pipe_price(&mut self, mat: PipeMaterial, diam: PipeDiameter, price: f32) {
        self.pipe_per_ft.insert(Self::pipe_key(mat, diam), price);
    }
}

/// Groups of fittings that share one price entry in the config.
/// Each tuple is (canonical_key, display_label, member_kinds).
pub const FITTING_GROUPS: &[(&str, &str, &[ComponentKind])] = &[
    ("Source",     "Source (Inlet)",      &[ComponentKind::Source]),
    ("Sink",       "Drain (Outlet)",      &[ComponentKind::Sink]),
    ("Toilet",     "Toilet",              &[ComponentKind::Toilet]),
    ("Faucet",     "Faucet/Sink",         &[ComponentKind::Faucet]),
    ("BasinSink",  "Basin Sink",          &[ComponentKind::BasinSink]),
    ("WaterHeater","Water Heater",        &[ComponentKind::WaterHeater]),
    ("ElbowNE",    "Elbow 90°",           &[ComponentKind::ElbowNE, ComponentKind::ElbowNW,
                                             ComponentKind::ElbowSE, ComponentKind::ElbowSW]),
    ("TeeNSE",     "Tee 3-way",           &[ComponentKind::TeeNSE, ComponentKind::TeeNSW,
                                             ComponentKind::TeeNEW, ComponentKind::TeeSEW]),
    ("ReducerTeeNSE","Reducer Tee",       &[ComponentKind::ReducerTeeNSE, ComponentKind::ReducerTeeNSW,
                                             ComponentKind::ReducerTeeNEW, ComponentKind::ReducerTeeSEW]),
    ("Cross",      "Cross 4-way",         &[ComponentKind::Cross]),
    ("BallValveH", "Ball Valve",          &[ComponentKind::BallValveH, ComponentKind::BallValveV]),
    ("CheckValveH","Check Valve",         &[ComponentKind::CheckValveH, ComponentKind::CheckValveV]),
    ("EndCap",     "End Cap",             &[ComponentKind::EndCap]),
    ("Reducer",    "Reducer",             &[ComponentKind::Reducer]),
    ("PressureGauge","Pressure Gauge",    &[ComponentKind::PressureGauge]),
    ("FlowMeterH", "Flow Meter",          &[ComponentKind::FlowMeterH, ComponentKind::FlowMeterV]),
    ("WaterSoftener","Water Softener",    &[ComponentKind::WaterSoftener]),
    ("WholeHouseFilter","Whole-House Filter",&[ComponentKind::WholeHouseFilter]),
    ("SedimentFilter","Sediment Filter",  &[ComponentKind::SedimentFilter]),
    ("UvFilter",   "UV Filter",           &[ComponentKind::UvFilter]),
    ("PressureReducingValve","PRV",       &[ComponentKind::PressureReducingValve]),
    ("ExpansionTank","Expansion Tank",    &[ComponentKind::ExpansionTank]),
];

fn default_pipe_costs() -> HashMap<String, f32> {
    use PipeDiameter::*;
    use PipeMaterial::*;

    let entries: &[(PipeMaterial, PipeDiameter, f32)] = &[
        (Copper,        Half,         1.20), (Copper,        ThreeQuarter, 1.85), (Copper,        One, 2.50),
        (PEX,           Half,         0.45), (PEX,           ThreeQuarter, 0.65), (PEX,           One, 0.90),
        (PE,            Half,         0.40), (PE,            ThreeQuarter, 0.60), (PE,            One, 0.85),
        (GalvanizedIron,Half,         2.50), (GalvanizedIron,ThreeQuarter, 3.50), (GalvanizedIron,One, 4.75),
        (BlackPlastic,  Half,         0.35), (BlackPlastic,  ThreeQuarter, 0.50), (BlackPlastic,  One, 0.75),
        (CastIron,      Half,         4.00), (CastIron,      ThreeQuarter, 5.50), (CastIron,      One, 7.00),
    ];

    entries.iter()
        .map(|&(m, d, p)| (CostConfig::pipe_key(m, d), p))
        .collect()
}

fn default_fitting_costs() -> HashMap<String, f32> {
    let entries: &[(&str, f32)] = &[
        ("Source",              200.00),
        ("Sink",                  5.00),
        ("Toilet",              200.00),
        ("Faucet",              150.00),
        ("BasinSink",           180.00),
        ("WaterHeater",         600.00),
        ("ElbowNE",               2.50),
        ("TeeNSE",                3.50),
        ("ReducerTeeNSE",         4.00),
        ("Cross",                 5.00),
        ("BallValveH",           15.00),
        ("CheckValveH",          12.00),
        ("EndCap",                1.50),
        ("Reducer",               3.00),
        ("PressureGauge",        25.00),
        ("FlowMeterH",           45.00),
        ("WaterSoftener",       800.00),
        ("WholeHouseFilter",    250.00),
        ("SedimentFilter",       75.00),
        ("UvFilter",            150.00),
        ("PressureReducingValve",65.00),
        ("ExpansionTank",        45.00),
    ];

    entries.iter().map(|&(k, v)| (k.to_string(), v)).collect()
}
