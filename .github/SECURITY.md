# Security policy

## Reporting a problem

Please do not report a security problem in a public issue. Use GitHub's private vulnerability
reporting instead: the repository's **Security** tab, then **Report a vulnerability**. Only the
maintainer sees the report.

Say what the problem is, how to reproduce it (a file that triggers it, if you can share one), and
which RawStride version you used (or the commit, if you built it yourself).

## What to expect

RawStride has one maintainer, so answers are best effort: you should hear back within a few weeks.
A confirmed problem is fixed on `main` and in a new release, and the report is published once that
release is out.

## Versions

Only the latest release is supported. A confirmed problem is fixed in a new patch release (for
example 0.1.1); older versions are not updated. Until 1.0.0, see the [changelog](../CHANGELOG.md)
for what each version changes.

## Scope

RawStride opens image files from memory cards and folders, so a file that makes it crash, hang or
write outside the folder it should is in scope. A problem in LibRaw, libjpeg-turbo or ExifTool
themselves is best reported to that project as well; tell us too, so RawStride can update.
