use super::{Body, MAX_DEPTH, Object, Symbol, Value};
use crate::{
    cursor::{Cursor, TryFromCursor},
    deserializer::{ErrorKind, MarshalDeserializeError},
    types::{
        fixnum::{FixNum, FixNumLen},
        float::RbFloat,
        string::RbStr,
        type_byte::MarshalTypeByte,
    },
    version_number::VersionNumber,
};
use num_bigint::BigInt;

// Each `match` arm below is a single call on purpose. Debug builds give every arm's temporaries
// their own stack slot, so fat arms on the recursive path (value -> object -> body -> value)
// multiply the stack used per nesting level.

type Result<T> = std::result::Result<T, MarshalDeserializeError>;

pub(super) fn from_bytes(bytes: &[u8]) -> Result<Value> {
    let mut reader = Reader {
        cursor: Cursor::new(bytes),
        symbols: Vec::new(),
        objects: Vec::new(),
        depth: 0,
    };
    let version: VersionNumber = reader.take()?;
    if !version.can_read() {
        return Err(ErrorKind::VersionNumber(version).into());
    }
    reader.value()
}

struct Reader<'a> {
    cursor: Cursor<'a>,
    symbols: Vec<Symbol>,
    /// The object table `@` links index into. `None` while that object is still being read.
    objects: Vec<Option<Value>>,
    depth: usize,
}

impl<'a> Reader<'a> {
    fn take<V>(&mut self) -> Result<V>
    where
        V: TryFromCursor<'a>,
        ErrorKind: From<V::Error>,
    {
        match self.cursor.try_take::<V>() {
            Some(Ok(v)) => Ok(v),
            Some(Err(e)) => Err(ErrorKind::from(e).into()),
            None => Err(ErrorKind::UnexpectedEof.into()),
        }
    }

    fn len(&mut self) -> Result<usize> {
        Ok(self.take::<FixNumLen>()?.inner())
    }

    fn bytes(&mut self) -> Result<Vec<u8>> {
        Ok(self.take::<&RbStr>()?.as_slice().to_vec())
    }

    fn value(&mut self) -> Result<Value> {
        if self.depth == MAX_DEPTH {
            return Err(ErrorKind::TooDeep.into());
        }
        self.depth += 1;
        let ty = self.take()?;
        let value = match ty {
            MarshalTypeByte::Nil => Ok(Value::Nil),
            MarshalTypeByte::True => Ok(Value::Bool(true)),
            MarshalTypeByte::False => Ok(Value::Bool(false)),
            MarshalTypeByte::Fixnum => self.fixnum(),
            MarshalTypeByte::Float => self.float(),
            MarshalTypeByte::Bignum => self.bignum(),
            MarshalTypeByte::Symbol | MarshalTypeByte::SymbolLink => {
                self.symbol_body(ty).map(Value::Symbol)
            }
            MarshalTypeByte::ObjectReference => self.link(),
            MarshalTypeByte::Instance if self.cursor.peek() == Some(b':') => self.encoded_symbol(),
            ty => self.object(ty),
        };
        self.depth -= 1;
        value
    }

    fn fixnum(&mut self) -> Result<Value> {
        Ok(Value::Int(self.take::<FixNum>()?.inner().into()))
    }

    // Numbers take up an object table slot even though they aren't `Ref`s.

    fn float(&mut self) -> Result<Value> {
        let value = Value::Float(self.take::<RbFloat>()?.inner());
        self.objects.push(Some(value.clone()));
        Ok(value)
    }

    fn bignum(&mut self) -> Result<Value> {
        let big: BigInt = self.take()?;
        let value = match i64::try_from(&big) {
            Ok(n) => Value::Int(n),
            Err(_) => Object::new(Body::BigInt(big)).into(),
        };
        self.objects.push(Some(value.clone()));
        Ok(value)
    }

    fn link(&mut self) -> Result<Value> {
        let index = self.take::<FixNum>()?.inner();
        match usize::try_from(index)
            .ok()
            .and_then(|i| self.objects.get(i))
        {
            Some(Some(value)) => Ok(value.clone()),
            Some(None) => Err(ErrorKind::CyclicValue(index).into()),
            None => Err(ErrorKind::InvalidObjectRef(index).into()),
        }
    }

    /// The part of a symbol after its type byte.
    fn symbol_body(&mut self, ty: MarshalTypeByte) -> Result<Symbol> {
        match ty {
            MarshalTypeByte::Symbol => {
                let name = std::str::from_utf8(self.take::<&RbStr>()?.as_slice())
                    .map_err(ErrorKind::from)?;
                let symbol = Symbol::from(name);
                self.symbols.push(symbol.clone());
                Ok(symbol)
            }
            MarshalTypeByte::SymbolLink => {
                let index = self.take::<FixNum>()?.inner();
                usize::try_from(index)
                    .ok()
                    .and_then(|i| self.symbols.get(i))
                    .cloned()
                    .ok_or_else(|| ErrorKind::InvalidSymbolLink(index).into())
            }
            other => Err(ErrorKind::ExpectedSymbol(other).into()),
        }
    }

    fn symbol(&mut self) -> Result<Symbol> {
        let ty = self.take()?;
        self.symbol_body(ty)
    }

