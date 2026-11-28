import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import process from "node:process";
import { DatabaseSync } from "node:sqlite";

console.log("JS package checks (commands only; not run by this script):");
console.log("  npm view fidmeter");
console.log("  npm search fid fidelity");

const probe = spawnSync("fid", ["--help"], { encoding: "utf8" });
if (probe.error || probe.status !== 0) {
  if (probe.error?.code === "ENOENT") {
    console.log(
      "STEP 0 FAILED: no fid command; JS has no package and needs `cargo install fidmeter`",
    );
  } else {
    const detail = probe.error?.message || probe.stderr.trim();
    console.log(`STEP 0 FAILED: fid --help did not run: ${detail}`);
  }
  process.exit(0);
}

const databasePath = process.argv[2] || "facts.db";
const database = new DatabaseSync(databasePath);
const temporaryDirectory = mkdtempSync(join(tmpdir(), "fid-newcomer-"));
const now = database
  .prepare("SELECT value FROM meta WHERE key = 'now'")
  .get().value;
const fits = [];

function runFid(args, input) {
  const result = spawnSync("fid", args, { input, encoding: "utf8" });
  if (result.error) throw result.error;
  return result;
}

function requireStatus(result, allowed, command) {
  if (!allowed.includes(result.status)) {
    const detail = (result.stderr || result.stdout).trim();
    throw new Error(`${command} exited ${result.status}: ${detail}`);
  }
  return result;
}

function stampFacts() {
  database.exec("ALTER TABLE facts ADD COLUMN artifact_json TEXT");
  const rows = database
    .prepare(
      `SELECT id, entity, field, value, source, measured_at
       FROM facts ORDER BY id`,
    )
    .all();
  const errors = [];
  const update = database.prepare(
    "UPDATE facts SET artifact_json = ? WHERE id = ?",
  );
  for (const row of rows) {
    try {
      const content = {
        id: row.id,
        entity: row.entity,
        field: row.field,
        value: row.value,
        source: row.source,
        measured_at: row.measured_at,
      };
      const halfLife = row.field === "ceo" ? "5.8y" : "3m";
      const result = requireStatus(
        runFid(
          [
            "stamp",
            "--source",
            row.source,
            "--source-class",
            "database",
            "--half-life",
            halfLife,
            "--now",
            row.measured_at,
          ],
          JSON.stringify(content),
        ),
        [0],
        "fid stamp",
      );
      const artifact = JSON.parse(result.stdout);
      update.run(JSON.stringify(artifact), row.id);
    } catch (error) {
      errors.push(`fact ${row.id}: ${error.message}`);
    }
  }
  if (errors.length) throw new Error(errors.join("; "));
}

function checkFacts() {
  const rows = database
    .prepare("SELECT id, artifact_json FROM facts ORDER BY id")
    .all();
  const errors = [];
  const statusNames = ["SIGNAL", "NOISE", "UNKNOWN"];
  for (const row of rows) {
    if (row.artifact_json === null) {
      errors.push(`fact ${row.id}: no stamped artifact`);
      continue;
    }
    try {
      const artifactPath = join(temporaryDirectory, `fact-${row.id}.json`);
      writeFileSync(artifactPath, row.artifact_json);
      const check = requireStatus(
        runFid([
          "check",
          artifactPath,
          "--floor",
          "0.7",
          "--now",
          now,
        ]),
        [0, 1, 2],
        "fid check",
      );
      const ttl = requireStatus(
        runFid(["ttl", artifactPath, "--floor", "0.7"]),
        [0, 1, 2],
        "fid ttl",
      );
      console.log(
        `fact ${row.id}: status ${statusNames[check.status]}; ` +
          `${check.stdout.trim()}; expires_at ${ttl.stdout.trim()}`,
      );
    } catch (error) {
      errors.push(`fact ${row.id}: ${error.message}`);
    }
  }
  if (errors.length) throw new Error(errors.join("; "));
}

function csvCell(value) {
  const text = String(value);
  return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

function fitFromReport(report) {
  const match = report.match(/half-life\s+([^,\s]+),\s*baseline\s+([^,\s]+)/);
  if (!match) {
    throw new Error("the calibration report has no half-life and baseline line");
  }
  return { halfLife: match[1], baseline: match[2] };
}

function calibrateRechecks() {
  const rows = database
    .prepare(
      `SELECT entity, correct, observed_at, checked_at
       FROM rechecks ORDER BY entity, observed_at`,
    )
    .all();
  const csvPath = join(dirname(databasePath), "rechecks.csv");
  const lines = ["entity,correct,observed_at,checked_at"];
  for (const row of rows) {
    lines.push(
      [row.entity, row.correct, row.observed_at, row.checked_at]
        .map(csvCell)
        .join(","),
    );
  }
  writeFileSync(csvPath, `${lines.join("\n")}\n`);
  console.log(`exported ${rows.length} re-checks to ${csvPath}`);

  const result = requireStatus(
    runFid(["calibrate", csvPath, "--type", "synthetic_rechecks"]),
    [0],
    "fid calibrate",
  );
  const registryPath = join(dirname(databasePath), "js-half-lives.yaml");
  writeFileSync(registryPath, result.stdout);
  fits.push({ method: "CLI", ...fitFromReport(result.stderr) });
  console.log(`CLI registry: ${registryPath}`);
}

function printFits() {
  if (!fits.length) {
    throw new Error("no fit available; truth is half-life 30d, baseline 0.30");
  }
  for (const fit of fits) {
    console.log(
      `${fit.method} fit: half-life ${fit.halfLife}, baseline ${fit.baseline} | ` +
        "truth: half-life 30d, baseline 0.30",
    );
  }
}

function runStep(number, step) {
  try {
    step();
    console.log(`STEP ${number} ok`);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.log(`STEP ${number} FAILED: ${message.replaceAll("\n", " | ")}`);
  }
}

try {
  runStep(1, stampFacts);
  runStep(2, checkFacts);
  runStep(3, calibrateRechecks);
  runStep(4, printFits);
} finally {
  database.close();
  rmSync(temporaryDirectory, { recursive: true, force: true });
}
