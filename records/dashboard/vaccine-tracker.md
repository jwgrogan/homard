# Family Vaccine Tracker (Initial Surface)

_As of 2026-04-27_

This dashboard is a source-backed tracker view. It intentionally does **not** infer schedules from age alone when records are missing.

| Child | Tracker file | Overall status | Due/overdue visibility | Source-backed? | Gaps |
|---|---|---|---|---|---|
| Élodie | `records/elodie/vaccines.md` | Unknown history | Not determinable from current sources | Partial (placeholder only) | Immunization records not checked in |
| Olivia | `records/olivia/vaccines.md` | Unknown history | Not determinable from current sources | Partial (placeholder only) | Immunization records not checked in |

## How due status is represented

- `due` and `overdue` are allowed statuses in the JSON schema and markdown rows.
- They should only be used when supported by a source document or clinician note.
- Until then, keep status as `unknown` and explicitly call out missing records.
