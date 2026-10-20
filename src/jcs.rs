//! RFC 8785 JSON Canonicalization Scheme: the text a version 1.0 envelope's
//! integrity hash covers (spec/draft-kumar-fid-00.md, Integrity).
//!
//! Object members sorted by their keys' UTF-16 code units, no whitespace,
//! strings escaped as ECMAScript's JSON.stringify does, and numbers written
//! as ECMAScript's Number.prototype.toString writes the binary64 value.

use serde_json::Value;

/// The canonical text of a JSON value. Numbers are taken as binary64; the
/// content of an envelope is I-JSON, so that loses nothing.
pub fn canonical(value: &Value) -> String {
    let mut out = String::new();
    write(value, &mut out);
    out
}

fn write(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&number(n.as_f64().unwrap_or(f64::NAN))),
        Value::String(s) => string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, v) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(v, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<(&String, Vec<u16>)> = map.keys().map(|k| (k, k.encode_utf16().collect())).collect();
            keys.sort_by(|a, b| a.1.cmp(&b.1));
            out.push('{');
            for (i, (k, _)) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                string(k, out);
                out.push(':');
                write(&map[k.as_str()], out);
            }
            out.push('}');
        }
    }
}

/// A string as JSON.stringify writes it: `"`, `\` and control characters
/// escaped (the short forms where JSON has them), everything else as is.
fn string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A binary64 number as ECMAScript writes it (ECMA-262, Number::toString;
/// RFC 8785, 3.2.2.3). NaN and infinities have no JSON form.
pub fn number(x: f64) -> String {
    if !x.is_finite() {
        return "null".to_string(); // unreachable for parsed JSON
    }
    if x == 0.0 {
        return "0".to_string(); // also -0
    }
    let sign = if x < 0.0 { "-" } else { "" };
    // Rust's {:e} is the shortest round-trip digits: "d.ddde±x"
    let sci = format!("{:e}", x.abs());
    let (mantissa, exp) = sci.split_once('e').expect("{:e} has an exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let k = digits.len() as i32;
    // n: where the decimal point falls, counted from the first digit
    let n = exp.parse::<i32>().expect("integer exponent") + 1;
    if k <= n && n <= 21 {
        format!("{}{}{}", sign, digits, "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}{}.{}", sign, &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("{}0.{}{}", sign, "0".repeat((-n) as usize), digits)
    } else {
        let e = n - 1;
        let rest = if k > 1 { format!(".{}", &digits[1..]) } else { String::new() };
        format!("{}{}{}e{}{}", sign, &digits[..1], rest, if e > 0 { "+" } else { "-" }, e.abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: integrity
    #[test]
    fn test_rfc_8785_example() {
        // RFC 8785, section 3.2.2
        let v: Value = serde_json::from_str(
            r#"{"numbers": [333333333.33333329, 1E30, 4.50, 2e-3, 0.000000000000000000000000001],
                "string": "\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/",
                "literals": [null, true, false]}"#,
        )
        .unwrap();
        assert_eq!(
            canonical(&v),
            r#"{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\\\"/"}"#
        );
    }

    // spec: integrity
    #[test]
    fn test_numbers_as_ecmascript_writes_them() {
        for (x, s) in [
            (1.0, "1"),
            (100.0, "100"),
            (1e20, "100000000000000000000"),
            (1e21, "1e+21"),
            (1.5e21, "1.5e+21"),
            (1e-6, "0.000001"),
            (1e-7, "1e-7"),
            (-2.5e-7, "-2.5e-7"),
            (0.1, "0.1"),
            (123.456, "123.456"),
            (5e-324, "5e-324"),
            (-0.0, "0"),
        ] {
            assert_eq!(number(x), s, "{}", x);
        }
    }

    // spec: integrity
    #[test]
    fn test_keys_sort_by_utf16_code_units() {
        // U+1F600 is D83D DE00 in UTF-16, before U+E000; by code point it would come after
        let v: Value = serde_json::from_str(r#"{"": 1, "😀": 2, "a": 3}"#).unwrap();
        assert_eq!(canonical(&v), "{\"a\":3,\"\u{1F600}\":2,\"\u{E000}\":1}");
    }
}
