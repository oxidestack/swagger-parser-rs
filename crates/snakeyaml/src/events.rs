//! `org.yaml.snakeyaml.events`

use crate::error::Mark;
use crate::options::{FlowStyle, ScalarStyle, Version};

/// `ImplicitTuple(plain, nonPlain)`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImplicitTuple {
    /// The tag may be omitted when the scalar is written plain.
    pub plain: bool,
    /// The tag may be omitted when the scalar is written in a quoted or block style.
    pub non_plain: bool,
}

impl ImplicitTuple {
    /// `canOmitTagInPlainScalar()`.
    pub fn can_omit_tag_in_plain_scalar(&self) -> bool {
        self.plain
    }
    /// `canOmitTagInNonPlainScalar()`.
    pub fn can_omit_tag_in_non_plain_scalar(&self) -> bool {
        self.non_plain
    }
    /// `bothFalse()`: the tag can be omitted in neither style.
    pub fn both_false(&self) -> bool {
        !self.plain && !self.non_plain
    }
}

/// What an [`Event`] is: one variant per `org.yaml.snakeyaml.events` class.
#[derive(Clone, Debug)]
pub enum EventKind {
    /// `StreamStartEvent`.
    StreamStart,
    /// `StreamEndEvent`.
    StreamEnd,
    /// `DocumentStartEvent`.
    DocumentStart {
        /// The document starts with an explicit `---`.
        explicit: bool,
        /// The `%YAML` directive's version, `None` when there is none.
        version: Option<Version>,
        /// The `%TAG` directives as `(handle, prefix)` pairs, `None` when there are none.
        tags: Option<Vec<(String, String)>>,
    },
    /// `DocumentEndEvent`.
    DocumentEnd {
        /// The document ends with an explicit `...`.
        explicit: bool,
    },
    /// `AliasEvent`: a `*alias` reference.
    Alias {
        /// The anchor name the alias refers to.
        anchor: String,
    },
    /// `ScalarEvent`.
    Scalar {
        /// The `&anchor` of the node, `None` when it has none.
        anchor: Option<String>,
        /// The explicit tag, `None` when the node has none.
        tag: Option<String>,
        /// Whether the tag may be omitted in plain and in non-plain style.
        implicit: ImplicitTuple,
        /// The scalar text.
        value: String,
        /// The quoting style.
        style: ScalarStyle,
    },
    /// `SequenceStartEvent`.
    SequenceStart {
        /// The `&anchor` of the node, `None` when it has none.
        anchor: Option<String>,
        /// The explicit tag, `None` when the node has none.
        tag: Option<String>,
        /// The tag may be omitted (it is the one the resolver would assign).
        implicit: bool,
        /// Flow (`[a, b]`) or block style.
        flow_style: FlowStyle,
    },
    /// `SequenceEndEvent`.
    SequenceEnd,
    /// `MappingStartEvent`.
    MappingStart {
        /// The `&anchor` of the node, `None` when it has none.
        anchor: Option<String>,
        /// The explicit tag, `None` when the node has none.
        tag: Option<String>,
        /// The tag may be omitted (it is the one the resolver would assign).
        implicit: bool,
        /// Flow (`{a: b}`) or block style.
        flow_style: FlowStyle,
    },
    /// `MappingEndEvent`.
    MappingEnd,
}

/// `org.yaml.snakeyaml.events.Event`: one parser or emitter event with its position in the input.
#[derive(Clone, Debug)]
pub struct Event {
    /// The event type and its data.
    pub kind: EventKind,
    /// `getStartMark()`: where the event starts, `None` for an event built by emitting code.
    pub start_mark: Option<Mark>,
    /// `getEndMark()`: where the event ends, `None` for an event built by emitting code.
    pub end_mark: Option<Mark>,
}

impl Event {
    /// An event spanning `start` to `end` in the input.
    pub fn new(kind: EventKind, start: Mark, end: Mark) -> Event {
        Event { kind, start_mark: Some(start), end_mark: Some(end) }
    }

    /// An event without marks (as built by emitting code).
    pub fn unmarked(kind: EventKind) -> Event {
        Event { kind, start_mark: None, end_mark: None }
    }
}
