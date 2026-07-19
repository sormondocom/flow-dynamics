use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::components::{ComponentKind, PipeDiameter, PipeMaterial};

// ── Palette tables used by the glyph editor ───────────────────────────────────

pub const CHAR_PALETTE_COLS: usize = 13;

/// Index in CHAR_PALETTE where the Alpha-Numerics section begins (7 rows × 13 cols).
pub const CHAR_PALETTE_SYMBOLICS_LEN: usize = 91;

/// All characters available in the glyph editor's character picker.
pub const CHAR_PALETTE: &[char] = &[
    // ── Symbolics (rows 0-6) ─────────────────────────────────────────────────
    // Row 0 – thin box-drawing
    '─', '│', '└', '┘', '┌', '┐', '├', '┤', '┬', '┴', '┼', '╌', '╎',
    // Row 1 – double box-drawing
    '═', '║', '╚', '╝', '╔', '╗', '╠', '╣', '╦', '╩', '╬', '╞', '╡',
    // Row 2 – mixed / dashed
    '┄', '┆', '┅', '┇', '┈', '┊', '╍', '╏', '╟', '╢', '╤', '╧', '╥',
    // Row 3 – shapes
    '●', '○', '◐', '◑', '■', '□', '◆', '◇', '▪', '▫', '▸', '◉', '◊',
    // Row 4 – arrows
    '→', '←', '↑', '↓', '►', '◄', '▲', '▼', '↗', '↙', '↕', '↔', '⇒',
    // Row 5 – special symbols
    '✕', '×', '⊕', '⊗', '⊙', '★', '☆', '♦', '⬡', '⊞', '⊟', '◎', '⊛',
    // Row 6 – math / units
    '≈', '≠', '±', '÷', '∞', 'Ω', 'µ', 'π', '°', '¢', '£', '¤', '§',
    // ── Alpha-Numerics (rows 7-14) ───────────────────────────────────────────
    // Row 7 – uppercase A-M
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
    // Row 8 – uppercase N-Z
    'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
    // Row 9 – lowercase a-m
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm',
    // Row 10 – lowercase n-z
    'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    // Row 11 – digits + ! @ #
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '!', '@', '#',
    // Row 12 – punctuation
    '$', '%', '^', '&', '*', '(', ')', '-', '_', '+', '=', '[', ']',
    // Row 13 – punctuation
    '{', '}', '\\', '|', ';', ':', '\'', '"', ',', '.', '/', '<', '>',
    // Row 14 – remaining
    '?', '`', '~', ' ',
];

pub const COLOR_PALETTE_COLS: usize = 6;

/// (r, g, b, label) – colors available in the glyph editor.
pub const COLOR_PALETTE: &[(u8, u8, u8, &str)] = &[
    // Row 0 – pipe materials
    (184, 115,  51, "Copper"),
    (210,  60,  60, "PEX"),
    ( 60, 160,  60, "PE"),
    (150, 150, 165, "Galv.Iron"),
    (160, 160, 160, "Blk Plstc"),
    (100,  90,  80, "Cast Iron"),
    // Row 1 – primaries / bright
    (240, 240,  20, "Yellow"),
    ( 20, 220, 220, "Cyan"),
    ( 60, 120, 240, "Blue"),
    (220,  40, 220, "Magenta"),
    (235, 235, 235, "White"),
    (110, 110, 110, "Gray"),
    // Row 2 – warm
    (255, 150,  20, "Orange"),
    (210,  40,  40, "Red"),
    (170,  85,  30, "Brown"),
    (130,  30,  30, "Maroon"),
    (255, 160, 130, "Salmon"),
    (200, 170, 110, "Tan"),
    // Row 3 – cool
    ( 20,  40, 150, "Navy"),
    ( 20, 160, 140, "Teal"),
    ( 30, 130,  60, "Forest"),
    (100, 120,  30, "Olive"),
    ( 60, 180, 180, "DkCyan"),
    ( 80,  40, 160, "Indigo"),
    // Row 4 – light / pastel
    (120, 190, 255, "SkyBlue"),
    (130, 220, 150, "PaleGreen"),
    (190, 160, 240, "Lavender"),
    (255, 195, 160, "Peach"),
    (150, 240, 200, "Mint"),
    (245, 235, 190, "Cream"),
    // Row 5 – dark
    ( 20,  20,  20, "NearBlack"),
    ( 50,  50,  50, "VDkGray"),
    ( 60,  35,  15, "DkBrown"),
    (100,  20,  20, "DkRed"),
    ( 15,  70,  25, "DkGreen"),
    ( 15,  25,  90, "DkBlue"),
];

