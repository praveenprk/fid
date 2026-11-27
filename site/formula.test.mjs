// The live board's formula is fid's: run the page's own fid-formula block
// against the `fidelity` and `time_to_floor` cases in conformance/vectors.json,
// so the site cannot drift from the library (live-site plan, rule 3; #60).
// Plain node, no dependencies: node site/formula.test.mjs
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

const block = read("shell.html").match(/\/\/ fid-formula:begin[\s\S]*?\/\/ fid-formula:end/);
if (!block) throw new Error("site/shell.html has no fid-formula:begin … fid-formula:end block");
const { atMeasurement, fidelity, timeToFloor } = new Function(
  `${block[0]}\nreturn { atMeasurement, fidelity, timeToFloor };`,
)();

const v = JSON.parse(read("../conformance/vectors.json"));
const failures = [];
const check = (group, name, got, want, tol) => {
  const ok = want === null ? got === Infinity : Math.abs(got - want) <= tol;
  if (!ok) failures.push(`${group} "${name}": got ${got}, want ${want === null ? "never" : want}`);
};

for (const c of v.fidelity) {
  const f = atMeasurement(c.source, c.noise);
  check(
    "fidelity",
    c.name,
    fidelity(f, c.half_life_seconds, c.baseline, c.age_seconds),
    c.fidelity,
    v.tolerance,
  );
}
// The draft: within 1e-6 s of the first crossing (the Rust tests use the same).
for (const c of v.time_to_floor) {
  check(
    "time_to_floor",
    c.name,
    timeToFloor(c.fidelity, c.half_life_seconds, c.baseline, c.floor),
    c.seconds,
    1e-6,
  );
}

const total = v.fidelity.length + v.time_to_floor.length;
if (failures.length) {
  console.error(failures.join("\n"));
  console.error(`${failures.length} of ${total} cases failed`);
  process.exit(1);
}
console.log(`site formula matches conformance/vectors.json: ${total} cases`);
