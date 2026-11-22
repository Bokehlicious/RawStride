//! The real image check of the safe metadata write (ADR 0007 I9d, T90).
//!
//! `pw-metadata`'s safe write transaction (ADR 0007 §9, T86) calls an
//! [`ImageCheck`](pw_metadata::safe_write::ImageCheck) given by the caller, because
//! `pw-metadata` must not depend on `pw-raw` or LibRaw. This crate has both and implements the
//! check with [`RawImageCheck`]:
//!
//! - **§9.1 point 2, sessions closed first:** [`ImageCheck::release`] calls the closure given to
//!   [`RawImageCheck::new`] (the loupe passes the view core's releaser, T91), so no RAW engine
//!   session holds the file open when it is checked and replaced. Before a move (ADR 0017 §2.6,
//!   T177), [`ImageCheck::release_for_move`] calls the closure given to
//!   [`RawImageCheck::with_move_release`] when one is set, otherwise the same release closure.
//! - **§9.3 point 3, "pw-raw finds the same previews (count, sizes, decodable JPEG)":** both
//!   files are inspected with `pw_raw::inspect`; the copy must have as many embedded previews,
//!   and each one (by index, smallest first) the same width, height and length and the **same
//!   bytes**. Measured by the reviewer: after a metadata write the embedded previews are byte for
//!   byte equal and only their offsets move. Equal bytes decode exactly as the original's, which
//!   is how "decodable JPEG" is read here; it also never refuses a file whose own preview was
//!   already broken. When the original cannot be inspected (for example a BigTIFF), the preview
//!   check is skipped: there is nothing to compare with.
//!   For a plain JPEG file, `pw_raw::inspect` lists the file itself as its only preview (offset
//!   0, the whole file). A metadata write always changes those bytes, so a preview at offset 0 is
//!   not an embedded preview and is left out on both sides (accepted in the review,
//!   2026-11-12); the JPEG's image data is checked by `ImageDataHash` (§9.3 point 1). An embedded
//!   preview never starts at offset 0, where the container's header is.
//! - The rest of §9.3 point 3 (`DNGVersion`, `NewSubfileType`, the IFD chain) is covered by the
//!   complete listing comparison of `pw_metadata::verify` (T84), where any changed or missing key
//!   outside the allowed ones is an error.
//! - **§9.3 point 4, "for DNG, LibRaw opens and unpacks the copy without a 'damaged' report":**
//!   `pw_libraw::Session::open_with_diagnostics` of the copy; the session is dropped at once, so
//!   the file is closed before the replace. When the copy does not open, the original is opened
//!   the same way: if it fails too, the check passes (the write did not make it worse); otherwise
//!   the error is the copy's LibRaw message. For a DNG this check runs before the preview check,
//!   so a copy cut short inside its RAW data is reported as LibRaw's `damaged:` error rather than
//!   as a missing preview. LibRaw runs only when the original had no image data hash
//!   ([`ImageCheck::check_copy_with`] with [`DataHash::Missing`]; ADR 0007 §9.7 point 4, Q23):
//!   with one, `verify_copy` has already refused a copy whose RAW data changed, so the safe write
//!   leaves LibRaw's 250–550 ms out. [`ImageCheck::check_copy`] keeps the whole check with LibRaw.
//!
//! Error texts are in English and name the check (`"preview 2 differs"`, `"the copy has 1
//! preview, the original 2"`, `"LibRaw: …: damaged: …"`); the Swedish texts are T91's.

use std::fmt;
use std::path::Path;

use pw_libraw::Session;
use pw_metadata::reconcile::FileKind;
use pw_metadata::safe_write::{DataHash, ImageCheck};
use pw_raw::PreviewRef;

/// Closes the RAW engine sessions of a file.
type Release = Box<dyn Fn(&Path) + Send + Sync>;

/// The [`ImageCheck`] of RawStride: previews with `pw-raw`, the DNG opened with LibRaw.
pub struct RawImageCheck {
    release: Option<Release>,
    move_release: Option<Release>,
}

impl RawImageCheck {
    /// A check whose [`ImageCheck::release`] calls `release` with the image's path.
    pub fn new(release: impl Fn(&Path) + Send + Sync + 'static) -> RawImageCheck {
        RawImageCheck {
            release: Some(Box::new(release)),
            move_release: None,
        }
    }

    /// A check whose [`ImageCheck::release`] does nothing (no RAW engine holds files open).
    pub fn without_release() -> RawImageCheck {
        RawImageCheck {
            release: None,
            move_release: None,
        }
    }

