//! # loami-plugin-abi
//!
//! The **stable C ABI** that every Loami storage plugin is built against.
//!
//! Loami's engine knows only the `mem://` scheme; every other storage backend is a separately
//! distributed artifact that registers itself by connection-string scheme — the JDBC / Go
//! `database/sql` driver model. This crate defines the binary contract at that seam: a
//! [`LoamiPluginV1`] descriptor returned from a versioned entry symbol, a [`LoamiVtable`] of
//! `extern "C"` operations, and the `#[repr(C)]` value types that cross the boundary.
//!
//! It is **definitions only** — no loader, no marshaling, no runtime. Those live in the two sibling
//! crates:
//!
//! | Crate | Role |
//! |---|---|
//! | `loami-plugin-abi` (this crate) | the `#[repr(C)]` contract, version and capability constants |
//! | `loami-plugin-host` | `dlopen` loader, verification, completion→future bridging |
//! | `loami-plugin-sdk` | author-side wrapper generating this ABI from a `StorageProvider` impl |
//!
//! ## Relationship to the static contract
//!
//! This ABI mirrors `loami-storage`'s [`StorageProvider`] and `FromUrl` traits so that **one
//! provider source builds both ways**: a normal `rlib` registered in-process via `Registry::add`,
//! and a `cdylib` plugin loaded over this ABI. The host normalizes both to
//! `Arc<dyn StorageProvider>`, so nothing above the registry can tell which path a provider took.
//!
//! Each type below documents the Rust type it corresponds to:
//!
//! | This ABI | `loami-storage` |
//! |---|---|
//! | [`LoamiPluginV1::scheme`] / [`LoamiPluginV1::open`] | `FromUrl::SCHEME` / `FromUrl::from_url` |
//! | [`LoamiVtable`] | `StorageProvider` |
//! | [`LoamiObjectMeta`] | `ObjectMeta` |
//! | [`LoamiGetResult`] / [`LoamiPutResult`] | `GetResult` / `PutResult` |
//! | [`LoamiPutOptions`] / [`LoamiPutMode`] | `PutOptions` / `PutMode` |
//! | [`LoamiStatus`] / [`LoamiError`] | `StorageError` |
//! | [`LoamiList`] | the `BoxStream` returned by `StorageProvider::list` |
//!
//! [`StorageProvider`]: https://docs.rs/loami-storage
//!
//! ## Async model
//!
//! Rust futures cannot cross a `dlopen` boundary, so every operation is **completion-based** — the
//! language-neutral async pattern. An operation:
//!
//! 1. takes caller-allocated `out` and `err` holders, a [`LoamiComplete`] callback, and a `cx`
//!    context pointer;
//! 2. returns immediately with a [`LoamiInflight`] handle;
//! 3. performs the work on the plugin's own runtime or threads, writes `*out` **or** `*err`, then
//!    invokes `complete(cx, status)` **exactly once**.
//!
//! The host bridges that callback to a oneshot channel and presents a normal Rust future. Dropping
//! the future calls [`LoamiInflight::cancel`]; the plugin must still complete the operation, with
//! [`LoamiStatus::Cancelled`].
//!
//! ## Normative rules
//!
//! These are binding on both sides of the boundary. A plugin that violates them is malformed, and
//! the resulting behavior is undefined.
//!
//! - **Borrow on input, own on output.** Every [`LoamiSlice`] passed *into* an operation is
//!   borrowed and valid only for the duration of that call; a plugin that needs to retain the bytes
//!   copies them. Every [`LoamiOwned`] returned *out* of an operation is owned by the receiver and
//!   is released through its own embedded [`drop`](LoamiOwned::drop) — never through the receiver's
//!   allocator, because the two sides of a dynamic library boundary may not share one.
//! - **Completion fires exactly once,** including on cancellation. The `out` and `err` holders must
//!   remain valid until it fires; the host guarantees this.
//! - **Success writes `out`; failure writes `err`.** The status passed to the completion callback
//!   selects which one the host reads — never both.
//! - **No panic and no foreign exception crosses the boundary.** The SDK wraps every `extern "C"`
//!   body in `catch_unwind` and maps an unwind to [`LoamiStatus::Backend`].
//! - **Operations may be issued concurrently** on one provider instance; a provider is effectively
//!   `Send + Sync`.
//! - **Only C-compatible types cross:** `#[repr(C)]` structs and unions, fixed-width integers, and
//!   pointers. No Rust enums carrying data, no `&str`, no `Box<dyn Trait>`, no trait objects.
//! - **Unknown bits and unknown status codes are tolerated,** not fatal: a host ignores capability
//!   bits it does not recognize, and treats an unrecognized status as
//!   [`LoamiStatus::Backend`]. This is what makes additive minor-version growth safe.
//!
//! ## Security
//!
//! Loading a plugin executes arbitrary native code in-process. Verification is therefore the
//! *host's* responsibility and happens **before** `dlopen`: plugins load only from allowlisted
//! locations and are checked against a trusted manifest. Nothing in this crate performs that check —
//! it defines only what a verified plugin must look like once loaded.
//!
//! ## Stability
//!
//! [`LOAMI_ABI_VERSION_MAJOR`] is the compatibility boundary: within it, growth is **additive
//! only** and signaled by [`LoamiPluginV1::abi_minor`]. Reordering a struct field, renumbering a
//! [`LoamiStatus`] discriminant, or changing a function signature is a breaking change requiring a
//! new major version and a new entry symbol. The crate's golden tests pin the wire values so such a
//! change fails CI rather than silently corrupting a loaded plugin.

#![no_std]

mod data;
mod error;
mod memory;
mod version;
mod vtable;

pub use data::{LoamiGetResult, LoamiObjectMeta, LoamiPutMode, LoamiPutOptions, LoamiPutResult};
pub use error::{LoamiError, LoamiErrorDetail, LoamiInvalidRange, LoamiQuotaExceeded, LoamiStatus};
pub use memory::{LoamiOwned, LoamiSlice};
pub use version::{
    LoamiPluginEntry, LoamiPluginV1, LOAMI_ABI_VERSION_MAJOR, LOAMI_ABI_VERSION_MINOR,
    LOAMI_CAP_CONDITIONAL_WRITE, LOAMI_CAP_LIST, LOAMI_CAP_RANGE_READ, LOAMI_PLUGIN_V1_SYMBOL,
};
pub use vtable::{LoamiComplete, LoamiInflight, LoamiList, LoamiProvider, LoamiVtable};
