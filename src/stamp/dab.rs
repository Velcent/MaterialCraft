//! [`DabSample`]: one stamp as the brush engine made it, in its own flat frame (screen units for 3D-view strokes,
//! texels for 2D-view strokes): where it sits, how far it reaches, and its coverage on the frame's pixel grid.

/// A dab's coverage on a pixel grid: `width × height` values, row by row, for pixels `x0 .. x0 + width`,
/// `y0 .. y0 + height`. The grid is the frame's (`scale` 1: pixel `(x, y)` has its centre at `(x + 0.5, y + 0.5)`)
/// or a coarser one for very large dabs (pixel centres at `((x + 0.5) · scale, (y + 0.5) · scale)`), so a dab a
/// close-up view makes thousands of units wide is not drawn at full resolution. Values already include the dab's
/// flow.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Footprint {
    pub x0: i32,
    pub y0: i32,
    pub width: usize,
    pub height: usize,
    pub values: Vec<f32>,
    /// Frame units per grid pixel (1, or more for very large dabs).
    scale: f32,
}

impl Footprint {
    /// A footprint with `values` (`width × height` of them) on the frame's grid; `None` if the count does not
    /// match.
    pub fn new(x0: i32, y0: i32, width: usize, height: usize, values: Vec<f32>) -> Option<Footprint> {
        (width.checked_mul(height)? == values.len()).then_some(Footprint { x0, y0, width, height, values, scale: 1.0 })
    }

    /// The footprint on a grid of `scale` frame units per pixel (`x0`, `y0` count grid pixels); scales that are not
    /// finite and positive keep the frame's grid.
    pub fn with_scale(self, scale: f32) -> Footprint {
        Footprint { scale: if scale.is_finite() && scale > 0.0 { scale } else { 1.0 }, ..self }
    }

    /// Frame units per grid pixel.
    pub fn scale(&self) -> f32 {
        if self.scale > 0.0 { self.scale } else { 1.0 }
    }

    /// The frame area the footprint covers: `[x_min, y_min, x_max, y_max]`.
    pub fn bounds(&self) -> [f32; 4] {
        let s = self.scale();
        let (x, y) = (self.x0 as f32, self.y0 as f32);
        [x * s, y * s, (x + self.width as f32) * s, (y + self.height as f32) * s]
    }

    fn at(&self, x: i64, y: i64) -> f32 {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return 0.0;
        }
        self.values.get(y as usize * self.width + x as usize).copied().unwrap_or(0.0)
    }

    /// Coverage at frame position `p`, interpolated bilinearly between pixel centres (0 outside).
    #[inline]
    pub fn sample(&self, p: [f32; 2]) -> f32 {
        let s = self.scale();
        let fx = p[0] / s - self.x0 as f32 - 0.5;
        let fy = p[1] / s - self.y0 as f32 - 0.5;
        if !(fx > -1.0 && fy > -1.0 && fx < self.width as f32 && fy < self.height as f32) {
            return 0.0;
        }
        let (x, y) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x, fy - y);
        let (x, y) = (x as i64, y as i64);
        let top = self.at(x, y) * (1.0 - tx) + self.at(x + 1, y) * tx;
        let bottom = self.at(x, y + 1) * (1.0 - tx) + self.at(x + 1, y + 1) * tx;
        top * (1.0 - ty) + bottom * ty
    }
}

/// One stamp of a stroke.
#[derive(Debug, Clone, PartialEq)]
pub struct DabSample {
    /// Centre in the frame.
    pub center: [f32; 2],
    /// Nominal radius in frame units (the footprint may reach a little further for soft tips).
    pub radius: f32,
    /// The stroke coverage this dab builds up to (the stroke's opacity ceiling for this dab), 0 … 1.
    pub opacity: f32,
    /// Position in the stroke (0 = first).
    pub index: u64,
    pub footprint: Footprint,
}
