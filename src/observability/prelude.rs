//! Observability prelude.
//!
//! This is conventionally `use`d `as o`.

// These are not all expected to be used, but they should be available in
// case they're needed.
#![allow(unused_imports, unused_macros)]

pub(crate) use tracing::{
    debug, debug_span, error, error_span, field::Empty, info, info_span,
    instrument, span::Span, trace, trace_span, warn, warn_span,
};

/// OTel status code.
pub(crate) enum OtelStatusCode {
    /// Unset.
    Unset,

    /// `ERROR`.
    Error,

    /// `OK`.
    Ok,
}

impl OtelStatusCode {
    /// Record the current value in the given span.
    pub(crate) fn record(&self, span: &Span) {
        match self {
            OtelStatusCode::Unset => (),
            OtelStatusCode::Error => {
                span.record("otel.status_code", "ERROR");
            }
            OtelStatusCode::Ok => {
                span.record("otel.status_code", "OK");
            }
        }
    }
}
