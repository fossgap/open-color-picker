use crate::i18n::{tr, Text};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Color(pub u8, pub u8, pub u8);

impl Default for Color {
    fn default() -> Self {
        Self(53, 132, 228)
    }
}

impl Color {
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }
    pub fn rgb(self) -> String {
        format!("rgb({}, {}, {})", self.0, self.1, self.2)
    }
    pub fn normalized(self) -> (f64, f64, f64) {
        (
            f64::from(self.0) / 255.0,
            f64::from(self.1) / 255.0,
            f64::from(self.2) / 255.0,
        )
    }
    pub fn from_normalized(r: f64, g: f64, b: f64) -> Self {
        Self(
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8,
        )
    }
    pub fn hsl(self) -> String {
        let (r, g, b) = self.normalized();
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let l = (max + min) / 2.0;
        let (h, s) = if d == 0.0 {
            (0.0, 0.0)
        } else {
            let h = if max == r {
                ((g - b) / d).rem_euclid(6.0)
            } else if max == g {
                (b - r) / d + 2.0
            } else {
                (r - g) / d + 4.0
            };
            (h * 60.0, d / (1.0 - (2.0 * l - 1.0).abs()))
        };
        format!(
            "hsl({}, {}%, {}%)",
            decimal(h),
            decimal(s * 100.0),
            decimal(l * 100.0)
        )
    }
    pub fn foreground(self) -> (f64, f64, f64) {
        let linear = |v: u8| {
            let c = f64::from(v) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let l = 0.2126 * linear(self.0) + 0.7152 * linear(self.1) + 0.0722 * linear(self.2);
        if l > 0.179 {
            (0.0, 0.0, 0.0)
        } else {
            (1.0, 1.0, 1.0)
        }
    }
}

fn decimal(v: f64) -> String {
    format!("{v:.1}").trim_end_matches(".0").to_owned()
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

impl FromStr for Color {
    type Err = String;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let text = input.trim();
        let hex = text.strip_prefix('#').unwrap_or(text);
        if matches!(hex.len(), 3 | 6) && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            let full = if hex.len() == 3 {
                hex.chars().flat_map(|c| [c, c]).collect::<String>()
            } else {
                hex.to_owned()
            };
            let number = u32::from_str_radix(&full, 16).map_err(|e| e.to_string())?;
            return Ok(Self(
                (number >> 16) as u8,
                (number >> 8) as u8,
                number as u8,
            ));
        }
        let lower = text.to_ascii_lowercase();
        for kind in ["rgb", "hsl"] {
            if let Some(body) = lower
                .strip_prefix(&format!("{kind}("))
                .and_then(|s| s.strip_suffix(')'))
            {
                let parts: Vec<_> = body.split(',').map(str::trim).collect();
                if parts.len() != 3 {
                    break;
                }
                if kind == "rgb" {
                    if parts.iter().all(|p| p.ends_with('%')) {
                        let values: Result<Vec<f64>, _> = parts
                            .iter()
                            .map(|p| p.trim_end_matches('%').parse::<f64>())
                            .collect();
                        if let Ok(v) = values {
                            if v.iter().all(|v| (0.0..=100.0).contains(v)) {
                                return Ok(Self::from_normalized(
                                    v[0] / 100.0,
                                    v[1] / 100.0,
                                    v[2] / 100.0,
                                ));
                            }
                        }
                    } else if let (Ok(r), Ok(g), Ok(b)) = (
                        parts[0].parse::<u8>(),
                        parts[1].parse::<u8>(),
                        parts[2].parse::<u8>(),
                    ) {
                        return Ok(Self(r, g, b));
                    }
                } else if parts[1].ends_with('%') && parts[2].ends_with('%') {
                    let values = (
                        parts[0].trim_end_matches("deg").parse::<f64>(),
                        parts[1][..parts[1].len() - 1].parse::<f64>(),
                        parts[2][..parts[2].len() - 1].parse::<f64>(),
                    );
                    if let (Ok(h), Ok(s), Ok(l)) = values {
                        if h.is_finite() && (0.0..=100.0).contains(&s) && (0.0..=100.0).contains(&l)
                        {
                            let (h, s, l) = (h.rem_euclid(360.0) / 60.0, s / 100.0, l / 100.0);
                            let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
                            let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
                            let (r, g, b) = match h as u8 {
                                0 => (c, x, 0.0),
                                1 => (x, c, 0.0),
                                2 => (0.0, c, x),
                                3 => (0.0, x, c),
                                4 => (x, 0.0, c),
                                _ => (c, 0.0, x),
                            };
                            let m = l - c / 2.0;
                            return Ok(Self::from_normalized(r + m, g + m, b + m));
                        }
                    }
                }
            }
        }
        Err(tr(Text::ColorFormatError).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_formats_and_hue_wrapping() {
        for value in [
            "#f00",
            "ff0000",
            "rgb(255, 0, 0)",
            "rgb(100%, 0%, 0%)",
            "hsl(360, 100%, 50%)",
            "hsl(-360deg,100%,50%)",
        ] {
            assert_eq!(value.parse::<Color>().unwrap(), Color(255, 0, 0), "{value}");
        }
        assert_eq!(" #aBcDeF ".parse::<Color>().unwrap().hex(), "#ABCDEF");
    }
    #[test]
    fn rejects_invalid_values_without_panicking() {
        for value in [
            "",
            "#12345",
            "💚",
            "rgb(256,0,0)",
            "rgb(-1,0,0)",
            "rgb(NaN,0,0)",
            "hsl(inf,100%,50%)",
            "hsl(0,101%,50%)",
            "hsl(0,1,1)",
            "rgba(1,2,3,0)",
        ] {
            assert!(value.parse::<Color>().is_err(), "{value}");
        }
    }
    #[test]
    fn hsl_round_trips_the_color_cube() {
        for r in (0..=255).step_by(17) {
            for g in (0..=255).step_by(17) {
                for b in (0..=255).step_by(17) {
                    let color = Color(r, g, b);
                    assert_eq!(
                        color.hsl().parse::<Color>().unwrap(),
                        color,
                        "{}",
                        color.hsl()
                    );
                }
            }
        }
    }
    #[test]
    fn grayscale_and_contrast() {
        assert_eq!(Color(0, 0, 0).hsl(), "hsl(0, 0%, 0%)");
        assert_eq!(Color(255, 255, 255).hsl(), "hsl(0, 0%, 100%)");
        assert_eq!(Color(255, 255, 255).foreground(), (0.0, 0.0, 0.0));
        assert_eq!(Color(0, 0, 0).foreground(), (1.0, 1.0, 1.0));
    }
}
