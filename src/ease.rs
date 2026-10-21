//! The curves -GradientEasing applies to the place along a gradient: Linear, and the In, Out and
//! InOut forms of Sine, Quad, Cubic, Quart, Quint, Expo, Circ, Back, Elastic and Bounce, as
//! easings.net writes them, with arith's sine, cosine and powers of two so that PSWriteColorEX's
//! ColorMath gets the same bits.

use crate::arith;

/// An easing curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Easing {
    #[default]
    Linear,
    InSine,
    OutSine,
    InOutSine,
    InQuad,
    OutQuad,
    InOutQuad,
    InCubic,
    OutCubic,
    InOutCubic,
    InQuart,
    OutQuart,
    InOutQuart,
    InQuint,
    OutQuint,
    InOutQuint,
    InExpo,
    OutExpo,
    InOutExpo,
    InCirc,
    OutCirc,
    InOutCirc,
    InBack,
    OutBack,
    InOutBack,
    InElastic,
    OutElastic,
    InOutElastic,
    InBounce,
    OutBounce,
    InOutBounce,
}

/// Back's overshoot, and the factor InOutBack scales it by: 1.70158 and 1.525.
const BACK: f64 = f64::from_bits(0x3FFB39ABF3387161);
const BACK_IN_OUT_SCALE: f64 = f64::from_bits(0x3FF8666666666666);

/// Bounce's 7.5625 and 2.75, both exact in binary.
const BOUNCE_SCALE: f64 = 7.5625;
const BOUNCE_SPAN: f64 = 2.75;

impl Easing {
    /// Every curve, in the order -GradientEasing lists them.
    pub const ALL: [Easing; 31] = [
        Easing::Linear,
        Easing::InSine,
        Easing::OutSine,
        Easing::InOutSine,
        Easing::InQuad,
        Easing::OutQuad,
        Easing::InOutQuad,
        Easing::InCubic,
        Easing::OutCubic,
        Easing::InOutCubic,
        Easing::InQuart,
        Easing::OutQuart,
        Easing::InOutQuart,
        Easing::InQuint,
        Easing::OutQuint,
        Easing::InOutQuint,
        Easing::InExpo,
        Easing::OutExpo,
        Easing::InOutExpo,
        Easing::InCirc,
        Easing::OutCirc,
        Easing::InOutCirc,
        Easing::InBack,
        Easing::OutBack,
        Easing::InOutBack,
        Easing::InElastic,
        Easing::OutElastic,
        Easing::InOutElastic,
        Easing::InBounce,
        Easing::OutBounce,
        Easing::InOutBounce,
    ];

