//! Process management.

use std::{
    borrow::Cow,
    ffi::{CString, OsStr},
    io::{self, Read as _, Write as _},
    os::{
        fd::{AsFd, AsRawFd, OwnedFd},
        unix::{
            ffi::OsStrExt as _,
            prelude::{BorrowedFd, RawFd},
        },
    },
    pin::Pin,
    process::ExitStatus,
    task::{Context, Poll, ready},
};

use async_pidfd::AsyncPidFd;
use nix::{
    errno::Errno,
    fcntl::{FcntlArg::F_SETFL, OFlag, fcntl},
    pty::{ForkptyResult, Winsize, forkpty},
    unistd,
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, unix::AsyncFd};

/// A child process.
pub(crate) struct Child {
    /// A PID FD that can be awaited for child process completion.
    pid_fd: AsyncPidFd,

    /// The child's PTY.
    pub(crate) pty: Option<AsyncPty>,
}

impl Child {
    /// Wait for the child process to exit.
    pub(crate) async fn wait(self) -> Result<ExitStatus, io::Error> {
        self.pid_fd.wait().await.map(|x| x.status())
    }
}

/// Spawn a process in a PTY.
pub(crate) fn spawn<S, I>(path: S, args: I) -> Result<Child, io::Error>
where
    S: AsRef<OsStr>,
    I: IntoIterator,
    I::Item: AsRef<OsStr>,
{
    let mut saw_nul = false;
    let path = os_to_c(path.as_ref(), &mut saw_nul);
    let mut c_args = Vec::new();
    c_args.push(Cow::Borrowed(&*path));

    for arg in args {
        c_args.push(Cow::Owned(os_to_c(arg.as_ref(), &mut saw_nul)));
    }

    if saw_nul {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "provided string contains nul byte",
        ));
    }

    let winsize = Winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    // SAFETY: `execvp` is called immediately after `forkpty` in the child.
    let (child, pty) = unsafe {
        match forkpty(&winsize, None)? {
            ForkptyResult::Parent {
                child,
                master,
            } => (child, master),
            ForkptyResult::Child => match unistd::execvp(&path, &c_args)? {},
        }
    };

    let pid_fd = AsyncPidFd::from_pid(child.as_raw())?;

    Ok(Child {
        pid_fd,
        pty: Some(AsyncPty::new(Pty(pty))?),
    })
}

/// Convert an [`OsStr`] to a [`CString`].
fn os_to_c(s: &OsStr, saw_nul: &mut bool) -> CString {
    CString::new(s.as_bytes()).unwrap_or_else(|_e| {
        *saw_nul = true;
        c"placeholder".to_owned()
    })
}

/// A PTY.
struct Pty(OwnedFd);

impl io::Read for Pty {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match unistd::read(self, buf) {
            Ok(x) => Ok(x),

            // This doesn't appear to be documented anywhere in any official
            // capacity, but this plus `read(2)` returning `-1` appears to be
            // intended to be interpreted to mean "the PTY is closed".
            Err(Errno::EIO) => Ok(0),

            Err(e) => Err(e.into()),
        }
    }
}

impl io::Write for Pty {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unistd::write(self, buf).map_err(Into::into)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsFd for Pty {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl AsRawFd for Pty {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

/// An async PTY.
pub(crate) struct AsyncPty(AsyncFd<Pty>);

impl AsyncPty {
    /// Create a new [`AsyncPty`].
    fn new(pty: Pty) -> Result<Self, io::Error> {
        fcntl(&pty, F_SETFL(OFlag::O_NONBLOCK))?;

        Ok(Self(AsyncFd::new(pty)?))
    }
}

impl AsyncRead for AsyncPty {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            let mut guard = ready!(self.0.poll_read_ready_mut(cx))?;

            let unfilled = buf.initialize_unfilled();
            match guard.try_io(|inner| inner.get_mut().read(unfilled)) {
                Ok(Ok(len)) => {
                    buf.advance(len);
                    return Poll::Ready(Ok(()));
                }
                Ok(Err(e)) => return Poll::Ready(Err(e)),
                Err(_) => (),
            }
        }
    }
}

impl AsyncWrite for AsyncPty {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        loop {
            let mut guard = ready!(self.0.poll_write_ready_mut(cx))?;

            if let Ok(x) = guard.try_io(|inner| inner.get_mut().write(buf)) {
                return Poll::Ready(x);
            }
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        self.0.get_mut().flush()?;
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
}
