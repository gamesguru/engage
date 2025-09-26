//! Semantic versioning functionality.

use semver::Version;

#[cfg(test)]
mod tests;

/// Returns [`true`] if `ver` is compatible with `req`, else [`false`].
pub(crate) fn compatible(req: &Version, ver: &Version) -> bool {
    fn eq(a: &Version, b: &Version) -> bool {
        a.major == b.major
            && a.minor == b.minor
            && a.patch == b.patch
            && a.pre == b.pre
    }

    // Pre-release in requirement requires exact match.
    if !req.pre.is_empty() {
        return eq(req, ver);
    }

    if req.major != 0
        && req.major == ver.major
        && req.minor <= ver.minor
        && req.patch <= ver.patch
    {
        return true;
    }

    if req.major == 0
        && req.minor != 0
        && req.major == ver.major
        && req.minor == ver.minor
        && req.patch <= ver.patch
    {
        return true;
    }

    if req.major == 0
        && req.minor == 0
        && req.patch != 0
        && req.major == ver.major
        && req.minor == ver.minor
        && req.patch == ver.patch
    {
        return true;
    }

    eq(req, ver)
}
