//! Where the sun is (PLAN 1.14, Lighting), from the NOAA solar position
//! formulas. Pure: the date and time are inputs, never the clock.

use glam::DVec3;

/// When and where, for the sun.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SunPlace {
    /// 1 to 12.
    pub month: u32,
    /// 1 to 31.
    pub day: u32,
    /// Local clock time in hours (13.5 is 1:30 pm).
    pub hours: f64,
    /// Hours ahead of UTC (Guam is +10).
    pub utc_offset: f64,
    /// Degrees, north positive.
    pub latitude: f64,
    /// Degrees, east positive.
    pub longitude: f64,
    /// Degrees from the green axis to north, clockwise seen from above.
    pub north: f64,
}

/// Day of the year (1 to 365), in a year that is not a leap year.
pub fn day_of_year(month: u32, day: u32) -> u32 {
    const BEFORE: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let m = month.clamp(1, 12) as usize;
    BEFORE[m - 1] + day.clamp(1, 31)
}

/// The unit direction toward the sun in model coordinates (z up). Below
/// the horizon when its z is negative.
pub fn sun_direction(p: &SunPlace) -> DVec3 {
    let doy = f64::from(day_of_year(p.month, p.day));
    // Fractional year, radians.
    let g = std::f64::consts::TAU / 365.0 * (doy - 1.0 + (p.hours - 12.0) / 24.0);
    // Equation of time (minutes) and declination (radians).
    let eqtime = 229.18
        * (0.000075 + 0.001868 * g.cos()
            - 0.032077 * g.sin()
            - 0.014615 * (2.0 * g).cos()
            - 0.040849 * (2.0 * g).sin());
    let decl = 0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2.0 * g).cos()
        + 0.000907 * (2.0 * g).sin()
        - 0.002697 * (3.0 * g).cos()
        + 0.00148 * (3.0 * g).sin();
    // True solar time (minutes) and the hour angle (radians, positive in
    // the afternoon).
    let solar = p.hours * 60.0 + eqtime + 4.0 * p.longitude - 60.0 * p.utc_offset;
    let ha = (solar / 4.0 - 180.0).to_radians();
    let lat = p.latitude.to_radians();
    // East, north and up at the place.
    let east = -decl.cos() * ha.sin();
    let north = decl.sin() * lat.cos() - decl.cos() * lat.sin() * ha.cos();
    let up = decl.sin() * lat.sin() + decl.cos() * lat.cos() * ha.cos();
    let t = p.north.to_radians();
    let n = DVec3::new(t.sin(), t.cos(), 0.0);
    let e = DVec3::new(t.cos(), -t.sin(), 0.0);
    (e * east + n * north + DVec3::Z * up).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(month: u32, day: u32, hours: f64, latitude: f64) -> SunPlace {
        SunPlace {
            month,
            day,
            hours,
            utc_offset: 0.0,
            latitude,
            longitude: 0.0,
            north: 0.0,
        }
    }

    #[test]
    fn equinox_noon_at_the_equator_is_overhead() {
        let d = sun_direction(&at(3, 20, 12.0, 0.0));
        assert!(d.z > 0.99, "{d}");
    }

    #[test]
    fn the_sun_rises_in_the_east_and_sets_in_the_west() {
        let rise = sun_direction(&at(3, 20, 6.5, 0.0));
        assert!(rise.x > 0.9 && rise.z > 0.0, "{rise}");
        let set = sun_direction(&at(3, 20, 17.5, 0.0));
        assert!(set.x < -0.9 && set.z > 0.0, "{set}");
    }

    #[test]
    fn the_pole_is_dark_in_december_and_noon_sun_is_south_in_the_north() {
        assert!(sun_direction(&at(12, 21, 12.0, 89.0)).z < 0.0);
        // Northern summer noon, 40° north: high and to the south (about
        // 73° up, NOAA).
        let d = sun_direction(&at(6, 21, 12.0, 40.0));
        let elevation = d.z.asin().to_degrees();
        assert!((elevation - 73.4).abs() < 1.0, "{elevation}");
        assert!(d.y < 0.0);
    }

    #[test]
    fn the_north_angle_turns_the_sun() {
        let a = sun_direction(&at(6, 21, 9.0, 40.0));
        let b = sun_direction(&SunPlace {
            north: 90.0,
            ..at(6, 21, 9.0, 40.0)
        });
        let turned = DVec3::new(a.y, -a.x, a.z);
        assert!(b.distance(turned) < 1e-9, "{a} {b}");
    }

    #[test]
    fn longitude_and_time_zone_cancel() {
        let a = sun_direction(&at(6, 21, 9.0, 13.0));
        let b = sun_direction(&SunPlace {
            longitude: 150.0,
            utc_offset: 10.0,
            ..at(6, 21, 9.0, 13.0)
        });
        assert!(a.distance(b) < 1e-9);
    }
}
