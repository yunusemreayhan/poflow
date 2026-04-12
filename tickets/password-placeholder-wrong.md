# BUG: Password placeholder says "min 6 chars" but backend requires 8+

## Found by
E2E test exploration via WebDriver

## Location
`gui/src/components/AuthScreen.tsx` — password input placeholder

## Expected
Placeholder should say `Password (min 8 chars, uppercase + digit)` to match the backend's `validate_password` rules.

## Actual
Placeholder says `Password (min 6 chars)` which misleads users into entering passwords that will be rejected.

## Backend validation (from user-registration.md flow)
- Min 8 characters, max 128
- Must contain at least one uppercase letter
- Must contain at least one digit

## Severity
Low — cosmetic/UX, but causes confusing registration failures.