/// All materials in cycle order (used by editor scope controls).
pub const ALL_MATERIALS: [PipeMaterial; 6] = [
    PipeMaterial::Copper,
    PipeMaterial::PEX,
    PipeMaterial::PE,
    PipeMaterial::GalvanizedIron,
    PipeMaterial::BlackPlastic,
    PipeMaterial::CastIron,
];

/// All diameters in cycle order (used by editor scope controls).
pub const ALL_DIAMETERS: [PipeDiameter; 3] = [
    PipeDiameter::Half,
    PipeDiameter::ThreeQuarter,
    PipeDiameter::One,
];

// ── Data structures (all serde-serializable) ──────────────────────────────────

/// A single glyph definition: symbol character + RGB foreground color.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphDef {
    pub symbol: char,
    pub fg: [u8; 3],
}

/// Which face of a composite a user-defined port is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortFace {
    West,
    East,
    North,
    South,
}

/// The functional role of a port on a composite component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortKind {
    /// Pressurized water enters the fixture here (e.g. hot/cold supply line to a tub).
    Inlet,
    /// Pressurized water exits the fixture here (e.g. mixing-valve output, pump discharge).
    Outlet,
    /// Waste water exits by gravity here (e.g. tub drain, basin drain, floor drain).
    Drain,
}

/// A user-defined connection port on a composite component.
/// Coordinates are in extended footprint space where fh = inner_h + 2, fw = inner_w + 2.
/// Valid positions: row/col must be on the box border (not the 1-cell buffer ring).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPort {
    pub name: String,
    pub kind: PortKind,
    /// Row in extended footprint (1..=fh-2 for the actual box rows).
    pub row: usize,
    /// Col in extended footprint (1..=fw-2 for the actual box cols).
    pub col: usize,
}

/// A user-defined component type stored in the glyph library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCompDef {
    /// Unique identifier (used as palette key).
    pub id: String,
    /// Human-readable name shown in the palette.
    pub label: String,
    pub glyph: GlyphDef,
    /// Port connections: [north, south, east, west].
    pub connections_nsew: [bool; 4],
    /// Equivalent friction length in pipe diameters.
    pub equiv_length_d: f32,
    /// If Some((w, h)), this component is rendered as a w×h composite box with the label inside.
    /// If None, it is a single-cell glyph component.  Minimum meaningful size is 3×3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composite_size: Option<(usize, usize)>,
    /// Per-cell character overrides for the composite grid. Key format: "row,col".
    /// Overrides the default composite_box_char rendering for that cell.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub cell_overrides: HashMap<String, char>,
    /// Per-cell foreground color overrides. Same key format as cell_overrides.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub cell_color_overrides: HashMap<String, [u8; 3]>,
    /// Explicit port definitions.  When non-empty these replace the legacy connections_nsew
    /// behavior: only cells adjacent to a defined port's external cell can connect.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<CustomPort>,
}

impl CustomCompDef {
    pub fn new(id: String, label: String, glyph: GlyphDef) -> Self {
        Self {
            id,
            label,
            glyph,
            connections_nsew: [false, false, true, true], // horizontal pass-through default
            equiv_length_d: 0.0,
            composite_size: None,
            cell_overrides: HashMap::new(),
            cell_color_overrides: HashMap::new(),
            ports: Vec::new(),
        }
    }

    /// Key used in `cell_overrides` for a given (row, col) within the composite footprint.
    pub fn override_key(row: usize, col: usize) -> String {
        format!("{row},{col}")
    }

