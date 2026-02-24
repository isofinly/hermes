//! Masscan API bindings.

/// Raw bindings to the Masscan.
///
/// Masscan does not provide any SDK/Public API beyond the raw `argv`.
pub mod raw {
    #![allow(dead_code)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(non_upper_case_globals)]
    #![allow(clippy::all)]

    include!(concat!(env!("OUT_DIR"), "/masscan_bindings.rs"));
}
