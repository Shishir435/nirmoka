# Notice and Attribution

Nirmoka is an independent open source project, licensed under the Apache License 2.0.

Nirmoka **does not include, copy, link, or redistribute** any code from the tools listed
below. It invokes them as separate processes on the user's own machine, reads their
documented structured output, and renders it. No backend binary is bundled with Nirmoka;
each must be installed independently by the user.

Because the connection is an arm's-length process boundary rather than linking or code
reuse, the licenses of these tools do not extend to Nirmoka's source. They are listed here
because credit is owed regardless of what the law requires.

---

## Backend tools

### Mole

- Project: <https://github.com/tw93/Mole>
- License: GNU General Public License v3.0
- Role: macOS backend, providing disk analysis, cleanup, and application uninstall

Mole's curated cleanup target lists, protected-path rules, and app-protection data are
part of Mole and remain under GPL-3.0. Nirmoka reads Mole's output; it must never
transcribe those lists into its own source. Doing so would make Nirmoka a derivative work.

"Mole" and the Mole logo are trademarks of the Mole project and are used here only to
refer to that project. Nirmoka is not affiliated with, endorsed by, or sponsored by Mole.
Mole for Mac (<https://mole.fit>) is a separate proprietary product; Nirmoka is unrelated
to it and is not a substitute for it.

### ncdu

- Project: <https://dev.yorhel.nl/ncdu>
- Author: Yoran Heling
- License: MIT
- Role: cross-platform baseline backend

Nirmoka uses ncdu's documented JSON export format as its internal wire format. The format
is a published specification; no ncdu source code is used.

### gdu

- Project: <https://github.com/dundee/gdu>
- License: MIT
- Role: planned cross-platform backend with ncdu-compatible export

---

## Trademarks

"Nirmoka" is the name of this project. Consistent with Section 6 of the Apache License
2.0, the license granted for this source code does not grant permission to use the project
name or its logo.

If you fork Nirmoka and publish the result, please give it a different name so users are
not misled about what they are installing, and do not imply that your fork is endorsed by
or affiliated with this project. This is the same courtesy Nirmoka extends to the projects
it builds on.
