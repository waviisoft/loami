//! Status codes and the typed, tagged error payload.
//!
//! Mirrors `loami_storage::StorageError`. Rather than flattening every variant's data into
//! anonymous `a`/`b`/`c` fields, the payload is a tagged union: [`LoamiError::code`] is the tag and
//! selects which member of [`LoamiErrorDetail`] — if any — is initialized. That keeps the structured
//! fields of a rich error (`InvalidRange`'s bounds, `QuotaExceeded`'s limits) typed all the way
//! across the boundary instead of being stringified into the message and re-parsed.

use crate::memory::LoamiOwned;

/// The outcome of an operation, passed to the completion callback and stored in
/// [`LoamiError::code`].
///
/// Non-negative values other than zero are errors and mirror `loami_storage::StorageError`;
/// negative values are non-error terminal outcomes. A host that receives a code it does not
/// recognize treats it as [`Backend`](Self::Backend) rather than failing — this is what lets a
/// later minor version add codes without breaking older hosts.
///
/// Discriminants are **wire values**: they are pinned by the crate's golden tests and may never be
/// renumbered within [`LOAMI_ABI_VERSION_MAJOR`](crate::LOAMI_ABI_VERSION_MAJOR).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoamiStatus {
    /// The operation succeeded; the caller reads the `out` holder.
    Ok = 0,

    /// A [`LoamiList`](crate::LoamiList) iterator has no further batches. Not an error.
    IterEnd = -1,

    /// The operation was cancelled via [`LoamiInflight::cancel`](crate::LoamiInflight::cancel) and
    /// completed without producing a result. Not an error in the backend sense; the host maps it to
    /// a dropped future.
    Cancelled = -2,

    /// No object exists at the requested key. Mirrors `StorageError::NotFound`.
    NotFound = 1,

    /// A [`LoamiPutMode::Create`](crate::LoamiPutMode::Create) write found an existing object.
    /// Mirrors `StorageError::AlreadyExists`.
    AlreadyExists = 2,

    /// A [`LoamiPutMode::Update`](crate::LoamiPutMode::Update) write failed its ETag precondition.
    /// Mirrors `StorageError::Precondition`.
    Precondition = 3,

    /// A range read fell outside the object's bounds. Carries
    /// [`LoamiErrorDetail::invalid_range`]. Mirrors `StorageError::InvalidRange`.
    InvalidRange = 4,

    /// The provider does not support the requested operation. Mirrors `StorageError::Unsupported`.
    ///
    /// A provider should prefer advertising the absence up front through the capability bits on
    /// [`LoamiPluginV1`](crate::LoamiPluginV1), so the engine can adapt before issuing the call
    /// rather than discovering the gap on failure.
    Unsupported = 5,

    /// The key is not well-formed. Mirrors `StorageError::InvalidKey`.
    ///
    /// The host validates keys before dispatching, so a conforming plugin rarely produces this; the
    /// code exists so a provider with stricter native naming rules can still report it, and so the
    /// mapping back to `StorageError` is total.
    InvalidKey = 6,

    /// A write would exceed a capacity limit configured on the provider. Carries
    /// [`LoamiErrorDetail::quota_exceeded`]. Mirrors `StorageError::QuotaExceeded`.
    QuotaExceeded = 7,

    /// An error originating in the underlying backend, or any failure without a more specific code —
    /// including a panic caught at the boundary. Mirrors `StorageError::Backend`. The
    /// [`message`](LoamiError::message) carries the detail.
    Backend = 8,
}

/// The structured payload of a [`LoamiStatus::InvalidRange`] error.
///
/// Mirrors the `start`, `end`, and `size` fields of `StorageError::InvalidRange`. The key is not
/// repeated here: the host already knows which key it dispatched.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoamiInvalidRange {
    /// The requested inclusive start offset.
    pub start: u64,
    /// The requested exclusive end offset.
    pub end: u64,
    /// The object's actual size in bytes.
    pub size: u64,
}

/// The structured payload of a [`LoamiStatus::QuotaExceeded`] error.
///
/// Mirrors the `limit` and `attempted` fields of `StorageError::QuotaExceeded`. Both are `u64` here
/// rather than the Rust side's `usize`, so the wire layout does not change with pointer width.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoamiQuotaExceeded {
    /// The configured byte limit.
    pub limit: u64,
    /// The total bytes the store would hold after the write.
    pub attempted: u64,
}

/// The typed payload of a [`LoamiError`], tagged by [`LoamiError::code`].
///
/// Reading a member other than the one the code selects is undefined behavior. Codes with no
/// structured payload leave the union in the all-zero state, readable through
/// [`zeroed`](Self::zeroed) and produced by [`none`](Self::none).
///
/// # Layout
///
/// [`zeroed`](Self::zeroed) is exactly the size of the largest typed member, so naming it does not
/// change the union's size or alignment and the C declaration — which has only the typed members —
/// stays equivalent. A golden test pins this: adding a member larger than
/// `[u64; 3]` fails CI until `zeroed` grows to match.
#[repr(C)]
#[derive(Clone, Copy)]
pub union LoamiErrorDetail {
    /// The undifferentiated payload, used to read or write the all-zero "no detail" state.
    pub zeroed: [u64; 3],
    /// Valid when the code is [`LoamiStatus::InvalidRange`].
    pub invalid_range: LoamiInvalidRange,
    /// Valid when the code is [`LoamiStatus::QuotaExceeded`].
    pub quota_exceeded: LoamiQuotaExceeded,
}

impl LoamiErrorDetail {
    /// The all-zero payload, for the codes that carry no structured detail.
    #[must_use]
    pub const fn none() -> Self {
        Self { zeroed: [0; 3] }
    }
}

/// An error returned from an operation, written into the caller-allocated `err` holder.
///
/// The host allocates this before dispatching (see [`new`](Self::new)) and reads it only when the
/// completion callback reports a failing [`LoamiStatus`]. The plugin fills it in before completing.
///
/// Mirrors `loami_storage::StorageError`, which the host reconstructs from the
/// [`code`](Self::code), the typed [`detail`](Self::detail), and the optional
/// [`message`](Self::message).
#[repr(C)]
pub struct LoamiError {
    /// Which error occurred, and the tag selecting the live member of [`detail`](Self::detail).
    pub code: LoamiStatus,

    /// Optional human-readable detail, owned by the receiver.
    ///
    /// Absent ([`LoamiOwned::none`]) when the plugin has nothing to add beyond the code. Required
    /// in practice for [`LoamiStatus::Backend`], where the code alone says nothing. The bytes are
    /// UTF-8 and are **not** NUL-terminated — the length is carried by the buffer.
    pub message: LoamiOwned,

    /// The typed payload selected by [`code`](Self::code), or the all-zero state for codes that
    /// carry none.
    pub detail: LoamiErrorDetail,
}

impl LoamiError {
    /// An empty holder for a plugin to fill: [`LoamiStatus::Ok`], no message, no detail.
    ///
    /// This is what a host allocates before dispatching an operation. Its `Ok` code is not a claim
    /// of success — the holder is read only when the completion callback reports failure.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            code: LoamiStatus::Ok,
            message: LoamiOwned::none(),
            detail: LoamiErrorDetail::none(),
        }
    }
}

impl Default for LoamiError {
    fn default() -> Self {
        Self::new()
    }
}
