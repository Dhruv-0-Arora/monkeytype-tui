//! Byte-exact port of npm `object-hash@3.0.0` with default options (sha1, hex,
//! respectType:true, unorderedObjects:true, unorderedArrays:false) for the JSON
//! subset emitted by CompletedEvent: object, array, string, number, bool, null.
//! The monkeytype backend recomputes `objectHash(result minus hash)` and rejects
//! mismatches with 461, so serialization must match byte for byte - parity is
//! proven by tests/objecthash_vectors.rs (vectors generated from the real npm
//! package by scripts/gen_objecthash_vectors.mjs).
//!
//! Grammar notes (from the object-hash source):
//! - `null` -> `Null`; `bool:true`; `number:{js}`; `string:{utf16 len}:{utf8}`.
//! - Arrays: `array:{len}:` then elements in order, no separators.
//! - Plain objects: `object:{keys+3}:` then three respectType meta entries
//!   (`prototype:Undefined`, `__proto__:<Object.prototype>`, `constructor:<fn>`)
//!   followed by the real keys sorted by UTF-16 code units; every entry is
//!   `{key}:{value},`.
//! - A context array records every visited object/array/function; revisits
//!   serialize as the string `[CIRCULAR:{index}]`. For JSON input only two
//!   singletons recur - `Object.prototype` and the `Object` constructor - so
//!   the context is simulated with a slot counter plus their two slots.

use serde_json::Value;
use sha1::{Digest, Sha1};

/// sha1-hex of the object-hash serialization - npm `objectHash(value)`.
pub fn object_hash(value: &Value) -> String {
    let mut hasher = Sha1::new();
    hasher.update(serialize(value).as_bytes());
    hex(&hasher.finalize())
}

/// The exact byte stream object-hash feeds to sha1 (npm `writeToStream`).
/// Exposed for tests and 461 forensics.
pub fn serialize(value: &Value) -> String {
    let mut s = Serializer::default();
    s.dispatch(value);
    s.out
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    out
}

#[derive(Default)]
struct Serializer {
    out: String,
    /// Next index in object-hash's `context` array (every object, array, and
    /// function visit consumes one).
    next_slot: usize,
    /// Context slot of `Object.prototype` once first serialized.
    proto_slot: Option<usize>,
    /// Context slot of the `Object` constructor once first serialized.
    ctor_slot: Option<usize>,
}

impl Serializer {
    fn dispatch(&mut self, value: &Value) {
        match value {
            Value::Null => self.out.push_str("Null"),
            Value::Bool(b) => {
                self.out.push_str("bool:");
                self.out.push_str(if *b { "true" } else { "false" });
            }
            Value::Number(n) => {
                // JS has a single number type; i64/u64/f64 all take the f64 path.
                self.out.push_str("number:");
                let x = n.as_f64().unwrap_or(f64::NAN);
                self.out.push_str(&js_number_string(x));
            }
            Value::String(s) => self.string(s),
            Value::Array(items) => {
                self.next_slot += 1; // context push
                self.out.push_str(&format!("array:{}:", items.len()));
                for item in items {
                    self.dispatch(item);
                }
            }
            Value::Object(map) => self.object(map),
        }
    }

    fn string(&mut self, s: &str) {
        self.out
            .push_str(&format!("string:{}:", s.encode_utf16().count()));
        self.out.push_str(s);
    }

    fn object(&mut self, map: &serde_json::Map<String, Value>) {
        self.next_slot += 1; // context push
        let mut keys: Vec<&String> = map.keys().collect();
        // JS Array.prototype.sort() compares UTF-16 code units.
        keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        // JS object semantics diverge for these key names; CompletedEvent never
        // uses them.
        debug_assert!(
            !map.contains_key("prototype")
                && !map.contains_key("__proto__")
                && !map.contains_key("constructor")
        );

        self.out.push_str(&format!("object:{}:", keys.len() + 3));

        // respectType meta entry 1: obj["prototype"] is undefined
        self.string("prototype");
        self.out.push(':');
        self.out.push_str("Undefined");
        self.out.push(',');

        // meta entry 2: obj["__proto__"] is Object.prototype
        self.string("__proto__");
        self.out.push(':');
        self.object_prototype();
        self.out.push(',');

        // meta entry 3: obj["constructor"] is the Object function
        self.string("constructor");
        self.out.push(':');
        self.object_constructor();
        self.out.push(',');

        for key in keys {
            self.string(key);
            self.out.push(':');
            self.dispatch(&map[key]);
            self.out.push(',');
        }
    }

