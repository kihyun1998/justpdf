# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the actual label strings used in this repo's issue tracker.

| Label in mattpocock/skills | Label in our tracker | Meaning                                  |
| -------------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue  |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information |
| `ready-for-agent`          | `ready-for-agent`    | Fully specified, ready for an AFK agent  |
| `ready-for-human`          | `ready-for-human`    | Requires human implementation            |
| `wontfix`                  | `wontfix`            | Will not be actioned                     |

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding label string from this table.

Edit the right-hand column to match whatever vocabulary you actually use.

## Area labels

A separate axis from the triage roles: `area:*` says where the fix lands, so concurrent sessions can each take one area without touching the same files (`gh issue list --label area:<name>`). Every issue carries at least one; an issue that spans crates carries each area it touches.

| Label                   | Where                                                                   |
| ----------------------- | ----------------------------------------------------------------------- |
| `area:core-parser`      | `justpdf-core`: parser, xref, object resolution                         |
| `area:core-writer`      | `justpdf-core`: `writer/` (modify, clean, linearize, compress), object serialization |
| `area:core-font`        | `justpdf-core`: `font/`, `text/`                                        |
| `area:core-docfeatures` | `justpdf-core`: `annot/`, `form/`, `sign/`, `outline/`, `ocg/`, `page_label`, text strings |
| `area:core-decode`      | `justpdf-core`: `image/`, `color/`, `function.rs`                       |
| `area:core-crypto`      | `justpdf-core`: `crypto/`, trailer `/ID`                                |
| `area:render`           | `justpdf-render`                                                        |
| `area:formats`          | `justpdf-formats`, `justpdf-special`, `justpdf-cli`                     |
| `area:build`            | workspace manifests, CI, bindings (wasm, ffi, compress-wasm)            |

Order between issues is not a label — it goes in the issue as a link ("blocked by #N").
