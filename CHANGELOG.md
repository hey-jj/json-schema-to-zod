# Changelog

## [0.2.0] - 2026-07-07

### Changed
- Object and array `const` values now match equal JSON values instead of object identity. (#18)
- Arrays with `uniqueItems: true` now reject duplicate object and array values. (#19)
- Typed schemas that also use `if`, `then`, or `else` now apply the conditional validation after the base type validation. (#20)
- Nested `contentSchema` output now follows the selected Zod version and active parser context. (#21)

## [0.2.0] - 2026-07-07

### Changed
- Object and array `const` values now match equal JSON values instead of object identity. (#18)
- Arrays with `uniqueItems: true` now reject duplicate object and array values. (#19)
- Typed schemas that also use `if`, `then`, or `else` now apply the conditional validation after the base type validation. (#20)
- Nested `contentSchema` output now follows the selected Zod version and active parser context. (#21)
