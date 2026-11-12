//! NURBS Spike (PLAN 6.0, brief 30): the same solid cases through both
//! pure-Rust kernels, each checked against a volume worked out by hand.
//! A case passes when the kernel returns a valid solid whose volume is
//! within 1 % (curved results are measured from a fine tessellation for
//! monstertruck, exactly by arris). Errors, panics and wrong volumes are
//! counted separately. Run with `cargo run --release`.

use std::f64::consts::PI;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

mod arris_cases;
mod mt_cases;

/// What one case should give.
pub struct Case {
    pub name: &'static str,
    /// What it tests, for the report.
    pub what: &'static str,
    pub volume: f64,
}

/// The cases, in the order both kernels run them. `k` is the index used by
/// each kernel's builder.
pub fn cases() -> Vec<Case> {
    let r = 1.0;
    let cyl_len = 6.0;
    // Two perpendicular cylinders of radii a ≤ b: V = 8 ∫₀ᵃ √(a²−x²)√(b²−x²) dx.
    let crossing = |a: f64, b: f64| {
        let n = 200_000;
        let h = a / n as f64;
        (0..n)
            .map(|i| {
                let x = (i as f64 + 0.5) * h;
                ((a * a - x * x) * (b * b - x * x)).sqrt()
            })
            .sum::<f64>()
            * h
            * 8.0
    };
    let lens = |r: f64, d: f64| PI * (4.0 * r + d) * (2.0 * r - d).powi(2) / 12.0;
    let ball = |r: f64| 4.0 / 3.0 * PI * r.powi(3);
    let (a, f): (f64, f64) = (2.0, 0.3);
    let rounded_cube = (a - 2.0 * f).powi(3)
        + 6.0 * f * (a - 2.0 * f).powi(2)
        + 3.0 * PI * f * f * (a - 2.0 * f)
        + 4.0 / 3.0 * PI * f.powi(3);
    vec![
        Case {
            name: "box - box",
            what: "planar boolean",
            volume: 8.0 - 1.0,
        },
        Case {
            name: "box - cylinder (hole)",
            what: "plane x cylinder: circles",
            volume: 8.0 - PI * 0.5 * 0.5 * 2.0,
        },
        Case {
            name: "box - tilted cylinder",
            what: "plane x cylinder: ellipses",
            // A cylinder at 30° through a 2-thick slab: the hole's volume
            // is the base area times the slab's thickness.
            volume: 8.0 - PI * 0.4 * 0.4 * 2.0 / (PI / 6.0).cos(),
        },
        Case {
            name: "cylinder AND cylinder, equal radii",
            what: "cylinder x cylinder, tangent branches (Steinmetz)",
            volume: 16.0 / 3.0 * r * r * r,
        },
        Case {
            name: "cylinder AND cylinder, radii 1 and 0.6",
            what: "cylinder x cylinder, two loops",
            volume: crossing(0.6, 1.0),
        },
        Case {
            name: "cylinder OR cylinder, equal radii",
            what: "union with tangent branches",
            volume: 2.0 * PI * r * r * cyl_len - 16.0 / 3.0 * r * r * r,
        },
        Case {
            name: "sphere AND box (half)",
            what: "plane x sphere",
            volume: ball(1.0) / 2.0,
        },
        Case {
            name: "sphere OR sphere",
            what: "sphere x sphere",
            volume: 2.0 * ball(1.0) - lens(1.0, 1.0),
        },
        Case {
            name: "sphere - cylinder",
            what: "sphere x cylinder, off centre",
            // Measured against a fine sample in the report (no closed form);
            // set below from a numeric integral.
            volume: {
                // Ball of radius 1 minus a cylinder of radius 0.4 along z at
                // x = 0.3: integrate the ball's height over the disc.
                let n = 1200;
                let mut v = 0.0;
                let h = 0.8 / n as f64;
                for i in 0..n {
                    for j in 0..n {
                        let x = 0.3 - 0.4 + (i as f64 + 0.5) * h;
                        let y = -0.4 + (j as f64 + 0.5) * h;
                        if (x - 0.3).powi(2) + y * y <= 0.16 {
                            let s = 1.0 - x * x - y * y;
                            if s > 0.0 {
                                v += 2.0 * s.sqrt() * h * h;
                            }
                        }
                    }
                }
                ball(1.0) - v
            },
        },
        Case {
            name: "torus - box (half)",
            what: "plane x torus",
            volume: PI * 0.5 * 0.5 * 2.0 * PI * 2.0 / 2.0,
        },
        Case {
            name: "fillet one cube edge",
            what: "constant-radius fillet",
            volume: 8.0 - (1.0 - PI / 4.0) * f * f * a,
        },
        Case {
            name: "fillet all 12 cube edges",
            what: "fillets meeting at corners",
            volume: rounded_cube,
        },
    ]
}

