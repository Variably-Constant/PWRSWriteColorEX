//! Sine, cosine, arctangent and powers of two built from +, -, *, / and square roots alone, which
//! IEEE 754 rounds the same everywhere, so PSWriteColorEX's ColorMath, which writes each step in
//! the same order, gets the same bits. A library's sin or pow may differ in the last bit between
//! Rust and .NET, or between platforms, and a gradient would then differ at a rounding edge.

/// pi, pi/2 and pi/4 as doubles.
pub const PI: f64 = f64::from_bits(0x400921FB54442D18);
const HALF_PI: f64 = f64::from_bits(0x3FF921FB54442D18);
const QUARTER_PI: f64 = f64::from_bits(0x3FE921FB54442D18);

/// 2/pi, and pi/2 split into a part whose low bits are zero and the rest, so that k x pi/2 comes
/// off a number without losing the bits beyond a double's pi/2.
const TWO_OVER_PI: f64 = f64::from_bits(0x3FE45F306DC9C883);
const HALF_PI_HIGH: f64 = f64::from_bits(0x3FF921FB54400000);
const HALF_PI_LOW: f64 = f64::from_bits(0x3DD0B4611A626331);

/// The natural logarithm of 2.
const LN2: f64 = f64::from_bits(0x3FE62E42FEFA39EF);

/// 1/n! for n from 0 to 18.
const INVERSE_FACTORIALS: [f64; 19] = [
    f64::from_bits(0x3FF0000000000000),
    f64::from_bits(0x3FF0000000000000),
    f64::from_bits(0x3FE0000000000000),
    f64::from_bits(0x3FC5555555555555),
    f64::from_bits(0x3FA5555555555555),
    f64::from_bits(0x3F81111111111111),
    f64::from_bits(0x3F56C16C16C16C17),
    f64::from_bits(0x3F2A01A01A01A01A),
    f64::from_bits(0x3EFA01A01A01A01A),
    f64::from_bits(0x3EC71DE3A556C734),
    f64::from_bits(0x3E927E4FB7789F5C),
    f64::from_bits(0x3E5AE64567F544E4),
    f64::from_bits(0x3E21EED8EFF8D898),
    f64::from_bits(0x3DE6124613A86D09),
    f64::from_bits(0x3DA93974A8C07C9D),
    f64::from_bits(0x3D6AE7F3E733B81F),
    f64::from_bits(0x3D2AE7F3E733B81F),
    f64::from_bits(0x3CE952C77030AD4A),
    f64::from_bits(0x3CA6827863B97D97),
];

/// The arctangent series' coefficients (-1)^k / (2k + 1) for k from 0 to 13.
const ATAN_SERIES: [f64; 14] = [
    f64::from_bits(0x3FF0000000000000),
    -f64::from_bits(0x3FD5555555555555),
    f64::from_bits(0x3FC999999999999A),
    -f64::from_bits(0x3FC2492492492492),
    f64::from_bits(0x3FBC71C71C71C71C),
    -f64::from_bits(0x3FB745D1745D1746),
    f64::from_bits(0x3FB3B13B13B13B14),
    -f64::from_bits(0x3FB1111111111111),
    f64::from_bits(0x3FAE1E1E1E1E1E1E),
    -f64::from_bits(0x3FAAF286BCA1AF28),
    f64::from_bits(0x3FA8618618618618),
    -f64::from_bits(0x3FA642C8590B2164),
    f64::from_bits(0x3FA47AE147AE147B),
    -f64::from_bits(0x3FA2F684BDA12F68),
];

/// The sine of a number from -pi/4 to pi/4: its Taylor series to the 17th power, in Horner form
/// on the square.
fn sine_near_zero(r: f64) -> f64 {
    let z = r * r;
    let mut sum = INVERSE_FACTORIALS[17];
    let mut n = 15;
    while n >= 3 {
        sum = if n % 4 == 3 { sum * z - INVERSE_FACTORIALS[n] } else { sum * z + INVERSE_FACTORIALS[n] };
        n -= 2;
    }
    r * (sum * z + 1.0)
}

