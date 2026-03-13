//! Error reporting.

use std::{collections::HashMap, fmt, ops::ControlFlow};

use crossterm::{
    Command as _,
    style::{
        Attribute, Attributes, Color, ContentStyle, Print, PrintStyledContent,
        Stylize as _,
    },
};
use derail::{Error, VisitContext, Visitor, VisitorExt as _};
use itertools::{Itertools as _, Position};

use crate::error::Details;

/// Attempt a fallible operation, storing the result and breaking on error.
macro_rules! attempt {
    ($e:expr, $result:expr $(,)?) => {{
        $result = $e;
        if $result.is_err() {
            return ControlFlow::Break(());
        }
    }};
}

/// Call [`Command::write_ansi`][0] on a series of [`Command`][1]s.
///
/// [0]: crossterm::Command::write_ansi
/// [1]: crossterm::Command
macro_rules! write_commands {
    ($writer:expr, $one:expr $(,)?) => {
        $one.write_ansi($writer)
    };
    ($writer:expr, $head:expr, $($tail:expr),+ $(,)?) => {
        write_commands!($writer, $head)
        $(
            .and_then(|_| $tail.write_ansi($writer))
        )+
    };
}

/// Report errors.
pub(crate) fn report<'a, I, E>(errors: I) -> impl fmt::Display
where
    I: IntoIterator<Item = &'a E> + Clone,
    E: Error<Details = Details> + ?Sized + 'a,
{
    DisplayImpl(errors)
}

/// [`Display`](fmt::Display) implementation.
struct DisplayImpl<I>(I);

impl<'a, I, E> fmt::Display for DisplayImpl<I>
where
    I: IntoIterator<Item = &'a E> + Clone,
    E: Error<Details = Details> + ?Sized + 'a,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut visitor = VisitorImpl::new(f);

        let _: ControlFlow<(), ()> = visitor.visit_many(self.0.clone());

        visitor.result
    }
}

/// Error style.
const ERROR_STYLE: ContentStyle = ContentStyle {
    foreground_color: Some(Color::Red),
    background_color: None,
    underline_color: None,
    attributes: Attributes::none().with(Attribute::Bold),
};

/// Parenthesized name style.
const PAREN_NAME_STYLE: ContentStyle = ContentStyle {
    foreground_color: None,
    background_color: None,
    underline_color: Some(Color::Grey),
    attributes: Attributes::none(),
};

/// Parenthisized text style.
const PAREN_STYLE: ContentStyle = ContentStyle {
    foreground_color: None,
    background_color: None,
    underline_color: None,
    attributes: Attributes::none().with(Attribute::Dim),
};

/// [`Visitor`] implementation.
struct VisitorImpl<W> {
    /// The writer to write to.
    writer: W,

    /// The write result.
    result: fmt::Result,

    /// Whether this is the first [`Visitor::visit`] call.
    first: bool,

    /// Tracks the current depth.
    depth: usize,

    /// Tracks count of nodes at each depth.
    depth_counts: Vec<usize>,

    /// Maps a depth and depth count to a name.
    names: HashMap<(usize, usize), u64>,

    /// The next name to use.
    next_name: u64,
}

impl<W> VisitorImpl<W> {
    /// Create a new [`VisitorImpl`].
    fn new(writer: W) -> Self {
        Self {
            writer,
            result: Ok(()),
            first: true,
            depth: 0,
            depth_counts: vec![0],
            names: HashMap::new(),
            next_name: 1,
        }
    }
}

impl<W> VisitorImpl<W>
where
    W: fmt::Write,
{
    /// Write the main content of an [`Error`].
    fn write_error<E>(&mut self, error: E) -> fmt::Result
    where
        E: Error<Details = Details>,
    {
        write_commands!(
            &mut self.writer,
            Print("Description".white().bold()),
            Print(": "),
            Print(&error),
        )?;

        let details = error.details();

        if let Some(help) = details.help {
            write_commands!(
                &mut self.writer,
                Print("\n"),
                Print("Help".cyan().bold()),
                Print(": "),
                Print(help),
            )?;
        }

        if let Some(note) = details.note {
            write_commands!(
                &mut self.writer,
                Print("\n"),
                Print("Note".cyan().bold()),
                Print(": "),
                Print(note),
            )?;
        }

        Ok(())
    }
}

