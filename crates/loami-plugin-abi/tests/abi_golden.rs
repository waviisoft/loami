//! Golden tests pinning the ABI's wire contract.
//!
//! This crate has no behavior to exercise — it is a set of definitions — so what needs testing is
//! that those definitions *do not move*. A plugin compiled against one version of this crate and a
//! host compiled against another agree only if the discriminants, bit values, symbol name, and
//! struct layouts match exactly; a mismatch is not a compile error, it is silent memory corruption
//! at runtime.
//!
//! These tests therefore assert the contract's observable values so that any accidental
//! renumbering, reordering, or field insertion fails CI instead of shipping. Changing an assertion
//! here is a deliberate act: within
//! [`LOAMI_ABI_VERSION_MAJOR`](loami_plugin_abi::LOAMI_ABI_VERSION_MAJOR) the only legitimate reason
//! is an additive change that appends without disturbing what came before.

use core::mem::{align_of, offset_of, size_of};

use loami_plugin_abi::{
    LoamiComplete, LoamiError, LoamiErrorDetail, LoamiGetResult, LoamiInflight, LoamiInvalidRange,
    LoamiList, LoamiObjectMeta, LoamiOwned, LoamiPluginV1, LoamiProvider, LoamiPutMode,
    LoamiPutOptions, LoamiPutResult, LoamiQuotaExceeded, LoamiSlice, LoamiStatus, LoamiVtable,
    LOAMI_ABI_VERSION_MAJOR, LOAMI_ABI_VERSION_MINOR, LOAMI_CAP_CONDITIONAL_WRITE, LOAMI_CAP_LIST,
    LOAMI_CAP_RANGE_READ, LOAMI_PLUGIN_V1_SYMBOL,
};

// ---------------------------------------------------------------------------
// Version and entry symbol
// ---------------------------------------------------------------------------

#[test]
fn abi_version_is_v1_0() {
    assert_eq!(LOAMI_ABI_VERSION_MAJOR, 1);
    assert_eq!(LOAMI_ABI_VERSION_MINOR, 0);
}

#[test]
fn entry_symbol_is_nul_terminated_and_names_the_major_version() {
    assert_eq!(LOAMI_PLUGIN_V1_SYMBOL, b"loami_plugin_v1\0");

    let (last, name) = LOAMI_PLUGIN_V1_SYMBOL
        .split_last()
        .expect("symbol must not be empty");
    assert_eq!(*last, 0, "the symbol must be NUL-terminated for dlsym");
    assert!(
        !name.contains(&0),
        "the symbol must contain exactly one NUL, at the end"
    );

    // The major version lives in the symbol name, so an incompatible plugin fails to resolve
    // rather than loading and then misbehaving. If the major version is ever bumped, this crate
    // gains a *new* symbol — it does not rename this one.
    let expected = format!("loami_plugin_v{LOAMI_ABI_VERSION_MAJOR}");
    assert_eq!(name, expected.as_bytes());
}

// ---------------------------------------------------------------------------
// Capability bits
// ---------------------------------------------------------------------------

#[test]
fn capability_bits_are_stable_and_disjoint() {
    assert_eq!(LOAMI_CAP_CONDITIONAL_WRITE, 1);
    assert_eq!(LOAMI_CAP_RANGE_READ, 2);
    assert_eq!(LOAMI_CAP_LIST, 4);

    let all = [
        LOAMI_CAP_CONDITIONAL_WRITE,
        LOAMI_CAP_RANGE_READ,
        LOAMI_CAP_LIST,
    ];
    for (i, a) in all.iter().enumerate() {
        assert_eq!(a.count_ones(), 1, "each capability is a single bit");
        for b in &all[i + 1..] {
            assert_eq!(a & b, 0, "capability bits must not overlap");
        }
    }
}

// ---------------------------------------------------------------------------
// Status codes
// ---------------------------------------------------------------------------

#[test]
fn status_discriminants_are_wire_values() {
    // Non-error outcomes.
    assert_eq!(LoamiStatus::Ok as i32, 0);
    assert_eq!(LoamiStatus::IterEnd as i32, -1);
    assert_eq!(LoamiStatus::Cancelled as i32, -2);

    // Errors, mirroring StorageError.
    assert_eq!(LoamiStatus::NotFound as i32, 1);
    assert_eq!(LoamiStatus::AlreadyExists as i32, 2);
    assert_eq!(LoamiStatus::Precondition as i32, 3);
    assert_eq!(LoamiStatus::InvalidRange as i32, 4);
    assert_eq!(LoamiStatus::Unsupported as i32, 5);
    assert_eq!(LoamiStatus::InvalidKey as i32, 6);
    assert_eq!(LoamiStatus::QuotaExceeded as i32, 7);
    assert_eq!(LoamiStatus::Backend as i32, 8);
}