    pub fn get_cell(&self, row: usize, col: usize) -> Option<char> {
        self.cell_overrides.get(&Self::override_key(row, col)).copied()
    }

    pub fn set_cell(&mut self, row: usize, col: usize, ch: char) {
        self.cell_overrides.insert(Self::override_key(row, col), ch);
    }

    pub fn clear_cell(&mut self, row: usize, col: usize) {
        self.cell_overrides.remove(&Self::override_key(row, col));
    }

    pub fn get_cell_color(&self, row: usize, col: usize) -> Option<[u8; 3]> {
        self.cell_color_overrides.get(&Self::override_key(row, col)).copied()
    }

    pub fn set_cell_color(&mut self, row: usize, col: usize, color: [u8; 3]) {
        self.cell_color_overrides.insert(Self::override_key(row, col), color);
    }

    pub fn clear_cell_color(&mut self, row: usize, col: usize) {
        self.cell_color_overrides.remove(&Self::override_key(row, col));
    }

    pub fn get_port_at(&self, row: usize, col: usize) -> Option<&CustomPort> {
        self.ports.iter().find(|p| p.row == row && p.col == col)
    }

    /// Set (or clear) a specific port type at (row, col) in canvas footprint space.
    /// Border = dc==0, dc==fw-1, dr==0, dr==fh-1.
    /// If the cell already has a port of the same kind, it is removed (toggle off).
    /// If the cell has a port of a different kind, it is replaced.
    /// Returns a short status message.
    pub fn set_port(&mut self, row: usize, col: usize, fw: usize, fh: usize, kind: PortKind) -> &'static str {
        let on_border = (col == 0 || col + 1 == fw) && row < fh
            || (row == 0 || row + 1 == fh) && col < fw;
        if !on_border {
            return "Move cursor to the box border to add a port.";
        }
        if let Some(idx) = self.ports.iter().position(|p| p.row == row && p.col == col) {
            if self.ports[idx].kind == kind {
                self.ports.remove(idx);
                return "Port removed.";
            }
            self.ports[idx].kind = kind;
            match self.ports[idx].kind {
                PortKind::Inlet  => "Port → Inlet.",
                PortKind::Outlet => "Port → Outlet.",
                PortKind::Drain  => "Port → Drain.",
            }
        } else {
            self.ports.push(CustomPort {
                name: format!("port_{row}_{col}"),
                kind,
                row,
                col,
            });
            match self.ports.last().unwrap().kind {
                PortKind::Inlet  => "Port set: Inlet.",
                PortKind::Outlet => "Port set: Outlet.",
                PortKind::Drain  => "Port set: Drain.",
            }
        }
    }

    /// Returns (row_offset, col_offset, PortFace) from the anchor cell to each port's
    /// external connection cell (the grid cell a pipe must occupy to connect here).
    /// Ports use canvas footprint coordinates: dc=0 (west edge), dc=fw-1 (east edge),
    /// dr=0 (north edge), dr=fh-1 (south edge).  composite_size = (canvas_w, canvas_h).
    pub fn port_external_offsets(&self) -> Vec<(isize, isize, PortFace)> {
        let (canvas_w, canvas_h) = match self.composite_size {
            Some(s) => s,
            None => return vec![],
        };
        let fw = canvas_w;
        let fh = canvas_h;
        let pr = fh / 2; // effective_port_row for this composite

        self.ports.iter().map(|p| {
            let dr = p.row as isize;
            let dc = p.col as isize;
            if p.col == 0 {
                // West edge — external is one col left of entire footprint
                (dr - pr as isize, -1isize, PortFace::West)
            } else if p.col + 1 == fw {
                // East edge — external is one col right of entire footprint
                (dr - pr as isize, fw as isize, PortFace::East)
            } else if p.row == 0 {
                // North edge — external is one row above entire footprint
                (-(pr as isize) - 1, dc, PortFace::North)
            } else {
                // South edge — external is one row below entire footprint
                (fh as isize - pr as isize, dc, PortFace::South)
            }
        }).collect()
    }

    /// Parse a "row,col" override key — mirrors the helper in app.rs.
    fn parse_key(key: &str) -> Option<(usize, usize)> {
        let (r, c) = key.split_once(',')?;
        Some((r.parse().ok()?, c.parse().ok()?))
    }

    /// Migrate a composite def from v1.0 extended coordinates (inner_w+2 footprint,
    /// buffer ring at dc=0/dc=fw-1, port at dc=1/dc=fw-2) to v2.0 canvas coordinates
    /// (composite_size = canvas dims directly, port at dc=0/dc=canvas_w-1).
    pub fn migrate_v1_to_v2(&mut self) {
        let (inner_w, inner_h) = match self.composite_size {
            Some(s) => s,
            None => return,
        };
        let fw = inner_w + 2;
        let fh = inner_h + 2;

        // Drop buffer-ring overrides (dc=0, dc=fw-1, dr=0, dr=fh-1) and shift rest by -1.
        let old = std::mem::take(&mut self.cell_overrides);
        for (key, val) in old {
            if let Some((r, c)) = Self::parse_key(&key) {
                if r == 0 || r + 1 == fh || c == 0 || c + 1 == fw { continue; }
                self.cell_overrides.insert(Self::override_key(r - 1, c - 1), val);
            }
        }
        let old_colors = std::mem::take(&mut self.cell_color_overrides);
        for (key, val) in old_colors {
            if let Some((r, c)) = Self::parse_key(&key) {
                if r == 0 || r + 1 == fh || c == 0 || c + 1 == fw { continue; }
                self.cell_color_overrides.insert(Self::override_key(r - 1, c - 1), val);
            }
        }
        // Shift port positions: old dc=1 → dc=0, old dc=fw-2 → dc=fw-3 = inner_w-1 = new canvas_w-1.
        for port in &mut self.ports {
            port.col = port.col.saturating_sub(1);
            port.row = port.row.saturating_sub(1);
        }
        // composite_size values stay the same but now mean canvas dims directly.
    }
}

