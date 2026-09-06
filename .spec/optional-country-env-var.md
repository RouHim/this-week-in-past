# Feature Specification: Home-country-aware location display

**Created**: 2026-09-06
**Status**: Approved
**Input**: New optional country env var; same-country photos hide the country, foreign photos show the country name.

## Goal
Photo slideshow viewers want location context only when it matters. A new optional `HOME_COUNTRY` environment variable records the viewer's home country as an ISO code. When a photo was taken in the home country the display omits the country suffix and stays as short as today; when taken abroad the display appends the English country name. When the variable is unset, behavior is exactly as today.

## User Scenarios
### Scenario 1 - Local photo stays short (P1)
An operator sets `HOME_COUNTRY=DE`. A photo taken in Bayenthal resolves to `Bayenthal, Köln` with no country suffix, just as without the feature.

**Acceptance**
1. Given `HOME_COUNTRY=DE` and a photo resolving to a German city, When the display value is built, Then it contains the city (and district, if any) and no country name.
2. Given `HOME_COUNTRY=de` (lowercase) or `HOME_COUNTRY= DE ` (whitespace), When the display value is built for a German photo, Then the result is identical to Scenario 1 step 1.

### Scenario 2 - Foreign photo gains country context (P1)
An operator sets `HOME_COUNTRY=DE`. A photo taken in Christianshavn resolves to `Christianshavn, København, Denmark`.

**Acceptance**
1. Given `HOME_COUNTRY=DE` and a photo resolving to a Danish city, When the display value is built, Then it ends with `, Denmark` after the city portion.
2. Given `HOME_COUNTRY=DE` and a photo resolving to a plain (non-district) foreign city, When the display value is built, Then it has the form `City, Country`.

### Scenario 3 - Variable unset preserves current behavior (P2)
No `HOME_COUNTRY` is set (or it is empty). All photos display exactly as before this feature: district/city with no country suffix, including foreign photos.

**Acceptance**
1. Given no `HOME_COUNTRY` and a photo resolving to a Danish city, When the display value is built, Then it contains no country name.
2. Given an empty `HOME_COUNTRY` value, When the display value is built, Then the result is identical to Scenario 3 step 1.

### Scenario 4 - Invalid value fails fast (P2)
`HOME_COUNTRY` is set to something that is not a known ISO-3166 alpha-2 code.

**Acceptance**
1. Given `HOME_COUNTRY=XX` (or any unassigned/ill-formed code), When the application starts, Then it refuses to start and reports a clear error naming the variable and the offending value.

## Functional Requirements
- **FR-001**: The application reads an optional `HOME_COUNTRY` environment variable at startup; the value is trimmed and matched case-insensitively as an ISO-3166 alpha-2 code.
- **FR-002**: The photo's country is the ISO-3166 alpha-2 code from the resolved city's GeoNames record; when a district resolves hierarchically to a parent city, the parent city's code decides.
- **FR-003**: When the photo country code equals `HOME_COUNTRY`, the display value contains no country portion (district/city portion unchanged).
- **FR-004**: When the photo country code differs from `HOME_COUNTRY`, the display value appends `, ` plus the English short country name after the city portion.
- **FR-005**: When `HOME_COUNTRY` is unset or empty, no country name is ever shown (behavior identical to before this feature).
- **FR-006**: When the photo resolves to a city with no determinable country code, or resolves to no city at all, the display falls back to city-only or nothing respectively; a bare country name is never shown.
- **FR-007**: When `HOME_COUNTRY` is set but is not a known ISO-3166 alpha-2 code, startup fails with an error that names the variable and the offending value.
- **FR-008**: Country resolution uses only the offline GeoNames data plus a bundled ISO-2 to English-name mapping; no network calls are introduced by this feature.

## Key Entities
- **Home country**: the operator-configured ISO-3166 alpha-2 code from `HOME_COUNTRY`; absent means "no home base".
- **Photo country**: the ISO-3166 alpha-2 code of the resolved city (or of the parent city for hierarchical district displays).

## Edge Cases
- `HOME_COUNTRY` with surrounding whitespace or lowercase letters is normalized before comparison.
- A district and its parent city in different countries uses the parent city's country for the home/foreign decision.
- Photos over ocean or otherwise unresolvable behave exactly as before (no city, no country, no error).
- Changing `HOME_COUNTRY` takes effect on restart; no hot-reload is required.
- Country names are English short names joined with `, ` (e.g. `København, Denmark`).

## Research Notes
- http://download.geonames.org/export/dump/readme.txt — the `geoname` table carries `country code: ISO-3166 2-letter` but no country name, so an ISO-2 to English-name mapping must be bundled; the country column must be (re-)parsed from the dump since it is not currently retained.

## Assumptions
- The brief's `HOEM_COUNTRY` was a typo for `HOME_COUNTRY` (confirmed during clarification).
- Country names shown are the English short names (e.g. `Denmark`, not `Dänemark`).
- The display separator between city and country portions is `, `.

## Success Criteria
- **SC-001**: With `HOME_COUNTRY=DE`, a Bayenthal photo displays `Bayenthal, Köln` and a Christianshavn photo displays `Christianshavn, København, Denmark`.
- **SC-002**: With the variable unset, foreign and local photos display exactly as before this feature (no country names anywhere).
- **SC-003**: With an invalid code, the application refuses to start with an error naming `HOME_COUNTRY` and the value.
- **SC-004**: No new network traffic and no new mandatory configuration are introduced; existing displays without the variable are byte-identical to before.
