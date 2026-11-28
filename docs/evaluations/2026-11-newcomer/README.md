# Newcomer test: Python and JavaScript against SQLite

Date: 2026-11-28. Q-010 decided an offline, static evaluation; there will be no
networked follow-up run. This report therefore separates what the public
documentation says, what the scripts infer, and what cannot be measured
without installing and running the published packages.

## Question and setup

The newcomer knows either Python or JavaScript and ordinary SQLite use, but has
never seen fid. They have a clean directory, a throwaway SQLite database, the
public package registry entries, and the public README. The question is: how
many commands, errors, and unsuccessful README lookups stand between that
starting point and a first useful result?

The allowed sources for using fid are deliberately narrow:

- the repository `README.md`;
- `bindings/python/README.md`, because that is the Python package's public
  PyPI documentation; and
- the installed tool's own `fid --help`, `fid <cmd> --help`, and Python
  `help(fid)` output.

Source code, bindings implementation code, and tests are not sources for this
walkthrough. In this offline authoring environment neither `import fid` nor a
`fid` command is installed, so their self-help output is unavailable.

The supplied `make_db.py` uses only Python's standard library and seed 7. It
creates three company-CEO facts and two ship positions in `facts`, a fixed
`2026-11-28T12:00:00Z` value in `meta`, and 2,000 synthetic re-checks for 200
entities in `rechecks`. Re-check ages range from 0 to 180 days. Their known
probability of being correct is `0.3 + 0.7 * 0.5^(age / 30 days)`, so the
truth against which calibration is compared is a half-life of 30 days and a
baseline of 0.30.

The four intended steps are:

1. Stamp every fact as source class `database`, using a 5.8-year half-life for
   CEOs and a 3-minute half-life for ship positions, and save each artifact as
   JSON in a new `facts.artifact_json` column.
2. Check every artifact against floor 0.7 and print its status and expiry.
3. Export the re-check ledger, calibrate it with the Python function and, when
   available, the CLI; the JavaScript route has only the CLI available.
4. Print fitted half-life and baseline beside the known truth.

Each runtime step catches and reports its error before the next step runs. A
missing JavaScript prerequisite is instead the explicit STEP 0 result described
below.

## Protocol

Stage the supplied files before starting the clock. In a clean Python
directory, place copies of `make_db.py` and `python/walkthrough.py`, renaming
the latter to `walkthrough.py`. In a separate clean JavaScript directory,
place copies of `make_db.py`, `js/walkthrough.mjs`, and `js/package.json`.
Separate directories matter because each walkthrough adds `artifact_json` to
its own database and writes its own ledger and registry outputs.

### Python route

From the prepared clean directory, in this order:

```sh
time python3 -m venv .venv
. .venv/bin/activate
time pip install fidmeter
python make_db.py facts.db
time python walkthrough.py facts.db
```

The first useful Python result is the per-fact status and expiry, followed by
the fitted decay parameters beside their known truth. The CLI calibration is
conditional: the walkthrough logs a skip when `fid` is not on `PATH` rather
than treating that documented packaging split as a crash.

### JavaScript route

From the prepared clean directory, in this order:

```sh
node --version
npm view fidmeter
npm search fid fidelity
if command -v cargo >/dev/null 2>&1; then
  time cargo install fidmeter
else
  printf 'Rust toolchain absent; cargo install skipped\n'
fi
python3 make_db.py facts.db
time node walkthrough.mjs facts.db
```

Node must be 22.5 or newer for the built-in `node:sqlite` module. The two npm
commands are the package-discovery checks; the walkthrough only prints those
commands and never silently substitutes an npm result. If no Rust toolchain is
available, retain the skip message and run the walkthrough: its STEP 0 outcome
is part of the evaluation.

### Measurements to record

Time the virtualenv creation, each package installation, and each walkthrough
run separately. Preserve the complete output and exit status of each timed
command.

Count:

- **Commands typed:** every command submitted at the shell, including
  activation, version checks, registry searches, the toolchain check, database
  creation, and failed attempts; a multiline command counts once, and commands
  spawned inside a walkthrough are not additional typed commands.
- **Errors hit:** every failed command, every `STEP n FAILED`, and every
  recoverable per-fact or calibration error, retaining its text and the step at
  which recovery occurred. An explicitly logged optional-CLI skip is recorded
  separately, not silently converted into a successful CLI calibration.
- **README lookups that failed:** every question for which the two READMEs and
  installed self-help do not supply an answer, including answers that had to be
  inferred from another language's example.

The static findings below supply the documentation lookups, not invented
command, timing, or runtime-error measurements.

## Static walkthrough