/// Serializable glyph library — saved/loaded as a JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphLibrary {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub version: String,
    /// Override map.  Key format: "Kind|Diameter|Material"
    /// Any slot may be "All" to match every value of that dimension.
    #[serde(default)]
    pub overrides: HashMap<String, GlyphDef>,
    #[serde(default)]
    pub custom_components: Vec<CustomCompDef>,
}

impl Default for GlyphLibrary {
    fn default() -> Self {
        Self {
            name: "My Glyph Library".into(),
            author: None,
            version: "1.0".into(),
            overrides: HashMap::new(),
            custom_components: Vec::new(),
        }
    }
}

impl GlyphLibrary {
    pub fn load(path: &Path) -> Result<Self, String> {
        let txt = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read '{}': {}", path.display(), e))?;
        let mut lib: Self = serde_json::from_str(&txt)
            .map_err(|e| format!("Parse error in '{}': {}", path.display(), e))?;
        if lib.version == "1.0" {
            for def in &mut lib.custom_components {
                def.migrate_v1_to_v2();
            }
            lib.version = "2.0".into();
        }
        Ok(lib)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Serialisation error: {}", e))?;
        std::fs::write(path, json)
            .map_err(|e| format!("Cannot write '{}': {}", path.display(), e))
    }
}

// ── Runtime registry ──────────────────────────────────────────────────────────

pub struct GlyphRegistry {
    pub library: GlyphLibrary,
    pub library_path: Option<PathBuf>,
}

impl GlyphRegistry {
    pub fn new() -> Self {
        Self { library: GlyphLibrary::default(), library_path: None }
    }

