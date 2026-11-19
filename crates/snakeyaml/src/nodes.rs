//! `org.yaml.snakeyaml.nodes`: the representation graph (shared, possibly recursive).

use crate::error::Mark;
use crate::options::{FlowStyle, ScalarStyle};
use parking_lot::RwLock;
use std::sync::Arc;

/// `Tag.PREFIX`: the prefix of the YAML 1.1 standard tags.
pub const TAG_PREFIX: &str = "tag:yaml.org,2002:";
/// `Tag.YAML`.
pub const TAG_YAML: &str = "tag:yaml.org,2002:yaml";
/// `Tag.MERGE`: the `<<` merge key.
pub const TAG_MERGE: &str = "tag:yaml.org,2002:merge";
/// `Tag.SET`.
pub const TAG_SET: &str = "tag:yaml.org,2002:set";
/// `Tag.PAIRS`.
pub const TAG_PAIRS: &str = "tag:yaml.org,2002:pairs";
/// `Tag.OMAP`.
pub const TAG_OMAP: &str = "tag:yaml.org,2002:omap";
/// `Tag.BINARY`: base64-encoded bytes.
pub const TAG_BINARY: &str = "tag:yaml.org,2002:binary";
/// `Tag.INT`.
pub const TAG_INT: &str = "tag:yaml.org,2002:int";
/// `Tag.FLOAT`.
pub const TAG_FLOAT: &str = "tag:yaml.org,2002:float";
/// `Tag.TIMESTAMP`.
pub const TAG_TIMESTAMP: &str = "tag:yaml.org,2002:timestamp";
/// `Tag.BOOL`.
pub const TAG_BOOL: &str = "tag:yaml.org,2002:bool";
/// `Tag.NULL`.
pub const TAG_NULL: &str = "tag:yaml.org,2002:null";
/// `Tag.STR`.
pub const TAG_STR: &str = "tag:yaml.org,2002:str";
/// `Tag.SEQ`.
pub const TAG_SEQ: &str = "tag:yaml.org,2002:seq";
/// `Tag.MAP`.
pub const TAG_MAP: &str = "tag:yaml.org,2002:map";

/// `Tag.standardTags`: the tags [`is_custom_global`] treats as standard.
pub const STANDARD_TAGS: &[&str] =
    &[TAG_YAML, TAG_MERGE, TAG_SET, TAG_PAIRS, TAG_OMAP, TAG_BINARY, TAG_INT, TAG_FLOAT, TAG_TIMESTAMP, TAG_BOOL, TAG_NULL, TAG_STR, TAG_SEQ, TAG_MAP];

/// `Tag.isCustomGlobal()`
pub fn is_custom_global(tag: &str) -> bool {
    tag.starts_with(TAG_PREFIX) && !STANDARD_TAGS.contains(&tag)
}

/// `org.yaml.snakeyaml.nodes.NodeId`: the kind of a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeId {
    /// A `ScalarNode`.
    Scalar,
    /// A `SequenceNode`.
    Sequence,
    /// A `MappingNode`.
    Mapping,
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            NodeId::Scalar => "scalar",
            NodeId::Sequence => "sequence",
            NodeId::Mapping => "mapping",
        })
    }
}

/// The content of a node, one variant per concrete SnakeYAML node class.
#[derive(Debug)]
pub enum NodeValue {
    /// `ScalarNode`.
    Scalar {
        /// `getValue()`: the scalar text.
        value: String,
        /// `getScalarStyle()`: the quoting style it was written in.
        style: ScalarStyle,
    },
    /// `SequenceNode.getValue()`: the items in order.
    Sequence(Vec<Node>),
    /// `MappingNode.getValue()`: the key/value pairs in document order.
    Mapping(Vec<(Node, Node)>),
}

/// The state of one node: the fields SnakeYAML's `Node` and its subclasses hold.
#[derive(Debug)]
pub struct NodeData {
    /// `getTag()`: the node's tag as a full URI-form string.
    pub tag: String,
    /// `Node.resolved`: `true` when the resolver assigned the tag, `false` for an explicit tag.
    pub resolved: bool,
    /// The node's content.
    pub value: NodeValue,
    /// `getStartMark()`: where the node starts in the input.
    pub start_mark: Mark,
    /// `getEndMark()`: where the node ends in the input, `None` when unknown.
    pub end_mark: Option<Mark>,
    /// `getFlowStyle()` for collections; ignored for scalars.
    pub flow_style: FlowStyle,
    /// `getAnchor()`: the `&anchor` the node was defined with, `None` when it has none.
    pub anchor: Option<String>,
    /// `isTwoStepsConstruction()`: the node takes part in a recursive structure and is built in two steps.
    pub two_steps_construction: bool,
    /// `MappingNode.isMerged()`: the mapping has a `<<` merge key.
    pub merged: bool,
}