#[test]
fn status_is_a_four_byte_signed_int() {
    // Negative discriminants require a signed repr; C consumers declare this as an `int`-typed
    // enum, so the width must not drift.
    assert_eq!(size_of::<LoamiStatus>(), 4);
    assert_eq!(align_of::<LoamiStatus>(), align_of::<i32>());
}

#[test]
fn put_mode_discriminants_are_wire_values() {
    assert_eq!(LoamiPutMode::Overwrite as i32, 0);
    assert_eq!(LoamiPutMode::Create as i32, 1);
    assert_eq!(LoamiPutMode::Update as i32, 2);
    assert_eq!(size_of::<LoamiPutMode>(), 4);

    // Overwrite is the default on both sides of the boundary, matching `PutMode`'s `#[default]`.
    assert_eq!(LoamiPutMode::default(), LoamiPutMode::Overwrite);
}

// ---------------------------------------------------------------------------
// Nullable function pointers
// ---------------------------------------------------------------------------

#[test]
fn optional_fn_pointers_are_plain_pointers() {
    // The whole vtable models "operation not implemented" as a null function pointer, which is only
    // sound because `Option<fn>` is guaranteed to use the null-pointer optimization. If this ever
    // stopped holding, every vtable would silently gain padding and no longer match its C
    // declaration.
    assert_eq!(
        size_of::<Option<LoamiComplete>>(),
        size_of::<LoamiComplete>()
    );
    assert_eq!(size_of::<Option<LoamiComplete>>(), size_of::<*const ()>());
}

// ---------------------------------------------------------------------------
// The error payload union
// ---------------------------------------------------------------------------

#[test]
fn error_detail_covers_every_typed_payload() {
    // `zeroed` exists so the all-zero "no detail" state is nameable without touching a typed
    // member. It must be exactly as large as the largest payload: smaller, and it cannot zero the
    // whole union; larger, and it would inflate the union past its C declaration.
    assert_eq!(size_of::<LoamiErrorDetail>(), size_of::<[u64; 3]>());
    assert!(size_of::<LoamiErrorDetail>() >= size_of::<LoamiInvalidRange>());
    assert!(size_of::<LoamiErrorDetail>() >= size_of::<LoamiQuotaExceeded>());
    assert_eq!(align_of::<LoamiErrorDetail>(), align_of::<u64>());
}

#[test]
fn error_detail_none_is_all_zero() {
    let detail = LoamiErrorDetail::none();

    // SAFETY: `zeroed` is the union member covering the full payload, and `none` initializes
    // exactly that member.
    let raw = unsafe { detail.zeroed };
    assert_eq!(raw, [0; 3]);

    // Reading a typed member of the all-zero payload is well-defined for these plain-integer
    // structs, and yields the zeroed struct.
    // SAFETY: every member is a `#[repr(C)]` struct of integers, fully initialized by `none`.
    let range = unsafe { detail.invalid_range };
    assert_eq!(
        range,
        LoamiInvalidRange {
            start: 0,
            end: 0,
            size: 0
        }
    );
}

#[test]
fn error_holder_starts_empty() {
    let err = LoamiError::new();
    assert_eq!(err.code, LoamiStatus::Ok);
    assert!(err.message.is_none(), "a fresh holder carries no message");

    // SAFETY: `new` initializes the payload through `LoamiErrorDetail::none`, i.e. `zeroed`.
    assert_eq!(unsafe { err.detail.zeroed }, [0; 3]);
}

// ---------------------------------------------------------------------------
// Buffer sentinels
// ---------------------------------------------------------------------------

#[test]
fn empty_slice_is_null_and_zero_length() {
    let slice = LoamiSlice::empty();
    assert!(slice.ptr.is_null());
    assert_eq!(slice.len, 0);
    assert!(slice.is_empty());
}

#[test]
fn absent_owned_buffer_has_no_destructor() {
    let owned = LoamiOwned::none();
    assert!(owned.is_none());
    assert!(owned.ptr.is_null());
    assert_eq!(owned.len, 0);
    assert!(owned.owner.is_null());
    assert!(
        owned.drop.is_none(),
        "an absent buffer must carry no destructor, so a receiver never invokes one"
    );
}

#[test]
fn default_put_options_are_an_unconditional_overwrite() {
    let options = LoamiPutOptions::default();
    assert_eq!(options.mode, LoamiPutMode::Overwrite);
    assert!(
        options.expected_etag.is_empty(),
        "a non-Update mode must leave the expected ETag empty"
    );
}

// ---------------------------------------------------------------------------
// Field order
// ---------------------------------------------------------------------------
//
// Sizes alone cannot catch a swap of two same-width fields, which is exactly the kind of edit that
// compiles cleanly and corrupts every loaded plugin. These pin the order itself.