    /// Default symbol character.
    ///
    /// Pipes and fittings use double-line box chars so that the background
    /// color (water when flowing) is visible as the pipe cavity between the
    /// two strokes.  1/2" diameter uses dashed variants to suggest a thinner
    /// pipe; 3/4" and 1" both use solid double-line.
    pub fn default_symbol(kind: ComponentKind, diameter: PipeDiameter) -> char {
        use ComponentKind::*;
        use PipeDiameter::*;

        // 1/2" uses dashed double-line; everything else uses solid double-line.
        let half = diameter == Half;

        match kind {
            PipeH    => if half { '╌' } else { '═' },
            PipeV    => if half { '╎' } else { '║' },
            ElbowNE  => if half { '└' } else { '╚' },
            ElbowNW  => if half { '┘' } else { '╝' },
            ElbowSE  => if half { '┌' } else { '╔' },
            ElbowSW  => if half { '┐' } else { '╗' },
            TeeNSE         => if half { '├' } else { '╠' },
            TeeNSW         => if half { '┤' } else { '╣' },
            TeeNEW         => if half { '┴' } else { '╩' },
            TeeSEW         => if half { '┬' } else { '╦' },
            ReducerTeeNSE  => if half { '├' } else { '╟' },
            ReducerTeeNSW  => if half { '┤' } else { '╢' },
            ReducerTeeNEW  => if half { '┴' } else { '╧' },
            ReducerTeeSEW  => if half { '┬' } else { '╤' },
            Cross          => if half { '┼' } else { '╬' },
            _              => kind.symbol(), // source, sink, valves, endcap, reducer
        }
    }

    /// Default foreground RGB for a material.
    pub fn material_color(material: PipeMaterial) -> (u8, u8, u8) {
        use PipeMaterial::*;
        match material {
            Copper       => (184, 115,  51),
            PEX          => (210,  60,  60),
            PE           => ( 60, 160,  60),
            GalvanizedIron => (155, 170, 210),
            BlackPlastic   => ( 65,  65,  75),
            CastIron     => (100,  90,  80),
        }
    }

    /// Resolve the effective glyph for a placed component.
    /// Override priority (most → least specific):
    ///   Kind|Diameter|Material → Kind|Diameter|All → Kind|All|Material → Kind|All|All → built-in
    pub fn resolve(
        &self,
        kind: ComponentKind,
        material: PipeMaterial,
        diameter: PipeDiameter,
    ) -> GlyphDef {
        let dk = kind_key(kind);
        let dd = diam_key(diameter);
        let dm = mat_key(material);

        for key in &[
            format!("{dk}|{dd}|{dm}"),
            format!("{dk}|{dd}|All"),
            format!("{dk}|All|{dm}"),
            format!("{dk}|All|All"),
        ] {
            if let Some(g) = self.library.overrides.get(key) {
                return g.clone();
            }
        }

        // Built-in default: scale material color by diameter so 1/2" is dim,
        // 3/4" is the reference, and 1" is noticeably brighter/bolder.
        let (r, g, b) = Self::material_color(material);
        let f = Self::diameter_brightness(diameter);
        let s = |v: u8| (v as f32 * f).clamp(0.0, 255.0) as u8;
        GlyphDef { symbol: Self::default_symbol(kind, diameter), fg: [s(r), s(g), s(b)] }
    }

    fn diameter_brightness(diameter: PipeDiameter) -> f32 {
        match diameter {
            PipeDiameter::Half         => 0.60,
            PipeDiameter::ThreeQuarter => 1.00,
            PipeDiameter::One          => 1.50,
        }
    }

    /// Save a glyph override.  Pass `None` for diameter/material to target "All".
    pub fn set_override(
        &mut self,
        kind: ComponentKind,
        diameter: Option<PipeDiameter>,
        material: Option<PipeMaterial>,
        glyph: GlyphDef,
    ) {
        let key = format!(
            "{}|{}|{}",
            kind_key(kind),
            diameter.map(diam_key).unwrap_or("All"),
            material.map(mat_key).unwrap_or("All"),
        );
        self.library.overrides.insert(key, glyph);
    }