impl<W> Visitor for VisitorImpl<W>
where
    W: fmt::Write,
{
    type Details = Details;

    fn visit(
        &mut self,
        visitee: &dyn Error<Details = Self::Details>,
        _ctx: VisitContext<'_, Self::Details>,
    ) -> ControlFlow<()> {
        if self.first {
            self.first = false;
        } else {
            // Separate errors with a blank line.
            attempt!(write!(self.writer, "\n\n"), self.result);
        }

        let x = self
            .depth_counts
            .get_mut(self.depth)
            .expect("`depth_counts` should be at least `depth` long");
        *x = x.checked_add(1).expect("should not overflow");

        // Allocate names depth-first.
        for (depth, depth_count) in self
            .depth_counts
            .iter()
            .take(self.depth.checked_add(1).expect("should not overflow"))
            .enumerate()
            .map(|(x, y)| (x, *y))
        {
            self.names.entry((depth, depth_count)).or_insert_with(|| {
                let name = self.next_name;
                self.next_name =
                    self.next_name.checked_add(1).expect("should not overflow");
                name
            });
        }

        // Iterate in reverse so the `(depth, depth_count)` for the current
        // error comes first and for the root error comes last.
        for (position, (depth, depth_count)) in self
            .depth_counts
            .iter()
            .take(self.depth.checked_add(1).expect("should not overflow"))
            .enumerate()
            .map(|(x, y)| (x, *y))
            .rev()
            .with_position()
        {
            let name = *self
                .names
                .get(&(depth, depth_count))
                .expect("all needed names should be pre-allocated");

            match position {
                Position::Only => attempt!(
                    write_commands!(
                        &mut self.writer,
                        PrintStyledContent(ERROR_STYLE.apply("Error")),
                        Print(" #"),
                        Print(name),
                    ),
                    self.result
                ),
                Position::First => attempt!(
                    write_commands!(
                        &mut self.writer,
                        PrintStyledContent(ERROR_STYLE.apply("Error")),
                        Print(" #"),
                        Print(name),
                        PrintStyledContent(
                            PAREN_STYLE.apply(" (which caused ")
                        ),
                    ),
                    self.result
                ),
                Position::Middle => attempt!(
                    write_commands!(
                        &mut self.writer,
                        PrintStyledContent(PAREN_NAME_STYLE.apply("#")),
                        PrintStyledContent(PAREN_NAME_STYLE.apply(name)),
                        PrintStyledContent(PAREN_STYLE.apply(", causing ")),
                    ),
                    self.result
                ),
                Position::Last => attempt!(
                    write_commands!(
                        &mut self.writer,
                        PrintStyledContent(PAREN_NAME_STYLE.apply("#")),
                        PrintStyledContent(PAREN_NAME_STYLE.apply(name)),
                        PrintStyledContent(PAREN_STYLE.apply(")")),
                    ),
                    self.result
                ),
            }
        }

        attempt!(writeln!(self.writer), self.result);

        attempt!(self.write_error(visitee), self.result);

        ControlFlow::Continue(())
    }

    fn push(&mut self) -> ControlFlow<()> {
        self.depth = self.depth.checked_add(1).expect("should not overflow");
        if self.depth > self.depth_counts.len().saturating_sub(1) {
            self.depth_counts.push(0);
        }

        ControlFlow::Continue(())
    }

    fn pop(&mut self) -> ControlFlow<()> {
        self.depth = self.depth.checked_sub(1).expect("should not underflow");
        // Don't pop `depth_counts` because we may revisit this `depth` again
        // later and we need to retain its count.

        ControlFlow::Continue(())
    }
}