/// The cosine of a number from -pi/4 to pi/4: its Taylor series to the 18th power, in Horner form
/// on the square.
fn cosine_near_zero(r: f64) -> f64 {
    let z = r * r;
    let mut sum = -INVERSE_FACTORIALS[18];
    let mut n = 16;
    while n >= 2 {
        sum = if n % 4 == 2 { sum * z - INVERSE_FACTORIALS[n] } else { sum * z + INVERSE_FACTORIALS[n] };
        n -= 2;
    }
    sum * z + 1.0
}

/// A number as k x pi/2 + r, r from -pi/4 to pi/4: k's last two bits, which pick the quarter turn,
/// and r.
fn quarter_turns(x: f64) -> (i64, f64) {
    let k = (x * TWO_OVER_PI).round_ties_even();
    let r = (x - k * HALF_PI_HIGH) - k * HALF_PI_LOW;
    ((k as i64).rem_euclid(4), r)
}

/// The sine of x in radians.
pub fn sin(x: f64) -> f64 {
    let (quarter, r) = quarter_turns(x);
    match quarter {
        0 => sine_near_zero(r),
        1 => cosine_near_zero(r),
        2 => -sine_near_zero(r),
        _ => -cosine_near_zero(r),
    }
}

/// The cosine of x in radians.
pub fn cos(x: f64) -> f64 {
    let (quarter, r) = quarter_turns(x);
    match quarter {
        0 => cosine_near_zero(r),
        1 => -sine_near_zero(r),
        2 => -cosine_near_zero(r),
        _ => sine_near_zero(r),
    }
}

/// The arctangent of a number from 0 to 1: taken from pi/4 above 1/2, then halved once with
/// atan u = 2 atan(u / (1 + sqrt(1 + u^2))), so the series runs on no more than 0.24.
fn arctangent_to_one(z: f64) -> f64 {
    let (base, u) = if z > 0.5 { (QUARTER_PI, (z - 1.0) / (z + 1.0)) } else { (0.0, z) };
    let half = u / (1.0 + (1.0 + u * u).sqrt());
    let w = half * half;
    let mut sum = ATAN_SERIES[13];
    let mut k = 12;
    loop {
        sum = sum * w + ATAN_SERIES[k];
        if k == 0 {
            break;
        }
        k -= 1;
    }
    base + 2.0 * (half * sum)
}

/// The angle of the point (x, y) in radians, from -pi to pi; 0 for the origin.
pub fn atan2(y: f64, x: f64) -> f64 {
    let ax = x.abs();
    let ay = y.abs();
    if ax == 0.0 && ay == 0.0 {
        return 0.0;
    }
    let mut angle = if ay <= ax { arctangent_to_one(ay / ax) } else { HALF_PI - arctangent_to_one(ax / ay) };
    if x < 0.0 {
        angle = PI - angle;
    }
    if y < 0.0 { -angle } else { angle }
}

