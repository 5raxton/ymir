use std::time::Duration;

/// Cap on the settle time of a spring in seconds. Guards against degenerate
/// spring configs (`stiffness == 0`, `damping_ratio == 0`, or pathological
/// `epsilon` values) that would otherwise animate forever or produce a NaN in
/// `Duration::from_secs_f64` (which panics).
const MAX_DURATION_S: f64 = 30.;

#[derive(Debug, Clone, Copy)]
pub struct SpringParams {
    pub damping: f64,
    pub mass: f64,
    pub stiffness: f64,
    pub epsilon: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Spring {
    pub from: f64,
    pub to: f64,
    pub initial_velocity: f64,
    pub params: SpringParams,
}

impl SpringParams {
    pub fn new(damping_ratio: f64, stiffness: f64, epsilon: f64) -> Self {
        // Reject NaN/Inf outright (the config parser accepts arbitrary f64s, and a NaN
        // `damping_ratio` would poison `critical_damping` and produce a NaN `damping`,
        // which then panics `duration()`'s `Duration::from_secs_f64`).
        let damping_ratio = if damping_ratio.is_finite() { damping_ratio.max(0.) } else { 0. };
        let stiffness = if stiffness.is_finite() { stiffness.max(0.) } else { 0. };
        let epsilon = if epsilon.is_finite() { epsilon.max(0.) } else { 0. };

        // epsilon must be positive for `ln()` in `duration()` to be meaningful.
        let epsilon = if epsilon > 0. { epsilon } else { 0.0001 };

        let mass = 1.;
        let critical_damping = 2. * (mass * stiffness).sqrt();
        let damping = damping_ratio * critical_damping;

        Self {
            damping,
            mass,
            stiffness,
            epsilon,
        }
    }
}

impl Spring {
    pub fn value_at(&self, t: Duration) -> f64 {
        self.oscillate(t.as_secs_f64())
    }

    // Based on libadwaita (LGPL-2.1-or-later):
    // https://gitlab.gnome.org/GNOME/libadwaita/-/blob/1.4.4/src/adw-spring-animation.c,
    // which itself is based on (MIT):
    // https://github.com/robb/RBBAnimation/blob/master/RBBAnimation/RBBSpringAnimation.m
    /// Computes and returns the duration until the spring is at rest.
    pub fn duration(&self) -> Duration {
        const DELTA: f64 = 0.001;
        // Cap the settle time so that degenerate configs (e.g. `stiffness == 0` or
        // `damping_ratio == 0`, both currently accepted by the config parser) can never
        // produce a duration-before-start `is_done()` that keeps an animation or the
        // compositor's redraw loop running forever. `Duration::MAX` used here previously
        // made `is_done()` unreachable, so any spring built with those params would
        // animate forever. Cap at a generous upper bound (~30 s) instead.

        let beta = self.params.damping / (2. * self.params.mass);

        if !beta.is_finite() || beta <= 0. || !self.params.epsilon.is_finite() || self.params.epsilon <= 0.
        {
            // Degenerate/unstable spring: settle immediately rather than animate forever
            // or crash.
            return Duration::ZERO;
        }

        if (self.to - self.from).abs() <= f64::EPSILON {
            return Duration::ZERO;
        }

        if !self.to.is_finite() || !self.from.is_finite() || !self.initial_velocity.is_finite() {
            return Duration::ZERO;
        }

        // First ansatz for all damping regimes: the time at which the decay envelope
        // (with the initial displacement as the amplitude) drops to `epsilon`. For the
        // underdamped and critically damped solutions this is only an estimate, because
        // their amplitudes also depend on the initial velocity and the polynomial factor
        // of the critically damped case, so the ansatz alone can undershoot the real
        // settling time. It still makes a good seed for the Newton refinement below.
        let mut x0 = (-self.params.epsilon.ln() / beta).clamp(0., MAX_DURATION_S);

        // Newton's root finding for the value crossing `to ± epsilon`.
        // https://en.wikipedia.org/wiki/Newton%27s_method
        let mut y0 = self.oscillate(x0);
        let m = (self.oscillate(x0 + DELTA) - y0) / DELTA;

        let mut x1 = if m != 0. && m.is_finite() {
            (self.to - y0 + m * x0) / m
        } else {
            x0
        };
        let mut y1 = self.oscillate(x1);

        let mut i = 0;
        while (self.to - y1).abs() > self.params.epsilon {
            if i > 1000 {
                // Failed to converge; snap to the cap so `is_done()` eventually fires.
                return Self::secs_capped(x0);
            }

            x0 = x1;
            y0 = y1;

            let m = (self.oscillate(x0 + DELTA) - y0) / DELTA;

            x1 = if m != 0. && m.is_finite() {
                (self.to - y0 + m * x0) / m
            } else {
                x0
            };
            y1 = self.oscillate(x1);

            // Some springs have numerical stability issues...
            if !y1.is_finite() {
                return Self::secs_capped(x0);
            }

            i += 1;

            // x1 can diverge to NaN/negative/Inf even while y1 stays finite (e.g. for
            // underdamped springs with certain ratios). Guard so we never feed an invalid
            // value into `Duration::from_secs_f64`, which panics.
            if !x1.is_finite() || x1 < 0. || x1 > MAX_DURATION_S {
                return Self::secs_capped(x0);
            }
        }

        Self::secs_capped(x1)
    }

