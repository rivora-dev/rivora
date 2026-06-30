//! Ergonomic builders for constructing graph entities.

use rivora_errors::RivoraError;
use rivora_types::NonEmptyString;

use crate::confidence::GraphConfidence;
use crate::edge::Edge;
use crate::graph::ContextGraph;
use crate::kind::{EdgeKind, NodeKind};
use crate::metadata::{EdgeMetadata, GraphMetadata, GraphTimestamps, GraphVersion, NodeMetadata};
use crate::node::Node;
use crate::provenance::GraphProvenance;
use crate::GraphId;

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct GraphBuilder {
    id: Option<GraphId>,
    metadata: Option<GraphMetadata>,
    timestamps: Option<GraphTimestamps>,
    version: Option<GraphVersion>,
}

impl GraphBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(GraphId::new_unchecked(id.into()));
        self
    }

    #[must_use]
    pub fn metadata(mut self, metadata: GraphMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    #[must_use]
    pub fn timestamps(mut self, timestamps: GraphTimestamps) -> Self {
        self.timestamps = Some(timestamps);
        self
    }

    #[must_use]
    pub fn version(mut self, version: GraphVersion) -> Self {
        self.version = Some(version);
        self
    }

    pub fn build(self) -> Result<ContextGraph, RivoraError> {
        let id = self
            .id
            .ok_or_else(|| RivoraError::invalid_value("graph_id", "id is required"))?;
        let metadata = self.metadata.unwrap_or_default();
        let timestamps = self.timestamps.ok_or_else(|| {
            RivoraError::invalid_value("graph_timestamps", "timestamps is required")
        })?;
        let version = self
            .version
            .ok_or_else(|| RivoraError::invalid_value("graph_version", "version is required"))?;
        Ok(ContextGraph::new(id, metadata, timestamps, version))
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeBuilder {
    id: Option<NonEmptyString>,
    kind: Option<NodeKind>,
    display_name: Option<NonEmptyString>,
    description: Option<NonEmptyString>,
    labels: Option<BTreeMap<NonEmptyString, NonEmptyString>>,
    metadata: Option<NodeMetadata>,
    provenance: Option<GraphProvenance>,
    confidence: Option<GraphConfidence>,
    timestamps: Option<GraphTimestamps>,
    version: Option<GraphVersion>,
}

impl NodeBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = NonEmptyString::new(id.into()).ok();
        self
    }

    #[must_use]
    pub fn kind(mut self, kind: NodeKind) -> Self {
        self.kind = Some(kind);
        self
    }

    #[must_use]
    pub fn display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = NonEmptyString::new(display_name.into()).ok();
        self
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = NonEmptyString::new(description.into()).ok();
        self
    }

    #[must_use]
    pub fn labels(mut self, labels: BTreeMap<NonEmptyString, NonEmptyString>) -> Self {
        self.labels = Some(labels);
        self
    }

    #[must_use]
    pub fn metadata(mut self, metadata: NodeMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    #[must_use]
    pub fn provenance(mut self, provenance: GraphProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    #[must_use]
    pub fn confidence(mut self, confidence: GraphConfidence) -> Self {
        self.confidence = Some(confidence);
        self
    }

    #[must_use]
    pub fn timestamps(mut self, timestamps: GraphTimestamps) -> Self {
        self.timestamps = Some(timestamps);
        self
    }

    #[must_use]
    pub fn version(mut self, version: GraphVersion) -> Self {
        self.version = Some(version);
        self
    }

    pub fn build(self) -> Result<Node, RivoraError> {
        let id = self
            .id
            .ok_or_else(|| RivoraError::invalid_value("node_id", "id is required"))?;
        let kind = self
            .kind
            .ok_or_else(|| RivoraError::invalid_value("node_kind", "kind is required"))?;
        let display_name = self.display_name.ok_or_else(|| {
            RivoraError::invalid_value("node_display_name", "display_name is required")
        })?;
        let provenance = self.provenance.ok_or_else(|| {
            RivoraError::invalid_value("node_provenance", "provenance is required")
        })?;
        let confidence = self.confidence.ok_or_else(|| {
            RivoraError::invalid_value("node_confidence", "confidence is required")
        })?;
        let timestamps = self.timestamps.ok_or_else(|| {
            RivoraError::invalid_value("node_timestamps", "timestamps is required")
        })?;
        let version = self
            .version
            .ok_or_else(|| RivoraError::invalid_value("node_version", "version is required"))?;
        Ok(Node {
            id,
            kind,
            display_name,
            description: self.description,
            labels: self.labels.unwrap_or_default(),
            metadata: self.metadata.unwrap_or_default(),
            provenance,
            confidence,
            timestamps,
            version,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct EdgeBuilder {
    id: Option<NonEmptyString>,
    kind: Option<EdgeKind>,
    from: Option<NonEmptyString>,
    to: Option<NonEmptyString>,
    display_name: Option<NonEmptyString>,
    labels: Option<BTreeMap<NonEmptyString, NonEmptyString>>,
    metadata: Option<EdgeMetadata>,
    provenance: Option<GraphProvenance>,
    confidence: Option<GraphConfidence>,
    timestamps: Option<GraphTimestamps>,
    version: Option<GraphVersion>,
}

impl EdgeBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = NonEmptyString::new(id.into()).ok();
        self
    }

    #[must_use]
    pub fn kind(mut self, kind: EdgeKind) -> Self {
        self.kind = Some(kind);
        self
    }

    #[must_use]
    pub fn from(mut self, from: impl Into<String>) -> Self {
        self.from = NonEmptyString::new(from.into()).ok();
        self
    }

    #[must_use]
    pub fn to(mut self, to: impl Into<String>) -> Self {
        self.to = NonEmptyString::new(to.into()).ok();
        self
    }

    #[must_use]
    pub fn display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = NonEmptyString::new(display_name.into()).ok();
        self
    }

    #[must_use]
    pub fn labels(mut self, labels: BTreeMap<NonEmptyString, NonEmptyString>) -> Self {
        self.labels = Some(labels);
        self
    }

    #[must_use]
    pub fn metadata(mut self, metadata: EdgeMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    #[must_use]
    pub fn provenance(mut self, provenance: GraphProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    #[must_use]
    pub fn confidence(mut self, confidence: GraphConfidence) -> Self {
        self.confidence = Some(confidence);
        self
    }

    #[must_use]
    pub fn timestamps(mut self, timestamps: GraphTimestamps) -> Self {
        self.timestamps = Some(timestamps);
        self
    }

    #[must_use]
    pub fn version(mut self, version: GraphVersion) -> Self {
        self.version = Some(version);
        self
    }

    pub fn build(self) -> Result<Edge, RivoraError> {
        let id = self
            .id
            .ok_or_else(|| RivoraError::invalid_value("edge_id", "id is required"))?;
        let kind = self
            .kind
            .ok_or_else(|| RivoraError::invalid_value("edge_kind", "kind is required"))?;
        let from = self
            .from
            .ok_or_else(|| RivoraError::invalid_value("edge_from", "from is required"))?;
        let to = self
            .to
            .ok_or_else(|| RivoraError::invalid_value("edge_to", "to is required"))?;
        let provenance = self.provenance.ok_or_else(|| {
            RivoraError::invalid_value("edge_provenance", "provenance is required")
        })?;
        let confidence = self.confidence.ok_or_else(|| {
            RivoraError::invalid_value("edge_confidence", "confidence is required")
        })?;
        let timestamps = self.timestamps.ok_or_else(|| {
            RivoraError::invalid_value("edge_timestamps", "timestamps is required")
        })?;
        let version = self
            .version
            .ok_or_else(|| RivoraError::invalid_value("edge_version", "version is required"))?;
        Ok(Edge {
            id,
            kind,
            from,
            to,
            display_name: self.display_name,
            labels: self.labels.unwrap_or_default(),
            metadata: self.metadata.unwrap_or_default(),
            provenance,
            confidence,
            timestamps,
            version,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProvenanceBuilder {
    source: Option<NonEmptyString>,
    source_kind: Option<NonEmptyString>,
    source_version: Option<NonEmptyString>,
    observed_at: Option<NonEmptyString>,
    receipt_id: Option<String>,
    raw_ref: Option<String>,
    connector_ref: Option<String>,
    inference_ref: Option<String>,
    ability_ref: Option<String>,
}

impl ProvenanceBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = NonEmptyString::new(source.into()).ok();
        self
    }

    #[must_use]
    pub fn source_kind(mut self, source_kind: impl Into<String>) -> Self {
        self.source_kind = NonEmptyString::new(source_kind.into()).ok();
        self
    }

    #[must_use]
    pub fn source_version(mut self, source_version: impl Into<String>) -> Self {
        self.source_version = NonEmptyString::new(source_version.into()).ok();
        self
    }

    #[must_use]
    pub fn observed_at(mut self, observed_at: impl Into<String>) -> Self {
        self.observed_at = NonEmptyString::new(observed_at.into()).ok();
        self
    }

    #[must_use]
    pub fn receipt_id(mut self, receipt_id: impl Into<String>) -> Self {
        self.receipt_id = Some(receipt_id.into());
        self
    }

    #[must_use]
    pub fn raw_ref(mut self, raw_ref: impl Into<String>) -> Self {
        self.raw_ref = Some(raw_ref.into());
        self
    }

    #[must_use]
    pub fn connector_ref(mut self, connector_ref: impl Into<String>) -> Self {
        self.connector_ref = Some(connector_ref.into());
        self
    }

    #[must_use]
    pub fn inference_ref(mut self, inference_ref: impl Into<String>) -> Self {
        self.inference_ref = Some(inference_ref.into());
        self
    }

    #[must_use]
    pub fn ability_ref(mut self, ability_ref: impl Into<String>) -> Self {
        self.ability_ref = Some(ability_ref.into());
        self
    }

    pub fn build(self) -> Result<GraphProvenance, RivoraError> {
        let source = self
            .source
            .ok_or_else(|| RivoraError::invalid_value("provenance_source", "source is required"))?;
        let source_kind = self.source_kind.ok_or_else(|| {
            RivoraError::invalid_value("provenance_source_kind", "source_kind is required")
        })?;
        let source_version = self.source_version.ok_or_else(|| {
            RivoraError::invalid_value("provenance_source_version", "source_version is required")
        })?;
        let observed_at = self.observed_at.ok_or_else(|| {
            RivoraError::invalid_value("provenance_observed_at", "observed_at is required")
        })?;
        Ok(GraphProvenance {
            source,
            source_kind,
            source_version,
            observed_at,
            receipt_id: self.receipt_id,
            raw_ref: self.raw_ref,
            connector_ref: self.connector_ref,
            inference_ref: self.inference_ref,
            ability_ref: self.ability_ref,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{GraphTimestamps, GraphVersion};
    use rivora_types::Version;

    fn timestamps() -> GraphTimestamps {
        GraphTimestamps::new("2026-06-25T12:00:00Z").unwrap()
    }

    fn version() -> GraphVersion {
        GraphVersion::new(Version::new(1, 0, 0), 1)
    }

    fn provenance() -> GraphProvenance {
        GraphProvenance::new("connector", "aws", "0.1.0", "2026-06-25T12:00:00Z").unwrap()
    }

    fn confidence() -> GraphConfidence {
        GraphConfidence::new(0.8, "Strong evidence").unwrap()
    }

    #[test]
    fn graph_builder_succeeds() {
        let g = GraphBuilder::new()
            .id("graph-1")
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap();
        assert_eq!(g.id.as_str(), "graph-1");
    }

    #[test]
    fn graph_builder_requires_id() {
        let err = GraphBuilder::new()
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("id"));
    }

    #[test]
    fn graph_builder_requires_timestamps() {
        let err = GraphBuilder::new()
            .id("graph-1")
            .version(version())
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("timestamps"));
    }

    #[test]
    fn graph_builder_requires_version() {
        let err = GraphBuilder::new()
            .id("graph-1")
            .timestamps(timestamps())
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("version"));
    }

    #[test]
    fn node_builder_succeeds() {
        let n = NodeBuilder::new()
            .id("node-1")
            .kind(NodeKind::Service)
            .display_name("api-gateway")
            .provenance(provenance())
            .confidence(confidence())
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap();
        assert_eq!(n.id.as_str(), "node-1");
        assert_eq!(n.kind, NodeKind::Service);
    }

    #[test]
    fn node_builder_requires_id() {
        let err = NodeBuilder::new()
            .kind(NodeKind::Service)
            .display_name("api-gateway")
            .provenance(provenance())
            .confidence(confidence())
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("id"));
    }

    #[test]
    fn node_builder_requires_kind() {
        let err = NodeBuilder::new()
            .id("node-1")
            .display_name("api-gateway")
            .provenance(provenance())
            .confidence(confidence())
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("kind"));
    }

    #[test]
    fn node_builder_with_optional_fields() {
        let n = NodeBuilder::new()
            .id("node-1")
            .kind(NodeKind::Service)
            .display_name("api-gateway")
            .description("The main API gateway")
            .provenance(provenance())
            .confidence(confidence())
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap();
        assert!(n.description.is_some());
    }

    #[test]
    fn edge_builder_succeeds() {
        let e = EdgeBuilder::new()
            .id("edge-1")
            .kind(EdgeKind::Owns)
            .from("org-1")
            .to("svc-1")
            .provenance(provenance())
            .confidence(confidence())
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap();
        assert_eq!(e.id.as_str(), "edge-1");
        assert_eq!(e.kind, EdgeKind::Owns);
    }

    #[test]
    fn edge_builder_requires_id() {
        let err = EdgeBuilder::new()
            .kind(EdgeKind::Owns)
            .from("org-1")
            .to("svc-1")
            .provenance(provenance())
            .confidence(confidence())
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("id"));
    }

    #[test]
    fn edge_builder_requires_from() {
        let err = EdgeBuilder::new()
            .id("edge-1")
            .kind(EdgeKind::Owns)
            .to("svc-1")
            .provenance(provenance())
            .confidence(confidence())
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("from"));
    }

    #[test]
    fn edge_builder_with_optional_fields() {
        let e = EdgeBuilder::new()
            .id("edge-1")
            .kind(EdgeKind::Owns)
            .from("org-1")
            .to("svc-1")
            .display_name("owns relationship")
            .provenance(provenance())
            .confidence(confidence())
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap();
        assert!(e.display_name.is_some());
    }

    #[test]
    fn provenance_builder_succeeds() {
        let p = ProvenanceBuilder::new()
            .source("connector")
            .source_kind("aws")
            .source_version("0.1.0")
            .observed_at("2026-06-25T12:00:00Z")
            .build()
            .unwrap();
        assert_eq!(p.source.as_str(), "connector");
    }

    #[test]
    fn provenance_builder_requires_source() {
        let err = ProvenanceBuilder::new()
            .source_kind("aws")
            .source_version("0.1.0")
            .observed_at("2026-06-25T12:00:00Z")
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("source"));
    }

    #[test]
    fn provenance_builder_with_optional_fields() {
        let p = ProvenanceBuilder::new()
            .source("connector")
            .source_kind("aws")
            .source_version("0.1.0")
            .observed_at("2026-06-25T12:00:00Z")
            .receipt_id("receipt_1")
            .connector_ref("aws-v1")
            .build()
            .unwrap();
        assert_eq!(p.receipt_id.as_deref(), Some("receipt_1"));
        assert!(p.connector_ref.is_some());
    }

    #[test]
    fn context_graph_builder_method() {
        let g = ContextGraph::builder()
            .id("graph-1")
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap();
        assert_eq!(g.id.as_str(), "graph-1");
    }

    #[test]
    fn node_builder_method() {
        let n = Node::builder()
            .id("node-1")
            .kind(NodeKind::Service)
            .display_name("api-gateway")
            .provenance(provenance())
            .confidence(confidence())
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap();
        assert_eq!(n.id.as_str(), "node-1");
    }

    #[test]
    fn edge_builder_method() {
        let e = Edge::builder()
            .id("edge-1")
            .kind(EdgeKind::Owns)
            .from("org-1")
            .to("svc-1")
            .provenance(provenance())
            .confidence(confidence())
            .timestamps(timestamps())
            .version(version())
            .build()
            .unwrap();
        assert_eq!(e.id.as_str(), "edge-1");
    }

    #[test]
    fn provenance_builder_method() {
        let p = GraphProvenance::builder()
            .source("connector")
            .source_kind("aws")
            .source_version("0.1.0")
            .observed_at("2026-06-25T12:00:00Z")
            .build()
            .unwrap();
        assert_eq!(p.source.as_str(), "connector");
    }
}
