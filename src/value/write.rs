use super::{Body, Object, Symbol, Value};
use num_bigint::Sign;
use std::{collections::HashMap, fmt::Write, sync::Arc};

pub(super) fn to_bytes(value: &Value) -> Vec<u8> {
    let mut writer = Writer::default();
    writer.out.extend([4, 8]);
    writer.value(value);
    writer.out
}

#[derive(Default)]
struct Writer {
    out: Vec<u8>,
    symbols: HashMap<Symbol, usize>,
    /// Objects already written, by address. The value tree keeps every `Arc` alive while we write, so an address can't be reused.
    objects: HashMap<*const Object, usize>,
    /// Ruby links repeated flonums by value.
    flonums: HashMap<u64, usize>,
    next_object: usize,
}

impl Writer {
    fn byte(&mut self, b: u8) {
        self.out.push(b);
    }

    /// Claim the next object table slot.
    fn claim(&mut self) -> usize {
        self.next_object += 1;
        self.next_object - 1
    }

    fn link(&mut self, index: usize) {
        self.byte(b'@');
        self.len(index);
    }

    fn value(&mut self, value: &Value) {
        match value {
            Value::Nil => self.byte(b'0'),
            Value::Bool(true) => self.byte(b'T'),
            Value::Bool(false) => self.byte(b'F'),
            Value::Int(n) if (-(1 << 30)..1 << 30).contains(n) => {
                self.byte(b'i');
                self.long(*n as i32);
            }
            Value::Int(n) => {
                self.claim();
                self.bignum(n.is_negative(), &n.unsigned_abs().to_le_bytes());
            }
            Value::Float(f) => {
                let bits = f.to_bits();
                if is_flonum(*f) {
                    if let Some(&index) = self.flonums.get(&bits) {
                        return self.link(index);
                    }
                    self.flonums.insert(bits, self.next_object);
                }
                self.claim();
                self.byte(b'f');
                self.bytes(float_text(*f).as_bytes());
            }
            Value::Symbol(symbol) => self.symbol(symbol),
            Value::Ref(object) => {
                let address = Arc::as_ptr(object);
                if let Some(&index) = self.objects.get(&address) {
                    return self.link(index);
                }
                // `_dump` objects claim their slot after their data and ivars, everything else
                // before its contents.
                let user_defined = matches!(object.body, Body::UserDefined { .. });
                if !user_defined {
                    let index = self.claim();
                    self.objects.insert(address, index);
                }
                self.object(object);
                if user_defined {
                    let index = self.claim();
                    self.objects.insert(address, index);
                }
            }
        }
    }

    fn object(&mut self, object: &Object) {
        // A plain object's ivars are its body; anything else gets them in an `I` wrapper.
        let wrapped_ivars = !object.ivars.is_empty() && !matches!(object.body, Body::Object { .. });
        if wrapped_ivars {
            self.byte(b'I');
        }
        for module in &object.extended {
            self.byte(b'e');
            self.symbol(module);
        }
        if let Some(class) = &object.user_class {
            self.byte(b'C');
            self.symbol(class);
        }
        match &object.body {
            Body::Object { class } => {
                self.byte(b'o');
                self.symbol(class);
                self.pairs(&object.ivars);
            }
            Body::String(bytes) => {
                self.byte(b'"');
                self.bytes(bytes);
            }
            Body::Regex { source, flags } => {
                self.byte(b'/');
                self.bytes(source);
                self.byte(*flags);
            }
            Body::Array(items) => {
                self.byte(b'[');
                self.len(items.len());
                items.iter().for_each(|item| self.value(item));
            }
            Body::Hash { pairs, default } => {
                self.byte(if default.is_some() { b'}' } else { b'{' });
                self.len(pairs.len());
                for (key, value) in pairs {
                    self.value(key);
                    self.value(value);
                }
                if let Some(default) = default {
                    self.value(default);
                }
            }
            Body::Struct { class, members } => {
                self.byte(b'S');
                self.symbol(class);
                self.pairs(members);
            }
            Body::BigInt(big) => {
                let (sign, magnitude) = big.to_bytes_le();
                self.bignum(sign == Sign::Minus, &magnitude);
            }
            Body::Class(name) => {
                self.byte(b'c');
                self.bytes(name);
            }
            Body::Module(name) => {
                self.byte(b'm');
                self.bytes(name);
            }
            Body::ClassOrModule(name) => {
                self.byte(b'M');
                self.bytes(name);
            }
            Body::UserDefined { class, data } => {
                self.byte(b'u');
                self.symbol(class);
                self.bytes(data);
            }
            Body::UserMarshal { class, data } => {
                self.byte(b'U');
                self.symbol(class);
                self.value(data);
            }
            Body::Data { class, data } => {
                self.byte(b'd');
                self.symbol(class);
                self.value(data);
            }
        }
        if wrapped_ivars {
            self.pairs(&object.ivars);
        }
    }