    /// Safely converts a seconds value into a `Duration` without panicking on
    /// NaN/negative/Inf inputs.
    fn secs_capped(secs: f64) -> Duration {
        if secs.is_finite() && secs >= 0. {
            Duration::from_secs_f64(secs.min(MAX_DURATION_S))
        } else {
            Duration::ZERO
        }
    }

    /// Computes and returns the duration until the spring reaches its target position.
    pub fn clamped_duration(&self) -> Option<Duration> {
        let beta = self.params.damping / (2. * self.params.mass);

        if !beta.is_finite() || beta <= 0. || !self.params.epsilon.is_finite() || self.params.epsilon <= 0.
        {
            // Degenerate/unstable spring: settle immediately rather than animate forever.
            return Some(Duration::ZERO);
        }

        if (self.to - self.from).abs() <= f64::EPSILON {
            return Some(Duration::ZERO);
        }

        if !self.to.is_finite() || !self.from.is_finite() || !self.initial_velocity.is_finite() {
            return Some(Duration::ZERO);
        }

        // The first frame is not that important and we avoid finding the trivial 0 for in-place
        // animations.
        let mut i = 1u16;
        let mut y = self.oscillate(f64::from(i) / 1000.);

        while (self.to - self.from > f64::EPSILON && self.to - y > self.params.epsilon)
            || (self.from - self.to > f64::EPSILON && y - self.to > self.params.epsilon)
        {
            if i > 3000 {
                return None;
            }

            i += 1;
            y = self.oscillate(f64::from(i) / 1000.);
        }

        Some(Duration::from_millis(u64::from(i)))
    }

    /// Returns the spring position at a given time in seconds.
    fn oscillate(&self, t: f64) -> f64 {
        let b = self.params.damping;
        let m = self.params.mass;
        let k = self.params.stiffness;
        let v0 = self.initial_velocity;

        let beta = b / (2. * m);
        let omega0 = (k / m).sqrt();

        let x0 = self.from - self.to;

        let envelope = (-beta * t).exp();

        // Solutions of the form C1*e^(lambda1*x) + C2*e^(lambda2*x)
        // for the differential equation m*ẍ+b*ẋ+kx = 0

        // f64::EPSILON is too small for this specific comparison, so we use
        // f32::EPSILON even though it's doubles.
        if (beta - omega0).abs() <= f64::from(f32::EPSILON) {
            // Critically damped.
            self.to + envelope * (x0 + (beta * x0 + v0) * t)
        } else if beta < omega0 {
            // Underdamped.
            let omega1 = ((omega0 * omega0) - (beta * beta)).sqrt();

            self.to
                + envelope
                    * (x0 * (omega1 * t).cos() + ((beta * x0 + v0) / omega1) * (omega1 * t).sin())
        } else {
            // Overdamped.
            let omega2 = ((beta * beta) - (omega0 * omega0)).sqrt();

            self.to
                + envelope
                    * (x0 * (omega2 * t).cosh() + ((beta * x0 + v0) / omega2) * (omega2 * t).sinh())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overdamped_spring_equal_from_to_nan() {
        let spring = Spring {
            from: 0.,
            to: 0.,
            initial_velocity: 0.,
            params: SpringParams::new(1.15, 850., 0.0001),
        };
        let _ = spring.duration();
        let _ = spring.clamped_duration();
        let _ = spring.value_at(Duration::ZERO);
    }

    #[test]
    fn overdamped_spring_duration_panic() {
        let spring = Spring {
            from: 0.,
            to: 1.,
            initial_velocity: 0.,
            params: SpringParams::new(6., 1200., 0.0001),
        };
        let _ = spring.duration();
        let _ = spring.clamped_duration();
        let _ = spring.value_at(Duration::ZERO);
    }

    #[test]
    fn duration_settles_to_value_for_underdamped_and_critical_ratios() {
        // `duration()` must return a time where the spring has actually settled within
        // `epsilon` of its target, not the loose decay-envelope estimate that used to
        // leave a visible residual (and later hard snap) at the end of scrolls.
        for ratio in [0.3, 0.6, 0.8, 1.0] {
            let epsilon = 0.0001;
            let spring = Spring {
                from: 0.,
                to: 1.,
                initial_velocity: 0.,
                params: SpringParams::new(ratio, 850., epsilon),
            };
            let duration = spring.duration();
            let residual = (spring.value_at(duration) - spring.to).abs();
            assert!(
                residual <= epsilon,
                "ratio {ratio}: residual {residual} exceeds epsilon {epsilon} at {duration:?}"
            );

            // Past the computed duration the value must stay settled.
            let later = spring.value_at(duration + Duration::from_secs_f64(0.5));
            assert!(
                (later - spring.to).abs() <= epsilon,
                "ratio {ratio}: drifted at +0.5s"
            );
        }
    }
}
