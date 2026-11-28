// The live cards' shared feed, space-weather and satellite rules, evaluated
// from the page itself (live-site plan, rules 1 and 5; #60).
// Plain node, no dependencies: node site/live.test.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const shell = readFileSync(new URL("shell.html", import.meta.url), "utf8");
const block = shell.match(/\/\/ live-rules:begin[\s\S]*?\/\/ live-rules:end/);
if (!block) throw new Error("site/shell.html has no live-rules:begin … live-rules:end block");
const { latestKp, latestWind, kpAge, feedUnknown, elementSetAge, tleUnknown, illustrationSeconds } =
  new Function(
    `${block[0]}\nreturn { latestKp, latestWind, kpAge, feedUnknown, elementSetAge, tleUnknown, illustrationSeconds };`,
  )();

const kpRows = [
  { time_tag: "2026-11-20T00:00:00", Kp: 0.67, a_running: 3, station_count: 8 },
  { time_tag: "2026-11-27T15:00:00", Kp: 0.33, a_running: 2, station_count: 8 },
];
const kpStart = Date.parse("2026-11-27T15:00:00Z") / 1000;
assert.deepEqual(latestKp(kpRows), { start: kpStart, kp: 0.33 });
assert.deepEqual(
  latestKp([...kpRows, { time_tag: "2026-11-27T18:00:00", Kp: null }]),
  { start: kpStart, kp: 0.33 },
  "a last row with null Kp is skipped",
);
assert.equal(latestKp([]), null);

const windRows = [
  {
    time_tag: "2026-11-27T18:49:00",
    active: false,
    source: "ACE",
    proton_speed: 325.28,
    proton_density: 6.04,
  },
  {
    time_tag: "2026-11-27T18:48:00",
    active: true,
    source: "SOLAR1",
    proton_speed: 307.1,
    proton_density: 5.9,
  },
];
const wind = {
  t: Date.parse("2026-11-27T18:48:00Z") / 1000,
  speed: 307.1,
  source: "SOLAR1",
};
assert.deepEqual(latestWind(windRows), wind);
assert.deepEqual(
  latestWind([
    { time_tag: "2026-11-27T18:50:00", active: true, source: "ACE", proton_speed: null },
    ...windRows,
  ]),
  wind,
  "an active row with null proton_speed is skipped",
);
assert.equal(latestWind([]), null);

assert.equal(kpAge(kpStart, kpStart + 3600), 0, "Kp age is zero while the interval runs");
assert.equal(kpAge(kpStart, kpStart + 4 * 3600), 3600, "Kp age starts at the interval end");
assert.equal(kpAge(kpStart, kpStart - 3600), 0, "Kp age is never negative");

assert.equal(feedUnknown(undefined, 0, 1000), false);
assert.equal(feedUnknown({ reached: 0, failed: true }, 0, 1000), true);
assert.equal(feedUnknown({ reached: null, failed: false }, 0, 1000), true);
assert.equal(feedUnknown({ reached: 0, failed: false }, 2001, 1000), true);
assert.equal(feedUnknown({ reached: 0, failed: false }, 2000, 1000), false);

const epoch = "2026-11-27T12:00:00";
const epochMs = Date.parse(epoch + "Z");
const fetchedMs = Date.parse("2026-11-27T13:00:00Z");
assert.equal(elementSetAge(epoch, epochMs + 90 * 1000), 90, "EPOCH is UTC without a zone");
const tle = { fetched_at: "2026-11-27T13:00:00Z" };
assert.equal(tleUnknown(null, fetchedMs), true);
assert.equal(tleUnknown(tle, fetchedMs + 6 * 60 * 60 * 1000 + 1), true);
assert.equal(tleUnknown(tle, fetchedMs + 6 * 60 * 60 * 1000), false);
assert.equal(illustrationSeconds(8), 0.125);

console.log("site live rules: 19 assertions passed");
