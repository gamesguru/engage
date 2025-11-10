#![doc = env!("CARGO_PKG_DESCRIPTION")]

// This program exposes a basic interface to POSIX semaphores. Notably, this
// program unlinks the semaphore after the first time waiting completes. This
// program can therefore be used as a barrier to synchronize two processes:
// process X will wait on the semaphore, process Y will post on the semaphore
// to unblock process X, and once processes X is unblocked, the semaphore is
// unlinked.
//
// This code is not very high quality, and neither is the `sem_safe` crate's
// API. While this is quite unfortunate, it should be good enough here since
// this program is only used in Engage's test suite.

use std::{env, error::Error, ffi::CString, process::ExitCode};

use sem_safe::named::{OpenFlags, Semaphore};

fn main() -> ExitCode {
    if let Err(e) = try_main() {
        eprintln!("Error: {e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

/// Fallible entrypoint.
fn try_main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os();

    let name = args.nth(1).ok_or("missing argv[1]: name")?;
    let name = CString::new(name.as_encoded_bytes())?;

    let mode = args.next().ok_or("missing argv[2]: mode")?;
    #[expect(
        clippy::map_err_ignore,
        reason = "original value doesn't explain the error"
    )]
    let mode = mode.into_string().map_err(|_| "invalid argv[2]: not UTF-8")?;

    let semaphore = Semaphore::open(
        &name,
        OpenFlags::Create {
            exclusive: false,
            mode: 0o600,
            value: 0,
        },
    )
    .map_err(|()| "failed to create or open semaphore")?;

    match &*mode {
        "wait" => {
            semaphore
                .sem_ref()
                .wait()
                .map_err(|()| "failed to wait on semaphore")?;

            Semaphore::unlink(&name)
                .map_err(|()| "failed to unlink semaphore")?;
        }
        "post" => {
            semaphore
                .sem_ref()
                .post()
                .map_err(|()| "failed to post to semaphore")?;
        }
        _ => {
            return Err("invalid argv[2]: unknown mode".into());
        }
    }

    // SAFETY: `semaphore` is not unnamed, nor is it used after this call, nor
    // will this process be waiting on it when this is called.
    unsafe {
        semaphore.close().map_err(|()| "failed to close semaphore")?;
    }

    Ok(())
}
