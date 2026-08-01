import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import test from "node:test";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const oxlint = path.join(root, "node_modules/oxlint/bin/oxlint");
const config = path.join(root, ".oxlintrc.json");

test("canonical class rule fixes Tailwind utilities only in class contexts", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "nirmoka-tailwind-lint-"));
  const fixture = path.join(directory, "fixture.tsx");
  const input = [
    'const label = "min-h-[640px]";',
    'const cn = (...values) => values.join(" ");',
    'const helperClasses = cn("rounded-[4px]", "max-w-[1320px]");',
    'const view = <div className="min-h-[640px] w-[204px]" />;',
    "const template = <div className={`h-[416px]`} />;",
    "",
  ].join("\n");

  try {
    await writeFile(fixture, input);
    execFileSync(
      process.execPath,
      [
        oxlint,
        "--config",
        config,
        "--allow",
        "all",
        "--deny",
        "nirmoka-tailwind/canonical-classes",
        "--fix",
        fixture,
      ],
      { cwd: root, stdio: "pipe" },
    );

    const output = await readFile(fixture, "utf8");
    assert.equal(
      output,
      input
        .replace('cn("rounded-[4px]", "max-w-[1320px]")', 'cn("rounded-lg", "max-w-330")')
        .replace('className="min-h-[640px] w-[204px]"', 'className="min-h-160 w-51"')
        .replace("className={`h-[416px]`}", "className={`h-104`}"),
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