    /// `Object.prototype` - a plain-ish object whose own `__proto__` is null and
    /// which has no enumerable keys, so it serializes as `object:3:` + meta.
    fn object_prototype(&mut self) {
        if let Some(slot) = self.proto_slot {
            self.string(&format!("[CIRCULAR:{slot}]"));
            return;
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        self.proto_slot = Some(slot);

        self.out.push_str("object:3:");
        self.string("prototype");
        self.out.push(':');
        self.out.push_str("Undefined");
        self.out.push(',');
        self.string("__proto__");
        self.out.push(':');
        self.out.push_str("Null");
        self.out.push(',');
        self.string("constructor");
        self.out.push(':');
        self.object_constructor();
        self.out.push(',');
    }

    /// The native `Object` function. `fn:` + name markers are re-emitted on
    /// every visit; only the trailing properties-object part is deduplicated
    /// through the context (native functions get no respectType meta keys).
    fn object_constructor(&mut self) {
        self.out.push_str("fn:");
        self.string("[native]");
        self.string("function-name:Object");
        if let Some(slot) = self.ctor_slot {
            self.string(&format!("[CIRCULAR:{slot}]"));
            return;
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        self.ctor_slot = Some(slot);
        self.out.push_str("object:0:");
    }
}

/// ECMA-262 `Number::toString(x, 10)` on top of Rust's shortest round-trip
/// float formatting (which is always positional, like JS inside the
/// -6 < n <= 21 window).
pub fn js_number_string(x: f64) -> String {
    if x == 0.0 {
        return "0".to_string(); // covers -0: JS (-0).toString() === "0"
    }
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if x < 0.0 {
        return format!("-{}", js_number_string(-x));
    }

    let s = format!("{x}");
    // Derive (digits, n) where x = 0.d1..dk * 10^n.
    let (digits, n): (String, i32) = match s.split_once('.') {
        None => {
            let trimmed = s.trim_end_matches('0');
            (trimmed.to_string(), s.len() as i32)
        }
        Some(("0", frac)) => {
            let zeros = frac.len() - frac.trim_start_matches('0').len();
            (frac.trim_start_matches('0').to_string(), -(zeros as i32))
        }
        Some((int, frac)) => (format!("{int}{frac}"), int.len() as i32),
    };

    if -6 < n && n <= 21 {
        // JS renders this window positionally, exactly like Rust already did.
        return s;
    }

    // Exponential: d1[.d2..dk]e±(n-1)
    let exp = n - 1;
    let mantissa = if digits.len() == 1 {
        digits
    } else {
        format!("{}.{}", &digits[..1], &digits[1..])
    };
    if exp >= 0 {
        format!("{mantissa}e+{exp}")
    } else {
        format!("{mantissa}e-{}", -exp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Serialization and hash of {a:1}, captured from the real object-hash@3.0.0.
    const A1_SERIALIZED: &str = "object:4:string:9:prototype:Undefined,string:9:__proto__:object:3:string:9:prototype:Undefined,string:9:__proto__:Null,string:11:constructor:fn:string:8:[native]string:20:function-name:Objectobject:0:,,string:11:constructor:fn:string:8:[native]string:20:function-name:Objectstring:12:[CIRCULAR:2],string:1:a:number:1,";

    #[test]
    fn serializes_simple_object_exactly() {
        assert_eq!(serialize(&json!({"a": 1})), A1_SERIALIZED);
    }

    #[test]
    fn hashes_simple_object_exactly() {
        assert_eq!(
            object_hash(&json!({"a": 1})),
            "ca1a41f90da606b052ecf10c8286817813bc8861"
        );
    }

    #[test]
    fn js_number_string_matches_number_tostring() {
        let cases: &[(f64, &str)] = &[
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.0, "-1"),
            (92.5, "92.5"),
            (122.01, "122.01"),
            (0.1 + 0.2, "0.30000000000000004"),
            (0.000001, "0.000001"),
            (1e-7, "1e-7"),
            (1e20, "100000000000000000000"),
            (1e21, "1e+21"),
            (9007199254740992.0, "9007199254740992"),
            (18446744073709551615.0, "18446744073709552000"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
        ];
        for (x, expected) in cases {
            assert_eq!(js_number_string(*x), *expected, "input {x:?}");
        }
    }

    #[test]
    fn utf16_length_for_strings() {
        // "héllo😀" is 7 UTF-16 code units (emoji is a surrogate pair)
        assert_eq!(serialize(&json!("héllo😀")), "string:7:héllo😀");
    }
}
