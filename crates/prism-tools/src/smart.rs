//! Smart Guides (PLAN 5.1, brief 33 part 4): while a group, component or
//! whole body is moved or pasted, its box lines up with its neighbours'
//! boxes (low side, centre, high side, per axis) and snaps to gaps equal to
//! the gaps between them.
//!
//! - Pure: the session builds an `Index` of the neighbours' boxes when the
//!   move starts, and `snap` adjusts the move for each cursor position.
//! - The index sorts every box's three values per axis, so alignment looks
//!   only at values within the pull (a binary search), never at every box.
//! - The time budget lives with the caller (the app measures; this crate
//!   reads no clock): it lowers the `Level`, which drops equal spacing
//!   first, then far objects, then everything.

use prism_math::DVec3;

/// An axis-aligned box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub lo: DVec3,
    pub hi: DVec3,
}

impl Bounds {
    pub fn new(lo: DVec3, hi: DVec3) -> Self {
        Self {
            lo: lo.min(hi),
            hi: lo.max(hi),
        }
    }

    /// The box round some points, or None if there are none.
    pub fn around(pts: impl IntoIterator<Item = DVec3>) -> Option<Self> {
        let mut it = pts.into_iter();
        let first = it.next()?;
        Some(it.fold(Self::new(first, first), |b, p| b.with(p)))
    }

    pub fn with(self, p: DVec3) -> Self {
        Self {
            lo: self.lo.min(p),
            hi: self.hi.max(p),
        }
    }

    pub fn union(self, o: Self) -> Self {
        Self {
            lo: self.lo.min(o.lo),
            hi: self.hi.max(o.hi),
        }
    }

    pub fn centre(&self) -> DVec3 {
        (self.lo + self.hi) * 0.5
    }

    pub fn moved(&self, d: DVec3) -> Self {
        Self {
            lo: self.lo + d,
            hi: self.hi + d,
        }
    }

    /// Low side, centre and high side along an axis.
    fn values(&self, a: usize) -> [f64; 3] {
        [self.lo[a], (self.lo[a] + self.hi[a]) * 0.5, self.hi[a]]
    }
}

/// How much the guides do. The time budget steps down through these, one
/// step for every few slow moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Alignment and equal spacing.
    #[default]
    Full,
    /// Alignment only.
    AlignOnly,
    /// Alignment with the objects near the moving one only.
    Near,
    /// Nothing, until the next move.
    Paused,
}

impl Level {
    pub fn lower(self) -> Self {
        match self {
            Self::Full => Self::AlignOnly,
            Self::AlignOnly => Self::Near,
            Self::Near | Self::Paused => Self::Paused,
        }
    }

    /// What the status bar says at this level.
    pub fn notice(self) -> Option<&'static str> {
        match self {
            Self::Full => None,
            Self::AlignOnly => Some(
                "Smart Guides: equal spacing paused (over the time budget); alignment still on",
            ),
            Self::Near => Some("Smart Guides: only nearby objects (over the time budget)"),
            Self::Paused => Some(
                "Smart Guides paused: too many nearby objects. They come back on the next move",
            ),
        }
    }
}

/// What a guide shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guide {
    /// The moving box lines up with `other` along `axis` at `at`.
    Align {
        axis: usize,
        at: f64,
        other: Bounds,
        /// A centre took part (the line then joins the centres).
        centre: bool,
        /// Drawn: a centre match shows only once it is really lined up.
        show: bool,
    },
    /// Two equal gaps along `axis`, each from one value to another.
    Space { axis: usize, gaps: Gaps, gap: f64 },
}

/// Two gaps along an axis, each from one value to another.
type Gaps = [(f64, f64); 2];

/// The neighbours' boxes, sorted per axis for alignment.
#[derive(Clone, Debug, Default)]
pub struct Index {
    boxes: Vec<Bounds>,
    /// Per axis: (value, box, which) for each box's low side (0), centre
    /// (1) and high side (2).
    sorted: [Vec<(f64, u32, u8)>; 3],
}

impl Index {
    pub fn new(boxes: Vec<Bounds>) -> Self {
        let sorted = [0, 1, 2].map(|a| {
            let mut v: Vec<(f64, u32, u8)> = boxes
                .iter()
                .enumerate()
                .flat_map(|(i, b)| {
                    let [l, c, h] = b.values(a);
                    [(l, i as u32, 0), (c, i as u32, 1), (h, i as u32, 2)]
                })
                .collect();
            v.sort_by(|p, q| p.0.total_cmp(&q.0));
            v
        });
        Self { boxes, sorted }
    }