#[test]
fn struct_field_order_is_pinned() {
    assert_eq!(offset_of!(LoamiSlice, ptr), 0);

    assert_eq!(offset_of!(LoamiOwned, ptr), 0);
    assert_eq!(offset_of!(LoamiError, code), 0);

    assert_eq!(offset_of!(LoamiInvalidRange, start), 0);
    assert_eq!(offset_of!(LoamiInvalidRange, end), size_of::<u64>());
    assert_eq!(offset_of!(LoamiInvalidRange, size), 2 * size_of::<u64>());

    assert_eq!(offset_of!(LoamiQuotaExceeded, limit), 0);
    assert_eq!(offset_of!(LoamiQuotaExceeded, attempted), size_of::<u64>());

    assert_eq!(offset_of!(LoamiPutOptions, mode), 0);
    assert_eq!(offset_of!(LoamiObjectMeta, key), 0);
    assert_eq!(offset_of!(LoamiGetResult, data), 0);
    assert_eq!(offset_of!(LoamiPutResult, etag), 0);

    assert_eq!(offset_of!(LoamiInflight, op), 0);
    assert_eq!(offset_of!(LoamiProvider, state), 0);
    assert_eq!(offset_of!(LoamiList, state), 0);

    assert_eq!(offset_of!(LoamiPluginV1, abi_minor), 0);

    // The vtable is append-only: these three anchor its head, so an operation inserted in the
    // middle rather than appended fails here.
    assert_eq!(offset_of!(LoamiVtable, get), 0);
    assert_eq!(offset_of!(LoamiVtable, get_range), size_of::<*const ()>());
    assert_eq!(offset_of!(LoamiVtable, head), 2 * size_of::<*const ()>());
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// Every struct in the contract is pointer-aligned, on any target.
#[test]
fn layout_is_pointer_aligned() {
    let word = align_of::<*const ()>();
    assert_eq!(align_of::<LoamiSlice>(), word);
    assert_eq!(align_of::<LoamiOwned>(), word);
    assert_eq!(align_of::<LoamiObjectMeta>(), word);
    assert_eq!(align_of::<LoamiVtable>(), word);
    assert_eq!(align_of::<LoamiPluginV1>(), word);
}

/// Sizes expressed in words rather than bytes, so the assertions hold on any pointer width and
/// still catch an added or removed field.
#[test]
fn layout_is_composed_of_the_expected_fields() {
    let word = size_of::<*const ()>();

    assert_eq!(size_of::<LoamiSlice>(), word + size_of::<usize>());
    assert_eq!(
        size_of::<LoamiOwned>(),
        2 * word + size_of::<usize>() + word
    );

    assert_eq!(size_of::<LoamiInflight>(), 2 * word);
    assert_eq!(size_of::<LoamiProvider>(), 2 * word);
    assert_eq!(size_of::<LoamiList>(), 3 * word);

    // get, get_range, head, put, del, list, destroy.
    assert_eq!(size_of::<LoamiVtable>(), 7 * word);

    assert_eq!(size_of::<LoamiPutResult>(), size_of::<LoamiOwned>());
    assert_eq!(
        size_of::<LoamiGetResult>(),
        size_of::<LoamiOwned>() + size_of::<LoamiObjectMeta>()
    );
}

/// The exact byte layout on the 64-bit targets Loami ships and tests on. Pinned separately from the
/// portable assertions above so a layout regression names a concrete number, and so a future 32-bit
/// target does not have to weaken the check for everyone else.
#[cfg(target_pointer_width = "64")]
#[test]
fn layout_is_byte_exact_on_64_bit() {
    assert_eq!(size_of::<LoamiSlice>(), 16);
    assert_eq!(size_of::<LoamiOwned>(), 32);

    assert_eq!(size_of::<LoamiInvalidRange>(), 24);
    assert_eq!(size_of::<LoamiQuotaExceeded>(), 16);
    assert_eq!(size_of::<LoamiErrorDetail>(), 24);
    assert_eq!(size_of::<LoamiError>(), 64);

    assert_eq!(size_of::<LoamiPutOptions>(), 24);
    assert_eq!(size_of::<LoamiObjectMeta>(), 88);
    assert_eq!(size_of::<LoamiGetResult>(), 120);
    assert_eq!(size_of::<LoamiPutResult>(), 32);

    assert_eq!(size_of::<LoamiInflight>(), 16);
    assert_eq!(size_of::<LoamiProvider>(), 16);
    assert_eq!(size_of::<LoamiVtable>(), 56);
    assert_eq!(size_of::<LoamiList>(), 24);

    assert_eq!(size_of::<LoamiPluginV1>(), 48);
}
