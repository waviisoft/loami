//! # loami-storage
//!
//! The storage **provider contract** for Loami.
//!
//! Loami's engine never talks to a concrete object store directly. Instead it depends on the
//! [`StorageProvider`] trait defined here, and a concrete backend (in-memory, local filesystem,
//! Azure Blob, …) is selected at the edge and handed to the engine as an `Arc<dyn StorageProvider>`.
//! Each backend lives in its own crate and must pass the [`conformance`] suite, which makes the
//! backends interchangeable.
//!
//! This crate intentionally defines its own trait and types rather than re-exporting a third-party
//! object-store API, so the contract stays stable while provider implementations are free to evolve
//! (or be swapped entirely) underneath it.
//!
//! ## Conditional writes
//!
//! [`PutMode`] exposes the optimistic-concurrency primitives Loami relies on:
//! [`PutMode::Create`] (write only if absent) and [`PutMode::Update`] (write only if the current
//! ETag matches). Every provider must implement these with the semantics asserted by the
//! conformance suite.

#[cfg(feature = "conformance")]
pub mod conformance;
mod error;
mod provider;
mod types;

pub use error::{Result, StorageError};
pub use provider::StorageProvider;
pub use types::{
    validate_segment, Etag, GetResult, ObjectKey, ObjectMeta, PutMode, PutOptions, PutResult,
};