/// A node shared by reference (aliases share the anchored node; recursive documents form cycles).
#[derive(Clone, Debug)]
pub struct Node(pub Arc<RwLock<NodeData>>);

impl Node {
    /// A shared node owning `data`.
    pub fn new(data: NodeData) -> Node {
        Node(Arc::new(RwLock::new(data)))
    }

    /// `true` when both handles point at the same node (an alias of the same anchor).
    pub fn ptr_eq(&self, other: &Node) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// A process-unique identity of the node, used where Java prints `System.identityHashCode`.
    pub fn id(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }

    /// `getNodeId()`: scalar, sequence or mapping.
    pub fn node_id(&self) -> NodeId {
        match self.0.read().value {
            NodeValue::Scalar { .. } => NodeId::Scalar,
            NodeValue::Sequence(_) => NodeId::Sequence,
            NodeValue::Mapping(_) => NodeId::Mapping,
        }
    }

    /// `getTag()`: a copy of the node's tag.
    pub fn tag(&self) -> String {
        self.0.read().tag.clone()
    }

    /// `setTag(tag)`: replaces the node's tag.
    pub fn set_tag(&self, tag: &str) {
        self.0.write().tag = tag.to_string();
    }

    /// `getStartMark()`: where the node starts in the input.
    pub fn start_mark(&self) -> Mark {
        self.0.read().start_mark.clone()
    }

    /// `isTwoStepsConstruction()`: the node takes part in a recursive structure.
    pub fn is_two_steps_construction(&self) -> bool {
        self.0.read().two_steps_construction
    }

    /// `ScalarNode.getValue()`: the scalar text, `None` for a collection.
    pub fn scalar_value(&self) -> Option<String> {
        match &self.0.read().value {
            NodeValue::Scalar { value, .. } => Some(value.clone()),
            _ => None,
        }
    }

    /// `SequenceNode.getValue()`: a copy of the item handles, empty for a non-sequence.
    pub fn sequence_children(&self) -> Vec<Node> {
        match &self.0.read().value {
            NodeValue::Sequence(v) => v.clone(),
            _ => Vec::new(),
        }
    }

    /// `MappingNode.getValue()`: a copy of the key/value handles, empty for a non-mapping.
    pub fn mapping_children(&self) -> Vec<(Node, Node)> {
        match &self.0.read().value {
            NodeValue::Mapping(v) => v.clone(),
            _ => Vec::new(),
        }
    }

    /// `MappingNode.setValue(value)`: replaces the node's content with the given pairs.
    pub fn set_mapping_children(&self, v: Vec<(Node, Node)>) {
        self.0.write().value = NodeValue::Mapping(v);
    }

    /// `Node.toString()` (identity hash codes rendered from the pointer).
    pub fn to_java_string(&self) -> String {
        let d = self.0.read();
        match &d.value {
            NodeValue::Scalar { value, .. } => format!("<org.yaml.snakeyaml.nodes.ScalarNode (tag={}, value={})>", d.tag, value),
            NodeValue::Sequence(children) => {
                let mut buf = String::new();
                for c in children {
                    if c.node_id() == NodeId::Scalar {
                        buf.push_str(&c.to_java_string());
                    } else {
                        buf.push_str(&format!("{}", c.id() as u32 & 0x7fff_ffff));
                    }
                    buf.push(',');
                }
                if !buf.is_empty() {
                    buf.pop();
                }
                format!("<org.yaml.snakeyaml.nodes.SequenceNode (tag={}, value=[{}])>", d.tag, buf)
            }
            NodeValue::Mapping(children) => {
                let mut buf = String::new();
                for (k, v) in children {
                    buf.push_str("{ key=");
                    buf.push_str(&k.to_java_string());
                    buf.push_str("; value=");
                    if v.node_id() == NodeId::Scalar {
                        buf.push_str(&v.to_java_string());
                    } else {
                        buf.push_str(&format!("{}", v.id() as u32 & 0x7fff_ffff));
                    }
                    buf.push_str(" }");
                }
                format!("<org.yaml.snakeyaml.nodes.MappingNode (tag={}, values={})>", d.tag, buf)
            }
        }
    }
}
