//! Foundational domain identifiers for Open Rivora.
//!
//! Each identifier is a [`rivora_types::TypedId`] tagged by a zero-sized
//! marker that implements [`rivora_types::IdTag`]. The type aliases below are
//! the *vocabulary* the rest of Rivora will use; they are distinct types so an
//! [`ObservationId`] can never be passed where a [`ReceiptId`] is expected.

use rivora_types::{IdTag, TypedId};

macro_rules! id_tag {
    ($marker:ident, $kind:literal) => {
        /// Zero-sized marker identifying a domain entity family.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $marker;
        impl IdTag for $marker {
            const KIND: &'static str = $kind;
        }
    };
}

id_tag!(Observation, "observation");
id_tag!(Ability, "ability");
id_tag!(Receipt, "receipt");
id_tag!(Service, "service");
id_tag!(Deployment, "deployment");
id_tag!(Incident, "incident");
id_tag!(Context, "context");
id_tag!(Organization, "organization");

/// Unique identifier for an observation emitted by a connector.
pub type ObservationId = TypedId<Observation>;
/// Unique identifier for an Ability (versioned organizational knowledge).
pub type AbilityId = TypedId<Ability>;
/// Unique identifier for a reliability receipt.
pub type ReceiptId = TypedId<Receipt>;
/// Unique identifier for an observed service.
pub type ServiceId = TypedId<Service>;
/// Unique identifier for a deployment event.
pub type DeploymentId = TypedId<Deployment>;
/// Unique identifier for a reliability incident.
pub type IncidentId = TypedId<Incident>;
/// Unique identifier for a context-graph node or edge.
pub type ContextId = TypedId<Context>;
/// Unique identifier for an organization (the owner of local memory).
pub type OrganizationId = TypedId<Organization>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_named_ids_construct_and_validate() {
        let obs = ObservationId::new("obs_1").unwrap();
        let ability = AbilityId::new("ability_1").unwrap();
        let receipt = ReceiptId::new("receipt_1").unwrap();
        let service = ServiceId::new("svc_1").unwrap();
        let deploy = DeploymentId::new("deploy_1").unwrap();
        let incident = IncidentId::new("inc_1").unwrap();
        let ctx = ContextId::new("ctx_1").unwrap();
        let org = OrganizationId::new("org_1").unwrap();

        assert_eq!(obs.as_str(), "obs_1");
        assert_eq!(ability.as_str(), "ability_1");
        assert_eq!(receipt.as_str(), "receipt_1");
        assert_eq!(service.as_str(), "svc_1");
        assert_eq!(deploy.as_str(), "deploy_1");
        assert_eq!(incident.as_str(), "inc_1");
        assert_eq!(ctx.as_str(), "ctx_1");
        assert_eq!(org.as_str(), "org_1");
    }

    #[test]
    fn kinded_strings_are_prefixed() {
        assert_eq!(
            ReceiptId::new("r1").unwrap().to_kinded_string(),
            "receipt:r1"
        );
        assert_eq!(
            OrganizationId::new("o1").unwrap().to_kinded_string(),
            "organization:o1"
        );
    }

    #[test]
    fn random_ids_are_valid() {
        let id = ServiceId::new_random();
        assert!(ServiceId::new(id.as_str()).is_ok());
    }

    #[test]
    fn distinct_id_types_do_not_compare() {
        // Compile-time: these are different types. We only compare inner values.
        let a = ServiceId::new("x").unwrap();
        let b = IncidentId::new("x").unwrap();
        assert_eq!(a.as_str(), b.as_str());
    }
}