    fn symbol(&mut self, symbol: &Symbol) {
        if let Some(&index) = self.symbols.get(symbol) {
            self.byte(b';');
            self.len(index);
            return;
        }
        self.symbols.insert(symbol.clone(), self.symbols.len());
        let name = symbol.as_str();
        if name.is_ascii() {
            self.byte(b':');
            self.bytes(name.as_bytes());
        } else {
            self.byte(b'I');
            self.byte(b':');
            self.bytes(name.as_bytes());
            self.len(1);
            self.symbol(&Symbol::from("E"));
            self.byte(b'T');
        }
    }

    fn pairs(&mut self, pairs: &[(Symbol, Value)]) {
        self.len(pairs.len());
        for (key, value) in pairs {
            self.symbol(key);
            self.value(value);
        }
    }

    /// `magnitude` is little-endian.
    fn bignum(&mut self, negative: bool, magnitude: &[u8]) {
        let used = magnitude.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        let shorts = used.div_ceil(2);
        self.byte(b'l');
        self.byte(if negative { b'-' } else { b'+' });
        self.len(shorts);
        self.out.extend(&magnitude[..used]);
        self.out.resize(self.out.len() + shorts * 2 - used, 0);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.len(bytes.len());
        self.out.extend(bytes);
    }

    fn len(&mut self, n: usize) {
        self.long(i32::try_from(n).expect("marshal can't encode lengths above i32::MAX"));
    }

    /// Ruby's `w_long`.
    fn long(&mut self, n: i32) {
        match n {
            0 => self.byte(0),
            1..=122 => self.byte(n as u8 + 5),
            -123..=-1 => self.byte((n - 5) as u8),
            _ => {
                // Little-endian bytes until the rest is all sign extension, prefixed with the
                // count (negated for negative numbers).
                let mut bytes = [0; 4];
                let mut rest = n;
                let mut count = 0;
                for byte in &mut bytes {
                    *byte = rest as u8;
                    rest >>= 8;
                    count += 1;
                    if rest == 0 || rest == -1 {
                        break;
                    }
                }
                self.byte(if n < 0 { (-count) as u8 } else { count as u8 });
                self.out.extend(&bytes[..count as usize]);
            }
        }
    }
}

/// Whether Ruby stores this float as an immediate "flonum" (64-bit builds), which `Marshal.dump`
/// links by value. Other floats are heap objects and are never linked.
fn is_flonum(f: f64) -> bool {
    let bits = f.to_bits();
    let top = (bits >> 60) & 0b111;
    bits == 0 || ((top == 3 || top == 4) && bits != 0x3000_0000_0000_0000)
}

/// Ruby's `w_float`: shortest round-trip digits, in exponent form only when that's shorter.
fn float_text(f: f64) -> String {
    if f.is_nan() {
        return "nan".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf" } else { "-inf" }.into();
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0" } else { "0" }.into();
    }
    let scientific = format!("{:e}", f.abs());
    let (mantissa, exponent) = scientific.split_once('e').expect("{:e} has an exponent");
    let digits = mantissa.replace('.', "");
    let point = exponent
        .parse::<i32>()
        .expect("{:e} exponent is an integer")
        + 1;
    let count = digits.len() as i32;

    let mut out = String::new();
    if f < 0.0 {
        out.push('-');
    }
    if point < -3 || point > count {
        out.push_str(&digits[..1]);
        if count > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        write!(out, "e{}", point - 1).expect("writing to a String can't fail");
    } else if point > 0 {
        let (whole, fraction) = digits.split_at(point as usize);
        out.push_str(whole);
        if !fraction.is_empty() {
            out.push('.');
            out.push_str(fraction);
        }
    } else {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', -point as usize));
        out.push_str(&digits);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_text_matches_ruby() {
        // Expected strings are Ruby 3.4's `Marshal.dump` output for each float.
        let cases = [
            (1.0, "1"),
            (0.1, "0.1"),
            (1e20, "1e20"),
            (1.5e-5, "1.5e-5"),
            (-0.0, "-0"),
            (100.0, "1e2"),
            (1e16, "1e16"),
            (123.456, "123.456"),
            (0.001, "0.001"),
            (0.0001, "0.0001"),
            (-2.5, "-2.5"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
            (f64::NAN, "nan"),
        ];
        for (f, text) in cases {
            assert_eq!(float_text(f), text, "{f}");
        }
    }

    #[test]
    fn long_matches_ruby() {
        let mut writer = Writer::default();
        let cases: [(i32, &[u8]); 10] = [
            (0, &[0x00]),
            (1, &[0x06]),
            (122, &[0x7f]),
            (123, &[0x01, 0x7b]),
            (255, &[0x01, 0xff]),
            (256, &[0x02, 0x00, 0x01]),
            (-1, &[0xfa]),
            (-123, &[0x80]),
            (-124, &[0xff, 0x84]),
            (-257, &[0xfe, 0xff, 0xfe]),
        ];
        for (n, bytes) in cases {
            writer.out.clear();
            writer.long(n);
            assert_eq!(writer.out, bytes, "{n}");
        }
    }
}