This section records how the scripts were written from the allowed documents.
It does not claim that a step passed: neither fid implementation was installed
when this was prepared.

### JavaScript STEP 0: discover a package and a command

The root README's install block says:

> `cargo install fidmeter        # the fid command`
> `pip install fidmeter          # import fid`

Its only language-specific usage section is headed `## Python`. There is no
JavaScript package example. The script consequently prints the two npm
discovery commands, probes `fid --help`, and stops with the specified STEP 0
message if no command exists. When the command exists, all four later steps
use `child_process.spawnSync`; there are no npm dependencies.

### STEP 1: stamp and persist the artifacts

The Python README gives this directly useful example:

> `artifact = fid.stamp(`
> `    {"ceo": "Ada"}, source="registry", source_class="database", half_life="5.8y"`
> `)`

Its function table also gives `stamp(content, *, source, source_class, noise,
half_life, baseline, type, state, condition, upstream, now)`. The root README's
configuration includes `company_ceo: 5.8y` and vessel-position half-lives in
minutes, which supply the duration syntax.

For Python, the script copies the fact fields into a content dictionary, calls
`fid.stamp` with the documented source class and half-life, serializes the
returned dictionary, and updates `artifact_json`. For an existing SQLite fact,
it passes `measured_at` as stamp's `now`; the fixed meta timestamp is reserved
for checking. That mapping is an inference about `now`, not an explained
SQLite example in the README (finding 1).

For JavaScript, the root README demonstrates stdin stamping and
`--source-class`, but its registry example has `--half-lives`, not a literal
half-life option. The script infers `--half-life 5.8y` or `--half-life 3m` from
the Python argument name, and `--now <measured_at>` from the Python signature.
It writes stdout through `JSON.parse` before storing it, so malformed artifact
output becomes a step error. Neither inferred CLI option could be checked
offline (finding 2).

### STEP 2: check status and expiry

The root README gives both CLI forms:

> `fid check patient-record.json --floor 0.70`
> `fid ttl record.json --floor 0.70      # prints expires_at, or "never"`

It also defines exits 0, 1, and 2 as `SIGNAL`, `NOISE`, and `UNKNOWN`. The
Python README gives `fid.check(artifact, floor=0.7).status` and
`fid.expires_at(artifact, floor=0.7)`, with `expires_at` returning a datetime
or `None`.

The Python route calls those two functions for every stored artifact and
prints the enum status and ISO expiry (or `never`). It supplies the fixed meta
value through check's documented `now` keyword. The JavaScript route writes
each artifact to a temporary JSON file because the documented CLI takes a file
path, accepts 0/1/2 as epistemic results rather than process failures, and runs
`ttl` for the absolute expiry. A fixed historical check also needs a CLI
evaluation time; the script infers `check --now`, which the CLI README does not
show (finding 3).

### STEP 3: export and calibrate the re-checks

The root README's **Calibrate your own data** section specifies the CSV:

> Input is CSV: `entity`, `correct`, and either `age` (seconds or a
> duration like `30d`) or `observed_at` and `checked_at` (RFC 3339)

Both routes therefore write exactly the header
`entity,correct,observed_at,checked_at`. The Python README additionally says:

> `# Calibrate from facts you re-checked: (entity, age, still correct?, [state])`
> `cal = fid.calibrate(rows, "policy_claim")`
> `open("half-lives.yaml", "w").write(cal.yaml)`
> `print(cal.report)`

The Python route converts each RFC 3339 pair to an age in seconds and a boolean
`correct` value, calls `fid.calibrate(rows, "synthetic_rechecks")`, and writes
`calibration.yaml`. It then runs
`fid calibrate rechecks.csv --type synthetic_rechecks` only when `shutil.which`
finds the command, writing stdout as the CLI registry and retaining stderr as
the report. The JavaScript route runs that same documented CLI calibration and
writes its own registry. A missing command is logged and skipped in Python; in
JavaScript it has already produced STEP 0. The pip-only route cannot complete
the second Python calibration (finding 5).

### STEP 4: compare fit with truth

The root README's example report contains:

> `company_ceo — 91799 observations, 283 entities`
> `  half-life 5.84y, baseline 0.00, correct when fresh 1.000`

The Python function table promises `Calibration` values `yaml`, `report`, and
`fits`, but does not name structured attributes on a fit for the half-life and
baseline. Rather than invent those attributes, both scripts extract the two
values from the documented report shape shown above. They print
`<route> fit: half-life ..., baseline ... | truth: half-life 30d, baseline
0.30`. This parsing resolution is finding 4; its success on the published
implementation is not claimed offline.

## Registry facts (given, not measured)

