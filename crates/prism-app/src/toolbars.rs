//! Where each toolbar is (brief 19): docked on the left rail or in the top
//! dock, floating inside the app, or closed. Pure, so it is tested here;
//! `chrome::dock` draws it. Saved in the user's settings, not documents.

use serde::{Deserialize, Serialize};

/// A dock along an edge of the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dock {
    /// The left rail: toolbars stack down it.
    Left,
    /// Between the menu bar and the document tabs: toolbars run across.
    Top,
}

impl Dock {
    /// Whether toolbars run down it (their buttons in columns).
    pub fn vertical(self) -> bool {
        self == Dock::Left
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Place {
    Docked(Dock),
    /// The top-left corner, in points from the app's corner.
    Floating {
        x: f32,
        y: f32,
    },
}

/// One open toolbar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    pub name: String,
    pub place: Place,
    /// How many buttons across it floats, if the user reshaped it (part 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<usize>,
}

/// Every open toolbar; a closed one is absent. Within a dock, toolbars
/// come in the order they are listed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub bars: Vec<Bar>,
}

/// Floating toolbars snap to an edge this close, in points.
pub const SNAP: f32 = 8.0;

impl Layout {
    /// Every toolbar on the left rail, in the given order (the default, and
    /// what Reset Toolbars gives).
    pub fn default_for(names: &[&str]) -> Layout {
        Layout {
            bars: names
                .iter()
                .map(|n| Bar {
                    name: (*n).to_owned(),
                    place: Place::Docked(Dock::Left),
                    columns: None,
                })
                .collect(),
        }
    }

    pub fn is_open(&self, name: &str) -> bool {
        self.bars.iter().any(|b| b.name == name)
    }

    pub fn place(&self, name: &str) -> Option<Place> {
        self.bars.iter().find(|b| b.name == name).map(|b| b.place)
    }

    pub fn close(&mut self, name: &str) {
        self.bars.retain(|b| b.name != name);
    }

    /// How many buttons across `name` (of `n` buttons) is when floating:
    /// the user's shape, else one column for five or fewer, two for more.
    pub fn columns(&self, name: &str, n: usize) -> usize {
        self.bars
            .iter()
            .find(|b| b.name == name)
            .and_then(|b| b.columns)
            .unwrap_or_else(|| float_columns(n))
            .clamp(1, n.max(1))
    }

    /// Reshape an open toolbar.
    pub fn set_columns(&mut self, name: &str, columns: usize) {
        if let Some(b) = self.bars.iter_mut().find(|b| b.name == name) {
            b.columns = Some(columns.max(1));
        }
    }

    /// Take `name` out, keeping its shape for where it goes next.
    fn take(&mut self, name: &str) -> Option<usize> {
        let columns = self
            .bars
            .iter()
            .find(|b| b.name == name)
            .and_then(|b| b.columns);
        self.close(name);
        columns
    }

    /// The toolbars in a dock, in order.
    pub fn in_dock(&self, dock: Dock) -> Vec<&str> {
        self.bars
            .iter()
            .filter(|b| b.place == Place::Docked(dock))
            .map(|b| b.name.as_str())
            .collect()
    }

    /// The floating toolbars and where they are.
    pub fn floating(&self) -> Vec<(&str, f32, f32)> {
        self.bars
            .iter()
            .filter_map(|b| match b.place {
                Place::Floating { x, y } => Some((b.name.as_str(), x, y)),
                Place::Docked(_) => None,
            })
            .collect()
    }

    /// Dock `name` (open or not) so it is the `index`-th toolbar of `dock`.
    pub fn dock(&mut self, name: &str, dock: Dock, index: usize) {
        let columns = self.take(name);
        // Before the index-th toolbar already there, else at the end.
        let at = self
            .bars
            .iter()
            .enumerate()
            .filter(|(_, b)| b.place == Place::Docked(dock))
            .nth(index)
            .map_or(self.bars.len(), |(i, _)| i);
        self.bars.insert(
            at,
            Bar {
                name: name.to_owned(),
                place: Place::Docked(dock),
                columns,
            },
        );
    }

    /// Float `name` (open or not) at `x`, `y`.
    pub fn float(&mut self, name: &str, x: f32, y: f32) {
        let columns = self.take(name);
        self.bars.push(Bar {
            name: name.to_owned(),
            place: Place::Floating { x, y },
            columns,
        });
    }

