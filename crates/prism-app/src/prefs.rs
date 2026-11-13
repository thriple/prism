//! Edit > Preferences (Everyday Commands part 3, brief 33): what the user
//! sets once for every document. The units a new document starts with,
//! and the inference engine's feel. The keymap is saved on its own
//! (`keymap.json`); the window changes its preset too.

use prism_infer::Settings;
use prism_math::DisplayUnits;
use serde::{Deserialize, Serialize};

/// The pick aperture's range (times the default capture radius).
pub const APERTURE: (f64, f64) = (0.5, 2.5);
/// How far past its radius the current snap holds (times).
pub const RELEASE: (f64, f64) = (1.0, 3.0);
/// How long a hover wakes a point (seconds).
pub const WAKE: (f64, f64) = (0.05, 0.8);
/// Smart Guides' time budget per cursor move (milliseconds).
pub const SMART_BUDGET: (f64, f64) = (0.5, 10.0);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// The units a new document starts with.
    pub units: DisplayUnits,
    /// The inference engine's feel, for every document.
    pub inference: Settings,
    /// View > Smart Guides (Ctrl+U), and its time budget (PLAN 5.1).
    pub smart_guides: bool,
    pub smart_budget_ms: f64,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            units: DisplayUnits::default(),
            inference: Settings::default(),
            smart_guides: true,
            smart_budget_ms: prism_tools::session::SMART_BUDGET_MS,
        }
    }
}

impl Prefs {
    /// Read saved preferences; anything missing or out of range falls
    /// back to the defaults.
    pub fn from_json(text: &str) -> Prefs {
        let mut p: Prefs = serde_json::from_str(text).unwrap_or_default();
        p.clamp();
        p
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Keep every value in the range the window offers.
    pub fn clamp(&mut self) {
        let d = Settings::default();
        let fit = |v: f64, (lo, hi): (f64, f64), dflt: f64| {
            if v.is_finite() { v.clamp(lo, hi) } else { dflt }
        };
        let i = &mut self.inference;
        i.aperture = fit(i.aperture, APERTURE, d.aperture);
        i.release = fit(i.release, RELEASE, d.release);
        i.wake = fit(i.wake, WAKE, d.wake);
        self.smart_budget_ms = fit(
            self.smart_budget_ms,
            SMART_BUDGET,
            prism_tools::session::SMART_BUDGET_MS,
        );
        self.units.decimals = self.units.decimals.min(prism_math::units::MAX_DECIMALS);
    }
}

/// A tolerance in millimetres, to three significant figures
/// (`0.00012 mm`, `1.5 mm`).
pub fn tolerance_text(metres: f64) -> String {
    if !(metres.is_finite() && metres > 0.0) {
        return "—".to_owned();
    }
    let mm = metres * 1e3;
    let digits = (2 - mm.log10().floor() as i32).clamp(0, 12) as usize;
    let t = format!("{mm:.digits$}");
    let t = if t.contains('.') {
        t.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        t
    };
    format!("{t} mm")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolerance_reads_in_millimetres() {
        assert_eq!(tolerance_text(1.2e-7), "0.00012 mm");
        assert_eq!(tolerance_text(0.0015), "1.5 mm");
        assert_eq!(tolerance_text(2.0), "2000 mm");
        assert_eq!(tolerance_text(0.0), "—");
    }
    use prism_math::units::LengthUnit;

    #[test]
    fn preferences_round_trip_and_fall_back_when_broken() {
        let mut p = Prefs::default();
        p.units.length = LengthUnit::Millimetres;
        p.units.decimals = 1;
        p.inference.aperture = 2.0;
        assert_eq!(Prefs::from_json(&p.to_json()), p);
        assert_eq!(Prefs::from_json("not json"), Prefs::default());
        // A file from before a setting existed keeps the rest.
        let old = Prefs::from_json(r#"{"units":{"length":"Millimetres","decimals":1}}"#);
        assert_eq!(old.units, p.units);
        assert_eq!(old.inference, Settings::default());
    }

    #[test]
    fn values_out_of_range_are_brought_back() {
        let p = Prefs::from_json(
            r#"{"inference":{"aperture":99.0,"release":0.0,"wake":0.3},
                "units":{"length":"Metres","decimals":200}}"#,
        );
        assert_eq!(p.inference.aperture, APERTURE.1);
        assert_eq!(p.inference.release, RELEASE.0);
        assert_eq!(p.inference.wake, 0.3);
        assert_eq!(p.units.decimals, prism_math::units::MAX_DECIMALS);
    }
}
