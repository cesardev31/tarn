# ADR 0001 — Name and file extension

Status: accepted (2026-10-05)

## Candidates considered

| Name   | Ext      | Notes |
|--------|----------|-------|
| **Tarn** | `.tarn` | A small, deep mountain lake. 4 letters, one syllable, same in English and Spanish. |
| Kestrel | `.kes` | Strong image, but 7 letters is long for a CLI typed hundreds of times a day. |
| Ostra  | `.ost`   | Pronounceable; `.ost` collides with Outlook offline storage files. |
| Vane   | `.vn`    | Short; `.vn` is a country TLD and visually noisy. |
| Lumo   | `.lumo`  | Friendly, but reads like a UI/design brand. |
| Corvo  | `.cv`    | `.cv` is strongly associated with résumés. |
| Brio   | `.brio`  | Taken by several JS/Ruby projects. |
| Ardo   | `.ard`   | Fine, but weak meaning and close to "Arduino" in searches. |

## Decision

- Language name: **Tarn**
- CLI command: `tarn`
- Source extension: `.tarn`
- VS Code language id: `tarn`; extension id `tarn-lang.tarn`
- Global cache: `~/.tarn/`

## Rationale

Short, pronounceable, not a portmanteau of existing languages, works as a
command and as an identifier. The image fits the goals: small, deep, clear,
self-contained. Searching "tarn lang" is unambiguous enough. Using the full name
as extension (like `.zig`, `.odin`) avoids collisions with short, overloaded
extensions.

Open item: a formal trademark / package-registry check was not done; revisit
before any public release.