    pub fn load_library(&mut self, path: &Path) -> Result<(), String> {
        self.library = GlyphLibrary::load(path)?;
        self.library_path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn save_library(&self, path: &Path) -> Result<(), String> {
        self.library.save(path)
    }

    pub fn custom_components(&self) -> &[CustomCompDef] {
        &self.library.custom_components
    }

    pub fn add_custom_component(&mut self, def: CustomCompDef) {
        if let Some(ex) = self.library.custom_components.iter_mut().find(|c| c.id == def.id) {
            *ex = def;
        } else {
            self.library.custom_components.push(def);
        }
    }
}

// ── Key-encoding helpers (pub so ui.rs can build override keys for display) ───

pub fn kind_key(k: ComponentKind) -> &'static str {
    match k {
        ComponentKind::Source       => "Source",
        ComponentKind::Sink         => "Sink",
        ComponentKind::PipeH        => "PipeH",
        ComponentKind::PipeV        => "PipeV",
        ComponentKind::ElbowNE      => "ElbowNE",
        ComponentKind::ElbowNW      => "ElbowNW",
        ComponentKind::ElbowSE      => "ElbowSE",
        ComponentKind::ElbowSW      => "ElbowSW",
        ComponentKind::TeeNSE          => "TeeNSE",
        ComponentKind::TeeNSW          => "TeeNSW",
        ComponentKind::TeeNEW          => "TeeNEW",
        ComponentKind::TeeSEW          => "TeeSEW",
        ComponentKind::ReducerTeeNSE   => "ReducerTeeNSE",
        ComponentKind::ReducerTeeNSW   => "ReducerTeeNSW",
        ComponentKind::ReducerTeeNEW   => "ReducerTeeNEW",
        ComponentKind::ReducerTeeSEW   => "ReducerTeeSEW",
        ComponentKind::Cross           => "Cross",
        ComponentKind::BallValveH   => "BallValveH",
        ComponentKind::BallValveV   => "BallValveV",
        ComponentKind::CheckValveH  => "CheckValveH",
        ComponentKind::CheckValveV  => "CheckValveV",
        ComponentKind::EndCap       => "EndCap",
        ComponentKind::Reducer        => "Reducer",
        ComponentKind::PressureGauge  => "PressureGauge",
        ComponentKind::WaterSoftener     => "WaterSoftener",
        ComponentKind::WholeHouseFilter  => "WholeHouseFilter",
        ComponentKind::SedimentFilter    => "SedimentFilter",
        ComponentKind::UvFilter          => "UvFilter",
        ComponentKind::Toilet            => "Toilet",
        ComponentKind::WaterHeater       => "WaterHeater",
        ComponentKind::Faucet            => "Faucet",
        ComponentKind::BasinSink         => "BasinSink",
        ComponentKind::SolidBlock        => "SolidBlock",
        ComponentKind::Label             => "Label",
        ComponentKind::Note              => "Note",
        ComponentKind::Link              => "Link",
        ComponentKind::Custom            => "Custom",
    }
}

pub fn diam_key(d: PipeDiameter) -> &'static str {
    match d {
        PipeDiameter::Half         => "Half",
        PipeDiameter::ThreeQuarter => "ThreeQuarter",
        PipeDiameter::One          => "One",
    }
}

pub fn mat_key(m: PipeMaterial) -> &'static str {
    match m {
        PipeMaterial::Copper        => "Copper",
        PipeMaterial::PEX           => "PEX",
        PipeMaterial::PE            => "PE",
        PipeMaterial::GalvanizedIron => "GalvanizedIron",
        PipeMaterial::BlackPlastic  => "BlackPlastic",
        PipeMaterial::CastIron      => "CastIron",
    }
}

// ── Glyph editor UI state ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlyphEditorFocus {
    ComponentList,
    /// Active when a composite custom is selected — navigates the mini tile grid.
    CompositeGrid,
    CharGrid,
    ColorPicker,
}

#[derive(Debug, Clone)]
pub struct GlyphEditorState {
    /// Index into ComponentKind::all_palette() ++ custom defs.
    pub kind_idx: usize,
    /// `None` = apply to all materials; `Some(i)` = index into ALL_MATERIALS.
    pub mat_scope: Option<usize>,
    /// `None` = apply to all diameters; `Some(i)` = index into ALL_DIAMETERS.
    pub diam_scope: Option<usize>,
    /// Selected character in CHAR_PALETTE.
    pub char_cursor: usize,
    /// Selected color in COLOR_PALETTE.
    pub color_cursor: usize,
    /// When set, overrides the palette selection with an arbitrary RGB value.
    pub custom_rgb: Option<[u8; 3]>,
    /// Cursor position (row, col) within the composite footprint when editing a composite.
    pub composite_cursor: (usize, usize),
    /// Top-left visible cell (row_off, col_off) for viewport scrolling on large composites.
    pub composite_viewport: (usize, usize),
    pub focus: GlyphEditorFocus,
    pub status: String,
}

