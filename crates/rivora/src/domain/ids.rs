//! Stable identifiers for Engineering Objects.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{RivoraError, RivoraResult};

macro_rules! define_id {
    ($name:ident, $label:literal) => {
        /// Stable unique identifier.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generate a new random identifier.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Create from a UUID.
            pub fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            /// Borrow the inner UUID.
            pub fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = RivoraError;

            fn from_str(s: &str) -> RivoraResult<Self> {
                Uuid::parse_str(s)
                    .map(Self)
                    .map_err(|e| RivoraError::validation(format!("invalid {}: {e}", $label)))
            }
        }
    };
}

define_id!(InvestigationId, "investigation id");
define_id!(ObjectId, "object id");
