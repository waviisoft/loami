//! The provider handle, its operation vtable, and the completion machinery.
//!
//! [`LoamiVtable`] is the ABI's counterpart to `loami_storage::StorageProvider`. Every operation
//! follows the same completion-based shape described at the [crate root](crate): it returns a
//! [`LoamiInflight`] immediately and fires a [`LoamiComplete`] callback exactly once when the work
//! finishes.

use core::ffi::c_void;

use crate::data::{LoamiGetResult, LoamiObjectMeta, LoamiPutOptions, LoamiPutResult};
use crate::error::{LoamiError, LoamiStatus};
use crate::memory::LoamiSlice;

/// The callback a plugin invokes exactly once to signal that an operation has finished.
///
/// `cx` is the opaque context the host supplied when it issued the operation; the plugin passes it
/// back unchanged. `status` selects which holder the host reads: [`LoamiStatus::Ok`] means the `out`
/// holder is initialized, anything else means the `err` holder is.
///
/// May be invoked from any thread, including — for an operation that completes without yielding —
/// the same thread that issued it, before the issuing call has returned. A host must therefore be
/// prepared for reentrant completion.
pub type LoamiComplete = unsafe extern "C" fn(cx: *mut c_void, status: LoamiStatus);

/// A handle to an operation that has been issued and not yet completed.
///
/// Returned synchronously by every operation. Dropping the corresponding host-side future calls
/// [`cancel`](Self::cancel); the operation must still complete afterwards, reporting
/// [`LoamiStatus::Cancelled`] if it did not otherwise finish. Cancellation is therefore a *request*,
/// not a guarantee — an operation already past its point of no return may complete successfully.
#[repr(C)]
#[derive(Debug)]
pub struct LoamiInflight {
    /// Opaque plugin-side handle for the in-flight operation, passed back to
    /// [`cancel`](Self::cancel).
    pub op: *mut c_void,
    /// Requests cancellation of `op`. Null if the operation cannot be cancelled, in which case the
    /// host waits for normal completion. Called at most once, and never after the completion
    /// callback has fired.
    pub cancel: Option<unsafe extern "C" fn(op: *mut c_void)>,
}

/// A live provider instance: opaque plugin state plus the vtable that operates on it.
///
/// Produced by [`LoamiPluginV1::open`](crate::LoamiPluginV1::open) and wrapped host-side in an
/// `impl StorageProvider`, so the engine sees no difference between this and a statically linked
/// provider.
#[repr(C)]
#[derive(Debug)]
pub struct LoamiProvider {
    /// Opaque plugin-owned state, passed as the first argument to every vtable function and
    /// released by [`LoamiVtable::destroy`].
    pub state: *mut c_void,
    /// The operations available on `state`. Points to a table with static lifetime, typically a
    /// `const` in the plugin, so the host may retain it for as long as the provider lives.
    pub vtable: *const LoamiVtable,
}

/// The operation table backing a [`LoamiProvider`]. Mirrors `loami_storage::StorageProvider`.
///
/// Every function takes the provider's `state`, the operation's inputs, caller-allocated `out` and
/// `err` holders, a completion callback, and its context — and returns a [`LoamiInflight`].
///
/// A null function pointer means the provider does not implement that operation; the host reports
/// [`LoamiStatus::Unsupported`] without calling into the plugin. `destroy` may never be null. A
/// provider should also declare what it supports through the capability bits on
/// [`LoamiPluginV1`](crate::LoamiPluginV1), so the engine can adapt before dispatching rather than
/// after failing.
///
/// Fields are ordered to match the C declaration and are **append-only**: a later minor version may
/// add operations at the end, and a host that knows only the earlier layout keeps working because
/// it never reads past the fields it knows. Reordering is a major-version break.
#[repr(C)]
pub struct LoamiVtable {
    /// Reads the full contents of the object at `key`, with its metadata. Missing object yields
    /// [`LoamiStatus::NotFound`].
    pub get: Option<
        unsafe extern "C" fn(
            state: *mut c_void,
            key: LoamiSlice,
            out: *mut LoamiGetResult,
            err: *mut LoamiError,
            complete: LoamiComplete,
            cx: *mut c_void,
        ) -> LoamiInflight,
    >,

