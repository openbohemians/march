//! Immutable text and independently lazy tuple operations. Storage is bounded,
//! invocation-local, and reclaimed by the existing explicit collector.
use super::*;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Primitive {
    TupleLength = 0,
    Nth = 1,
    Set = 2,
    TextBytes = 3,
    TextChars = 4,
    TextConcat = 5,
    TextSlice = 6,
}
impl Primitive {
    pub fn arity(self) -> usize {
        match self {
            Self::TupleLength | Self::TextBytes | Self::TextChars => 1,
            Self::Nth | Self::TextConcat => 2,
            Self::Set | Self::TextSlice => 3,
        }
    }
}
pub(super) const PRIMITIVES: &[(&str, Primitive)] = &[
    ("tuple-length", Primitive::TupleLength),
    ("nth", Primitive::Nth),
    ("tuple-set", Primitive::Set),
    ("text-bytes", Primitive::TextBytes),
    ("text-chars", Primitive::TextChars),
    ("text-concat", Primitive::TextConcat),
    ("text-slice", Primitive::TextSlice),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TextRef {
    Program(usize),
    Runtime(usize),
}

impl Program {
    pub fn intern_text(&mut self, text: &str) -> Result<usize, Error> {
        if let Some(&id) = self.text_ids.get(text) {
            return Ok(id);
        }
        if self.text_bytes.saturating_add(text.len()) > 32 * 1024 * 1024 {
            return Err(Error::StorageLimit);
        }
        let text: Arc<str> = Arc::from(text);
        let id = self.texts.len();
        self.text_bytes += text.len();
        self.text_ids.insert(text.clone(), id);
        self.texts.push(text);
        Ok(id)
    }
    pub fn text(&self, id: usize) -> Result<&str, Error> {
        self.texts
            .get(id)
            .map(|s| s.as_ref())
            .ok_or(Error::Type("unknown text literal"))
    }
}

/// Decode exactly one conventional quoted string. The caller owns recognition
/// policy and cursor advancement; this operation does not tokenize source.
pub(super) fn read_text(input: &str, start: usize) -> Result<(String, usize), Error> {
    let fail = |s: &str| Error::Compiler(s.into());
    if input.as_bytes().get(start) != Some(&b'"') {
        return Err(fail("expected opening quote"));
    }
    let mut out = String::new();
    let mut chars = input[start + 1..].char_indices();
    while let Some((at, c)) = chars.next() {
        match c {
            '"' => return Ok((out, start + 1 + at + 1)),
            '\\' => out.push(match chars.next().map(|(_, c)| c) {
                Some('"') => '"',
                Some('\\') => '\\',
                Some('n') => '\n',
                Some('r') => '\r',
                Some('t') => '\t',
                Some(_) => return Err(fail("unknown text escape")),
                None => return Err(fail("unterminated text escape")),
            }),
            c => out.push(c),
        }
    }
    Err(fail("unterminated text literal"))
}

impl Executor<'_> {
    pub(super) fn make_tuple(&mut self, fields: Vec<usize>) -> Result<Datum, Error> {
        Ok(match fields.as_slice() {
            [] => Datum::Unit,
            [a, b] => Datum::Pair(*a, *b),
            _ => {
                if self.tuple_fields.saturating_add(fields.len()) > self.tuple_field_limit {
                    return Err(Error::StorageLimit);
                }
                self.tuple_fields += fields.len();
                let id = self.tuples.len();
                self.tuples.push(fields);
                Datum::Tuple(id)
            }
        })
    }
    pub(super) fn is_tuple(&self, v: Datum) -> bool {
        matches!(v, Datum::Unit | Datum::Pair(..) | Datum::Tuple(_))
    }
    pub(super) fn tuple_len(&self, v: Datum) -> Result<usize, Error> {
        match v {
            Datum::Unit => Ok(0),
            Datum::Pair(..) => Ok(2),
            Datum::Tuple(t) => Ok(self.tuples[t].len()),
            _ => Err(Error::Type("expected tuple")),
        }
    }
    pub(super) fn tuple_field(&self, v: Datum, i: usize) -> Result<usize, Error> {
        let n = self.tuple_len(v)?;
        if i >= n {
            return Err(Error::Output(i));
        }
        Ok(match v {
            Datum::Pair(a, b) => {
                if i == 0 {
                    a
                } else {
                    b
                }
            }
            Datum::Tuple(t) => self.tuples[t][i],
            _ => unreachable!(),
        })
    }
    pub(super) fn text_value(&self, t: TextRef) -> Result<&str, Error> {
        match t {
            TextRef::Program(t) => self.program.text(t),
            TextRef::Runtime(t) => Ok(&self.texts[t]),
        }
    }
    fn expect_text(&self, v: Datum) -> Result<&str, Error> {
        match v {
            Datum::Text(t) => self.text_value(t),
            _ => Err(Error::Type("expected text")),
        }
    }
    pub(super) fn store_text(&mut self, text: String) -> Result<Datum, Error> {
        self.reserve_text(text.len())?;
        let id = self.texts.len();
        self.text_bytes += text.len();
        self.texts.push(Arc::from(text));
        Ok(Datum::Text(TextRef::Runtime(id)))
    }
    fn reserve_text(&self, len: usize) -> Result<(), Error> {
        if self.text_bytes.saturating_add(len) > self.text_byte_limit
            || self.texts.len() >= self.cell_limit
        {
            Err(Error::StorageLimit)
        } else {
            Ok(())
        }
    }
    fn index(&self, v: Datum) -> Result<usize, Error> {
        match v {
            Datum::Int(i) => usize::try_from(i).map_err(|_| Error::Type("negative index")),
            _ => Err(Error::Type("index must be integer")),
        }
    }
    pub(super) fn data_charge(&mut self, n: usize) -> Result<(), Error> {
        self.remaining = self.remaining.checked_sub(n).ok_or(Error::Budget)?;
        self.stats.steps += n;
        Ok(())
    }
    pub(super) fn scalar_binary(&mut self, op: Binary, a: Datum, b: Datum) -> Result<Datum, Error> {
        if op == Binary::Eq {
            if matches!(a, Datum::CompilerState(_) | Datum::Frame(_))
                || matches!(b, Datum::CompilerState(_) | Datum::Frame(_))
            {
                return Err(Error::Type("value is not comparable"));
            }
            if std::mem::discriminant(&a) != std::mem::discriminant(&b) {
                return Ok(Datum::Bool(false));
            }
        }
        if let (Datum::Text(a), Datum::Text(b)) = (a, b) {
            self.data_charge(self.text_value(a)?.len().min(self.text_value(b)?.len()))?;
            return match op {
                Binary::Eq => Ok(Datum::Bool(self.text_value(a)? == self.text_value(b)?)),
                Binary::Lt => Ok(Datum::Bool(
                    self.text_value(a)?.as_bytes() < self.text_value(b)?.as_bytes(),
                )),
                _ => Err(Error::Type("binary operand types")),
            };
        }
        binary(op, a, b)
    }
    pub(super) fn equal_values(&mut self, a: Datum, b: Datum) -> Result<(), Error> {
        self.comparison = true;
        if self.is_tuple(a) && self.is_tuple(b) {
            if self.tuple_len(a)? != self.tuple_len(b)? {
                self.comparison = false;
            } else {
                self.work.push(Task::EqualFields(a, b, 0));
            }
        } else {
            let Datum::Bool(equal) = self.scalar_binary(Binary::Eq, a, b)? else {
                unreachable!()
            };
            self.comparison = equal;
        }
        Ok(())
    }
    pub(super) fn equal_fields(&mut self, a: Datum, b: Datum, index: usize) -> Result<(), Error> {
        if !self.comparison || index == self.tuple_len(a)? {
            return Ok(());
        }
        let left = self.tuple_field(a, index)?;
        let right = self.tuple_field(b, index)?;
        // LIFO: both Needs (including nested equalities) finish before EqualValues
        // overwrites comparison for the following EqualFields continuation.
        self.work.push(Task::EqualFields(a, b, index + 1));
        self.work.push(Task::EqualValues(left, right));
        self.work.push(Task::Need(right));
        self.work.push(Task::Need(left));
        Ok(())
    }
    pub(super) fn data_step(&mut self, dst: usize, phase: usize) -> Result<(), Error> {
        let cell = self.cells[dst];
        let frame = &self.frames[cell.frame];
        let Op::Data {
            primitive,
            arguments,
        } = &self.program.words[frame.word].ops[cell.slot]
        else {
            unreachable!()
        };
        let primitive = *primitive;
        let arguments: Vec<_> = arguments.iter().map(|s| frame.base + s).collect();
        // Validate the container before demanding indices or any replacement.
        if phase == 1 {
            match primitive {
                Primitive::TupleLength | Primitive::Nth | Primitive::Set => {
                    self.tuple_len(self.value(arguments[0]))?;
                }
                _ => {
                    self.expect_text(self.value(arguments[0]))?;
                }
            }
        }
        let demanded = if primitive == Primitive::Set {
            2
        } else {
            arguments.len()
        };
        if phase < demanded {
            self.work.push(Task::Data(dst, phase + 1));
            self.work.push(Task::Need(arguments[phase]));
            return Ok(());
        }
        let a = self.value(arguments[0]);
        let result = match primitive {
            Primitive::TupleLength => Datum::Int(self.tuple_len(a)? as i64),
            Primitive::Nth | Primitive::Set => {
                let index = self.index(self.value(arguments[1]))?;
                let field = self.tuple_field(a, index)?;
                if primitive == Primitive::Nth {
                    self.copy_later(dst, field);
                    return Ok(());
                }
                let n = self.tuple_len(a)?;
                self.data_charge(n)?;
                let mut fields = (0..n)
                    .map(|i| self.tuple_field(a, i))
                    .collect::<Result<Vec<_>, _>>()?;
                fields[index] = arguments[2];
                self.make_tuple(fields)?
            }
            Primitive::TextBytes => Datum::Int(self.expect_text(a)?.len() as i64),
            Primitive::TextChars => {
                self.data_charge(self.expect_text(a)?.len())?;
                Datum::Int(self.expect_text(a)?.chars().count() as i64)
            }
            Primitive::TextConcat => {
                let b = self.value(arguments[1]);
                let len = self
                    .expect_text(a)?
                    .len()
                    .checked_add(self.expect_text(b)?.len())
                    .ok_or(Error::StorageLimit)?;
                self.reserve_text(len)?;
                self.data_charge(len)?;
                let mut text = String::with_capacity(len);
                text.push_str(self.expect_text(a)?);
                text.push_str(self.expect_text(b)?);
                self.store_text(text)?
            }
            Primitive::TextSlice => {
                let start = self.index(self.value(arguments[1]))?;
                let end = self.index(self.value(arguments[2]))?;
                let text = self
                    .expect_text(a)?
                    .get(start..end)
                    .ok_or(Error::Type("invalid UTF-8 slice boundaries"))?;
                let len = text.len();
                self.reserve_text(len)?;
                self.data_charge(len)?;
                self.store_text(self.expect_text(a)?[start..end].to_owned())?
            }
        };
        self.ready(dst, result);
        Ok(())
    }
}
