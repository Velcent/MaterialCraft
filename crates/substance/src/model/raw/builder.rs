//! [`RawTree`]: builds the whole object tree of a `document.bin` in memory from the streaming walk.
//! Bulky classes (brush strokes with their thousands of points) are kept as pruned stubs: their class is
//! known and counted, their content is skipped. Every object, pruned or not, is counted per class.

use super::tree::{RawObject, RawValue};
use crate::decode::document::{walk, DocumentHeader, Scalar, ValueTag, Visitor};
use crate::decode::DecodeError;
use std::collections::BTreeMap;

/// Classes whose content is not kept (only counted): strokes hold the bulk of a document.
const PRUNED: &[&str] = &["DataStroke", "DataStroke3D"];
/// Upper bound for preallocating a list; real lengths can be larger and simply grow.
const MAX_PREALLOC: usize = 1024;

/// A document's object tree plus what the walk saw.
#[derive(Debug, Clone, PartialEq)]
pub struct RawTree {
    pub format_version: u32,
    /// `None` if the root value is not an object.
    pub root: Option<RawObject>,
    /// Number of objects per class, including those inside pruned objects.
    pub classes: BTreeMap<String, usize>,
    /// Value tags the walker could not size (the rest of their container was skipped).
    pub unknown_tags: usize,
}

impl RawTree {
    pub fn parse(data: &[u8]) -> Result<RawTree, DecodeError> {
        let format_version = DocumentHeader::parse(data)?.format_version;
        let mut builder = Builder::default();
        walk(data, &mut builder)?;
        Ok(RawTree {
            format_version,
            root: builder.root,
            classes: builder.classes,
            unknown_tags: builder.unknown_tags,
        })
    }
}

/// An open container and, for objects, the field whose value comes next.
enum Frame {
    Object { object: RawObject, pending: Option<String> },
    List(Vec<RawValue>),
}

#[derive(Default)]
struct Builder {
    frames: Vec<Frame>,
    root: Option<RawObject>,
    classes: BTreeMap<String, usize>,
    unknown_tags: usize,
    /// Open containers inside a pruned object (0 = not inside one).
    skipping: usize,
}

impl Builder {
    /// Adds a finished value to the innermost open container.
    fn attach(&mut self, value: RawValue) {
        match self.frames.last_mut() {
            Some(Frame::Object { object, pending }) => {
                let name = pending.take().unwrap_or_default();
                object.fields.push((name, value));
            }
            Some(Frame::List(items)) => items.push(value),
            None => {
                if let RawValue::Object(o) = value {
                    self.root = Some(*o);
                }
            }
        }
    }

    /// A field announced without a value was an absent pointer.
    fn close_pending(&mut self) {
        if let Some(Frame::Object { object, pending }) = self.frames.last_mut() {
            if let Some(name) = pending.take() {
                object.fields.push((name, RawValue::Absent));
            }
        }
    }
}

impl Visitor for Builder {
    fn object(&mut self, class: &str) {
        *self.classes.entry(class.to_string()).or_default() += 1;
        if self.skipping > 0 {
            self.skipping += 1;
            return;
        }
        let mut object = RawObject::new(class);
        if PRUNED.contains(&class) {
            object.pruned = true;
            self.skipping = 1;
        }
        self.frames.push(Frame::Object { object, pending: None });
    }

    fn end_object(&mut self) {
        if self.skipping > 1 {
            self.skipping -= 1;
            return;
        }
        self.skipping = 0;
        self.close_pending();
        if let Some(Frame::Object { object, .. }) = self.frames.pop() {
            self.attach(RawValue::Object(Box::new(object)));
        }
    }

    fn field(&mut self, name: &[u8]) {
        if self.skipping > 0 {
            return;
        }
        self.close_pending();
        if let Some(Frame::Object { pending, .. }) = self.frames.last_mut() {
            *pending = Some(String::from_utf8_lossy(name).into_owned());
        }
    }

    fn scalar(&mut self, tag: ValueTag, payload: &[u8]) {
        if self.skipping > 0 {
            return;
        }
        // `Scalar::decode` only fails for container tags, which never reach this callback.
        if let Some(s) = Scalar::decode(tag, payload) {
            self.attach(RawValue::Scalar(s));
        }
    }

    fn list(&mut self, count: usize) {
        if self.skipping > 0 {
            self.skipping += 1;
            return;
        }
        self.frames
            .push(Frame::List(Vec::with_capacity(count.min(MAX_PREALLOC))));
    }

    fn end_list(&mut self) {
        if self.skipping > 0 {
            self.skipping -= 1;
            return;
        }
        if let Some(Frame::List(items)) = self.frames.pop() {
            self.attach(RawValue::List(items));
        }
    }

    fn unknown_tag(&mut self, _tag: u8, _offset: usize) {
        self.unknown_tags += 1;
    }
}