    /// Reads the half-open byte range `start..end` of the object at `key`, with the metadata of the
    /// whole object. A range outside the object's bounds yields [`LoamiStatus::InvalidRange`] and
    /// its typed detail.
    pub get_range: Option<
        unsafe extern "C" fn(
            state: *mut c_void,
            key: LoamiSlice,
            start: u64,
            end: u64,
            out: *mut LoamiGetResult,
            err: *mut LoamiError,
            complete: LoamiComplete,
            cx: *mut c_void,
        ) -> LoamiInflight,
    >,

    /// Returns metadata for the object at `key` without its body.
    pub head: Option<
        unsafe extern "C" fn(
            state: *mut c_void,
            key: LoamiSlice,
            out: *mut LoamiObjectMeta,
            err: *mut LoamiError,
            complete: LoamiComplete,
            cx: *mut c_void,
        ) -> LoamiInflight,
    >,

    /// Writes `data` to `key` according to `options`, returning the new object's ETag.
    pub put: Option<
        unsafe extern "C" fn(
            state: *mut c_void,
            key: LoamiSlice,
            data: LoamiSlice,
            options: LoamiPutOptions,
            out: *mut LoamiPutResult,
            err: *mut LoamiError,
            complete: LoamiComplete,
            cx: *mut c_void,
        ) -> LoamiInflight,
    >,

    /// Deletes the object at `key`. Idempotent: deleting a key that does not exist succeeds. Named
    /// `del` because `delete` is a keyword in C++ and other consumers of this header.
    pub del: Option<
        unsafe extern "C" fn(
            state: *mut c_void,
            key: LoamiSlice,
            err: *mut LoamiError,
            complete: LoamiComplete,
            cx: *mut c_void,
        ) -> LoamiInflight,
    >,

    /// Opens an iterator over the metadata of every object under `prefix`, matched on `/`-segment
    /// boundaries rather than as a raw string prefix.
    ///
    /// Opening the iterator is itself asynchronous; the host then drives
    /// [`LoamiList::next_batch`] and presents the engine a lazy stream.
    pub list: Option<
        unsafe extern "C" fn(
            state: *mut c_void,
            prefix: LoamiSlice,
            out: *mut LoamiList,
            err: *mut LoamiError,
            complete: LoamiComplete,
            cx: *mut c_void,
        ) -> LoamiInflight,
    >,

    /// Releases the provider's `state`. Called exactly once, after every in-flight operation has
    /// completed. Never null.
    pub destroy: Option<unsafe extern "C" fn(state: *mut c_void)>,
}

/// An open iterator over object metadata, produced by [`LoamiVtable::list`].
///
/// Batched and pull-based: the host asks for up to `cap` entries at a time, so the cost of crossing
/// the boundary is amortized while memory stays bounded and the caller can still stop early. The
/// host presents it to the engine as the `BoxStream` that `StorageProvider::list` returns.
#[repr(C)]
pub struct LoamiList {
    /// Opaque plugin-owned iterator state, released by [`destroy`](Self::destroy).
    pub state: *mut c_void,

    /// Fills up to `cap` entries into the caller-owned `out` array.
    ///
    /// On success it writes the number of entries produced to `filled`, and sets `done` to `true`
    /// once the iterator is exhausted — which may happen on the same call that returns the final
    /// entries, so a host must consume `filled` even when `done` is set. Each
    /// [`LoamiObjectMeta`] written is owned by the host, which releases its buffers after use.
    ///
    /// Asynchronous like every other operation, completing through the callback. Never null.
    pub next_batch: Option<
        unsafe extern "C" fn(
            state: *mut c_void,
            out: *mut LoamiObjectMeta,
            cap: usize,
            filled: *mut usize,
            done: *mut bool,
            err: *mut LoamiError,
            complete: LoamiComplete,
            cx: *mut c_void,
        ) -> LoamiInflight,
    >,

    /// Releases the iterator's `state`. Called exactly once, including when the host abandons the
    /// iterator before exhausting it. Never null.
    pub destroy: Option<unsafe extern "C" fn(state: *mut c_void)>,
}