/// The outcome of one case on one kernel.
pub enum Outcome {
    /// A valid solid of this volume.
    Solid(f64),
    /// The kernel said no.
    Refused(String),
}

fn run(f: impl FnOnce() -> Outcome) -> (Result<Outcome, String>, f64) {
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(f)).map_err(|e| {
        e.downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_else(|| "panic".into())
    });
    (r, t.elapsed().as_secs_f64() * 1e3)
}

fn short(s: &str) -> String {
    let s = s.replace('\n', " ");
    if s.len() > 70 {
        format!("{}…", &s[..70])
    } else {
        s
    }
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let cases = cases();
    println!("| case | tests | arris | ms | monstertruck | ms |");
    println!("|---|---|---|---|---|---|");
    let mut score = [0usize; 2];
    for (k, c) in cases.iter().enumerate() {
        let mut cells = Vec::new();
        let kernels: [&dyn Fn(usize) -> Outcome; 2] = [&arris_cases::run, &mt_cases::run];
        for (j, kernel) in kernels.into_iter().enumerate() {
            let s = &mut score[j];
            let (r, ms) = run(|| kernel(k));
            let text = match r {
                Ok(Outcome::Solid(v)) => {
                    let err = (v - c.volume).abs() / c.volume;
                    if err <= 0.01 {
                        *s += 1;
                        format!("ok ({:.3} %)", err * 100.0)
                    } else {
                        format!("WRONG volume {v:.4} (want {:.4})", c.volume)
                    }
                }
                Ok(Outcome::Refused(e)) => format!("refused: {}", short(&e)),
                Err(p) => format!("PANIC: {}", short(&p)),
            };
            cells.push((text, ms));
        }
        println!(
            "| {} | {} | {} | {:.0} | {} | {:.0} |",
            c.name, c.what, cells[0].0, cells[0].1, cells[1].0, cells[1].1
        );
    }
    println!();
    println!(
        "Passed: arris {}/{}, monstertruck {}/{}",
        score[0],
        cases.len(),
        score[1],
        cases.len()
    );
    // Random cuts: cylinders through a box at random tilts and offsets.
    let n = 40;
    let mut ok = [0usize; 2];
    let mut agree = 0usize;
    let mut seed = 12345u64;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for _ in 0..n {
        let tilt = rnd() * 0.9;
        let turn = rnd() * 2.0 * PI;
        let off = [rnd() - 0.5, rnd() - 0.5];
        let radius = 0.15 + rnd() * 0.4;
        let (a, _) = run(|| arris_cases::random_cut(tilt, turn, off, radius));
        let (b, _) = run(|| mt_cases::random_cut(tilt, turn, off, radius));
        let vol = |r: &Result<Outcome, String>| match r {
            Ok(Outcome::Solid(v)) if *v > 0.0 && *v < 8.0 => Some(*v),
            _ => None,
        };
        let (va, vb) = (vol(&a), vol(&b));
        ok[0] += usize::from(va.is_some());
        ok[1] += usize::from(vb.is_some());
        if let (Some(x), Some(y)) = (va, vb) {
            agree += usize::from((x - y).abs() / x < 0.01);
        }
    }
    println!(
        "Random tilted holes ({n}): arris {} valid, monstertruck {} valid, {} agree within 1 %",
        ok[0], ok[1], agree
    );
    let (r, ms) = run(|| match arris_cases::nurbs_consistency() {
        Ok((a, b)) => Outcome::Solid(a.max(b)),
        Err(e) => Outcome::Refused(e),
    });
    match r {
        Ok(Outcome::Solid(worst)) => println!(
            "arris, NURBS lofts in booleans (loft vs box, loft vs loft): consistent to {:.2e} ({ms:.0} ms)",
            worst
        ),
        Ok(Outcome::Refused(e)) => {
            println!("arris, NURBS lofts in booleans: refused: {}", short(&e))
        }
        Err(p) => println!("arris, NURBS lofts in booleans: PANIC {}", short(&p)),
    }
}