    /// Drop toolbars that no longer exist, and repeats.
    pub fn keep_known(&mut self, known: &[&str]) {
        let mut seen = Vec::new();
        self.bars.retain(|b| {
            let keep = known.contains(&b.name.as_str()) && !seen.contains(&b.name);
            seen.push(b.name.clone());
            keep
        });
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// A saved layout, with toolbars that no longer exist left out.
    pub fn from_json(text: &str, known: &[&str]) -> Option<Layout> {
        let mut l: Layout = serde_json::from_str(text).ok()?;
        l.keep_known(known);
        Some(l)
    }
}

/// How many columns a floating toolbar of `n` buttons has: one for five or
/// fewer, two for six or more.
pub fn float_columns(n: usize) -> usize {
    if n <= 5 { 1 } else { 2 }
}

/// How many buttons thick a toolbar `columns` across is in a dock: it
/// keeps its shape, turned so its long side runs along the dock.
pub fn docked_thickness(columns: usize, n: usize) -> usize {
    let columns = columns.clamp(1, n.max(1));
    columns.min(n.max(1).div_ceil(columns))
}

/// How many buttons fit along `len` points (buttons `button` wide with
/// `gap` between), for a toolbar of `n`: the nearest count, 1 to `n`.
/// Reshaping a floating toolbar by its edge uses this.
pub fn fit(len: f32, button: f32, gap: f32, n: usize) -> usize {
    let k = ((len + gap) / (button + gap)).round();
    (k.max(1.0) as usize).min(n.max(1))
}

/// Where button `k` of `n` sits, as (column, row), and the grid's size, for
/// a toolbar `lines` buttons thick. Running down (`vertical`) it fills rows
/// of `lines` buttons; running across it takes the same shape turned
/// sideways (columns of `lines` buttons).
pub fn grid(n: usize, lines: usize, vertical: bool) -> (Vec<(usize, usize)>, (usize, usize)) {
    let lines = lines.clamp(1, n.max(1));
    let long = n.div_ceil(lines);
    let cells = (0..n)
        .map(|k| {
            let (a, b) = (k % lines, k / lines);
            if vertical { (a, b) } else { (b, a) }
        })
        .collect();
    let size = if vertical {
        (lines, long)
    } else {
        (long, lines)
    };
    (cells, size)
}

/// A rectangle in points: left, top, width, height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box2 {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Box2 {
    fn xs(&self) -> [f32; 2] {
        [self.x, self.x + self.w]
    }
    fn ys(&self) -> [f32; 2] {
        [self.y, self.y + self.h]
    }
}

