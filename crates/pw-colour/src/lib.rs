//! Colour conversion between ICC profiles (ADR 0016 §4.1, §6).
//!
//! One place that turns RGB pixels from a source profile into a destination profile (the monitor
//! profile), testable without a screen:
//! - [`Profile`]: a parsed RGB profile (sRGB, Adobe RGB, ProPhoto RGB or ICC bytes) and its
//!   identity;
//! - [`Transform`]: an 8-bit and a 16-bit RGB → RGB transform with relative colorimetric intent
//!   (ADR 0016 Q6), converting in place in chunks; the same profile on both sides is an identity
//!   that leaves the data untouched (F3);
//! - [`TransformCache`]: builds each transform once per pair of profiles.
//!
//! The conversion itself is done by `moxcms` (ADR 0016 Q5). The crate is not used yet; the loupe,
//! the loupe core, the thumbnails and `pw-tiff` follow in later steps (ADR 0016 C2–C4).

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use moxcms::{
    ColorProfile, DataColorSpace, Layout, RenderingIntent, Transform8BitExecutor,
    Transform16BitExecutor, TransformExecutor, TransformOptions,
};

#[cfg(test)]
mod tests;

/// Pixels converted per call to `moxcms`, so the scratch buffer stays small (ADR 0016 §4.1).
const CHUNK_PIXELS: usize = 65_536;

/// Why a profile or a transform could not be made, or the pixel data could not be converted.
/// Each `String` is the detail that `Display` puts into its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColourError {
    /// `moxcms` could not parse the ICC bytes ("ICC profile: {detail}").
    Profile(String),
    /// The profile is not an RGB profile ("ICC profile is not RGB ({colour space})").
    Unsupported(String),
    /// `moxcms` could not build or run the transform ("colour transform: {detail}").
    Transform(String),
    /// The data is not whole RGB pixels (its length is not a multiple of 3).
    Length { len: usize },
}

impl fmt::Display for ColourError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColourError::Profile(detail) => write!(f, "ICC profile: {detail}"),
            ColourError::Unsupported(space) => write!(f, "ICC profile is not RGB ({space})"),
            ColourError::Transform(detail) => write!(f, "colour transform: {detail}"),
            ColourError::Length { len } => {
                write!(f, "pixel data of {len} values is not whole RGB pixels")
            }
        }
    }
}

impl std::error::Error for ColourError {}

/// What a profile is: one of the built-in profiles, or ICC bytes (identified by the bytes).
#[derive(Clone, PartialEq, Eq, Hash)]
enum ProfileId {
    Srgb,
    AdobeRgb,
    ProPhotoRgb,
    Icc(Arc<[u8]>),
}

impl fmt::Debug for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileId::Srgb => f.write_str("Srgb"),
            ProfileId::AdobeRgb => f.write_str("AdobeRgb"),
            ProfileId::ProPhotoRgb => f.write_str("ProPhotoRgb"),
            ProfileId::Icc(bytes) => write!(f, "Icc({} bytes)", bytes.len()),
        }
    }
}

/// A parsed RGB profile and its identity.
#[derive(Clone)]
pub struct Profile {
    id: ProfileId,
    profile: Arc<ColorProfile>,
}

impl fmt::Debug for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Profile")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Profile {
    /// The sRGB profile.
    pub fn srgb() -> Profile {
        Profile::built_in(ProfileId::Srgb, ColorProfile::new_srgb())
    }

    /// The Adobe RGB (1998) profile.
    pub fn adobe_rgb() -> Profile {
        Profile::built_in(ProfileId::AdobeRgb, ColorProfile::new_adobe_rgb())
    }

    /// The ProPhoto RGB profile.
    pub fn pro_photo_rgb() -> Profile {
        Profile::built_in(ProfileId::ProPhotoRgb, ColorProfile::new_pro_photo_rgb())
    }

    fn built_in(id: ProfileId, profile: ColorProfile) -> Profile {
        Profile {
            id,
            profile: Arc::new(profile),
        }
    }

    /// Parses an ICC profile. Bytes `moxcms` cannot parse give `Profile`; a profile whose data
    /// colour space is not RGB (gray, CMYK, Lab, …) gives `Unsupported`.
    pub fn from_icc(bytes: &[u8]) -> Result<Profile, ColourError> {
        let profile =
            ColorProfile::new_from_slice(bytes).map_err(|e| ColourError::Profile(e.to_string()))?;
        if profile.color_space != DataColorSpace::Rgb {
            return Err(ColourError::Unsupported(format!(
                "{:?}",
                profile.color_space
            )));
        }
        Ok(Profile {
            id: ProfileId::Icc(Arc::from(bytes)),
            profile: Arc::new(profile),
        })
    }
}

