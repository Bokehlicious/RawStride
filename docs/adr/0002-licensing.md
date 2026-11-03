# ADR 0002: Licensing and distribution

- Status: **open**
- Date: 2026-11-03

## Context

- **LibRaw** is dual-licensed under LGPL-2.1 or CDDL-1.0. With static linking under
  LGPL, users must be able to relink, so the CDDL option is likely the simpler one.
  This needs to be confirmed before the first distribution.
- **ExifTool** uses the same terms as Perl (Artistic License or GPL). Running it as a
  separate, unmodified executable is the common way to ship it.
- PhotoWorkflow's own licence has not been decided (proprietary or open source).

## Decision

Pending.

## Todo

- [ ] Choose a licence for PhotoWorkflow
- [ ] Confirm the LibRaw licence option and the notices required
- [ ] Decide how ExifTool is shipped (bundled or a user-installed path)
- [ ] Add a THIRD-PARTY-NOTICES file before the first release
