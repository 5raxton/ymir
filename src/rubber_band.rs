#[derive(Debug, Clone, Copy)]
pub struct RubberBand {
    pub stiffness: f64,
    pub limit: f64,
}

impl RubberBand {
    pub fn band(&self, x: f64) -> f64 {
        let c = self.stiffness;
        let d = self.limit;

        (1. - (1. / (x * c / d + 1.))) * d
    }

    pub fn derivative(&self, x: f64) -> f64 {
        let c = self.stiffness;
        let d = self.limit;

        c * d * d / (c * x + d).powi(2)
    }

    pub fn clamp(&self, min: f64, max: f64, x: f64) -> f64 {
        // `f64::clamp` panics on NaN input; the scroll/pointer deltas feeding this can in
        // principle produce a NaN, so bail out with a finite fallback instead of crashing the
        // compositor.
        if !x.is_finite() || !min.is_finite() || !max.is_finite() {
            return if x < min { min } else { max };
        }

        let clamped = x.clamp(min, max);
        let sign = if x < clamped { -1. } else { 1. };
        let diff = (x - clamped).abs();

        clamped + sign * self.band(diff)
    }

    pub fn clamp_derivative(&self, min: f64, max: f64, x: f64) -> f64 {
        if min <= x && x <= max {
            return 1.;
        }

        if !x.is_finite() || !min.is_finite() || !max.is_finite() {
            return 0.;
        }

        let clamped = x.clamp(min, max);
        let diff = (x - clamped).abs();
        self.derivative(diff)
    }
}
