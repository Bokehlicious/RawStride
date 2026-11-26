# Changelog

What changes in each version of RawStride, written for its users. The form follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the version numbers follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html): while the first number is 0,
anything may change between versions, the files RawStride writes included.

Downloads are on the [Releases page](https://github.com/Bokehlicious/RawStride/releases).

## [Unreleased]

### Added

- Exposure and shadows for RAW images in the preview, for the session only, never saved in the
  images: Ctrl+↑ and Ctrl+↓ (View ▸ Exposure) brighten or darken in steps of ⅓ EV from −2 to
  +3 EV, Shift+Backspace resets them, B turns on auto exposure from the RAW histogram and
  Shift+B the camera's own brightness value; O (View ▸ Show Shadows) lifts the dark tones by the
  amount set on the Settings page Preview. Marks at the bottom right show them; in the compare
  both sides follow. If one of these keys was assigned to another action in your keyboard
  shortcuts, that action goes back to its default key.
- Buttons −, A and + after USM in the status bar: − and + make the RAW preview darker or
  brighter by ⅓ EV, and A turns auto exposure on and off.
- The setting Auto exposure at start (Settings ▸ Preview ▸ Exposure), on by default: auto
  exposure is on when RawStride starts; B still turns it off for the session.

### Changed

- Stepping to another image is quicker with fine details on or between 50 % and 100 %: the
  view still being made of the image left is stopped instead of finished.
- Between 50 % and 100 %, the first drag of an image no longer stands still once the view has
  rested for a second.

## [0.6.0] - 2026-11-25

### Added

- Fine details (D, View ▸ Show Fine Details): below 50 %, RAW views are made from the
  full-resolution rendering; a "Fine" mark at the bottom right shows it is on. If D was assigned
  to another action in your keyboard shortcuts, that action goes back to its default key.
- Sharpening (U, View ▸ Show Sharpening, or the "USM" button beside the RAW/JPEG control above
  the preview): at every zoom level, the preview is shown with an unsharp mask, never saved in
  the images; its amount, radius and threshold are set on the new Settings page Preview, and a
  "USM" mark at the bottom right shows it is on. Reset Adjustments
  (Backspace, View ▸ Reset Adjustments) turns fine details and sharpening off. If U or Backspace
  was assigned to another action in your keyboard shortcuts, that action goes back to its
  default key.

### Changed

- Views below 100 % are reduced with a sharper filter.
- Stepping to the next image at a zoom level between 50 % and 100 % is quicker: the images
  around the current one are prepared at that zoom level.
- Dragging the image at a zoom level between 50 % and 100 % is smoother after the first drag on
  an image: a larger area around the visible part is then prepared at that zoom level.

## [0.5.0] - 2026-11-24

### Added

- Every release also has an installer, `RawStride-<version>-windows-x64.msi`: it installs
  RawStride for the current user, without administrator rights, with a Start menu shortcut;
  uninstall it in Installed apps, which keeps your settings.

## [0.4.0] - 2026-11-24

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
- The metadata window has Load Template… and Save as Template… buttons: Load Template… puts
  a template file's fields into the form as unsaved changes and names what it could not load;
  Save as Template… saves the fields the window shows as a template file.
- The right-click menu on images has an Apply Template submenu, listing the saved templates and
  Choose File…: after one question the chosen template is written to the marked images (or the
  clicked one), with its progress in the status bar.

### Changed

- The import dialog's IPTC Template fields, their modes and the "Clear fields?" question are
  replaced by a Metadata Template row with Choose…, Edit… and None; Choose… also opens the chosen
  template in the template window. The chosen template file is remembered and read when the
  import starts: a file that is gone, cannot be read, has no field or has an unknown variable
  stops the import before anything is copied. Variables are filled with each image's own values,
  and the notices of the template's writes (for example a text cut to IIM's limit) are listed in
  the import's summary. Use for This Import in the template window changes the import's template
  for this import only, without saving the file.
- A metadata template's list with its + box checked (such as Keywords) is no longer written to an
  image whose IIM and XMP values of that list disagree: the list is left as it was and named in
  the import's summary, as the metadata window does.
- The Settings page Keyboard Shortcuts is a table with the columns Action, First Key and Second
  Key, whose headings stay at the top while the rows scroll. The Action column's width is changed
  by dragging the line beside its heading and is kept between sessions; a long name is cut and
  shown whole on hover.
- The metadata window shows its sections in two columns when it is wide enough, and opens wide
  enough for them when the program window allows; narrowed, it shows one column.

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

[Unreleased]: https://github.com/Bokehlicious/RawStride/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/Bokehlicious/RawStride/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/Bokehlicious/RawStride/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/Bokehlicious/RawStride/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/Bokehlicious/RawStride/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Bokehlicious/RawStride/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Bokehlicious/RawStride/releases/tag/v0.1.0