/// 2 to the power x: 2^floor(x) by halving or doubling, times e^(f ln 2) for the fraction f by
/// its Taylor series to the 18th power.
pub fn exp2(x: f64) -> f64 {
    let whole = x.floor();
    let w = (x - whole) * LN2;
    let mut sum = INVERSE_FACTORIALS[18];
    let mut k = 17;
    loop {
        sum = sum * w + INVERSE_FACTORIALS[k];
        if k == 0 {
            break;
        }
        k -= 1;
    }
    let mut power = whole as i64;
    while power > 0 {
        sum *= 2.0;
        power -= 1;
    }
    while power < 0 {
        sum *= 0.5;
        power += 1;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The functions agree with the platform's to within a few units in the last place: the
    /// check that they compute what they are named for, while their exact bits are what
    /// PSWriteColorEX must reproduce.
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 4.0 * f64::EPSILON * b.abs().max(1.0)
    }

    #[test]
    fn sine_and_cosine_follow_the_platform() {
        let mut x = -25.0;
        while x <= 25.0 {
            assert!(close(sin(x), x.sin()), "sin {x}: {} against {}", sin(x), x.sin());
            assert!(close(cos(x), x.cos()), "cos {x}: {} against {}", cos(x), x.cos());
            x += 0.0137;
        }
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
    }

    #[test]
    fn arctangent_follows_the_platform_in_every_quarter() {
        for i in -40..=40 {
            for j in -40..=40 {
                let (y, x) = (f64::from(i) * 0.37, f64::from(j) * 0.29);
                if x == 0.0 && y == 0.0 {
                    continue;
                }
                assert!(close(atan2(y, x), y.atan2(x)), "atan2({y}, {x}): {} against {}", atan2(y, x), y.atan2(x));
            }
        }
        assert_eq!(atan2(0.0, 0.0), 0.0);
        assert_eq!(atan2(0.0, 1.0), 0.0);
        assert_eq!(atan2(1.0, 1.0), QUARTER_PI);
    }

    #[test]
    fn powers_of_two_follow_the_platform() {
        let mut x = -20.0;
        while x <= 10.0 {
            assert!(close(exp2(x), x.exp2()), "exp2 {x}: {} against {}", exp2(x), x.exp2());
            x += 0.0173;
        }
        assert_eq!(exp2(0.0), 1.0);
        assert_eq!(exp2(-10.0), 1.0 / 1024.0);
        assert_eq!(exp2(3.0), 8.0);
    }

    /// The bits each function returns, which PSWriteColorEX's ColorMath must return too; its
    /// tests hold the same table.
    #[test]
    fn bits_the_powershell_side_must_match() {
        let sines = [
            (1.0, 0x3FEAED548F090CEE, 0x3FE14A280FB5068C),
            (-2.75, 0xBFD86D2239C183FB, 0xBFED93E294FAED14),
            (0.3, 0x3FD2E9CD95BABA33, 0x3FEE921DD42F09BA),
            (7.0, 0x3FE50608C26D0A09, 0x3FE81FF79ED92017),
            (-22.5, 0x3FDF2DDE019D9BC6, 0xBFEBF21C9294B1C4),
        ];
        for (x, sine, cosine) in sines {
            assert_eq!(sin(x).to_bits(), sine, "sin {x}");
            assert_eq!(cos(x).to_bits(), cosine, "cos {x}");
        }
        let angles = [
            (1.0, 2.0, 0x3FDDAC670561BB4F),
            (-0.4, -0.9, 0xC005C97554F4B39E),
            (3.0, 0.1, 0x3FF8997FBB8B19C0),
            (0.0, -1.0, 0x400921FB54442D18),
        ];
        for (y, x, angle) in angles {
            assert_eq!(atan2(y, x).to_bits(), angle, "atan2 {y} {x}");
        }
        let powers = [(-2.5, 0x3FC6A09E667F3BCC), (-9.999, 0x3F5002D711C79A94), (-0.001, 0x3FEFFA52DE61C11F), (0.0, 0x3FF0000000000000)];
        for (x, power) in powers {
            assert_eq!(exp2(x).to_bits(), power, "exp2 {x}");
        }
    }

    /// Writes each function's bits over a grid of inputs, one line each with the inputs' bits,
    /// for comparing PSWriteColorEX's ColorMath with them after a change to either:
    /// cargo test dump_bits -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_bits() {
        for i in 0..1600 {
            let x = -30.0 + f64::from(i) * 0.0371;
            println!("sin {:016X} {:016X}", x.to_bits(), sin(x).to_bits());
            println!("cos {:016X} {:016X}", x.to_bits(), cos(x).to_bits());
        }
        for i in -30..=30 {
            for j in -30..=30 {
                let (y, x) = (f64::from(i) * 0.413, f64::from(j) * 0.317);
                println!("atan2 {:016X} {:016X} {:016X}", y.to_bits(), x.to_bits(), atan2(y, x).to_bits());
            }
        }
        for i in 0..2200 {
            let x = -20.0 + f64::from(i) * 0.0137;
            println!("exp2 {:016X} {:016X}", x.to_bits(), exp2(x).to_bits());
        }
    }
}
