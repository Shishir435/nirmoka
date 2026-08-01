# Fixtures

Recorded output from real backends, committed so the contract suite runs
everywhere — including on CI machines where the backend cannot be installed.

- `ncdu/<version>/` — produced by `scripts/record-ncdu-fixture.sh`.
- `gdu/<version>/` — produced by `scripts/record-gdu-fixture.sh`.
- `mole/<version>/` — recorded analyzer evidence produced by
  `scripts/record-mole-fixture.sh`; these are not wire-format fixtures.

## Rules

- **Recorded, not written.** Every file here came out of a real backend. Making
  one up defeats the purpose: the suite exists to catch the difference between
  what a backend documents and what it emits.
- **One directory per backend version.** A format drift should appear as a new
  directory beside the old one, so the previous evidence survives.
- **One normalisation, and only one.** The scan root is rewritten to
  `/fixtures/<name>` so the recording machine's home directory does not end up
  in the repository and so path assertions are stable. Nothing else is touched.
- `unreadable/` is recorded from a real `chmod 000` directory, so the
  `read_error` flag in these files is genuine.
