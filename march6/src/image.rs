use crate::cid::{Cid, put_bytes, put_cid, put_u64};
use crate::net::{Atom, Clause, Node, Store};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const MAGIC: &[u8] = b"MARCH6-IMAGE\0V1";

/// A canonical, self-contained snapshot rooted in one or more graph objects.
/// Only nodes reachable from the ordered roots are serialized, so unrelated
/// store history cannot change the image identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub roots: Vec<Cid>,
    bytes: Vec<u8>,
}

impl Image {
    pub fn from_store(store: &Store, roots: &[Cid]) -> Result<Self, ImageError> {
        for root in roots {
            if !store.nodes.contains_key(root) {
                return Err(ImageError::MissingNode(*root));
            }
        }
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        put_u64(&mut bytes, roots.len() as u64);
        for root in roots {
            put_cid(&mut bytes, *root);
        }
        let mut reachable = BTreeSet::new();
        let mut pending = roots.to_vec();
        while let Some(cid) = pending.pop() {
            if !reachable.insert(cid) {
                continue;
            }
            let node = store.nodes.get(&cid).ok_or(ImageError::MissingNode(cid))?;
            pending.extend(node.children());
        }
        put_u64(&mut bytes, reachable.len() as u64);
        for cid in reachable {
            let node = store.nodes.get(&cid).ok_or(ImageError::MissingNode(cid))?;
            put_cid(&mut bytes, cid);
            put_bytes(&mut bytes, &node.canonical_bytes());
        }
        Ok(Self {
            roots: roots.to_vec(),
            bytes,
        })
    }

