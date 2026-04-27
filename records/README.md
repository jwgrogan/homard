# Pediatric Immunization Records

This folder tracks vaccine history as source-backed records for each child.

## Principles

- **Source-backed only:** do not infer completed vaccines or future due dates without a cited source document.
- **Unknown stays unknown:** missing or uncertain history must be explicitly marked `unknown`.
- **Separation of concerns:** schedule recommendation logic can be layered later; this tracker only stores evidence-backed facts and clearly stated due/overdue statuses when explicitly documented.

## File Layout

- `records/_schemas/immunization-tracker.schema.json` — canonical JSON shape.
- `records/<child>/vaccines.md` — human-maintained markdown source of truth.
- `records/<child>/vaccines.generated.json` — machine-readable projection for dashboards/automation.
- `records/dashboard/vaccine-tracker.md` — first usable cross-child tracker surface.
