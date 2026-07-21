<div align="center">

<img src="mascot.svg" alt="Flow Dynamics — Pipe Network Simulator" width="900">

<br><br>

<a href="https://www.buymeacoffee.com/sormondocom">
  <img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png"
       alt="Buy Me A Coffee" width="217" height="60">
</a>

<br><br>

# Flow Dynamics

**A terminal pipe network designer and fluid flow simulator — built with Rust and ratatui.**

<br>

![Rust](https://img.shields.io/badge/rust-1.75%2B-orange?logo=rust&logoColor=white)
![ratatui](https://img.shields.io/badge/ratatui-0.28-5e81f4?logo=rust&logoColor=white)
![crossterm](https://img.shields.io/badge/crossterm-0.28-blue)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgray)
![License](https://img.shields.io/badge/license-MIT-green)

</div>

---

> Draw plumbing layouts on a terminal canvas, run live pressure and flow simulations,
> annotate your designs, and export everything to text or JSON.

---

## Features

- **Live simulation** — pressure and gallons per minute (GPM) propagate through your network in real time
- **Flood animation** — open pipe ends leak animated `~`/`≈` water characters into adjacent empty cells during simulation, making dead-ends and unfinished runs immediately visible
- **Full pipe set** — straight runs, elbows, tees, crosses, reducers, ball valves, check valves, endcaps, gauges
- **Fixture components** — Toilet, Faucet/Sink, Basin Sink (with overflow animation), Water Heater, Water Softener, Whole House Filter, Sediment Filter, Ultraviolet (UV) Filter, Pressure Gauge, Flow Meter
- **Pressure Reducing Valve (PRV)** — inline valve that caps downstream pressure at an editable setpoint; protects fixtures and appliances from high municipal supply pressures
- **Expansion Tank** — dead-end branch component that absorbs thermal expansion in closed systems (required whenever a Pressure Reducing Valve or check valve is present)
- **Hot/Cold line coloring** — mark supply pipes as Cold (blue), Hot (red), or Recirculation (orange) with the `H` key; visual-only drafting aid that survives save/load
- **Cost Estimator** — press `$` to open a full-screen editor for pipe and fitting prices; the right panel shows a live Bill of Materials (BOM) cost total from the current canvas; prices persist across sessions
- **Drain-Waste-Vent (DWV) mode** — press `W` to toggle a second design layer for gravity-drain plumbing; adds six drain-side component types (Horizontal Drain, Vertical Stack, P-Trap, Vent Pipe, Drain Wye, Cleanout) with Drain Fixture Unit (DFU) counting and code-check validation in the footer
- **Annotations** — inline Labels, multi-line framed Notes, and Diagram Links (⇒) placed directly on the canvas
- **Diagram links** — Link (⇒) components store a path to another `.json` layout; press `Enter` on a placed link to follow it (prompts to save first if needed)
- **Assembly system** — save any region as a named assembly and stamp it anywhere
- **Glyph editor** — remap any component's character and color; design fully custom multi-cell composites with inlet/outlet/drain ports
- **Undo / Redo** — every edit is undoable, up to 50 steps (`Ctrl+Z` / `Ctrl+Y`)
- **Export** — dump the canvas as a UTF-8 text file or JSON layout
- **Search** — press `/` in the palette to filter components by name; press `/` in help to search the help text
- **Materials** — Copper, PEX, Galvanized Iron, Polyethylene (PE), Black Plastic (ABS/PVC), Cast Iron — each color-coded; Hazen-Williams friction applied per material
- **Grid scale** — set how many inches one canvas cell represents (6 in / 12 in / 18 in / 24 in) in the Settings screen; affects default pipe lengths and the Bill of Materials (BOM) scale note
- **Settings** — persistent config with auto-loadable glyph library files and grid scale
- **Custom splash screen** — save any layout as `splash.json` for a live animated boot screen

---

## Quick Start

```bash
git clone https://github.com/sormond/flow-dynamics.git
cd flow-dynamics
cargo build --release
./target/release/flow-dynamics        # Linux / macOS
target\release\flow-dynamics.exe      # Windows
```

Requires Rust 1.75+ and a terminal that supports Unicode and 256 colors
(Windows Terminal, iTerm2, Alacritty, Kitty, or any modern terminal emulator).

---

## How It Works

```
  [Tab]  ──  switch focus between canvas and palette
  [P]    ──  start simulation
  [S]    ──  stop simulation
  [?]    ──  open in-app help (scrollable, hot-reloads help.txt)
```

1. **Focus the palette** (`Tab`) and navigate to the component you want.
2. **Focus the canvas** (`Tab`) and move the cursor with arrow keys.
3. Press **`Enter`** to place the component.
4. Connect pipes from a **Source** to a **Sink**, then press **`P`** to simulate.

---

## Key Bindings — Quick Reference

### Global

| Key | Action |
|-----|--------|
| `?` | Open / close help |
| `Q` | Quit |
| `Tab` | Switch focus (Canvas ↔ Palette) |
| `N` | New diagram |
| `Ctrl+S` | Save layout |
| `Ctrl+O` | Open layout |
| `Ctrl+Z` | Undo |
| `Ctrl+Y` | Redo |
| `P` | Run simulation |
| `S` | Stop simulation |
| `Space` | Pause / resume |
| `X` | Export (Text or JSON) |
| `B` | Bill of Materials (BOM) overlay |
| `$` | Cost Estimator |
| `W` | Toggle Drain-Waste-Vent (DWV) mode |
| `Y` | Assembly browser |
| `R` | Rectangle selection |
| `G` | Glyph editor |
| `C` | Settings |
| `A` | Toggle dimension annotations |
| `F` | Cycle fluid type |
| `1`–`6` | Select material |

### Canvas (Build mode)

| Key | Action |
|-----|--------|
| `Arrow keys` | Move cursor |
| `Home` / `End` | Jump to first / last component |
| `Enter` | Place component (on a Link: follow it) |
| `Del` | Delete component at cursor |
| `V` | Toggle valve open/closed |
| `M` | Cycle material |
| `D` | Cycle diameter (supply) or drain diameter (Drain-Waste-Vent components) |
| `H` | Cycle line temperature — Cold / Hot / Recirculation / Unset |
| `T` | Cycle drain/sink type |
| `+` / `-` | Adjust pipe length (Shift = ±6 in) |
| `I` / `Shift+I` | Increase / decrease source or Pressure Reducing Valve (PRV) setpoint |
| `P` | On a Source or Pressure Reducing Valve (PRV): set exact pressure; otherwise run simulation |
| `L` | Enter exact pipe length |
| `E` | Edit label, note, or link path at cursor |

### Cost Estimator

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate price list |
| `PageUp` / `PageDown` | Scroll 10 rows |
| `Home` / `End` | Jump to first / last row |
| `Enter` / `E` | Edit price for selected row |
| `$` / `Esc` | Close the Cost Estimator |

### Palette (component list)

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move one item |
| `PageUp` / `PageDown` | Jump 10 items |
| `Home` / `End` | Jump to first / last item |
| `/` | Activate search — type to filter by name |
| `↑` / `↓` *(search active)* | Jump between matching items |
| `Esc` *(search active)* | Clear search |
| `Enter` | Accept selection, return focus to Canvas |

### All Other Lists (File Browser, Assembly Browser, etc.)

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move one item |
| `PageUp` / `PageDown` | Jump 10 items |
| `Home` / `End` | Jump to first / last item |

---

## Cost Estimator

Press `$` to open a full-screen pricing screen.

**Left panel** — lists every pipe type (material × diameter) and every fitting/fixture type.
Navigate with `↑`/`↓`, then press `Enter` to edit the price for that row.
- Pipe prices are entered as a cost per foot.
- Fitting prices are entered as a cost per unit.

**Right panel** — recalculates automatically from what is placed on the current canvas:
- Pipe subtotals: price per foot × total length for each material/diameter combination
- Fitting subtotals: unit price × count for each component type
- Grand total at the bottom

Prices are saved to `flow-dynamics.config.json` and persist across all future sessions —
update prices once and every diagram benefits.

---

## Drain-Waste-Vent (DWV) Mode

Drain-Waste-Vent (DWV) is the branch of plumbing that carries used water away from
fixtures by gravity. Unlike supply plumbing (which works under pressure), drain pipes
slope downhill at ¼" per foot and rely on air venting to flow freely. Vent pipes run
up through the roof to prevent siphoning of the water seals in P-traps — the U-shaped
bends that block sewer gases from entering the building.

Press `W` from Build mode to toggle Drain-Waste-Vent (DWV) mode on or off.

When active, the footer shows:
- Total **Drain Fixture Unit (DFU)** load — a standardized measure of drain flow demand per fixture type
- **✓ Trapped** or **✗ Trapped** — whether every fixture has a P-trap within 10 pipe segments
- **✓ Vented** or **✗ Vented** — whether at least one vent pipe is present

Drain-Waste-Vent (DWV) components render in warm brown/yellow/green to distinguish them from supply-side pipes. Both supply and drain components can coexist on the same canvas.

### DWV Component Types

| Symbol | Component | Description |
|--------|-----------|-------------|
| `─` | Horizontal Drain | Horizontal drain run; E/W ports; press `D` to cycle diameter |
| `│` | Vertical Stack | Vertical drain stack; N/S ports; press `D` to cycle diameter |
| `⊓` | P-Trap | U-bend that holds water to seal out sewer gases; E/W ports; renders in yellow |
| `↑` | Vent Pipe | Air-inlet riser to the roof; N/S ports; renders in green |
| `╁` | Drain Wye | Y-fitting connecting a branch into a run or stack; N/S/W ports |
| `⊠` | Cleanout | Removable plug for drain-snake access; E/W ports; sealed terminal |

### Drain Pipe Sizing (International Plumbing Code minimums)

| Diameter | Maximum Load |
|----------|-------------|
| 1½" | 3 Drain Fixture Units (DFU) |
| 2" | 6 Drain Fixture Units (DFU) |
| 3" | 20 Drain Fixture Units (DFU) |
| 4" | 180 Drain Fixture Units (DFU) |

### Default Drain Fixture Unit (DFU) Values

| Fixture | DFU |
|---------|-----|
| Toilet | 6 |
| Kitchen Sink | 2 |
| Faucet / Lavatory | 1 |
| Basin Sink | 1 |

---

## Pressure Reducing Valve (PRV)

A Pressure Reducing Valve (PRV) is an inline fitting that automatically steps down
high incoming pressure to a safe, adjustable setpoint. Municipal water mains often
deliver water at 80–150 psi — well above the 40–80 psi that household fixtures and
appliances are designed for. Without a Pressure Reducing Valve, excess pressure
causes leaks, water hammer (banging pipes), and premature wear on fittings and appliances.

Place the Pressure Reducing Valve inline on a horizontal supply run (East/West ports).
The valve creates a fixed-pressure boundary in the simulation: everything downstream
sees exactly the setpoint pressure regardless of the upstream supply.

| Key | Action |
|-----|--------|
| `I` / `Shift+I` | Increase / decrease setpoint by 1 psi |
| `P` *(cursor on PRV)* | Open dialog to type an exact psi value |

Default setpoint: 60 psi. Typical residential target: 40–65 psi.

---

## Expansion Tank

When a Pressure Reducing Valve (PRV) or check valve creates a "closed" plumbing system,
water has no path back to the municipal main. As the water heater raises the water
temperature, the water expands — and with nowhere to go, pressure builds. An expansion
tank provides a cushion of compressed air (separated from the water by a rubber bladder)
that absorbs this thermal expansion and prevents unsafe pressure spikes.

Most building codes require an expansion tank on any closed system.

Place the expansion tank as a dead-end branch off a Tee fitting on the cold-water supply
line, near the water heater. It is a sealed terminal and does not count as a drain outlet.

---

## Hot/Cold Line Coloring

Mark individual supply pipes and fittings to distinguish hot lines from cold lines at
a glance — especially useful in complex layouts where both supplies share the same canvas.

Press `H` on a supply pipe or fitting to cycle through:

| Marking | Color tint | Meaning |
|---------|-----------|---------|
| Unset | (none) | No designation |
| Cold | Blue tint | Cold water supply |
| Hot | Red tint | Hot water supply |
| Recirculation | Orange tint | Hot water recirculation return line |

The marking is purely visual — it does not affect simulation results. It is saved with
the layout and persists across sessions. The footer shows a badge (❄ COLD, 🔥 HOT,
↺ RECIRC) when the cursor is on a marked component.

---

## Grid Scale

Sets how many real-world inches one canvas grid cell represents.

Open Settings (`C`) and press `G` to cycle through the available scales:

| Scale | Best for |
|-------|----------|
| 6 in / cell | Tight quarters, bathroom rough-in |
| 12 in / cell | Default — typical residential layout |
| 18 in / cell | Larger floor plans |
| 24 in / cell | Whole-house overview |

The scale affects the default pipe length when placing new segments and appears as a
note in the Bill of Materials (BOM) header so printed counts can be converted to
real-world lengths. The change saves automatically.

---

## Annotations & Links

Place a **Label**, **Note**, or **Link** from the palette, then press `Enter` to type your text or path.

- **Labels** — single-line text that spreads across empty canvas cells in bright yellow
- **Notes** — multi-line text in a double-line framed box. In the note editor, arrow keys
  move the cursor, `Shift+Enter` inserts a line break, and `Enter` confirms. You may also
  type `|` as a line separator. An `[E]dit` hint appears when your cursor is on the box.
- **Links** ⇒ — amber anchor that stores a path to another `.json` diagram. Press `Enter`
  on a placed link to load the target file (prompts to save first if the canvas has content).
  Press `E` to edit the stored path.

All three annotation types are excluded from the simulation and Bill of Materials (BOM).

---

## Palette Colors

When a component that supports color overrides is selected (custom glyph components), a
**Palette Colors** panel appears at the bottom of the palette. Cycle focus to it with `Tab`.

| Key | Action |
|-----|--------|
| `Arrow` | Navigate the color swatch grid |
| `Home` / `End` | Jump to first / last material |
| `E` | Enter a custom Red-Green-Blue (RGB) color (R,G,B prompt) |
| `M` | Cycle the material scope for this color override |

---

## Export

Press `X` from any build or simulation screen.

| Option | Output |
|--------|--------|
| `T` — Text | UTF-8 canvas dump (labels and note frames included) |
| `J` — JSON | Full layout data (same format as a saved layout) |

The text export is ideal for pasting pipe diagrams into documentation or sharing as ASCII art.

---

## Glyph Editor

Press `G` to open. Customize any component's display character and color, or design
entirely new multi-cell composites with directional ports:

| Key | Action |
|-----|--------|
| `Tab` | Cycle panel focus: Component list → Symbol grid → Color picker |
| `Arrow` | Navigate within the focused panel |
| `Home` / `End` | Jump to first / last component |
| `Enter` | Apply selected character + color as glyph override |
| `M` | Cycle material scope |
| `D` | Cycle diameter scope (or set Drain port on a border cell) |
| `W` | Set composite width |
| `I` | Set border cell as **Inlet** |
| `O` | Set border cell as **Outlet** |
| `E` | Enter custom Red-Green-Blue (RGB) color (R,G,B prompt) |
| `N` | New custom component |
| `Del` | Clear tile under the composite editor cursor |
| `S` / `L` | Save / Load glyph library |
| `Q` / `G` | Exit the glyph editor |

Custom components participate fully in simulation — their ports define how fluid flows through them.

---

## Help Screen Search

Press `/` while the help screen is open to search the help text.

| Key | Action |
|-----|--------|
| `/` | Activate search |
| Any character | Append to search query |
| `↑` / `↓` | Jump between matching lines |
| `Backspace` | Delete last character |
| `Esc` | Clear search |

Matching lines are highlighted in green. The search bar appears at the bottom of the help overlay.

---

## File Layout

```
flow-dynamics/
├── src/               Rust source
├── glyphs.json        Default glyph overrides (optional)
├── splash.json        Animated splash screen layout (optional)
├── help.txt           In-app help content (hot-reloadable)
└── flow-dynamics.config.json   Persistent settings and prices (auto-created)
```

---

## License

MIT — see [LICENSE](LICENSE) for details.

---

<div align="center">

If Flow Dynamics saves you time or sparks joy, a coffee keeps the valves flowing!

<a href="https://www.buymeacoffee.com/sormondocom">
  <img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png"
       alt="Buy Me A Coffee" width="217" height="60">
</a>

</div>