/// Move a floating toolbar's box so its edges meet nearby edges: the other
/// toolbars' (either side of them) and the area's, each axis on its own,
/// the nearest within [`SNAP`]. Returns the new top-left corner.
pub fn snap(r: Box2, others: &[Box2], area: Box2) -> (f32, f32) {
    let best = |mine: [f32; 2], theirs: Vec<f32>| -> f32 {
        let mut d = 0.0f32;
        let mut found = SNAP;
        for m in mine {
            for t in &theirs {
                let gap = t - m;
                if gap.abs() < found {
                    found = gap.abs();
                    d = gap;
                }
            }
        }
        d
    };
    let mut xs: Vec<f32> = others.iter().flat_map(Box2::xs).collect();
    xs.extend(area.xs());
    let mut ys: Vec<f32> = others.iter().flat_map(Box2::ys).collect();
    ys.extend(area.ys());
    (r.x + best(r.xs(), xs), r.y + best(r.ys(), ys))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAMES: [&str; 3] = ["Principal", "Draw", "Edit"];

    #[test]
    fn default_and_reset_put_everything_on_the_left() {
        let l = Layout::default_for(&NAMES);
        assert_eq!(l.in_dock(Dock::Left), NAMES);
        assert!(l.in_dock(Dock::Top).is_empty());
        assert!(l.floating().is_empty());
    }

    #[test]
    fn close_reopen_dock_and_float() {
        let mut l = Layout::default_for(&NAMES);
        l.close("Draw");
        assert!(!l.is_open("Draw"));
        l.dock("Draw", Dock::Top, 0);
        assert_eq!(l.in_dock(Dock::Top), ["Draw"]);
        // Docked before the first toolbar on the left.
        l.dock("Edit", Dock::Left, 0);
        assert_eq!(l.in_dock(Dock::Left), ["Edit", "Principal"]);
        // An index past the end puts it last.
        l.dock("Draw", Dock::Left, 9);
        assert_eq!(l.in_dock(Dock::Left), ["Edit", "Principal", "Draw"]);
        l.float("Principal", 10.0, 20.0);
        assert_eq!(l.floating(), [("Principal", 10.0, 20.0)]);
        assert_eq!(l.in_dock(Dock::Left), ["Edit", "Draw"]);
        assert_eq!(l.bars.len(), 3, "each toolbar once");
    }

    #[test]
    fn shapes() {
        assert_eq!(float_columns(3), 1);
        assert_eq!(float_columns(5), 1);
        assert_eq!(float_columns(6), 2);
        // Down the left rail, two across, row by row.
        let (cells, size) = grid(5, 2, true);
        assert_eq!(size, (2, 3));
        assert_eq!(cells[..3], [(0, 0), (1, 0), (0, 1)]);
        // Across the top dock: the same shape turned sideways.
        let (cells, size) = grid(5, 2, false);
        assert_eq!(size, (3, 2));
        assert_eq!(cells[..3], [(0, 0), (0, 1), (1, 0)]);
        // A one-button toolbar is one button whatever the thickness.
        assert_eq!(grid(1, 2, true).1, (1, 1));
    }

    #[test]
    fn a_shape_is_kept_through_moves_and_cleared_by_reset() {
        let mut l = Layout::default_for(&NAMES);
        assert_eq!(l.columns("Draw", 8), 2, "default: two for six or more");
        assert_eq!(l.columns("Principal", 3), 1);
        l.set_columns("Draw", 4);
        assert_eq!(l.columns("Draw", 8), 4);
        l.float("Draw", 5.0, 5.0);
        l.dock("Draw", Dock::Top, 0);
        assert_eq!(l.columns("Draw", 8), 4, "kept");
        // Never more than its buttons, never fewer than one.
        l.set_columns("Edit", 9);
        assert_eq!(l.columns("Edit", 3), 3);
        l.set_columns("Edit", 0);
        assert_eq!(l.columns("Edit", 3), 1);
        // A closed toolbar comes back in its default shape.
        l.close("Draw");
        l.dock("Draw", Dock::Left, 9);
        assert_eq!(l.columns("Draw", 8), 2);
        l.set_columns("Draw", 4);
        assert_eq!(Layout::default_for(&NAMES).columns("Draw", 8), 2);
    }

    #[test]
    fn docked_toolbars_keep_their_shape_turned_along_the_dock() {
        // The defaults: one thick for five or fewer, two for more.
        assert_eq!(docked_thickness(float_columns(3), 3), 1);
        assert_eq!(docked_thickness(float_columns(8), 8), 2);
        // A one-row strip docks one thick; a 3 by 3 block three thick.
        assert_eq!(docked_thickness(8, 8), 1);
        assert_eq!(docked_thickness(3, 9), 3);
        assert_eq!(docked_thickness(4, 8), 2);
        assert_eq!(docked_thickness(1, 1), 1);
    }

    #[test]
    fn an_edge_dragged_to_a_length_gives_the_nearest_count() {
        // Buttons 44 wide, 4 apart: 2 buttons are 92 long.
        assert_eq!(fit(92.0, 44.0, 4.0, 8), 2);
        assert_eq!(fit(110.0, 44.0, 4.0, 8), 2);
        assert_eq!(fit(130.0, 44.0, 4.0, 8), 3);
        assert_eq!(fit(5.0, 44.0, 4.0, 8), 1);
        assert_eq!(fit(9999.0, 44.0, 4.0, 8), 8);
    }

    #[test]
    fn floating_toolbars_snap_to_nearby_edges_only() {
        let area = Box2 {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 800.0,
        };
        let other = Box2 {
            x: 100.0,
            y: 100.0,
            w: 50.0,
            h: 200.0,
        };
        // 5 points right of the other's right edge: snaps to touch it, and
        // its top lines up with the other's top (3 points away).
        let r = Box2 {
            x: 155.0,
            y: 103.0,
            w: 50.0,
            h: 100.0,
        };
        assert_eq!(snap(r, &[other], area), (150.0, 100.0));
        // 30 points away: stays put.
        let far = Box2 {
            x: 180.0,
            y: 400.0,
            w: 50.0,
            h: 100.0,
        };
        assert_eq!(snap(far, &[other], area), (180.0, 400.0));
        // Near the area's corner: snaps into it.
        let corner = Box2 {
            x: 4.0,
            y: 795.0 - 100.0,
            w: 50.0,
            h: 100.0,
        };
        assert_eq!(snap(corner, &[], area), (0.0, 700.0));
    }

    #[test]
    fn a_saved_layout_reads_back_without_unknown_toolbars() {
        let mut l = Layout::default_for(&NAMES);
        l.float("Edit", 1.0, 2.0);
        let text = l.to_json();
        assert_eq!(Layout::from_json(&text, &NAMES), Some(l.clone()));
        let fewer = Layout::from_json(&text, &["Principal"]).expect("reads");
        assert_eq!(fewer.in_dock(Dock::Left), ["Principal"]);
        assert!(fewer.floating().is_empty());
        assert!(Layout::from_json("not json", &NAMES).is_none());
        // Part 1's layouts had no shapes; they still read.
        let old = r#"{"bars":[{"name":"Draw","place":{"Docked":"Top"}}]}"#;
        let l = Layout::from_json(old, &NAMES).expect("reads");
        assert_eq!(l.in_dock(Dock::Top), ["Draw"]);
        assert_eq!(l.columns("Draw", 8), 2);
    }
}

#[cfg(test)]
mod view_toolbar {
    use crate::actions::registry::Registry;

    /// The View toolbar has X-ray and Hidden Geometry, and every toolbar
    /// button's icon parses.
    #[test]
    fn the_view_toolbar_and_its_icons() {
        let reg = Registry::new();
        let view = reg
            .dock()
            .iter()
            .find(|(n, _)| *n == "View")
            .expect("a View toolbar");
        assert_eq!(view.1, ["view.xray", "view.hidden"]);
        for (_, ids) in reg.dock() {
            for id in *ids {
                let svg = reg.icon(id).unwrap_or_else(|| panic!("{id} has no icon"));
                prism_ui::icon::Icon::parse(svg).unwrap_or_else(|e| panic!("{id}: {e}"));
            }
        }
    }
}
