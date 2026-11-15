//! Domain core for **hearth**, a self-hosting control plane.
//!
//! This crate is deliberately free of async and of host side effects so it can
//! be unit-tested exhaustively:
//!
//! * [`manifest`]: the app manifest format (one YAML file per catalog app).
//! * [`catalog`]: loading and indexing a directory of manifests.
//! * [`capacity`]: deciding whether an app fits on this machine right now.

pub mod backup;
pub mod capacity;
pub mod catalog;
pub mod compose_config;
pub mod discover;
pub mod manifest;
pub mod storage;

pub use capacity::{CapacityPolicy, Fit, HostSnapshot, Verdict};
pub use catalog::{Catalog, CatalogError};
pub use manifest::{AppManifest, AuthPolicy, ManifestError, Resources};
