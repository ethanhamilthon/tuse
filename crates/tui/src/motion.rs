use crate::style::Palette;
use crossterm::style::Color;
use std::time::Duration;

/// The gradient that travels along the input while the agent works.
pub struct Motion {
    from: (u8, u8, u8),
    to: (u8, u8, u8),
    period: usize,
    shift: usize,
}

fn rgb(c: Option<Color>) -> Option<(u8, u8, u8)> {
    match c? {
        Color::Rgb { r, g, b } => Some((r, g, b)),
        _ => None,
    }
}

impl Motion {
    /// `None` when idle, when the theme sets `motion: false`, or when the gradient
    /// colors are not `#rrggbb`.
    pub fn new(p: &Palette, busy: bool, elapsed: Duration) -> Option<Motion> {
        let t = &p.t;
        if !busy || !t.motion {
            return None;
        }
        let period = (t.motion_period as usize).max(2);
        let step = (elapsed.as_millis() / u128::from(t.motion_ms.max(1))) as usize;
        Some(Motion {
            from: rgb(p.motion_from)?,
            to: rgb(p.motion_to)?,
            period,
            shift: step % period,
        })
    }

    /// The color at `f` (0..1) of the way along a short edge, such as the bar.
    pub fn along(&self, f: f32) -> Option<Color> {
        self.at((f * self.period as f32) as usize)
    }

    /// The color of cell `i`: a wave that moves toward higher `i`.
    pub fn at(&self, i: usize) -> Option<Color> {
        let pos = (i + self.period - self.shift) % self.period;
        let t = pos as f32 / self.period as f32;
        let k = 1.0 - (2.0 * t - 1.0).abs();
        let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * k).round() as u8;
        Some(Color::Rgb {
            r: mix(self.from.0, self.to.0),
            g: mix(self.from.1, self.to.1),
            b: mix(self.from.2, self.to.2),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_runs_between_the_ends_and_moves() {
        let m = |shift| Motion {
            from: (0, 0, 0),
            to: (200, 100, 50),
            period: 10,
            shift,
        };
        let r = |m: &Motion, i| match m.at(i) {
            Some(Color::Rgb { r, .. }) => r,
            _ => panic!(),
        };
        assert_eq!(r(&m(0), 0), 0);
        assert_eq!(r(&m(0), 5), 200);
        assert_eq!(r(&m(0), 10), 0);
        assert_eq!(r(&m(2), 7), 200);
    }
}
