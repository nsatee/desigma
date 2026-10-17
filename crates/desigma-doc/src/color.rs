//! Native OKLCH authoring support.
//!
//! sRGB stays the single render truth: [`Color::from_oklch`] converts to
//! gamma-encoded sRGB on construction and retains the OKLCH source for a
//! lossless round-trip. No renderer, picker, or export code reads `oklch`.

use serde::{Deserialize, Serialize};

use super::Color;

/// OKLCH authoring source retained alongside the sRGB render value.
/// `h` is in degrees, 0..360.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

impl Color {
    /// Builds from OKLCH per CSS Color 4, converting to sRGB for rendering
    /// while retaining the source for round-trip. `a` passes through untouched.
    pub fn from_oklch(l: f32, c: f32, h: f32, a: f32) -> Self {
        let (r, g, b) = oklch_to_srgb(l, c, h);
        Self { r, g, b, a, oklch: Some(Oklch { l, c, h }) }
    }

    /// Whether this color carries an OKLCH authoring source.
    pub fn is_oklch(&self) -> bool {
        self.oklch.is_some()
    }
}

/// OKLCH (D65) -> gamma-encoded sRGB, channels clamped to 0..=1.
/// Matrices from CSS Color 4, sampled at <https://www.w3.org/TR/css-color-4/#color-conversion-code>.
pub(crate) fn oklch_to_srgb(l: f32, c: f32, h_deg: f32) -> (f32, f32, f32) {
    // Oklch -> Oklab.
    let h = h_deg.to_radians();
    let a = c * h.cos();
    let b = c * h.sin();
    // Oklab -> nonlinear LMS -> LMS (cubed) -> linear sRGB.
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;
    let (l, m, s) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
    let g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
    let bl = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;
    (lin_to_srgb(r), lin_to_srgb(g), lin_to_srgb(bl))
}

fn lin_to_srgb(u: f32) -> f32 {
    // Negatives take the linear branch, then clamp to zero below.
    let v = if u <= 0.0031308 { 12.92 * u } else { 1.055 * u.powf(1.0 / 2.4) - 0.055 };
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srgb(l: f32, c: f32, h: f32) -> (f32, f32, f32) {
        let col = Color::from_oklch(l, c, h, 1.0);
        assert!(col.is_oklch());
        (col.r, col.g, col.b)
    }

    #[test]
    fn white() {
        let (r, g, b) = srgb(1.0, 0.0, 0.0);
        assert!((r - 1.0).abs() < 1e-3 && (g - 1.0).abs() < 1e-3 && (b - 1.0).abs() < 1e-3);
    }

    #[test]
    fn black() {
        assert_eq!(srgb(0.0, 0.0, 0.0), (0.0, 0.0, 0.0));
    }

    #[test]
    fn red() {
        // Rounded sRGB-red inputs land back on ~sRGB red: (1.0, 0.0, ~0.0125).
        let (r, g, b) = srgb(0.628, 0.258, 29.0);
        assert!(r > 0.99, "r={r}");
        assert!(g < 0.02, "g={g}");
        assert!(b < 0.03, "b={b}");
    }

    #[test]
    fn gamut_clamp() {
        // Far outside sRGB gamut: every channel still lands in 0..=1.
        for (l, c, h) in [(1.5, 0.5, 20.0), (-0.5, 0.4, 200.0), (0.7, 0.5, 340.0)] {
            let (r, g, b) = srgb(l, c, h);
            assert!((0.0..=1.0).contains(&r) && (0.0..=1.0).contains(&g) && (0.0..=1.0).contains(&b));
        }
    }

    #[test]
    fn alpha_passthrough() {
        assert_eq!(Color::from_oklch(0.628, 0.258, 29.0, 0.37).a, 0.37);
    }

    #[test]
    fn srgb_json_byte_identical() {
        let raw = r#"{"r":1.0,"g":0.5,"b":0.25,"a":1.0}"#;
        let c: Color = serde_json::from_str(raw).unwrap();
        assert!(!c.is_oklch());
        assert_eq!(serde_json::to_string(&c).unwrap(), raw);
    }

    #[test]
    fn oklch_json_roundtrip() {
        let c = Color::from_oklch(0.628, 0.258, 29.0, 0.8);
        let s = serde_json::to_string(&c).unwrap();
        assert!(s.contains("\"oklch\""), "{s}");
        let back: Color = serde_json::from_str(&s).unwrap();
        assert_eq!(back, c);
        assert!(back.is_oklch());
    }
}
