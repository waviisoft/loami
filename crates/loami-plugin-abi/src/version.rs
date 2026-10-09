//! The entry symbol, the version constants, and the plugin descriptor.
//!
//! Loading proceeds in one direction only: the host resolves [`LOAMI_PLUGIN_V1_SYMBOL`], calls it,
//! and inspects the returned [`LoamiPluginV1`] descriptor *before* it opens anything. Scheme, ABI
//! minor version, and capabilities are all readable from that descriptor without invoking a single
//! operation, so a host can reject an incompatible plugin before it has any state to unwind.

use core::ffi::{c_char, c_void};

use crate::error::LoamiError;
use crate::memory::LoamiSlice;
use crate::vtable::{LoamiComplete, LoamiInflight, LoamiProvider};

/// The major version of the ABI this crate defines — the compatibility boundary.
///
/// It appears in the entry symbol's name rather than in the descriptor, so an incompatible plugin
/// simply fails to resolve instead of loading and then misbehaving. A plugin may export several
/// major versions' symbols to serve a range of hosts; the host resolves the newest it supports.
pub const LOAMI_ABI_VERSION_MAJOR: u32 = 1;

/// The minor version of the ABI this crate defines.
///
/// Within a major version, growth is **additive only**: new capability bits, new status codes, new
/// vtable entries appended to the end. A plugin reports the minor version it was built against in
/// [`LoamiPluginV1::abi_minor`], and the two sides intersect — a host uses only what both
/// understand. A host must accept a plugin whose minor version is *higher* than its own, ignoring
/// what it does not recognize.
pub const LOAMI_ABI_VERSION_MINOR: u32 = 0;

/// The symbol a v1 plugin exports, NUL-terminated for `dlsym`.
///
/// Its type is [`LoamiPluginEntry`]. In C:
///
/// ```c
/// const LoamiPluginV1* loami_plugin_v1(void);
/// ```
pub const LOAMI_PLUGIN_V1_SYMBOL: &[u8] = b"loami_plugin_v1\0";

/// Capability bit: the provider implements conditional writes — [`LoamiPutMode::Create`] and
/// [`LoamiPutMode::Update`] — natively, atomically, against its backend.
///
/// Without this bit, the engine knows it must emulate compare-and-swap rather than assuming the
/// backend enforces it. This is the difference the flag exists for: a provider that cannot do
/// atomic CAS should *not* pretend to, because a silently non-atomic CAS is a correctness bug the
/// engine cannot detect at runtime.
///
/// [`LoamiPutMode::Create`]: crate::LoamiPutMode::Create
/// [`LoamiPutMode::Update`]: crate::LoamiPutMode::Update
pub const LOAMI_CAP_CONDITIONAL_WRITE: u64 = 1 << 0;

/// Capability bit: the provider serves range reads natively, without fetching the whole object.
///
/// Its absence is a performance property rather than a correctness one — a host may still call
/// [`LoamiVtable::get_range`](crate::LoamiVtable::get_range) if the function pointer is non-null.
pub const LOAMI_CAP_RANGE_READ: u64 = 1 << 1;

/// Capability bit: the provider can enumerate keys under a prefix.
///
/// A write-only or strictly key-addressed backend may legitimately lack enumeration; the engine
/// avoids the operations that would require it.
pub const LOAMI_CAP_LIST: u64 = 1 << 2;

/// The signature of the exported entry symbol named by [`LOAMI_PLUGIN_V1_SYMBOL`].
///
/// Returns a descriptor with static lifetime — typically a `const` in the plugin, so the host may
/// retain it for as long as the library stays loaded — or null if the plugin declines to load. It
/// is called before any other plugin code and must not block, allocate on the host's behalf, or
/// panic.
pub type LoamiPluginEntry = unsafe extern "C" fn() -> *const LoamiPluginV1;

/// What a plugin declares about itself, and the constructor that opens a provider from a URL.
///
/// Together, [`scheme`](Self::scheme) and [`open`](Self::open) are this ABI's counterpart to
/// `loami_storage::FromUrl` — the same contract of "a provider owns its scheme and knows how to
/// parse the tail of its own connection string", expressed in C.
#[repr(C)]
pub struct LoamiPluginV1 {
    /// The ABI minor version this plugin was built against. See [`LOAMI_ABI_VERSION_MINOR`].
    pub abi_minor: u32,

    /// The connection-string scheme this plugin answers to — the part before `://`, such as
    /// `azure`. A NUL-terminated, static, ASCII string. Counterpart to `FromUrl::SCHEME`.
    ///
    /// Several plugins may claim one scheme (an official S3 provider and an S3-compatible clone);
    /// resolving that is the deployment's business, not the ABI's.
    pub scheme: *const c_char,

    /// A human-readable name for the plugin, for diagnostics. NUL-terminated, static, UTF-8.
    pub name: *const c_char,

    /// The plugin's own release version, for diagnostics — distinct from the ABI version.
    /// NUL-terminated, static, UTF-8.
    pub version: *const c_char,

    /// The `LOAMI_CAP_*` bits this provider supports, OR-ed together.
    ///
    /// A host **ignores bits it does not recognize** instead of rejecting the plugin, which is what
    /// makes adding capabilities in a later minor version safe.
    pub capabilities: u64,

    /// Opens a provider instance from `rest` — everything after `scheme://` in the connection
    /// string, borrowed for the duration of the call. Counterpart to `FromUrl::from_url`.
    ///
    /// Asynchronous like every other operation, because a provider may need I/O to come up: open a
    /// connection, fetch a token, create a container. On success it writes a [`LoamiProvider`] into
    /// `out`; on failure it writes `err`. Never null.
    pub open: Option<
        unsafe extern "C" fn(
            rest: LoamiSlice,
            out: *mut LoamiProvider,
            err: *mut LoamiError,
            complete: LoamiComplete,
            cx: *mut c_void,
        ) -> LoamiInflight,
    >,
}
