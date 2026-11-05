//! Lengths typed into the measurement box, and how they are shown.
//!
//! Units as in the feel test (`parseLen(`): mm, cm, m (the default), in or
//! `"`, ft or `'`, and feet and inches such as `2'6"`. Values between them
//! are separated as DECISIONS #5 says: `,` where the decimal mark is `.`,
//! `;` where it is `,`, and `;` always.

/// How numbers are written where the user is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberFormat {
    /// `.` or `,`.
    pub decimal: char,
}

impl Default for NumberFormat {
    fn default() -> Self {
        Self { decimal: '.' }
    }
}

impl NumberFormat {
    /// Characters that separate two values.
    pub fn separators(&self) -> &'static [char] {
        if self.decimal == ',' {
            &[';']
        } else {
            &[',', ';']
        }
    }

    pub fn is_separator(&self, c: char) -> bool {
        self.separators().contains(&c)
    }

    fn number(&self, s: &str) -> Option<f64> {
        let t: String = s
            .chars()
            .map(|c| if c == self.decimal { '.' } else { c })
            .collect();
        if t.is_empty() || t.contains(',') {
            return None;
        }
        t.parse::<f64>().ok().filter(|v| v.is_finite())
    }

    /// A length in metres.
    pub fn length(&self, s: &str) -> Option<f64> {
        let s: String = s
            .trim()
            .to_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if s.is_empty() {
            return None;
        }
        // Feet and inches: 2'6" or 2'6 or 2'.
        if let Some((ft, rest)) = s.split_once('\'') {
            let feet = self.number(ft)?;
            let rest = rest.strip_suffix('"').unwrap_or(rest);
            let inches = if rest.is_empty() {
                0.0
            } else {
                self.number(rest)?
            };
            if inches < 0.0 {
                return None;
            }
            let sign = if feet < 0.0 || ft.starts_with('-') {
                -1.0
            } else {
                1.0
            };
            return Some(sign * (feet.abs() * 12.0 + inches) * 0.0254);
        }
        let units: [(&str, f64); 6] = [
            ("mm", 0.001),
            ("cm", 0.01),
            ("in", 0.0254),
            ("ft", 0.3048),
            ("m", 1.0),
            ("\"", 0.0254),
        ];
        for (u, k) in units {
            if let Some(n) = s.strip_suffix(u) {
                return self.number(n).map(|v| v * k);
            }
        }
        self.number(&s)
    }

    /// One or two lengths ("2", "2,1.5"). A second value left empty means
    /// the same as the first.
    pub fn lengths(&self, s: &str) -> Option<(f64, Option<f64>)> {
        let mut parts = s.split(|c| self.is_separator(c));
        let a = self.length(parts.next()?)?;
        let b = match parts.next() {
            None => None,
            Some(t) if t.trim().is_empty() => Some(a),
            Some(t) => Some(self.length(t)?),
        };
        if parts.next().is_some() {
            return None;
        }
        Some((a, b))
    }

    /// A length as the measurement box shows it: three decimals, metres.
    pub fn show(&self, metres: f64) -> String {
        let t = format!("{metres:.3} m");
        if self.decimal == ',' {
            t.replace('.', ",")
        } else {
            t
        }
    }

    /// Two lengths ("2.000, 1.500").
    pub fn show_pair(&self, a: f64, b: f64) -> String {
        let sep = if self.decimal == ',' { "; " } else { ", " };
        let (x, y) = (self.show(a), self.show(b));
        format!("{}{sep}{}", x.trim_end_matches(" m"), y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOT: NumberFormat = NumberFormat { decimal: '.' };
    const COMMA: NumberFormat = NumberFormat { decimal: ',' };

    fn close(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-12)
    }

    #[test]
    fn every_unit() {
        assert!(close(DOT.length("2.5"), 2.5));
        assert!(close(DOT.length("2.5m"), 2.5));
        assert!(close(DOT.length("40cm"), 0.4));
        assert!(close(DOT.length("750mm"), 0.75));
        assert!(close(DOT.length("12in"), 0.3048));
        assert!(close(DOT.length("12\""), 0.3048));
        assert!(close(DOT.length("3ft"), 0.9144));
        assert!(close(DOT.length("2'6\""), 0.762));
        assert!(close(DOT.length("2'6"), 0.762));
        assert!(close(DOT.length("2'"), 0.6096));
        assert!(close(DOT.length(" 1.5 M "), 1.5));
        assert!(close(DOT.length("-2"), -2.0));
        assert!(close(DOT.length(".5"), 0.5));
    }

    #[test]
    fn bad_input_is_rejected() {
        for s in ["", "abc", "2x", "1.2.3", "m", "2'x", "--1", "1,5"] {
            assert_eq!(DOT.length(s), None, "{s:?}");
        }
    }

    #[test]
    fn separators_follow_the_decimal_mark() {
        assert_eq!(DOT.lengths("2,1.5"), Some((2.0, Some(1.5))));
        assert_eq!(DOT.lengths("2;1.5"), Some((2.0, Some(1.5))));
        assert_eq!(DOT.lengths("2"), Some((2.0, None)));
        assert_eq!(DOT.lengths("2,"), Some((2.0, Some(2.0))));
        assert_eq!(COMMA.lengths("2,5;1,5"), Some((2.5, Some(1.5))));
        assert_eq!(COMMA.length("2,5"), Some(2.5));
        assert_eq!(DOT.lengths("1,2,3"), None);
        assert!(close(
            DOT.lengths("300mm,20cm").map(|p| p.1.unwrap_or(0.0)),
            0.2
        ));
    }

    #[test]
    fn shown_with_three_decimals() {
        assert_eq!(DOT.show(1.23456), "1.235 m");
        assert_eq!(COMMA.show(1.5), "1,500 m");
        assert_eq!(DOT.show_pair(2.0, 1.5), "2.000, 1.500 m");
    }
}
