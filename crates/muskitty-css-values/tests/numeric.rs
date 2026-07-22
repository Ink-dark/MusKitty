//! 数值类型测试 — CSS Values Level 4 §4-§6。

#![cfg(test)]

use muskitty_css_values::numeric::{
    Angle, AngleUnit, Frequency, FrequencyUnit, Integer, Length, LengthUnit, Number, Percentage,
    Ratio, Resolution, ResolutionUnit, Time, TimeUnit,
};

// ── Length ──────────────────────────────────────────────────────────

#[test]
fn parse_px_length() {
    let len = Length::parse("10px").unwrap();
    assert_eq!(len.value, 10.0);
    assert_eq!(len.unit, LengthUnit::Px);
}

#[test]
fn parse_em_length() {
    let len = Length::parse("1.5em").unwrap();
    assert_eq!(len.value, 1.5);
    assert_eq!(len.unit, LengthUnit::Em);
}

#[test]
fn parse_negative_length() {
    // CSS Syntax tokenizer 接受负数 dimension（`-` 是数字的一部分）
    let len = Length::parse("-3px").unwrap();
    assert_eq!(len.value, -3.0);
    assert_eq!(len.unit, LengthUnit::Px);
}

#[test]
fn parse_absolute_length_units() {
    for (s, unit) in [
        ("1cm", LengthUnit::Cm),
        ("1mm", LengthUnit::Mm),
        ("1in", LengthUnit::In),
        ("1pt", LengthUnit::Pt),
        ("1pc", LengthUnit::Pc),
        ("1Q", LengthUnit::Q),
    ] {
        let len = Length::parse(s).unwrap();
        assert_eq!(len.unit, unit, "failed for input {s:?}");
    }
}

#[test]
fn parse_relative_length_units() {
    for (s, unit) in [
        ("1em", LengthUnit::Em),
        ("1rem", LengthUnit::Rem),
        ("1ex", LengthUnit::Ex),
        ("1ch", LengthUnit::Ch),
        ("1vw", LengthUnit::Vw),
        ("1vh", LengthUnit::Vh),
        ("1vmin", LengthUnit::Vmin),
        ("1vmax", LengthUnit::Vmax),
    ] {
        let len = Length::parse(s).unwrap();
        assert_eq!(len.unit, unit, "failed for input {s:?}");
    }
}

#[test]
fn parse_length_case_insensitive_unit() {
    // CSS units are ASCII case-insensitive
    let len = Length::parse("10PX").unwrap();
    assert_eq!(len.unit, LengthUnit::Px);
    let len = Length::parse("10Em").unwrap();
    assert_eq!(len.unit, LengthUnit::Em);
}

#[test]
fn reject_unitless_number_as_length() {
    assert!(Length::parse("10").is_err());
}

#[test]
fn reject_unknown_unit() {
    assert!(Length::parse("10foo").is_err());
}

#[test]
fn parse_length_with_whitespace() {
    let len = Length::parse("  10px  ").unwrap();
    assert_eq!(len.value, 10.0);
    assert_eq!(len.unit, LengthUnit::Px);
}

#[test]
fn reject_multiple_values_as_length() {
    assert!(Length::parse("10px 20px").is_err());
}

// ── Percentage ──────────────────────────────────────────────────────

#[test]
fn parse_percentage() {
    let p = Percentage::parse("50%").unwrap();
    assert_eq!(p.value, 50.0);
}

#[test]
fn parse_negative_percentage() {
    let p = Percentage::parse("-12.5%").unwrap();
    assert_eq!(p.value, -12.5);
}

#[test]
fn reject_non_percentage() {
    assert!(Percentage::parse("50").is_err());
    assert!(Percentage::parse("50px").is_err());
}

// ── Number ──────────────────────────────────────────────────────────

#[test]
fn parse_number() {
    let n = Number::parse("2.5").unwrap();
    assert_eq!(n.value, 2.5);
}

#[test]
fn parse_integer_as_number() {
    // Integers are valid numbers
    let n = Number::parse("42").unwrap();
    assert_eq!(n.value, 42.0);
}