    /// The name -GradientEasing takes.
    pub fn name(self) -> &'static str {
        const NAMES: [&str; 31] = [
            "Linear", "InSine", "OutSine", "InOutSine", "InQuad", "OutQuad", "InOutQuad", "InCubic", "OutCubic", "InOutCubic", "InQuart",
            "OutQuart", "InOutQuart", "InQuint", "OutQuint", "InOutQuint", "InExpo", "OutExpo", "InOutExpo", "InCirc", "OutCirc", "InOutCirc",
            "InBack", "OutBack", "InOutBack", "InElastic", "OutElastic", "InOutElastic", "InBounce", "OutBounce", "InOutBounce",
        ];
        NAMES[self as usize]
    }

    /// The curve a name means, compared without regard to case.
    pub fn from_name(name: &str) -> Option<Easing> {
        Easing::ALL.into_iter().find(|easing| easing.name().eq_ignore_ascii_case(name))
    }

    /// The curve's value at x from 0 to 1. Back and Elastic overshoot below 0 and above 1.
    pub fn apply(self, x: f64) -> f64 {
        let pi = arith::PI;
        match self {
            Easing::Linear => x,
            Easing::InSine => 1.0 - arith::cos((x * pi) / 2.0),
            Easing::OutSine => arith::sin((x * pi) / 2.0),
            Easing::InOutSine => -(arith::cos(pi * x) - 1.0) / 2.0,
            Easing::InQuad => x * x,
            Easing::OutQuad => {
                let rest = 1.0 - x;
                1.0 - rest * rest
            }
            Easing::InOutQuad => {
                if x < 0.5 {
                    2.0 * x * x
                } else {
                    let rest = -2.0 * x + 2.0;
                    1.0 - rest * rest / 2.0
                }
            }
            Easing::InCubic => x * x * x,
            Easing::OutCubic => {
                let rest = 1.0 - x;
                1.0 - rest * rest * rest
            }
            Easing::InOutCubic => {
                if x < 0.5 {
                    4.0 * x * x * x
                } else {
                    let rest = -2.0 * x + 2.0;
                    1.0 - rest * rest * rest / 2.0
                }
            }
            Easing::InQuart => x * x * x * x,
            Easing::OutQuart => {
                let rest = 1.0 - x;
                1.0 - rest * rest * rest * rest
            }
            Easing::InOutQuart => {
                if x < 0.5 {
                    8.0 * x * x * x * x
                } else {
                    let rest = -2.0 * x + 2.0;
                    1.0 - rest * rest * rest * rest / 2.0
                }
            }
            Easing::InQuint => x * x * x * x * x,
            Easing::OutQuint => {
                let rest = 1.0 - x;
                1.0 - rest * rest * rest * rest * rest
            }
            Easing::InOutQuint => {
                if x < 0.5 {
                    16.0 * x * x * x * x * x
                } else {
                    let rest = -2.0 * x + 2.0;
                    1.0 - rest * rest * rest * rest * rest / 2.0
                }
            }
            Easing::InExpo => {
                if x == 0.0 {
                    0.0
                } else {
                    arith::exp2(10.0 * x - 10.0)
                }
            }
            Easing::OutExpo => {
                if x == 1.0 {
                    1.0
                } else {
                    1.0 - arith::exp2(-10.0 * x)
                }
            }
            Easing::InOutExpo => {
                if x == 0.0 {
                    0.0
                } else if x == 1.0 {
                    1.0
                } else if x < 0.5 {
                    arith::exp2(20.0 * x - 10.0) / 2.0
                } else {
                    (2.0 - arith::exp2(-20.0 * x + 10.0)) / 2.0
                }
            }
            Easing::InCirc => 1.0 - (1.0 - x * x).sqrt(),
            Easing::OutCirc => {
                let rest = x - 1.0;
                (1.0 - rest * rest).sqrt()
            }
            Easing::InOutCirc => {
                if x < 0.5 {
                    let twice = 2.0 * x;
                    (1.0 - (1.0 - twice * twice).sqrt()) / 2.0
                } else {
                    let rest = -2.0 * x + 2.0;
                    ((1.0 - rest * rest).sqrt() + 1.0) / 2.0
                }
            }
            Easing::InBack => {
                let over = BACK + 1.0;
                over * x * x * x - BACK * x * x
            }
            Easing::OutBack => {
                let over = BACK + 1.0;
                let rest = x - 1.0;
                1.0 + over * rest * rest * rest + BACK * rest * rest
            }
            Easing::InOutBack => {
                let scaled = BACK * BACK_IN_OUT_SCALE;
                if x < 0.5 {
                    let twice = 2.0 * x;
                    (twice * twice * ((scaled + 1.0) * 2.0 * x - scaled)) / 2.0
                } else {
                    let rest = 2.0 * x - 2.0;
                    (rest * rest * ((scaled + 1.0) * (x * 2.0 - 2.0) + scaled) + 2.0) / 2.0
                }
            }
            Easing::InElastic => {
                if x == 0.0 {
                    0.0
                } else if x == 1.0 {
                    1.0
                } else {
                    let period = (2.0 * pi) / 3.0;
                    -arith::exp2(10.0 * x - 10.0) * arith::sin((x * 10.0 - 10.75) * period)
                }
            }
            Easing::OutElastic => {
                if x == 0.0 {
                    0.0
                } else if x == 1.0 {
                    1.0
                } else {
                    let period = (2.0 * pi) / 3.0;
                    arith::exp2(-10.0 * x) * arith::sin((x * 10.0 - 0.75) * period) + 1.0
                }
            }
            Easing::InOutElastic => {
                if x == 0.0 {
                    0.0
                } else if x == 1.0 {
                    1.0
                } else {
                    let period = (2.0 * pi) / 4.5;
                    if x < 0.5 {
                        -(arith::exp2(20.0 * x - 10.0) * arith::sin((20.0 * x - 11.125) * period)) / 2.0
                    } else {
                        (arith::exp2(-20.0 * x + 10.0) * arith::sin((20.0 * x - 11.125) * period)) / 2.0 + 1.0
                    }
                }
            }
            Easing::InBounce => 1.0 - out_bounce(1.0 - x),
            Easing::OutBounce => out_bounce(x),
            Easing::InOutBounce => {
                if x < 0.5 {
                    (1.0 - out_bounce(1.0 - 2.0 * x)) / 2.0
                } else {
                    (1.0 + out_bounce(2.0 * x - 1.0)) / 2.0
                }
            }
        }
    }
}

