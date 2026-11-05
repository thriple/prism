//! App settings that change how the UI moves and redraws.

/// The choices for "reduce motion".
pub const REDUCE_MOTION_CHOICES: [&str; 3] = ["Follow system", "On", "Off"];

/// UI settings, shown in the theme demo panel. The defaults: follow the
/// system for reduce motion, a still aurora, and redraw only on change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    /// Index into [`REDUCE_MOTION_CHOICES`].
    pub reduce_motion: usize,
    /// Let the aurora behind glass slowly move (off by default).
    pub animate_aurora: bool,
    /// Redraw every frame even when nothing changes (for the fps counter).
    pub continuous_redraw: bool,
}

impl Settings {
    /// Whether motion is reduced, given what the system says (if anything).
    pub fn reduces_motion(&self, system: Option<bool>) -> bool {
        match self.reduce_motion {
            1 => true,
            2 => false,
            _ => system.unwrap_or(false),
        }
    }

    /// The motion settings for the theme layer.
    pub fn motion(&self, system: Option<bool>) -> prism_ui::MotionSettings {
        prism_ui::MotionSettings {
            reduce_motion: self.reduces_motion(system),
            animate_aurora: self.animate_aurora,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_system_by_default() {
        let s = Settings::default();
        assert!(s.reduces_motion(Some(true)));
        assert!(!s.reduces_motion(Some(false)));
        assert!(!s.reduces_motion(None));
    }

    #[test]
    fn on_and_off_override_the_system() {
        let on = Settings {
            reduce_motion: 1,
            ..Default::default()
        };
        let off = Settings {
            reduce_motion: 2,
            ..Default::default()
        };
        assert!(on.reduces_motion(Some(false)));
        assert!(!off.reduces_motion(Some(true)));
    }

    #[test]
    fn aurora_is_still_by_default() {
        assert!(!Settings::default().animate_aurora);
        assert!(!Settings::default().continuous_redraw);
    }
}