    pub fn parse(bytes: &[u8]) -> Result<(Self, Store), ImageError> {
        let mut input = Cursor::new(bytes);
        if input.take(MAGIC.len())? != MAGIC {
            return Err(ImageError::BadMagic);
        }
        let root_count = input.usize()?;
        let roots = (0..root_count)
            .map(|_| input.cid())
            .collect::<Result<Vec<_>, _>>()?;
        let node_count = input.usize()?;
        let mut nodes = BTreeMap::new();
        for _ in 0..node_count {
            let expected = input.cid()?;
            let payload = input.bytes()?;
            let node = decode_node(payload)?;
            if node.canonical_bytes() != payload {
                return Err(ImageError::NonCanonical(expected));
            }
            let actual = Cid::digest(b"march6/node/v1", payload);
            if actual != expected {
                return Err(ImageError::CidMismatch { expected, actual });
            }
            if nodes.insert(expected, node).is_some() {
                return Err(ImageError::DuplicateNode(expected));
            }
        }
        input.finish()?;
        for root in &roots {
            if !nodes.contains_key(root) {
                return Err(ImageError::MissingNode(*root));
            }
        }
        for node in nodes.values() {
            for child in node.children() {
                if !nodes.contains_key(&child) {
                    return Err(ImageError::MissingNode(child));
                }
            }
        }
        Ok((
            Self {
                roots,
                bytes: bytes.to_vec(),
            },
            Store {
                nodes,
                validated_code: BTreeSet::new(),
            },
        ))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn cid(&self) -> Cid {
        Cid::digest(b"march6/image/v1", &self.bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageError {
    BadMagic,
    Truncated,
    TrailingBytes,
    InvalidUtf8,
    UnknownTag(u8),
    LengthOverflow,
    MissingNode(Cid),
    DuplicateNode(Cid),
    NonCanonical(Cid),
    CidMismatch { expected: Cid, actual: Cid },
    RecordFieldsNotCanonical,
    InvalidBool(u8),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => f.write_str("not a March 6 image"),
            Self::Truncated => f.write_str("truncated March 6 image"),
            Self::TrailingBytes => f.write_str("trailing bytes in canonical object"),
            Self::InvalidUtf8 => f.write_str("invalid UTF-8 in canonical object"),
            Self::UnknownTag(tag) => write!(f, "unknown canonical node tag {tag}"),
            Self::LengthOverflow => f.write_str("canonical length does not fit this platform"),
            Self::MissingNode(cid) => write!(f, "image is missing referenced node {cid}"),
            Self::DuplicateNode(cid) => write!(f, "image contains node {cid} twice"),
            Self::NonCanonical(cid) => write!(f, "node {cid} is not canonically encoded"),
            Self::CidMismatch { expected, actual } => {
                write!(
                    f,
                    "node claims CID {expected}, but its bytes hash to {actual}"
                )
            }
            Self::RecordFieldsNotCanonical => {
                f.write_str("record fields are duplicated or not in canonical order")
            }
            Self::InvalidBool(value) => write!(f, "noncanonical Boolean byte {value}"),
        }
    }
}

impl std::error::Error for ImageError {}

struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ImageError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(ImageError::LengthOverflow)?;
        let bytes = self
            .input
            .get(self.offset..end)
            .ok_or(ImageError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, ImageError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ImageError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, ImageError> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn usize(&mut self) -> Result<usize, ImageError> {
        self.u64()?
            .try_into()
            .map_err(|_| ImageError::LengthOverflow)
    }

    fn i64(&mut self) -> Result<i64, ImageError> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn cid(&mut self) -> Result<Cid, ImageError> {
        Ok(Cid(self.take(32)?.try_into().unwrap()))
    }

    fn bytes(&mut self) -> Result<&'a [u8], ImageError> {
        let count = self.usize()?;
        self.take(count)
    }

    fn string(&mut self) -> Result<String, ImageError> {
        std::str::from_utf8(self.bytes()?)
            .map(str::to_owned)
            .map_err(|_| ImageError::InvalidUtf8)
    }

    fn finish(&self) -> Result<(), ImageError> {
        if self.offset == self.input.len() {
            Ok(())
        } else {
            Err(ImageError::TrailingBytes)
        }
    }
}

fn decode_node(bytes: &[u8]) -> Result<Node, ImageError> {
    let mut input = Cursor::new(bytes);
    let node = match input.u8()? {
        0 => Node::Const(decode_atom(&mut input)?),
        1 => Node::Hole(input.string()?),
        2 => Node::Param(input.u16()?),
        3 => Node::Add(input.cid()?, input.cid()?),
        4 => Node::Mul(input.cid()?, input.cid()?),
        5 => Node::Eq(input.cid()?, input.cid()?),
        6 => Node::If {
            condition: input.cid()?,
            when_true: input.cid()?,
            when_false: input.cid()?,
        },
        7 => Node::Pair(input.cid()?, input.cid()?),
        8 => Node::First(input.cid()?),
        9 => Node::Second(input.cid()?),
        10 => {
            let count = input.usize()?;
            let mut fields = Vec::with_capacity(count);
            for _ in 0..count {
                fields.push((input.string()?, input.cid()?));
            }
            if fields
                .windows(2)
                .any(|pair| pair[0].0.as_str() >= pair[1].0.as_str())
            {
                return Err(ImageError::RecordFieldsNotCanonical);
            }
            Node::Record(fields)
        }
        11 => Node::Get {
            record: input.cid()?,
            field: input.string()?,
        },
        12 => Node::Put {
            record: input.cid()?,
            field: input.string()?,
            value: input.cid()?,
        },
        13 => Node::Quote {
            params: input.u16()?,
            body: input.cid()?,
        },
        14 => {
            let function = input.cid()?;
            let count = input.usize()?;
            let arguments = (0..count)
                .map(|_| input.cid())
                .collect::<Result<Vec<_>, _>>()?;
            Node::Apply {
                function,
                arguments,
            }
        }
        15 => Node::Emit {
            token: input.cid()?,
            message: input.cid()?,
        },
        16 => {
            let parameters = input.u16()?;
            let count = input.usize()?;
            let clauses = (0..count)
                .map(|_| {
                    Ok(Clause {
                        guard: input.cid()?,
                        body: input.cid()?,
                    })
                })
                .collect::<Result<Vec<_>, ImageError>>()?;
            Node::Family {
                parameters,
                clauses,
            }
        }
        17 => {
            let family = input.cid()?;
            let count = input.usize()?;
            let arguments = (0..count)
                .map(|_| input.cid())
                .collect::<Result<Vec<_>, _>>()?;
            Node::Dispatch { family, arguments }
        }
        18 => {
            let count = input.usize()?;
            Node::Recur(
                (0..count)
                    .map(|_| input.cid())
                    .collect::<Result<Vec<_>, _>>()?,
            )
        }
        tag => return Err(ImageError::UnknownTag(tag)),
    };
    input.finish()?;
    Ok(node)
}

fn decode_atom(input: &mut Cursor<'_>) -> Result<Atom, ImageError> {
    match input.u8()? {
        0 => Ok(Atom::Int(input.i64()?)),
        1 => match input.u8()? {
            0 => Ok(Atom::Bool(false)),
            1 => Ok(Atom::Bool(true)),
            value => Err(ImageError::InvalidBool(value)),
        },
        2 => Ok(Atom::Text(input.string()?)),
        3 => {
            let count = input.usize()?;
            let entries = (0..count)
                .map(|_| input.string())
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Atom::Trace(entries))
        }
        4 => Ok(Atom::Unit),
        tag => Err(ImageError::UnknownTag(tag)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_round_trip_preserves_bytes_cids_and_behavior_graph() {
        let mut store = Store::new();
        let one = store.intern(Node::Const(Atom::Int(1)));
        let input = store.intern(Node::Hole("input".into()));
        let root = store.intern(Node::Add(input, one));
        let image = Image::from_store(&store, &[root]).unwrap();
        let image_cid = image.cid();
        let (loaded_image, loaded_store) = Image::parse(image.as_bytes()).unwrap();
        assert_eq!(loaded_image.cid(), image_cid);
        assert_eq!(loaded_image.as_bytes(), image.as_bytes());
        assert_eq!(loaded_image.roots, vec![root]);
        assert_eq!(loaded_store.format(root), "(+ ?input 1)");
    }

    #[test]
    fn store_insertion_order_does_not_change_image() {
        let mut left = Store::new();
        let left_one = left.intern(Node::Const(Atom::Int(1)));
        let left_two = left.intern(Node::Const(Atom::Int(2)));
        let left_root = left.intern(Node::Pair(left_one, left_two));

        let mut right = Store::new();
        let right_two = right.intern(Node::Const(Atom::Int(2)));
        let right_one = right.intern(Node::Const(Atom::Int(1)));
        let right_root = right.intern(Node::Pair(right_one, right_two));

        let left = Image::from_store(&left, &[left_root]).unwrap();
        let right = Image::from_store(&right, &[right_root]).unwrap();
        assert_eq!(left_root, right_root);
        assert_eq!(left.as_bytes(), right.as_bytes());
        assert_eq!(left.cid(), right.cid());
    }

    #[test]
    fn corruption_is_detected() {
        let mut store = Store::new();
        let root = store.intern(Node::Const(Atom::Int(1)));
        let image = Image::from_store(&store, &[root]).unwrap();
        let mut bytes = image.as_bytes().to_vec();
        *bytes.last_mut().unwrap() ^= 1;
        assert!(matches!(
            Image::parse(&bytes),
            Err(ImageError::CidMismatch { .. })
                | Err(ImageError::UnknownTag(_))
                | Err(ImageError::NonCanonical(_))
        ));
    }

    #[test]
    fn unreachable_store_history_does_not_change_an_image() {
        let mut clean = Store::new();
        let clean_root = clean.intern(Node::Const(Atom::Int(1)));
        let clean = Image::from_store(&clean, &[clean_root]).unwrap();

        let mut historical = Store::new();
        historical.intern(Node::Const(Atom::Text("unreachable history".into())));
        let historical_root = historical.intern(Node::Const(Atom::Int(1)));
        let historical = Image::from_store(&historical, &[historical_root]).unwrap();
        assert_eq!(clean_root, historical_root);
        assert_eq!(clean.as_bytes(), historical.as_bytes());
        assert_eq!(clean.cid(), historical.cid());
    }

    #[test]
    fn decoder_rejects_noncanonical_record_fields_and_booleans() {
        let cid = Cid([0; 32]);
        let mut record = vec![10];
        put_u64(&mut record, 2);
        crate::cid::put_str(&mut record, "b");
        put_cid(&mut record, cid);
        crate::cid::put_str(&mut record, "a");
        put_cid(&mut record, cid);
        assert_eq!(
            decode_node(&record),
            Err(ImageError::RecordFieldsNotCanonical)
        );

        assert_eq!(decode_node(&[0, 1, 2]), Err(ImageError::InvalidBool(2)));
    }
}
