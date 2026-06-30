//! # rivora-graph
//!
//! Typed Context Graph model for Open Rivora.
//!
//! The context graph is a directed, labeled graph where nodes represent
//! infrastructure entities (services, deployments, incidents, etc.) and edges
//! represent relationships between them (owns, depends-on, deployed-to, etc.).
//!
//! ## Schema
//!
//! A [`ContextGraph`] is the top-level type. It contains:
//!
//! - A unique [`GraphId`] (a [`TypedId<Graph>`](rivora_types::TypedId))
//! - A set of [`Node`]s keyed by ID
//! - A set of [`Edge`]s keyed by ID
//! - Graph-level [`GraphMetadata`], [`GraphTimestamps`], and [`GraphVersion`]
//!
//! ## Validation
//!
//! All graphs are validated before being surfaced. See the
//! [`validation`] module for the canonical validation rules.

pub mod builders;
pub mod confidence;
pub mod edge;
pub mod fixtures;
pub mod graph;
pub mod kind;
pub mod metadata;
pub mod node;
pub mod provenance;
pub mod snapshot;
pub mod validation;

pub use confidence::{GraphConfidence, GraphConfidenceLevel};
pub use edge::Edge;
pub use graph::ContextGraph;
pub use kind::{EdgeKind, NodeKind};
pub use metadata::{EdgeMetadata, GraphMetadata, GraphTimestamps, GraphVersion, NodeMetadata};
pub use node::Node;
pub use provenance::GraphProvenance;
pub use snapshot::GraphSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Graph;

impl rivora_types::IdTag for Graph {
    const KIND: &'static str = "graph";
}

pub type GraphId = rivora_types::TypedId<Graph>;
