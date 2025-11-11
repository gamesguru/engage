//! Custom [`Layer`] implementation.

use std::{
    collections::HashMap,
    fmt, io,
    str::FromStr as _,
    sync::{Arc, Mutex, OnceLock, RwLock},
};

use crossterm::{
    execute,
    style::{
        Attribute, Color, ContentStyle, Print, PrintStyledContent,
        SetAttribute, Stylize as _,
    },
};
use tracing::{
    Event, Id, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Record},
};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

use crate::{name::Name, observability::prelude as o, ui::OutputKind};

mod unicode {
    #![allow(missing_docs)]
    #![allow(clippy::missing_docs_in_private_items)]

    //! Unicode characters used in the output.

    pub(crate) const BLACK_LEFT_POINTING: char = '\u{25C0}';
    pub(crate) const BLACK_RIGHT_POINTING: char = '\u{25B6}';
    pub(crate) const LIGHT_ARC_DOWN_AND_RIGHT: char = '\u{256D}';
    pub(crate) const LIGHT_ARC_UP_AND_RIGHT: char = '\u{2570}';
    pub(crate) const LIGHT_DOWN_AND_HORIZONTAL: char = '\u{252C}';
    pub(crate) const LIGHT_HORIZONTAL: char = '\u{2500}';
    pub(crate) const LIGHT_UP_AND_HORIZONTAL: char = '\u{2534}';
    pub(crate) const LIGHT_VERTICAL: char = '\u{2502}';
}

/// A start or end sequence.
#[derive(Clone, Copy)]
pub(crate) enum Sequence {
    /// The start sequence.
    Start,

    /// The end sequence.
    End,
}

/// Displays a [`Sequence`] left-padded with spaces by [`usize`].
pub(crate) struct DisplaySequence(Sequence, usize);

impl fmt::Display for DisplaySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (arc, t, arrow) = match self.0 {
            Sequence::Start => (
                unicode::LIGHT_ARC_DOWN_AND_RIGHT,
                unicode::LIGHT_DOWN_AND_HORIZONTAL,
                unicode::BLACK_LEFT_POINTING,
            ),
            Sequence::End => (
                unicode::LIGHT_ARC_UP_AND_RIGHT,
                unicode::LIGHT_UP_AND_HORIZONTAL,
                unicode::BLACK_RIGHT_POINTING,
            ),
        };

        for _ in 0..self.1 {
            write!(f, " ")?;
        }

        write!(f, " {arc}{}{t}{arrow}", unicode::LIGHT_HORIZONTAL)
    }
}

/// [`Layer`] implementation.
pub(crate) struct Impl {
    /// Longest task name.
    longest_name: Arc<OnceLock<usize>>,

    /// Map of IDs to task names.
    task_names: RwLock<HashMap<Id, Box<Name>>>,

    /// OTel status codes of spans.
    otel_status_codes: Mutex<HashMap<Id, o::OtelStatusCode>>,
}

impl Impl {
    /// Create a new [`Impl`].
    pub(crate) fn new(longest_name: Arc<OnceLock<usize>>) -> Self {
        Self {
            longest_name,
            task_names: RwLock::default(),
            otel_status_codes: Mutex::default(),
        }
    }

    /// Handle a new `run_graph` span.
    fn on_new_span_run_graph(&self) {
        execute!(
            io::stdout(),
            PrintStyledContent(ContentStyle::new().with(Color::Blue).apply(
                DisplaySequence(
                    Sequence::Start,
                    *self.longest_name.get().expect("value should be set")
                )
            )),
            Print(' '),
            PrintStyledContent(
                ContentStyle::new().with(Color::Green).bold().apply("starting")
            ),
            Print('\n'),
        )
        .expect("should be able to write to stdout");
    }

    /// Handle a new `run_task` span.
    fn on_new_span_run_task(&self, attrs: &Attributes<'_>, id: &Id) {
        struct Impl {
            /// The task name found, if any.
            name: Option<Box<Name>>,
        }

        impl Visit for Impl {
            fn record_debug(&mut self, field: &Field, _value: &dyn fmt::Debug) {
                assert_ne!(
                    field.name(),
                    "task.name",
                    "the task.name field should be a string"
                );
            }

            fn record_str(&mut self, field: &Field, value: &str) {
                if field.name() != "task.name" {
                    return;
                }

                self.name = Some(
                    <Box<Name>>::from_str(value)
                        .expect("value should be valid"),
                );
            }
        }

        let mut visitor = Impl {
            name: None,
        };
        attrs.values().record(&mut visitor);

        let mut task_names =
            self.task_names.write().expect("lock should not be poisoned");

        task_names.insert(
            id.clone(),
            visitor.name.take().expect("name should be set"),
        );
    }
}

