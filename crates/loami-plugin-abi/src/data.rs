//! The value types an operation reads and returns.
//!
//! Each mirrors a type from `loami-storage`, with owned outputs expressed as [`LoamiOwned`] and
//! borrowed inputs as [`LoamiSlice`](crate::LoamiSlice).

use crate::memory::{LoamiOwned, LoamiSlice};

/// How a put should behave with respect to any existing object. Mirrors `loami_storage::PutMode`.
///
/// The Rust enum carries the expected ETag inside its `Update` variant; a C enum cannot carry data,
/// so the ETag travels beside the mode in [`LoamiPutOptions::expected_etag`].
///
/// Discriminants are wire values, pinned by the crate's golden tests.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LoamiPutMode {
    /// Unconditionally write, replacing any existing object.
    #[default]
    Overwrite = 0,
    /// Write only if no object currently exists at the key. Conflict yields
    /// [`LoamiStatus::AlreadyExists`](crate::LoamiStatus::AlreadyExists).
    Create = 1,
    /// Write only if the current ETag equals
    /// [`LoamiPutOptions::expected_etag`] (compare-and-swap). Mismatch yields
    /// [`LoamiStatus::Precondition`](crate::LoamiStatus::Precondition).
    Update = 2,
}

/// Options controlling a put. Mirrors `loami_storage::PutOptions`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct LoamiPutOptions {
    /// The conditional-write mode.
    pub mode: LoamiPutMode,
    /// The ETag the caller expects the current object to carry.
    ///
    /// Read only when [`mode`](Self::mode) is [`LoamiPutMode::Update`]; otherwise it is
    /// [`LoamiSlice::empty`] and must be ignored. Borrowed for the duration of the call.
    pub expected_etag: LoamiSlice,
}

impl LoamiPutOptions {
    /// Unconditional overwrite — the default mode with no expected ETag.
    #[must_use]
    pub const fn overwrite() -> Self {
        Self {
            mode: LoamiPutMode::Overwrite,
            expected_etag: LoamiSlice::empty(),
        }
    }
}

impl Default for LoamiPutOptions {
    fn default() -> Self {
        Self::overwrite()
    }
}

/// Metadata describing a stored object. Mirrors `loami_storage::ObjectMeta`.
///
/// Returned by `head`, and in batches by [`LoamiList`](crate::LoamiList). Both owned buffers belong
/// to the receiver once the operation completes.
#[repr(C)]
#[derive(Debug)]
pub struct LoamiObjectMeta {
    /// The object's key, as UTF-8 bytes without a trailing NUL.
    pub key: LoamiOwned,

    /// The object's size in bytes.
    ///
    /// For a range read this is the size of the **whole object**, not the length of the returned
    /// slice — matching `GetResult::meta` in the static contract.
    pub size: u64,

    /// The object's current ETag, as opaque bytes. Compared only for equality; the format is
    /// provider-defined.
    pub etag: LoamiOwned,

    /// When the object was last modified, in nanoseconds since the Unix epoch. Read only when
    /// [`has_last_modified`](Self::has_last_modified) is `true`.
    pub last_modified_unix_nanos: i64,

    /// Whether [`last_modified_unix_nanos`](Self::last_modified_unix_nanos) holds a value.
    ///
    /// Together the two fields encode the static contract's `Option<SystemTime>`, which has no C
    /// equivalent. Laid out as a C99 `bool` — one byte, where `0` is false and `1` is true; no other
    /// bit pattern is valid.
    pub has_last_modified: bool,
}

/// The result of a read: the bytes together with the object's metadata. Mirrors
/// `loami_storage::GetResult`.
///
/// Metadata travels with the body so a caller captures the ETag of *exactly* the bytes it read,
/// without a separate `head` call and without the read-then-head race that would let the ETag drift
/// before a follow-up conditional write.
#[repr(C)]
#[derive(Debug)]
pub struct LoamiGetResult {
    /// The bytes read — the whole object for `get`, or the requested slice for `get_range`.
    pub data: LoamiOwned,
    /// Metadata for the object as a whole.
    pub meta: LoamiObjectMeta,
}

/// The outcome of a successful put. Mirrors `loami_storage::PutResult`.
#[repr(C)]
#[derive(Debug)]
pub struct LoamiPutResult {
    /// The ETag of the newly written object, for use in a subsequent conditional update.
    pub etag: LoamiOwned,
}