    /// Non-ASCII symbols carry an encoding ivar (`I:`). Symbols are UTF-8 here, so it is read and
    /// dropped, and the writer puts it back.
    fn encoded_symbol(&mut self) -> Result<Value> {
        self.cursor.take_1();
        let symbol = self.symbol_body(MarshalTypeByte::Symbol)?;
        self.pairs()?;
        Ok(Value::Symbol(symbol))
    }

    fn pairs(&mut self) -> Result<Vec<(Symbol, Value)>> {
        // Plain loops rather than `collect`, which costs several stack frames per nesting level
        // in debug builds. No capacity is reserved because lengths come from untrusted input.
        let mut pairs = Vec::new();
        for _ in 0..self.len()? {
            pairs.push((self.symbol()?, self.value()?));
        }
        Ok(pairs)
    }

    /// A heap object, starting after its type byte. Ruby writes its wrappers in a fixed order:
    /// `I`, then any `e`s, then `C`, then the object itself, then the `I` ivars.
    fn object(&mut self, mut ty: MarshalTypeByte) -> Result<Value> {
        let mut object = Object::new(Body::Array(Vec::new()));
        let wrapped_ivars = matches!(ty, MarshalTypeByte::Instance);
        if wrapped_ivars {
            ty = self.take()?;
        }
        while let MarshalTypeByte::Extended = ty {
            object.extended.push(self.symbol()?);
            ty = self.take()?;
        }
        if let MarshalTypeByte::UserString = ty {
            object.user_class = Some(self.symbol()?);
            ty = self.take()?;
        }
        // Ruby claims an object's link index before reading its contents, so anything nested
        // gets a later index. `_load` objects are the exception: Ruby can only create them, and
        // claim their index, after their data and ivars have been read.
        let slot = (!matches!(ty, MarshalTypeByte::UserDefined)).then(|| self.reserve());
        object.body = self.body(ty, &mut object.ivars)?;
        if wrapped_ivars {
            let ivars = self.pairs()?;
            object.ivars.extend(ivars);
        }
        let slot = slot.unwrap_or_else(|| self.reserve());
        let value = Value::from(object);
        self.objects[slot] = Some(value.clone());
        Ok(value)
    }

    fn reserve(&mut self) -> usize {
        self.objects.push(None);
        self.objects.len() - 1
    }

    /// A plain object's fields are its ivars, so they are read into `ivars`.
    fn body(&mut self, ty: MarshalTypeByte, ivars: &mut Vec<(Symbol, Value)>) -> Result<Body> {
        match ty {
            MarshalTypeByte::Object => self.plain_object(ivars),
            MarshalTypeByte::String => self.bytes().map(Body::String),
            MarshalTypeByte::RegularExpression => self.regex(),
            MarshalTypeByte::Array => self.array(),
            MarshalTypeByte::Hash | MarshalTypeByte::HashDefault => {
                self.hash(matches!(ty, MarshalTypeByte::HashDefault))
            }
            MarshalTypeByte::Struct => self.ruby_struct(),
            MarshalTypeByte::Class => self.bytes().map(Body::Class),
            MarshalTypeByte::Module => self.bytes().map(Body::Module),
            MarshalTypeByte::ClassOrModule => self.bytes().map(Body::ClassOrModule),
            MarshalTypeByte::UserDefined => self.user_defined(),
            MarshalTypeByte::UserMarshal | MarshalTypeByte::Data => self.classed_value(ty),
            other => Err(ErrorKind::TypeMismatch {
                expected: "object",
                got: other.type_name(),
            }
            .into()),
        }
    }

    fn plain_object(&mut self, ivars: &mut Vec<(Symbol, Value)>) -> Result<Body> {
        let class = self.symbol()?;
        *ivars = self.pairs()?;
        Ok(Body::Object { class })
    }

    fn regex(&mut self) -> Result<Body> {
        Ok(Body::Regex {
            source: self.bytes()?,
            flags: self.take()?,
        })
    }

    fn array(&mut self) -> Result<Body> {
        let mut items = Vec::new();
        for _ in 0..self.len()? {
            items.push(self.value()?);
        }
        Ok(Body::Array(items))
    }

    fn hash(&mut self, has_default: bool) -> Result<Body> {
        let mut pairs = Vec::new();
        for _ in 0..self.len()? {
            pairs.push((self.value()?, self.value()?));
        }
        let default = if has_default {
            Some(self.value()?)
        } else {
            None
        };
        Ok(Body::Hash { pairs, default })
    }

    fn ruby_struct(&mut self) -> Result<Body> {
        Ok(Body::Struct {
            class: self.symbol()?,
            members: self.pairs()?,
        })
    }

    fn user_defined(&mut self) -> Result<Body> {
        Ok(Body::UserDefined {
            class: self.symbol()?,
            data: self.bytes()?,
        })
    }

    fn classed_value(&mut self, ty: MarshalTypeByte) -> Result<Body> {
        let class = self.symbol()?;
        let data = self.value()?;
        Ok(match ty {
            MarshalTypeByte::UserMarshal => Body::UserMarshal { class, data },
            _ => Body::Data { class, data },
        })
    }
}