    /// The same check whose [`ImageCheck::release_for_move`] calls `release` with the image's
    /// path in place of the release closure (T177): a move has no write answer, so its release
    /// must not note anything for one.
    pub fn with_move_release(
        self,
        release: impl Fn(&Path) + Send + Sync + 'static,
    ) -> RawImageCheck {
        RawImageCheck {
            move_release: Some(Box::new(release)),
            ..self
        }
    }
}

impl fmt::Debug for RawImageCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawImageCheck")
            .field("release", &self.release.is_some())
            .field("move_release", &self.move_release.is_some())
            .finish()
    }
}

impl ImageCheck for RawImageCheck {
    fn release(&self, image: &Path) {
        if let Some(release) = &self.release {
            release(image);
        }
    }

    fn release_for_move(&self, image: &Path) {
        match &self.move_release {
            Some(release) => release(image),
            None => self.release(image),
        }
    }

    /// The whole check with LibRaw for a DNG, as when the original has no image data hash.
    fn check_copy(&self, original: &Path, copy: &Path, kind: FileKind) -> Result<(), String> {
        self.check_copy_with(original, copy, kind, DataHash::Missing)
    }

    /// LibRaw opens a DNG copy only when `hash` is [`DataHash::Missing`] (ADR 0007 §9.7 point 4,
    /// Q23); the previews are compared in every case.
    fn check_copy_with(
        &self,
        original: &Path,
        copy: &Path,
        kind: FileKind,
        hash: DataHash,
    ) -> Result<(), String> {
        if kind == FileKind::Dng && hash == DataHash::Missing {
            check_dng(original, copy)?;
        }
        check_previews(original, copy)
    }
}

/// The embedded previews of `path`: every preview `pw_raw::inspect` lists except one at offset
/// 0, which is the file itself (a plain JPEG), not an embedded preview.
fn embedded_previews(path: &Path) -> pw_raw::Result<Vec<PreviewRef>> {
    let info = pw_raw::inspect(path)?;
    Ok(info
        .previews
        .into_iter()
        .filter(|preview| preview.offset != 0)
        .collect())
}

fn previews_text(count: usize) -> &'static str {
    if count == 1 { "preview" } else { "previews" }
}

/// §9.3 point 3: the copy has the same embedded previews as the original, byte for byte.
fn check_previews(original: &Path, copy: &Path) -> Result<(), String> {
    let Ok(before) = embedded_previews(original) else {
        // Nothing to compare with (for example a BigTIFF pw-raw cannot read).
        return Ok(());
    };
    let after = embedded_previews(copy)
        .map_err(|error| format!("pw-raw cannot inspect the copy: {error}"))?;
    if before.len() != after.len() {
        return Err(format!(
            "the copy has {} {}, the original {}",
            after.len(),
            previews_text(after.len()),
            before.len()
        ));
    }
    for (index, (old, new)) in before.iter().zip(&after).enumerate() {
        let number = index + 1;
        if (old.width, old.height, old.length) != (new.width, new.height, new.length) {
            return Err(format!(
                "preview {number} differs: {} × {}, {} bytes in the original, {} × {}, {} bytes \
                 in the copy",
                old.width, old.height, old.length, new.width, new.height, new.length
            ));
        }
        let old_bytes = pw_raw::read_preview(original, old)
            .map_err(|error| format!("preview {number} of the original cannot be read: {error}"))?;
        let new_bytes = pw_raw::read_preview(copy, new)
            .map_err(|error| format!("preview {number} of the copy cannot be read: {error}"))?;
        if old_bytes != new_bytes {
            return Err(format!("preview {number} differs"));
        }
    }
    Ok(())
}

/// §9.3 point 4: LibRaw opens and unpacks the copy of a DNG without a "damaged" report, unless
/// the original does not open either.
fn check_dng(original: &Path, copy: &Path) -> Result<(), String> {
    let (copy_opened, _) = Session::open_with_diagnostics(copy);
    let copy_error = match copy_opened {
        // The session is dropped here: the copy is closed before the replace.
        Ok(session) => {
            drop(session);
            return Ok(());
        }
        Err(error) => error,
    };
    let (original_opened, _) = Session::open_with_diagnostics(original);
    match original_opened {
        Ok(session) => {
            drop(session);
            Err(copy_error.to_string())
        }
        // The original does not open either: the write did not make it worse.
        Err(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests;