    pub fn len(&self) -> usize {
        self.boxes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }

    /// The values along `a` within `tol` of `v`, with their box and
    /// whether each is a centre.
    fn near(&self, a: usize, v: f64, tol: f64) -> impl Iterator<Item = (f64, usize, bool)> + '_ {
        let s = &self.sorted[a];
        let start = s.partition_point(|(x, _, _)| *x < v - tol);
        s[start..]
            .iter()
            .take_while(move |(x, _, _)| *x <= v + tol)
            .map(|(x, i, k)| (*x, *i as usize, *k == 1))
    }
}

/// How the guides are asked.
#[derive(Clone, Copy, Debug)]
pub struct Ask {
    /// The moving box where the move started.
    pub moving: Bounds,
    /// The move so far (from the cursor).
    pub delta: DVec3,
    /// The pull, in model units at the moving box.
    pub tol: f64,
    /// The axes it may line up along (a lock leaves one).
    pub axes: [bool; 3],
    pub level: Level,
    /// The most neighbours looked at for equal spacing.
    pub cap: usize,
}

/// Neighbours looked at in `Level::Near`: within this many of the moving
/// box's size from its centre.
const NEAR_SIZES: f64 = 3.0;

/// The move adjusted by the guides, and the guides to draw.
pub fn snap(index: &Index, ask: &Ask) -> (DVec3, Vec<Guide>) {
    let mut d = ask.delta;
    let mut guides = Vec::new();
    if ask.level == Level::Paused || index.is_empty() || ask.tol <= 0.0 {
        return (d, guides);
    }
    let tol = ask.tol;
    let now = ask.moving.moved(d);
    let centre = now.centre();
    let reach = (ask.moving.hi - ask.moving.lo).length().max(tol) * NEAR_SIZES;
    let near_enough =
        |b: &Bounds| ask.level != Level::Near || (b.centre() - centre).length() <= reach;
    // Only axes it is moving along: a box sliding on the ground is always
    // lined up in Z, which says nothing.
    let axes: Vec<usize> = (0..3)
        .filter(|&a| ask.axes[a] && ask.delta[a].abs() > tol)
        .collect();

    // 1. Alignment: low, centre or high against the same of a neighbour.
    for &a in &axes {
        let mine = now.values(a);
        let mut best: Option<(f64, f64, usize, bool, f64)> = None;
        for (i, v) in mine.iter().enumerate() {
            for (x, j, is_centre) in index.near(a, *v, tol) {
                let b = &index.boxes[j];
                if !near_enough(b) {
                    continue;
                }
                let df = x - v;
                let dist = (b.centre() - centre).length();
                let centre_used = i == 1 || is_centre;
                let better = match best {
                    None => true,
                    Some((bdf, _, _, _, bdist)) => {
                        df.abs() < bdf.abs() - 1e-12
                            || ((df.abs() - bdf.abs()).abs() <= 1e-12 && dist < bdist)
                    }
                };
                if better {
                    best = Some((df, x, j, centre_used, dist));
                }
            }
        }
        if let Some((df, at, j, centre_used, _)) = best {
            d[a] += df;
            guides.push(Guide::Align {
                axis: a,
                at,
                other: index.boxes[j],
                centre: centre_used,
                show: !centre_used || df.abs() < tol / 4.0,
            });
        }
    }

    // 2. Equal spacing, with the neighbours in the same row.
    if ask.level == Level::Full {
        let mut pool: Vec<&Bounds> = index.boxes.iter().collect();
        if pool.len() > ask.cap {
            pool.select_nth_unstable_by(ask.cap, |p, q| {
                (p.centre() - centre)
                    .length_squared()
                    .total_cmp(&(q.centre() - centre).length_squared())
            });
            pool.truncate(ask.cap);
        }
        for &a in &axes {
            let now = ask.moving.moved(d);
            let (b1, b2) = ((a + 1) % 3, (a + 2) % 3);
            let in_row = |o: &Bounds| {
                [b1, b2]
                    .iter()
                    .all(|&q| o.lo[q] <= now.hi[q] + tol && o.hi[q] >= now.lo[q] - tol)
            };
            let mut row: Vec<&Bounds> = pool.iter().copied().filter(|o| in_row(o)).collect();
            row.sort_by(|p, q| p.lo[a].total_cmp(&q.lo[a]));
            let size = now.hi[a] - now.lo[a];
            let cur = now.lo[a];
            let mut best: Option<(f64, f64, Gaps)> = None;
            for w in row.windows(2) {
                let (p, q) = (w[0], w[1]);
                let g = q.lo[a] - p.hi[a];
                if g < tol {
                    continue;
                }
                // After the pair, before it, or in the middle of its gap.
                let mut cands = vec![
                    (q.hi[a] + g, g, [(p.hi[a], q.lo[a]), (q.hi[a], q.hi[a] + g)]),
                    (
                        p.lo[a] - g - size,
                        g,
                        [(p.lo[a] - g, p.lo[a]), (p.hi[a], q.lo[a])],
                    ),
                ];
                if g - size > 2.0 * tol {
                    let half = (g - size) / 2.0;
                    let t = p.hi[a] + half;
                    cands.push((t, half, [(p.hi[a], t), (t + size, q.lo[a])]));
                }
                for (t, shown, gaps) in cands {
                    let df = t - cur;
                    if df.abs() < tol && best.is_none_or(|(bdf, _, _)| df.abs() < bdf.abs()) {
                        best = Some((df, shown, gaps));
                    }
                }
            }
            if let Some((df, gap, gaps)) = best {
                d[a] += df;
                guides.push(Guide::Space { axis: a, gaps, gap });
            }
        }
    }
    (d, guides)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(x: f64) -> Bounds {
        Bounds::new(DVec3::new(x, 0.0, 0.0), DVec3::new(x + 1.0, 1.0, 1.0))
    }

    fn ask(moving: Bounds, delta: DVec3) -> Ask {
        Ask {
            moving,
            delta,
            tol: 0.1,
            axes: [true; 3],
            level: Level::Full,
            cap: 200,
        }
    }

    #[test]
    fn a_box_lines_up_with_a_neighbour_on_the_axis_it_moves_along() {
        // A neighbour at x 0..4, the moving box starts at y = 3.
        let index = Index::new(vec![Bounds::new(DVec3::ZERO, DVec3::new(4.0, 1.0, 1.0))]);
        let start = Bounds::new(DVec3::new(5.0, 3.0, 0.0), DVec3::new(6.0, 4.0, 1.0));
        // Slide it left to x = 0.05: its low side meets the neighbour's.
        let (d, g) = snap(&index, &ask(start, DVec3::new(-4.95, 0.5, 0.0)));
        assert!((start.lo.x + d.x).abs() < 1e-12, "{d}");
        assert_eq!(d.y, 0.5, "y is free: nothing lines up within reach");
        assert!(matches!(g[0], Guide::Align { axis: 0, at, .. } if at == 0.0));
        // Z did not move, so no guide in Z although the boxes share it.
        assert!(g.iter().all(|g| !matches!(g, Guide::Align { axis: 2, .. })));
    }

    #[test]
    fn a_centre_match_snaps_but_shows_only_when_close() {
        let index = Index::new(vec![Bounds::new(DVec3::ZERO, DVec3::new(4.0, 1.0, 1.0))]);
        let start = Bounds::new(DVec3::new(10.0, 3.0, 0.0), DVec3::new(11.0, 4.0, 1.0));
        // Its centre (10.5) goes to 2.0 + 0.08: snaps, but too far to show.
        let (d, g) = snap(&index, &ask(start, DVec3::new(-8.42, 0.0, 0.0)));
        assert!((start.centre().x + d.x - 2.0).abs() < 1e-12);
        assert!(matches!(
            g[0],
            Guide::Align {
                centre: true,
                show: false,
                ..
            }
        ));
        let (_, g) = snap(&index, &ask(start, DVec3::new(-8.49, 0.0, 0.0)));
        assert!(matches!(
            g[0],
            Guide::Align {
                centre: true,
                show: true,
                ..
            }
        ));
    }

    #[test]
    fn a_gap_equal_to_the_neighbours_gap_snaps_after_before_and_between() {
        // Two cubes 1 apart: 0..1 and 2..3.
        let index = Index::new(vec![cube(0.0), cube(2.0)]);
        let start = cube(10.0);
        // After: low side at 4 (gap 1 after the second cube).
        let (d, g) = snap(&index, &ask(start, DVec3::new(-5.95, 0.0, 0.0)));
        assert!((start.lo.x + d.x - 4.0).abs() < 1e-12, "{d}");
        let space = g.iter().find_map(|g| match g {
            Guide::Space { gaps, gap, .. } => Some((*gaps, *gap)),
            _ => None,
        });
        assert_eq!(space, Some(([(1.0, 2.0), (3.0, 4.0)], 1.0)));
        // Before: high side at -1.
        let (d, _) = snap(&index, &ask(start, DVec3::new(-11.96, 0.0, 0.0)));
        assert!((start.hi.x + d.x + 1.0).abs() < 1e-12, "{d}");
        // Between, in a wider gap: cubes at 0 and 4, a cube of 1 in the middle.
        let index = Index::new(vec![cube(0.0), cube(4.0)]);
        let (d, g) = snap(&index, &ask(start, DVec3::new(-7.94, 0.0, 0.0)));
        assert!((start.lo.x + d.x - 2.0).abs() < 1e-12, "{d}");
        assert!(
            g.iter()
                .any(|g| matches!(g, Guide::Space { gap, .. } if (*gap - 1.0).abs() < 1e-12))
        );
    }

    #[test]
    fn only_neighbours_in_the_same_row_count_for_spacing() {
        // The pair is 5 m away in y: not in the moving cube's row.
        let far = |x: f64| cube(x).moved(DVec3::new(0.0, 5.0, 0.0));
        let index = Index::new(vec![far(0.0), far(2.0)]);
        let (_, g) = snap(&index, &ask(cube(10.0), DVec3::new(-5.95, 0.0, 0.0)));
        assert!(g.iter().all(|g| !matches!(g, Guide::Space { .. })));
    }

    #[test]
    fn levels_drop_spacing_then_far_objects_then_everything() {
        let index = Index::new(vec![
            cube(0.0),
            cube(2.0),
            cube(40.0).moved(DVec3::new(0.0, 0.0, 0.0)),
        ]);
        let a = ask(cube(10.0), DVec3::new(-5.95, 0.0, 0.0));
        let kinds = |l: Level| {
            let (_, g) = snap(&index, &Ask { level: l, ..a });
            g.iter()
                .map(|g| matches!(g, Guide::Space { .. }))
                .collect::<Vec<_>>()
        };
        assert!(kinds(Level::Full).contains(&true));
        assert!(!kinds(Level::AlignOnly).contains(&true));
        // Paused: the move is left alone.
        let (d, g) = snap(
            &index,
            &Ask {
                level: Level::Paused,
                ..a
            },
        );
        assert_eq!(d, a.delta);
        assert!(g.is_empty());
        // Near: a far box's side does not pull.
        let index = Index::new(vec![cube(100.0)]);
        let start = cube(108.0);
        let delta = DVec3::new(-7.95, 0.0, 0.0);
        let (d, _) = snap(
            &index,
            &Ask {
                level: Level::Near,
                ..ask(start, delta)
            },
        );
        assert!(
            (start.lo.x + d.x - 100.0).abs() < 1e-12,
            "near enough: 3 sizes"
        );
        let index = Index::new(vec![cube(0.0)]);
        let start = cube(100.0);
        let delta = DVec3::new(-99.95, 50.0, 0.0);
        let (d, _) = snap(
            &index,
            &Ask {
                level: Level::Near,
                ..ask(start, delta)
            },
        );
        assert_eq!(d, delta, "too far in Near");
        assert_eq!(Level::Full.lower().lower().lower(), Level::Paused);
        assert!(Level::Paused.notice().is_some() && Level::Full.notice().is_none());
    }

    #[test]
    fn a_lock_leaves_one_axis() {
        let index = Index::new(vec![cube(0.0)]);
        let start = Bounds::new(DVec3::new(5.0, 3.05, 0.0), DVec3::new(6.0, 4.05, 1.0));
        let mut a = ask(start, DVec3::new(-4.95, -3.0, 0.0));
        a.axes = [false, true, false];
        let (d, _) = snap(&index, &a);
        assert_eq!(d.x, -4.95, "x is not pulled under a y lock");
        assert!((start.lo.y + d.y).abs() < 1e-12);
    }

    #[test]
    fn the_index_finds_values_in_range_only() {
        let index = Index::new((0..100).map(|i| cube(f64::from(i) * 3.0)).collect());
        let found: Vec<f64> = index.near(0, 30.0, 0.6).map(|(x, _, _)| x).collect();
        assert_eq!(found, vec![30.0, 30.5]);
        assert_eq!(index.len(), 100);
    }
}
