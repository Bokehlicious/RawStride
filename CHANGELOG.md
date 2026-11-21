# Changelog

What changes in each version of RawStride, written for its users. The form follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the version numbers follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html): while the first number is 0,
anything may change between versions, the files RawStride writes included.

Downloads are on the [Releases page](https://github.com/Bokehlicious/RawStride/releases).

## [Unreleased]

### Added

- An image whose XMP says it is turned (`tiff:Orientation` in a RAW's sidecar, or in the XMP of a
  DNG, JPEG or TIFF) is shown turned in the preview and the contact sheet.
- Rotate Left (Ctrl+←) and Rotate Right (Ctrl+→) in the Image menu turn the marked images, or
  the current one, 90° at a time. The turn is shown at once and saved: for a RAW file in its
  XMP sidecar, for a DNG, JPEG or TIFF in the file's own orientation tag. The pixels are never
  changed.

### Changed

- The Label submenu now comes right after the Rating submenu, in the Image menu and in the
  right-click menu on images.

### Fixed

- Turning a JPEG from a Sony ILCE-1, or writing its Exif fields, no longer fails with a refused
  write.

## [0.1.0] - 2026-11-20

The first release, for Windows 10 and 11 (x64), as a portable zip. RawStride copies photos
from a memory card to a destination and an optional backup, with folder and file name
templates; shows them in a contact sheet and a fast loupe (RAW files from their embedded
preview or rendered with LibRaw, colour managed); sets star ratings, reject and colour
labels in XMP the way other photo programs read them; and edits IPTC metadata (Core and
Extension in XMP, and the older IIM), into an XMP sidecar for a proprietary RAW file and
safely into DNG, JPEG and TIFF files. Folders open in tabs, with filters and multiple
selection; images can be moved, deleted to the Recycle Bin and opened in another program.
ExifTool and the Microsoft Visual C++ runtime are included; the program is not signed yet.

[Unreleased]: https://github.com/Bokehlicious/RawStride/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Bokehlicious/RawStride/releases/tag/v0.1.0