impl<S> Layer<S> for Impl
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attrs: &Attributes<'_>,
        id: &Id,
        _ctx: Context<'_, S>,
    ) {
        match attrs.metadata().name() {
            "run_graph" => self.on_new_span_run_graph(),
            "run_task" => self.on_new_span_run_task(attrs, id),
            _ => (),
        }
    }

    fn on_record(&self, span: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        struct Impl {
            /// OTel status code.
            otel_status_code: o::OtelStatusCode,
        }

        impl Visit for Impl {
            fn record_debug(&mut self, field: &Field, _value: &dyn fmt::Debug) {
                assert_ne!(
                    field.name(),
                    "otel.status_code",
                    "the otel.status_code field should be a string"
                );
            }

            fn record_str(&mut self, field: &Field, value: &str) {
                if field.name() != "otel.status_code" {
                    return;
                }

                match value {
                    "OK" => self.otel_status_code = o::OtelStatusCode::Ok,
                    "ERROR" => {
                        self.otel_status_code = o::OtelStatusCode::Error;
                    }
                    _ => panic!("invalid value"),
                }
            }
        }

        if ctx.metadata(span).is_none_or(|x| x.name() != "run_graph") {
            return;
        }

        let mut visitor = Impl {
            otel_status_code: o::OtelStatusCode::Unset,
        };

        values.record(&mut visitor);

        self.otel_status_codes
            .lock()
            .expect("lock should not be poisoned")
            .insert(span.clone(), visitor.otel_status_code);
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        if ctx.metadata(&id).is_none_or(|x| x.name() != "run_graph") {
            return;
        }

        let otel_status_code = self
            .otel_status_codes
            .lock()
            .expect("lock should not be poisoned")
            .remove(&id)
            .expect("run_graph span should have recorded otel.status_code");

        let res = match otel_status_code {
            o::OtelStatusCode::Unset => {
                panic!("run_graph span should have recorded otel.status_code")
            }
            o::OtelStatusCode::Error => {
                ContentStyle::new().with(Color::Red).bold().apply("failure")
            }
            o::OtelStatusCode::Ok => {
                ContentStyle::new().with(Color::Green).bold().apply("success")
            }
        };

        execute!(
            io::stdout(),
            PrintStyledContent(ContentStyle::new().with(Color::Blue).apply(
                DisplaySequence(
                    Sequence::End,
                    *self.longest_name.get().expect("value should be set")
                )
            )),
            Print(' '),
            PrintStyledContent(res),
            Print('\n'),
        )
        .expect("should be able to write to stdout");
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        struct Scanner {
            /// Output kind.
            output_kind: Option<OutputKind>,
        }

        impl Visit for Scanner {
            fn record_debug(&mut self, field: &Field, _value: &dyn fmt::Debug) {
                assert_ne!(
                    field.name(),
                    "kind",
                    "the kind field should be a string"
                );
            }

            fn record_str(&mut self, field: &Field, value: &str) {
                if field.name() != "kind" {
                    return;
                }

                self.output_kind = Some(OutputKind::from_str(value));
            }
        }

        struct Printer<'a> {
            /// Current task name.
            name: &'a Name,

            /// Longest task name.
            longest_name: usize,

            /// Output kind.
            output_kind: OutputKind,
        }

        impl Visit for Printer<'_> {
            fn record_debug(&mut self, field: &Field, _value: &dyn fmt::Debug) {
                assert_ne!(
                    field.name(),
                    "data",
                    "the data field should be a string"
                );
            }

            fn record_str(&mut self, field: &Field, value: &str) {
                if field.name() != "data" {
                    return;
                }

                execute!(
                    io::stdout(),
                    Print(format_args!(
                        "{:>x$} ",
                        self.name,
                        x = self.longest_name
                    )),
                    PrintStyledContent(
                        ContentStyle::new()
                            .with(Color::Blue)
                            .apply(unicode::LIGHT_VERTICAL)
                    ),
                    Print(self.output_kind.to_char()),
                    PrintStyledContent(
                        ContentStyle::new()
                            .with(Color::Blue)
                            .apply(unicode::LIGHT_VERTICAL)
                    ),
                    Print(' '),
                    Print(value),
                    SetAttribute(Attribute::Reset),
                    Print("\n"),
                )
                .expect("should be able to write to stdout");
            }
        }

        let Some((id, metadata)) = ctx
            .current_span()
            .id()
            .and_then(|x| ctx.metadata(x).map(|y| (x.clone(), y)))
        else {
            return;
        };

        if metadata.name() != "run_task" {
            return;
        }

        let mut visitor = Scanner {
            output_kind: None,
        };
        event.record(&mut visitor);

        let task_names =
            self.task_names.read().expect("lock should not be poisoned");
        let name = task_names.get(&id).expect("task name should be known");

        let mut visitor = Printer {
            name,
            longest_name: *self
                .longest_name
                .get()
                .expect("value should be set"),
            output_kind: visitor
                .output_kind
                .expect("output kind should be known"),
        };

        event.record(&mut visitor);
    }
}