impl Default for GlyphEditorState {
    fn default() -> Self {
        Self {
            kind_idx: 0,
            mat_scope: None,
            diam_scope: None,
            char_cursor: 0,
            color_cursor: 0,
            custom_rgb: None,
            composite_cursor: (0, 0),
            composite_viewport: (0, 0),
            focus: GlyphEditorFocus::ComponentList,
            status: "  [Tab] switch panel  [Enter] apply  [N] new  [R] rename  [C] copy  [W] composite  [Del] clear cell  [S] save  [L] load  [G/Q] exit".into(),
        }
    }
}

impl GlyphEditorState {
    /// Currently selected character.
    pub fn current_symbol(&self) -> char {
        CHAR_PALETTE[self.char_cursor.min(CHAR_PALETTE.len() - 1)]
    }

    /// Currently selected RGB color (custom_rgb overrides palette selection).
    pub fn current_color(&self) -> [u8; 3] {
        if let Some(rgb) = self.custom_rgb {
            return rgb;
        }
        let (r, g, b, _) = COLOR_PALETTE[self.color_cursor.min(COLOR_PALETTE.len() - 1)];
        [r, g, b]
    }

    /// Set an arbitrary RGB color, clearing any palette cursor highlight.
    pub fn set_custom_rgb(&mut self, r: u8, g: u8, b: u8) {
        self.custom_rgb = Some([r, g, b]);
    }

    pub fn mat_label(&self) -> &'static str {
        match self.mat_scope {
            None    => "All Materials",
            Some(i) => ALL_MATERIALS[i].label(),
        }
    }

    pub fn diam_label(&self) -> &'static str {
        match self.diam_scope {
            None    => "All Diameters",
            Some(i) => ALL_DIAMETERS[i].label(),
        }
    }

    pub fn cycle_mat_scope(&mut self) {
        self.mat_scope = match self.mat_scope {
            None                               => Some(0),
            Some(i) if i + 1 < ALL_MATERIALS.len() => Some(i + 1),
            Some(_)                            => None,
        };
    }

    pub fn cycle_diam_scope(&mut self) {
        self.diam_scope = match self.diam_scope {
            None                               => Some(0),
            Some(i) if i + 1 < ALL_DIAMETERS.len() => Some(i + 1),
            Some(_)                            => None,
        };
    }

    pub fn nav_kind(&mut self, delta: isize, total_len: usize) {
        self.kind_idx = (self.kind_idx as isize + delta).rem_euclid(total_len as isize) as usize;
    }

    pub fn nav_char(&mut self, dr: isize, dc: isize) {
        let total = CHAR_PALETTE.len();
        let cols  = CHAR_PALETTE_COLS as isize;
        let rows  = ((total + CHAR_PALETTE_COLS - 1) / CHAR_PALETTE_COLS) as isize;
        let row = (self.char_cursor as isize / cols + dr).rem_euclid(rows);
        let col = (self.char_cursor as isize % cols + dc).rem_euclid(cols);
        self.char_cursor = ((row * cols + col) as usize).min(total - 1);
    }

    pub fn nav_color(&mut self, dr: isize, dc: isize) {
        self.custom_rgb = None; // switching to palette clears any custom color
        let total = COLOR_PALETTE.len();
        let cols  = COLOR_PALETTE_COLS as isize;
        let rows  = ((total + COLOR_PALETTE_COLS - 1) / COLOR_PALETTE_COLS) as isize;
        let row = (self.color_cursor as isize / cols + dr).rem_euclid(rows);
        let col = (self.color_cursor as isize % cols + dc).rem_euclid(cols);
        self.color_cursor = ((row * cols + col) as usize).min(total - 1);
    }
}