This test was written with no network. The following facts were checked on
2026-11-28 outside the test and are included as given, not as measurements made
by these scripts:

- fidmeter 0.2.1 is on PyPI (Python >= 3.9) and on crates.io.
- npm has no `fidmeter` package.
- The PyPI wheel provides `import fid` but no `fid` command.
- There is no JavaScript binding.

## What an offline test cannot show

- **Install time and install errors from PyPI and crates.io:** no package
  download, resolver run, wheel selection, native build, or installer failure
  was exercised, so the report cannot characterize how long installation takes
  or which errors a newcomer would encounter.
- **Whether the published wheel matches the README:** the wheel was not
  imported or inspected, so the documented Python signatures, returned report
  shape, and behavior on these scripts cannot be compared with the published
  artifact.

The static report likewise does not turn inferred CLI flags or unexecuted fit
parsing into measured runtime successes.

## JS result (decided offline)

Given no npm `fidmeter` package, no JavaScript binding, and no `fid` command
from the PyPI wheel, a JavaScript newcomer who has only the Python public
package stops before database work. The decided result is:

```text
STEP 0 FAILED: no fid command; JS has no package and needs `cargo install fidmeter`
```

This is a finding about the public onboarding path, not a script bug to hide.
A separately installed Rust toolchain and `cargo install fidmeter` are the
documented route that could make the remaining CLI steps possible, but their
runtime outcome was not measured.

## Findings so far

These findings come only from the static walkthrough. “Observed offline” marks
a documentation or given-registry observation; “not checkable offline” marks
an inferred interface whose runtime acceptance cannot be tested here.

1. **The timestamp needed to stamp an existing database fact is not explained.**
   README line: `bindings/python/README.md` line 62 lists `now` in `stamp(...)`
   but the usage example at lines 35–40 only stamps a fresh literal. Resolution:
   pass `measured_at` to stamp and the fixed meta time to check, explicitly as
   an inference. Suggested fix: add an SQLite-style example stating that an
   existing observation's `measured_at` is supplied as stamp's `now` while
   check's `now` is the evaluation time. Status: **observed offline**.

2. **The README does not document literal half-life or observation-time flags
   for CLI stamping.** README line: root `README.md` lines 38–39 show
   `fid stamp ... --half-lives half-lives.yaml --type vessel_position`, while
   the literal `half_life` and `now` names appear only in the Python API.
   Resolution: infer kebab-case `--half-life` and `--now`, and leave any CLI
   rejection as the step's error. Suggested fix: add a complete CLI example
   that stamps an old JSON record with a literal half-life and observation
   timestamp. Status: **not checkable offline**.

3. **The README does not say how to make CLI checking use a fixed evaluation
   time.** README line: root `README.md` line 29 shows
   `fid check patient-record.json --floor 0.70`, whereas
   `bindings/python/README.md` line 63 includes `check(stamped, floor, *, now)`.
   Resolution: infer `fid check ... --now 2026-11-28T12:00:00Z`; `ttl` needs no
   such flag because it reports the artifact's absolute expiry. Suggested fix:
   document check's evaluation-time option beside the exit-code example.
   Status: **not checkable offline**.

4. **Structured access to calibrated half-life and baseline is not documented.**
   README line: `bindings/python/README.md` line 70 says each entry in
   `Calibration.fits` has 95% ranges but gives no half-life or baseline
   attribute; root `README.md` lines 205–208 show those values only in report
   text. Resolution: parse the illustrated report line rather than guess object
   internals. Suggested fix: document stable fit attributes for half-life and
   baseline, including their units and baseline-omission behavior. Status:
   **observed offline**.

5. **The pip-only Python route cannot perform the README's CLI calibration.**
   README line: root `README.md` lines 141–143 assign the command to
   `cargo install fidmeter` and only the import to `pip install fidmeter`, but
   the calibration example at line 205 assumes `fid` is already present.
   Resolution: detect the command, log `CLI calibrate skipped: no fid command
   on PATH`, and retain the Python-function result. Suggested fix: put a short
   prerequisite note beside the calibration example saying that the CLI needs
   the separate Cargo installation. Status: **observed offline**.

6. **There is no documented JavaScript onboarding path when only public
   language packages are considered.** README line: root `README.md` lines
   183–194 provide a `## Python` API section but no JavaScript counterpart,
   while lines 141–143 provide the CLI only through Cargo; the given registry
   facts add that npm has no package and there is no JS binding. Resolution:
   print the npm discovery commands, probe the CLI, and record STEP 0 rather
   than emulate fid in JavaScript. Suggested fix: add a JavaScript note stating
   that no binding or npm package exists and that the CLI from Cargo is
   required. Status: **observed offline**.
