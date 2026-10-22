//! stdout 契约 §2.1 的编译期浮点格式化。
//!
//! 字节由 Rust 核心 `fmt`（最短往返）生成并在此归一化指数拼写，不经 libc、
//! 不读 `LC_NUMERIC`；同一编译器下跨平台字节一致。特殊值按契约小写输出。

/// f64：普通十进制阈值 `-4 ≤ e < 16`，其余科学记数法。
pub fn format_f64(v: f64) -> String {
    format_float(v, 16)
}

/// f32：普通十进制阈值 `-4 ≤ e < 7`，其余科学记数法。
pub fn format_f32(v: f32) -> String {
    format_float(v, 7)
}

trait Float: Copy + PartialEq {
    fn is_nan(self) -> bool;
    fn is_infinite(self) -> bool;
    fn is_sign_negative(self) -> bool;
    /// Rust 的核心格式化：最短往返，`Display` 为普通十进制，`LowerExp` 为科学记数法。
    fn display(self) -> String;
    fn lower_exp(self) -> String;
}

impl Float for f64 {
    fn is_nan(self) -> bool {
        f64::is_nan(self)
    }
    fn is_infinite(self) -> bool {
        f64::is_infinite(self)
    }
    fn is_sign_negative(self) -> bool {
        f64::is_sign_negative(self)
    }
    fn display(self) -> String {
        format!("{}", self)
    }
    fn lower_exp(self) -> String {
        format!("{:e}", self)
    }
}

impl Float for f32 {
    fn is_nan(self) -> bool {
        f32::is_nan(self)
    }
    fn is_infinite(self) -> bool {
        f32::is_infinite(self)
    }
    fn is_sign_negative(self) -> bool {
        f32::is_sign_negative(self)
    }
    fn display(self) -> String {
        format!("{}", self)
    }
    fn lower_exp(self) -> String {
        format!("{:e}", self)
    }
}

fn format_float<T: Float>(v: T, scientific_at: i32) -> String {
    if v.is_nan() {
        return "nan".to_string();
    }
    if v.is_infinite() {
        return if v.is_sign_negative() {
            "-inf".to_string()
        } else {
            "inf".to_string()
        };
    }
    let sci = v.lower_exp();
    let (mantissa, exp) = split_exp(&sci);
    if (-4..scientific_at).contains(&exp) {
        // 普通十进制：Rust Display 保持同一最短数字串，只改小数点位置。
        v.display()
    } else {
        normalize_sci(mantissa, exp)
    }
}

fn split_exp(s: &str) -> (&str, i32) {
    let (mantissa, exp) = s.split_once('e').expect("LowerExp always contains 'e'");
    let exp = exp.parse::<i32>().expect("LowerExp exponent is an integer");
    (mantissa, exp)
}

/// `d[.ddd]e±XX`：小写 e，符号恒写，指数至少两位。
fn normalize_sci(mantissa: &str, exp: i32) -> String {
    let sign = if exp < 0 { '-' } else { '+' };
    format!("{}e{}{:02}", mantissa, sign, exp.abs())
}

#[cfg(test)]
mod tests {
    use super::{format_f32, format_f64, normalize_sci};

    #[test]
    fn f64_contract_examples() {
        assert_eq!(format_f64(1.0), "1");
        assert_eq!(format_f64(1.5), "1.5");
        assert_eq!(format_f64(-2.25), "-2.25");
        assert_eq!(format_f64(0.1), "0.1");
        assert_eq!(format_f64(1e-4), "0.0001");
        assert_eq!(format_f64(1e-5), "1e-05");
        assert_eq!(format_f64(1e15), "1000000000000000");
        assert_eq!(format_f64(1e16), "1e+16");
        assert_eq!(format_f64(-0.0), "-0");
        assert_eq!(format_f64(f64::NAN), "nan");
        assert_eq!(format_f64(f64::INFINITY), "inf");
        assert_eq!(format_f64(f64::NEG_INFINITY), "-inf");
    }

    #[test]
    fn f32_contract_examples() {
        assert_eq!(format_f32(1.1), "1.1");
        assert_eq!(format_f32(1e6), "1000000");
        assert_eq!(format_f32(1e7), "1e+07");
        assert_eq!(format_f32(1e-5), "1e-05");
        assert_eq!(format_f32(-0.0), "-0");
        assert_eq!(format_f32(f32::NAN), "nan");
        assert_eq!(format_f32(f32::INFINITY), "inf");
        assert_eq!(format_f32(f32::NEG_INFINITY), "-inf");
    }

    #[test]
    fn shortest_round_trip_for_f64_and_f32() {
        assert_eq!(format_f64(0.30000000000000004), "0.30000000000000004");
        assert_eq!(format_f32(0.30000001192092896_f32), "0.3");
        // 末位非零且需要 17 位有效数字的经典值。
        assert_eq!(format_f64(2.2250738585072014e-308), "2.2250738585072014e-308");
        assert_eq!(format_f32(1.1754944e-38), "1.1754944e-38");
    }

    #[test]
    fn exponent_normalization_pads_and_signs() {
        assert_eq!(normalize_sci("1", 7), "1e+07");
        assert_eq!(normalize_sci("1", -5), "1e-05");
        assert_eq!(normalize_sci("1", 0), "1e+00");
        assert_eq!(normalize_sci("-1.5", 324), "-1.5e+324");
    }

    #[test]
    fn subnormal_uses_scientific_form() {
        assert_eq!(format_f64(5e-324), "5e-324");
        assert_eq!(
            format_f64(f64::MIN_POSITIVE / 2.0),
            "1.1125369292536007e-308"
        );
    }
}