#[test]
fn parse_negative_number() {
    let n = Number::parse("-0.5").unwrap();
    assert_eq!(n.value, -0.5);
}

#[test]
fn reject_dimension_as_number() {
    assert!(Number::parse("10px").is_err());
}

// ── Integer ─────────────────────────────────────────────────────────

#[test]
fn parse_integer() {
    let i = Integer::parse("42").unwrap();
    assert_eq!(i.value, 42);
}

#[test]
fn parse_negative_integer() {
    let i = Integer::parse("-7").unwrap();
    assert_eq!(i.value, -7);
}

#[test]
fn reject_fractional_integer() {
    assert!(Integer::parse("3.14").is_err());
}

// ── Angle ───────────────────────────────────────────────────────────

#[test]
fn parse_angle_units() {
    for (s, unit) in [
        ("45deg", AngleUnit::Deg),
        ("100grad", AngleUnit::Grad),
        ("1.5rad", AngleUnit::Rad),
        ("0.25turn", AngleUnit::Turn),
    ] {
        let a = Angle::parse(s).unwrap();
        assert_eq!(a.unit, unit, "failed for input {s:?}");
    }
}

#[test]
fn parse_angle_bare_zero() {
    // §6.1: bare 0 is a valid angle (0deg)
    let a = Angle::parse("0").unwrap();
    assert_eq!(a.value, 0.0);
    assert_eq!(a.unit, AngleUnit::Deg);
}

#[test]
fn reject_nonzero_bare_number_as_angle() {
    assert!(Angle::parse("45").is_err());
}

// ── Time ────────────────────────────────────────────────────────────

#[test]
fn parse_time_units() {
    let s = Time::parse("2s").unwrap();
    assert_eq!(s.value, 2.0);
    assert_eq!(s.unit, TimeUnit::S);

    let ms = Time::parse("250ms").unwrap();
    assert_eq!(ms.value, 250.0);
    assert_eq!(ms.unit, TimeUnit::Ms);
}

#[test]
fn reject_bare_number_as_time() {
    assert!(Time::parse("2").is_err());
}

// ── Frequency ───────────────────────────────────────────────────────

#[test]
fn parse_frequency_units() {
    let hz = Frequency::parse("440Hz").unwrap();
    assert_eq!(hz.value, 440.0);
    assert_eq!(hz.unit, FrequencyUnit::Hz);

    let khz = Frequency::parse("1kHz").unwrap();
    assert_eq!(khz.value, 1.0);
    assert_eq!(khz.unit, FrequencyUnit::KHz);
}

// ── Resolution ──────────────────────────────────────────────────────

#[test]
fn parse_resolution_units() {
    for (s, unit) in [
        ("96dpi", ResolutionUnit::Dpi),
        ("38dpcm", ResolutionUnit::Dpcm),
        ("1dppx", ResolutionUnit::Dppx),
    ] {
        let r = Resolution::parse(s).unwrap();
        assert_eq!(r.unit, unit, "failed for input {s:?}");
    }
}

// ── Ratio ───────────────────────────────────────────────────────────

#[test]
fn parse_ratio_single_number() {
    let r = Ratio::parse("16").unwrap();
    assert_eq!(r.width, 16.0);
    assert_eq!(r.height, 1.0);
}

#[test]
fn parse_ratio_two_numbers() {
    let r = Ratio::parse("16 / 9").unwrap();
    assert_eq!(r.width, 16.0);
    assert_eq!(r.height, 9.0);
}

#[test]
fn parse_ratio_no_spaces_around_slash() {
    let r = Ratio::parse("16/9").unwrap();
    assert_eq!(r.width, 16.0);
    assert_eq!(r.height, 9.0);
}

#[test]
fn reject_negative_ratio() {
    assert!(Ratio::parse("-1").is_err());
    assert!(Ratio::parse("16 / -9").is_err());
}

#[test]
fn reject_non_number_as_ratio() {
    assert!(Ratio::parse("10px").is_err());
}

#[test]
fn reject_empty_ratio() {
    assert!(Ratio::parse("").is_err());
}
