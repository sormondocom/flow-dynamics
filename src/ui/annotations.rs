use std::collections::HashMap;

use ratatui::style::{Color, Modifier, Style};

use crate::app::App;
use crate::components::ComponentKind;
use crate::grid::Grid;

use super::format_pipe_length;

/// Build a map of (row, col) → (char, Style) for all pipe-run dimension lines.
pub(super) fn compute_annotations(app: &App) -> HashMap<(usize, usize), (char, Style)> {
    let mut map: HashMap<(usize, usize), (char, Style)> = HashMap::new();
    let grid = &app.grid;
    let ann  = Style::default().fg(Color::Rgb(180, 180, 60));
    let bold = Style::default().fg(Color::Rgb(220, 220, 80)).add_modifier(Modifier::BOLD);

    // ── Horizontal runs ───────────────────────────────────────────────────────
    for r in 0..grid.height {
        let mut c_start: Option<usize> = None;
        let mut run_ft: f32 = 0.0;

        for c in 0..=grid.width {
            let is_h = c < grid.width
                && matches!(
                    grid.get(r, c).map(|co| co.kind),
                    Some(ComponentKind::PipeH)
                );

            if is_h {
                if c_start.is_none() {
                    c_start = Some(c);
                }
                run_ft += grid.get(r, c).unwrap().pipe_length;
            } else if let Some(cs) = c_start.take() {
                let ce = c.saturating_sub(1);
                if r > 0 {
                    place_h_annotation(&mut map, grid, r - 1, cs, ce, run_ft, ann, bold);
                }
                run_ft = 0.0;
            }
        }
    }

    // ── Vertical runs ─────────────────────────────────────────────────────────
    for c in 0..grid.width {
        let mut r_start: Option<usize> = None;
        let mut run_ft: f32 = 0.0;

        for r in 0..=grid.height {
            let is_v = r < grid.height
                && matches!(
                    grid.get(r, c).map(|co| co.kind),
                    Some(ComponentKind::PipeV)
                );

            if is_v {
                if r_start.is_none() {
                    r_start = Some(r);
                }
                run_ft += grid.get(r, c).unwrap().pipe_length;
            } else if let Some(rs) = r_start.take() {
                let re = r.saturating_sub(1);
                if c > 0 {
                    place_v_annotation(&mut map, grid, c - 1, rs, re, run_ft, ann, bold);
                }
                run_ft = 0.0;
            }
        }
    }

    map
}

fn place_h_annotation(
    map: &mut HashMap<(usize, usize), (char, Style)>,
    grid: &Grid,
    ann_row: usize,
    c_start: usize,
    c_end: usize,
    total_ft: f32,
    base: Style,
    bold: Style,
) {
    let span = c_end + 1 - c_start;
    let label = format_pipe_length(total_ft);
    let chars = build_h_ann_chars(span, &label);

    for (i, &ch) in chars.iter().enumerate() {
        let ac = c_start + i;
        if grid.get(ann_row, ac).is_none() {
            let s = if i == 0 || i + 1 == span { bold } else { base };
            map.entry((ann_row, ac)).or_insert((ch, s));
        }
    }
}

fn place_v_annotation(
    map: &mut HashMap<(usize, usize), (char, Style)>,
    grid: &Grid,
    ann_col: usize,
    r_start: usize,
    r_end: usize,
    total_ft: f32,
    base: Style,
    bold: Style,
) {
    for ar in r_start..=r_end {
        if grid.get(ar, ann_col).is_none() {
            let ch = if ar == r_start { '╥' } else if ar == r_end { '╨' } else { '║' };
            let s  = if ar == r_start || ar == r_end { bold } else { base };
            map.entry((ar, ann_col)).or_insert((ch, s));
        }
    }

    let label = format_pipe_length(total_ft);
    let mid = r_start + (r_end - r_start) / 2;
    if ann_col >= label.len() {
        let lc_start = ann_col - label.len();
        for (i, lch) in label.chars().enumerate() {
            let lc = lc_start + i;
            if grid.get(mid, lc).is_none() {
                map.entry((mid, lc)).or_insert((lch, bold));
            }
        }
    }
}

fn build_h_ann_chars(span: usize, label: &str) -> Vec<char> {
    let mut chars = vec!['─'; span.max(1)];
    if span >= 2 {
        chars[0] = '├';
        chars[span - 1] = '┤';
    }
    if label.len() + 2 <= span {
        let start = (span - label.len()) / 2;
        for (i, ch) in label.chars().enumerate() {
            chars[start + i] = ch;
        }
    }
    chars
}
