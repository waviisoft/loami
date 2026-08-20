//! Buffer types and their ownership rules.
//!
//! Two types carry bytes across the boundary, and the distinction between them is the crate's most
//! important invariant: [`LoamiSlice`] is **borrowed**, [`LoamiOwned`] is **owned**.

use core::ffi::c_void;

/// A **borrowed** view of bytes owned by the caller.
///
/// Used for every value passed *into* an operation — keys, prefixes, payloads, expected ETags. The
/// bytes are guaranteed valid only for the duration of the call that received them; a plugin that
/// needs to retain them past that point must copy them.
///
/// A slice with a null `ptr` and zero `len` is the canonical empty slice, produced by
/// [`empty`](Self::empty). Corresponds to a `&[u8]` on the Rust side of either boundary.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct LoamiSlice {
    /// Pointer to the first byte. May be null only when `len` is zero.
    pub ptr: *const u8,
    /// Number of bytes readable from `ptr`.
    pub len: usize,
}

impl LoamiSlice {
    /// The canonical empty slice: a null pointer with zero length.
    ///
    /// Used wherever the C contract wants "no value" for a borrowed input, such as
    /// [`LoamiPutOptions::expected_etag`](crate::LoamiPutOptions::expected_etag) when the mode is
    /// not [`LoamiPutMode::Update`](crate::LoamiPutMode::Update).
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            ptr: core::ptr::null(),
            len: 0,
        }
    }

    /// Returns whether the slice carries no bytes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// An **owned** buffer that carries its own destructor.
///
/// Used for every value returned *out* of an operation — object bodies, keys, ETags, error
/// messages. The receiver owns the buffer and must release it by calling
/// [`drop`](Self::drop) with [`owner`](Self::owner), exactly once.
///
/// # Why the embedded destructor
///
/// A plugin and its host are separate dynamic objects and may be linked against different
/// allocators; freeing a plugin's allocation with the host's `free` is undefined behavior. Carrying
/// the destructor alongside the pointer means **whoever allocated the buffer also frees it**,
/// whatever allocator that was. It also lets a plugin hand out a buffer it does not own outright —
/// a slice of a memory-mapped file, a reference-counted chunk — by making `owner` the thing that
/// actually needs releasing, which need not be `ptr`.
///
/// # Absence
///
/// An all-null `LoamiOwned` — produced by [`none`](Self::none) — represents an absent value, used
/// for optional outputs such as [`LoamiError::message`](crate::LoamiError::message). A receiver
/// checks [`is_none`](Self::is_none) before dereferencing, and must not invoke the destructor of an
/// absent buffer.
#[repr(C)]
#[derive(Debug)]
pub struct LoamiOwned {
    /// Pointer to the first byte, or null if the value is absent.
    pub ptr: *const u8,
    /// Number of bytes readable from `ptr`.
    pub len: usize,
    /// Opaque handle passed to [`drop`](Self::drop). Not necessarily equal to `ptr`: it identifies
    /// the allocation or reference count backing the bytes, which may be a larger object.
    pub owner: *mut c_void,
    /// Releases `owner`. Null only when the value is absent. Called exactly once by the receiver,
    /// and never for an absent buffer.
    pub drop: Option<unsafe extern "C" fn(owner: *mut c_void)>,
}

impl LoamiOwned {
    /// The canonical absent value: every field null or zero.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            ptr: core::ptr::null(),
            len: 0,
            owner: core::ptr::null_mut(),
            drop: None,
        }
    }

    /// Returns whether the buffer is absent — that is, carries no value at all.
    ///
    /// Distinct from carrying an empty-but-present buffer, which has a non-null `owner` and a live
    /// destructor that must still be invoked.
    ///
    /// Not a `const fn`: `<*const T>::is_null` only became const-callable in Rust 1.84, after this
    /// workspace's minimum supported version.
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.ptr.is_null() && self.owner.is_null()
    }
}
