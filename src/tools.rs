use std::cmp::{max, min};
use std::ops::Range;

use egui::Color32;

pub trait RangeExt {
    fn intersects(&self, other: &Self) -> bool;
    fn union(&self, other: &Self) -> Self;
}

impl RangeExt for Range<usize> {
    // Check if ranges overlap
    fn intersects(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    // Returns the convex hull (the smallest range containing both)
    fn union(&self, other: &Self) -> Self {
        min(self.start, other.start)..max(self.end, other.end)
    }
}

pub fn random_color(num_existing: usize) -> [u8; 3] {
    let c = colorous::WARM.eval_rational(num_existing, 40);
    [c.r, c.g, c.b]
}

pub fn to_color32(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

pub trait ReadableText {
    /// Returns a grayscale color that is readable against `self` as a background.
    fn readable_text_color(&self) -> Color32;
}

impl ReadableText for Color32 {
    fn readable_text_color(&self) -> Color32 {
        // Relative luminance using sRGB coefficients
        let luminance = 0.299 * self.r() as f32 + 0.587 * self.g() as f32 + 0.114 * self.b() as f32;
        if luminance > 150.0 {
            Color32::from_gray(30)
        } else {
            Color32::from_gray(230)
        }
    }
}

pub fn mix_colors(c1: Color32, c2: Color32) -> Color32 {
    Color32::from_rgb(
        ((c1.r() as u16 + c2.r() as u16) / 2) as u8,
        ((c1.g() as u16 + c2.g() as u16) / 2) as u8,
        ((c1.b() as u16 + c2.b() as u16) / 2) as u8,
    )
}