/// The two `moxcms` transforms of a non-identity [`Transform`].
struct Executors {
    rgb8: Arc<Transform8BitExecutor>,
    rgb16: Arc<Transform16BitExecutor>,
}

/// An RGB → RGB transform from one profile to another, for 8-bit and 16-bit data.
pub struct Transform {
    /// `None` when source and destination are the same profile.
    executors: Option<Executors>,
}

impl fmt::Debug for Transform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transform")
            .field("identity", &self.is_identity())
            .finish()
    }
}

impl Transform {
    /// Builds the transform from `source` to `destination` with relative colorimetric intent
    /// (ADR 0016 Q6; `moxcms` has no black-point compensation). The same profile on both sides
    /// gives an identity transform and builds nothing.
    pub fn new(source: &Profile, destination: &Profile) -> Result<Transform, ColourError> {
        if source.id == destination.id {
            return Ok(Transform { executors: None });
        }
        let options = TransformOptions {
            rendering_intent: RenderingIntent::RelativeColorimetric,
            ..Default::default()
        };
        let src = &source.profile;
        let dst = &destination.profile;
        let rgb8 = src
            .create_transform_8bit(Layout::Rgb, dst, Layout::Rgb, options)
            .map_err(|e| ColourError::Transform(e.to_string()))?;
        let rgb16 = src
            .create_transform_16bit(Layout::Rgb, dst, Layout::Rgb, options)
            .map_err(|e| ColourError::Transform(e.to_string()))?;
        Ok(Transform {
            executors: Some(Executors { rgb8, rgb16 }),
        })
    }

    /// True when source and destination are the same profile: the conversions leave the data
    /// untouched (F3).
    pub fn is_identity(&self) -> bool {
        self.executors.is_none()
    }

    /// Converts 8-bit RGB data in place. A length that is not a multiple of 3 gives `Length`
    /// and leaves the data unchanged.
    pub fn convert_rgb8(&self, data: &mut [u8]) -> Result<(), ColourError> {
        check_length(data.len())?;
        match &self.executors {
            None => Ok(()),
            Some(executors) => convert_in_place(executors.rgb8.as_ref(), data),
        }
    }

    /// Converts 16-bit RGB data in place. A length that is not a multiple of 3 gives `Length`
    /// and leaves the data unchanged.
    pub fn convert_rgb16(&self, data: &mut [u16]) -> Result<(), ColourError> {
        check_length(data.len())?;
        match &self.executors {
            None => Ok(()),
            Some(executors) => convert_in_place(executors.rgb16.as_ref(), data),
        }
    }
}

fn check_length(len: usize) -> Result<(), ColourError> {
    if len.is_multiple_of(3) {
        Ok(())
    } else {
        Err(ColourError::Length { len })
    }
}

/// Converts `data` in chunks of at most [`CHUNK_PIXELS`] pixels through one scratch buffer, so a
/// large image never needs a second full copy. If `moxcms` fails on a chunk, the chunks before it
/// are already converted.
fn convert_in_place<V, E>(executor: &E, data: &mut [V]) -> Result<(), ColourError>
where
    V: Copy + Default,
    E: TransformExecutor<V> + ?Sized,
{
    let chunk_len = CHUNK_PIXELS * 3;
    let mut scratch: Vec<V> = Vec::with_capacity(data.len().min(chunk_len));
    for chunk in data.chunks_mut(chunk_len) {
        scratch.clear();
        scratch.extend_from_slice(chunk);
        executor
            .transform(&scratch, chunk)
            .map_err(|e| ColourError::Transform(e.to_string()))?;
    }
    Ok(())
}

/// Builds each transform once per pair of profiles and shares it (ADR 0016 §4.1).
#[derive(Debug, Default)]
pub struct TransformCache {
    transforms: Mutex<HashMap<(ProfileId, ProfileId), Arc<Transform>>>,
}

impl TransformCache {
    /// The transform from `source` to `destination`: the cached one, or a new one that is then
    /// cached. A failed build is returned and not cached.
    pub fn get(
        &self,
        source: &Profile,
        destination: &Profile,
    ) -> Result<Arc<Transform>, ColourError> {
        let key = (source.id.clone(), destination.id.clone());
        let mut transforms = self
            .transforms
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(transform) = transforms.get(&key) {
            return Ok(Arc::clone(transform));
        }
        let transform = Arc::new(Transform::new(source, destination)?);
        transforms.insert(key, Arc::clone(&transform));
        Ok(transform)
    }
}
