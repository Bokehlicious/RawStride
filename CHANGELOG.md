# Changelog

What changes in each version of RawStride, written for its users. The form follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the version numbers follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html): while the first number is 0,
anything may change between versions, the files RawStride writes included.

Downloads are on the [Releases page](https://github.com/Bokehlicious/RawStride/releases).

## [Unreleased]

### Added

- Text fields in the metadata window can hold variables that become each image's own value when
  it is saved: `{date}`, `{year}`, `{month}`, `{day}`, `{time}` (from the capture time), `{make}`,
  `{model}`, `{name}` (the file name without its extension) and `{folder}`. Headline
  `{date} {name}` applied to many marked images gives each its own date and name. Write `{{` or
  `}}` for a brace. An unknown variable is shown as an error before anything is saved; an image
  without a capture time is not saved when a date variable is used, while the others are.
- File ▸ Metadata Template… opens a window to fill in a metadata template, with a check box
  per field (only the checked fields are kept) and a + box on lists such as Keywords to add
  the template's entries instead of replacing the image's. Templates are saved and loaded as
  XMP files.

### Changed

- The Settings page Keyboard Shortcuts is a table with the columns Action, First Key and Second
  Key, whose headings stay at the top while the rows scroll. The Action column's width is changed
  by dragging the line beside its heading and is kept between sessions; a long name is cut and
  shown whole on hover.

## [0.3.0] - 2026-11-22

### Added

- Settings has the pages General, External Programs and Metadata Window. On Metadata Window
  you choose which sections and fields the metadata window shows, and in which order; what is
  hidden is shown read-only under Other Metadata, and Copy All Fields and Paste All Fields
  take only what is shown. Restore Defaults brings back the full form.
- The keyboard shortcuts can be changed on the Settings page Keyboard Shortcuts: each action has
  two keys, each set with Change and removed with Clear, and Restore Defaults brings back the
  defaults. A key that is taken or fixed is refused. The menus and buttons show the keys you
  chose, and only changed keys are saved, in `keys.txt` beside the settings.
- Companion File Suffixes on the Settings page General: the suffixes of the companion files that
  move with an image (`dop, pp3, on1` when empty).

### Changed

- The Settings window opens larger, in the middle of the program's window, and can be resized by
  its edge; every page scrolls, with Save and Cancel always at the bottom. Its size is not kept:
  each opening starts at the default size.

- An import opens each destination folder in its own tab as soon as its first image is in place
  with the template, behind the import dialog, and the tab fills while the import goes on; the
  first folder's tab is active, and each later folder's tab is made active when it opens if you
  have not moved. The import writes the template image by image, so its progress shows only the copy.
  Thumbnails already shown no longer flicker when a tab's files change.
- A rating, a turn or an edit applied to several images, and the import's template, are
  written by up to four ExifTool processes at once, so a batch is saved several times as
  fast. Each file is still written and checked on its own.
- The zip includes ExifTool 13.64 instead of 13.59.
- RawStride now needs ExifTool 13.64 or later to save ratings and metadata (12.67 before);
  with an older ExifTool it still reads them and tells you at start.
- Saving metadata to a TIFF or DNG is faster: the IIM checksum is written in the same step as
  the rest, and the written copy is still refused unless its checksum is current.
- A written TIFF or DNG with one more ExifTool validation warning than the original is now
  always refused; before, one more warning was allowed when the write created the file's
  Photoshop block, which ExifTool 13.64 does without a new warning.
- Saving metadata to a DNG is faster: when the DNG has an image data hash, the written copy's
  RAW data is checked by that hash alone, without opening it with LibRaw; its previews are
  still compared.

### Fixed

- The list of images in the import dialog no longer disappears and comes back while you type
  in the IPTC template, so the window no longer jumps. While a template or path is changed,
  the last list stays, dimmed, until the new one is ready.

## [0.2.0] - 2026-11-21

### Added

- An image whose XMP says it is turned (`tiff:Orientation` in a RAW's sidecar, or in the XMP of a
  DNG, JPEG or TIFF) is shown turned in the preview and the contact sheet.
- Rotate Left (Ctrl+←) and Rotate Right (Ctrl+→) in the Image menu turn the marked images, or
  the current one, 90° at a time. The turn is shown at once and saved: for a RAW file in its
  XMP sidecar, for a DNG, JPEG or TIFF in the file's own orientation tag. The pixels are never
  changed.
- File ▸ Reload Folder (F5) lists the folder again and reads every image's rating and metadata
  anew; new files are placed in the chosen order, and the current image, the marks, the filter
  and the place in the contact sheet are kept.
- Four buttons on a thumbnail in the contact sheet, shown while the pointer is over it: Rotate
  Left, Rotate Right, Metadata… and Preview. View ▸ Buttons on Thumbnails turns them off.
- With the image in the metadata window marked and two or more images marked, Previous, Next
  and Save and Next step only between the marked images.
- A folder open in a tab follows what other programs do in it: new images appear in the
  contact sheet (under a filter or a rating order once their rating is read), removed ones
  go, and the status line says how many. A tab not shown follows when it is shown again. The
  thumbnail of an image still being copied in is shown whole once the copy has ended.

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

[Unreleased]: https://github.com/Bokehlicious/RawStride/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/Bokehlicious/RawStride/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Bokehlicious/RawStride/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Bokehlicious/RawStride/releases/tag/v0.1.0