/// easings.net's easeOutBounce: four parabolas, each lower bounce a quarter as high.
fn out_bounce(x: f64) -> f64 {
    if x < 1.0 / BOUNCE_SPAN {
        BOUNCE_SCALE * x * x
    } else if x < 2.0 / BOUNCE_SPAN {
        let rest = x - 1.5 / BOUNCE_SPAN;
        BOUNCE_SCALE * rest * rest + 0.75
    } else if x < 2.5 / BOUNCE_SPAN {
        let rest = x - 2.25 / BOUNCE_SPAN;
        BOUNCE_SCALE * rest * rest + 0.9375
    } else {
        let rest = x - 2.625 / BOUNCE_SPAN;
        BOUNCE_SCALE * rest * rest + 0.984375
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_curve_runs_from_0_to_1() {
        for easing in Easing::ALL {
            assert!(easing.apply(0.0).abs() < 1e-12, "{easing:?} at 0 is {}", easing.apply(0.0));
            assert!((easing.apply(1.0) - 1.0).abs() < 1e-12, "{easing:?} at 1 is {}", easing.apply(1.0));
        }
    }

    /// Values from easings.net's formulas evaluated with the platform's functions.
    #[test]
    fn curves_match_easings_net_at_their_middles() {
        let near = |easing: Easing, x: f64, want: f64| assert!((easing.apply(x) - want).abs() < 1e-12, "{easing:?} at {x}: {} against {want}", easing.apply(x));
        near(Easing::InSine, 0.5, 1.0 - (std::f64::consts::PI / 4.0).cos());
        near(Easing::OutSine, 0.5, (std::f64::consts::PI / 4.0).sin());
        near(Easing::InOutSine, 0.25, -((std::f64::consts::PI * 0.25).cos() - 1.0) / 2.0);
        near(Easing::InQuad, 0.5, 0.25);
        near(Easing::InOutCubic, 0.75, 1.0 - 0.5f64.powi(3) / 2.0);
        near(Easing::OutQuint, 0.5, 1.0 - 0.5f64.powi(5));
        near(Easing::InExpo, 0.5, 2f64.powf(-5.0));
        near(Easing::OutCirc, 0.5, (1.0 - 0.25f64).sqrt());
        near(Easing::InBack, 0.5, 2.70158 * 0.125 - 1.70158 * 0.25);
        near(Easing::OutElastic, 0.5, 2f64.powf(-5.0) * ((5.0 - 0.75) * (2.0 * std::f64::consts::PI) / 3.0).sin() + 1.0);
        near(Easing::OutBounce, 0.5, 7.5625 * (0.5 - 1.5 / 2.75) * (0.5 - 1.5 / 2.75) + 0.75);
        near(Easing::InOutBounce, 0.5, 0.5);
    }

    #[test]
    fn back_and_elastic_overshoot() {
        assert!(Easing::InBack.apply(0.2) < 0.0);
        assert!(Easing::OutBack.apply(0.8) > 1.0);
        assert!(Easing::OutElastic.apply(0.1) > 1.0);
    }

    #[test]
    fn names_compare_without_case_and_cover_every_curve() {
        assert_eq!(Easing::from_name("inoutsine"), Some(Easing::InOutSine));
        assert_eq!(Easing::from_name("OUTBOUNCE"), Some(Easing::OutBounce));
        assert_eq!(Easing::from_name("EaseInSine"), None);
        for easing in Easing::ALL {
            assert_eq!(Easing::from_name(easing.name()), Some(easing));
        }
    }
}
