//! Lengths typed into the measurement box, and how they are shown.
//!
//! Units as in the feel test (`parseLen(`): mm, cm, m (the default), in or
//! `"`, ft or `'`, and feet and inches such as `2'6"`. Values between them
//! are separated as DECISIONS #5 says: `,` where the decimal mark is `.`,
//! `;` where it is `,`, and `;` always.

use prism_math::{DisplayUnits, LengthUnit};

/// How numbers are written where the user is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberFormat {
    /// `.` or `,`.
    pub decimal: char,
    /// The unit lengths are shown in (File > Document Settings).
    pub units: DisplayUnits,
}

impl Default for NumberFormat {
    fn default() -> Self {
        Self {
            decimal: '.',
            units: DisplayUnits::default(),
        }
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

    /// A number, or a whole number and a fraction ("6 1/2", "1/2").
    fn with_fraction(&self, s: &str) -> Option<f64> {
        let s = s.trim();
        let Some((left, den)) = s.split_once('/') else {
            return self.number(&s.replace(' ', ""));
        };
        let den = self.number(den.trim())?;
        let (whole, num) = match left.trim().rsplit_once(' ') {
            Some((w, n)) => (self.number(w.trim())?, self.number(n.trim())?),
            None => (0.0, self.number(left.trim())?),
        };
        if den <= 0.0 || num < 0.0 || whole < 0.0 {
            return None;
        }
        Some(whole + num / den)
    }

    /// A length in metres.
    pub fn length(&self, s: &str) -> Option<f64> {
        // Inches with a fraction keep their space: 2' 6 1/2" or 6 1/2".
        let low = s.trim().to_lowercase();
        if low.contains('/') {
            let (ft, inch) = match low.split_once('\'') {
                Some((f, i)) => (Some(f.trim()), i),
                None => (None, low.as_str()),
            };
            let inch = inch.trim();
            let inch = inch
                .strip_suffix('"')
                .or_else(|| inch.strip_suffix("in"))
                .unwrap_or(inch);
            let inches = self.with_fraction(inch)?;
            let (sign, feet) = match ft {
                Some(f) => {
                    let v = self.number(f)?;
                    (if f.starts_with('-') { -1.0 } else { 1.0 }, v.abs())
                }
                None => (1.0, 0.0),
            };
            return Some(sign * (feet * 12.0 + inches) * 0.0254);
        }
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

    /// A length, or a sum, difference, product or quotient of them with
    /// brackets: `2*25.4mm`, `(1m+20cm)/2`. A plain number may multiply or
    /// divide a length; two lengths may not be multiplied.
    pub fn expression(&self, s: &str) -> Option<f64> {
        if let Some(v) = self.length(s) {
            return Some(v);
        }
        let mut tokens: Vec<String> = Vec::new();
        let mut word = String::new();
        let flush = |word: &mut String, tokens: &mut Vec<String>| {
            if !word.trim().is_empty() {
                tokens.push(word.trim().to_owned());
            }
            word.clear();
        };
        for c in s.chars() {
            let operand_before = !word.trim().is_empty()
                || tokens
                    .last()
                    .is_some_and(|t| t == ")" || !"+-*/(".contains(t.as_str()));
            match c {
                '+' | '*' | '/' | '(' | ')' => {
                    flush(&mut word, &mut tokens);
                    tokens.push(c.to_string());
                }
                '-' if operand_before => {
                    flush(&mut word, &mut tokens);
                    tokens.push("-".into());
                }
                _ => word.push(c),
            }
        }
        flush(&mut word, &mut tokens);
        let mut at = 0;
        let (v, _) = self.sum(&tokens, &mut at)?;
        (at == tokens.len() && v.is_finite()).then_some(v)
    }

    /// An operand's value, and whether it carried a unit.
    fn operand(&self, t: &str) -> Option<(f64, bool)> {
        let unit = t
            .chars()
            .any(|c| c.is_alphabetic() || c == '"' || c == '\'');
        Some((self.length(t)?, unit))
    }

    fn sum(&self, t: &[String], at: &mut usize) -> Option<(f64, bool)> {
        let (mut v, mut unit) = self.product(t, at)?;
        while let Some(op) = t.get(*at).filter(|o| *o == "+" || *o == "-") {
            *at += 1;
            let (w, u) = self.product(t, at)?;
            v = if op == "+" { v + w } else { v - w };
            unit |= u;
        }
        Some((v, unit))
    }

    fn product(&self, t: &[String], at: &mut usize) -> Option<(f64, bool)> {
        let (mut v, mut unit) = self.atom(t, at)?;
        while let Some(op) = t.get(*at).filter(|o| *o == "*" || *o == "/") {
            *at += 1;
            let (w, u) = self.atom(t, at)?;
            if op == "*" {
                if unit && u {
                    return None;
                }
                v *= w;
                unit |= u;
            } else {
                if w == 0.0 || (!unit && u) {
                    return None;
                }
                v /= w;
                unit = unit && !u;
            }
        }
        Some((v, unit))
    }

    fn atom(&self, t: &[String], at: &mut usize) -> Option<(f64, bool)> {
        let tok = t.get(*at)?;
        *at += 1;
        if tok == "(" {
            let r = self.sum(t, at)?;
            if t.get(*at)? != ")" {
                return None;
            }
            *at += 1;
            return Some(r);
        }
        if "+-*/)".contains(tok.as_str()) {
            return None;
        }
        self.operand(tok)
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

    fn fixed(&self, v: f64) -> String {
        let d = usize::from(self.units.decimals.min(prism_math::units::MAX_DECIMALS));
        let mut t = format!("{v:.d$}");
        // No "-0.000".
        if t.starts_with('-') && t[1..].chars().all(|c| c == '0' || c == '.') {
            t.remove(0);
        }
        if self.decimal == ',' {
            t = t.replace('.', ",");
        }
        t
    }

    /// A length in the document's unit, with or without the unit's mark.
    fn show_in(&self, metres: f64, mark: bool) -> String {
        let u = self.units.length;
        let v = metres / u.metres();
        match u {
            LengthUnit::FeetInches => {
                let d = i32::from(self.units.decimals.min(prism_math::units::MAX_DECIMALS));
                let step = 10f64.powi(-d);
                // Round the inches first, so 11.9999" shows as the next foot.
                let total = (v.abs() / step).round() * step;
                let feet = (total / 12.0 + 1e-9).floor();
                let inches = (total - feet * 12.0).max(0.0);
                let sign = if v < 0.0 && total > 0.0 { "-" } else { "" };
                format!("{sign}{}' {}\"", feet as i64, self.fixed(inches))
            }
            _ => {
                let t = self.fixed(v);
                if !mark {
                    return t;
                }
                let m = match u {
                    LengthUnit::Millimetres => " mm",
                    LengthUnit::Centimetres => " cm",
                    LengthUnit::Metres => " m",
                    _ => "\"",
                };
                format!("{t}{m}")
            }
        }
    }

    /// A length as the measurement box shows it, in the document's unit.
    pub fn show(&self, metres: f64) -> String {
        self.show_in(metres, true)
    }

    /// Two lengths ("2.000, 1.500 m").
    pub fn show_pair(&self, a: f64, b: f64) -> String {
        let sep = if self.decimal == ',' { "; " } else { ", " };
        format!("{}{sep}{}", self.show_in(a, false), self.show(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOT: NumberFormat = NumberFormat {
        decimal: '.',
        units: DisplayUnits {
            length: LengthUnit::Metres,
            decimals: 3,
        },
    };
    const COMMA: NumberFormat = NumberFormat {
        decimal: ',',
        ..DOT
    };

    fn shown_in(length: LengthUnit, decimals: u8) -> NumberFormat {
        NumberFormat {
            units: DisplayUnits { length, decimals },
            ..DOT
        }
    }

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
    fn expressions_with_units() {
        assert!(close(DOT.expression("2*25.4mm"), 0.0508));
        assert!(close(DOT.expression("(1m+20cm)/2"), 0.6));
        assert!(close(DOT.expression("1.5m - 50cm"), 1.0));
        assert!(close(DOT.expression("-2*3"), -6.0));
        assert!(close(DOT.expression("2'6\""), 0.762));
        assert!(close(DOT.expression("2*(3+1)"), 8.0));
        assert!(close(DOT.expression("1m/2m*3mm"), 0.0015));
        for bad in ["1m*2m", "2/0", "2/1m", "(1+2", "1+", "*3", "", "abc"] {
            assert_eq!(DOT.expression(bad), None, "{bad:?}");
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

    #[test]
    fn shown_in_each_unit() {
        assert_eq!(shown_in(LengthUnit::Millimetres, 0).show(0.75), "750 mm");
        assert_eq!(shown_in(LengthUnit::Centimetres, 1).show(0.4), "40.0 cm");
        assert_eq!(shown_in(LengthUnit::Metres, 2).show(-0.0001), "0.00 m");
        assert_eq!(shown_in(LengthUnit::Inches, 2).show(0.3048), "12.00\"");
        let fi = shown_in(LengthUnit::FeetInches, 1);
        assert_eq!(fi.show(0.762 + 0.0127), "2' 6.5\"");
        assert_eq!(fi.show(0.3048 * 3.0 - 1e-9), "3' 0.0\"");
        assert_eq!(fi.show(-0.762), "-2' 6.0\"");
        assert_eq!(
            shown_in(LengthUnit::FeetInches, 0).show(0.0254 * 11.6),
            "1' 0\""
        );
        assert_eq!(
            shown_in(LengthUnit::Millimetres, 0).show_pair(2.0, 1.5),
            "2000, 1500 mm"
        );
        let c = NumberFormat {
            decimal: ',',
            ..shown_in(LengthUnit::Centimetres, 1)
        };
        assert_eq!(c.show(0.125), "12,5 cm");
    }

    #[test]
    fn shown_lengths_read_back() {
        for u in LengthUnit::ALL {
            let nf = shown_in(u, 4);
            for v in [0.0, 0.0254, 0.762, 1.5, 12.3456, -2.0] {
                let back = nf.length(&nf.show(v)).expect("reads back");
                assert!((back - v).abs() < 0.0254 * 1e-4 + 1e-9, "{u:?} {v} {back}");
            }
        }
    }

    #[test]
    fn inches_with_fractions() {
        assert!(close(DOT.length("2' 6 1/2\""), 0.7747));
        assert!(close(DOT.length("6 1/2\""), 0.1651));
        assert!(close(DOT.length("1/2in"), 0.0127));
        assert!(close(DOT.length("-1' 1/2"), -(12.5 * 0.0254)));
        assert_eq!(DOT.length("1/0\""), None);
    }
}
