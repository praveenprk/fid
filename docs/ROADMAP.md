# fid: where we are, in plain language, and what comes next

## The goal in one sentence

fid tells a system how much it can still trust a stored fact right now, using how fast that kind of fact really goes stale (measured, not guessed), so stale facts get re-checked before anyone acts on them.

## About "the guess"

You're right. In `docs/FINDINGS.md` I wrote "the first day back carries the same air mass", and nothing tested it. That sentence is already committed. First fix: say only what was measured. The first echo is stronger than the model predicts: 0.43 seen against 0.39 predicted. The air-mass idea is then either dropped or written down as an untested question. It could be tested later by comparing days when air pressure changed a lot with days when it didn't.

## One real example: a coast guard rescue

At 10:00 a ship reports its GPS position. At 10:20 a rescue boat is about to be sent there. Should it trust the 10:00 position?

| Word | What it means here |
|---|---|
| **fact** | "Ship X is at position P", stored at 10:00 |
| **source fidelity** | How often a fresh report is right. GPS: about 97 in 100. A rumour over the radio: about 17. |
| **half-life** | Time until half of such facts are wrong. A moving ship: **3 minutes**. A ship tied up in port: **1.4 days**. Same ships, same GPS; what matters is what the thing is doing (its *state*). |
| **decay** | Trust drops as time passes. A 20-minute-old position of a moving ship is nearly worthless. The same-age report of a moored ship is still nearly certain. |
| **baseline** | How often a stale fact is right by luck. "The weather at this airport is clear" is still right 71% of the time days later, because most days are clear. Trust falls toward 71%, not toward 0. |
| **echo** | Some facts come back. Yesterday's 3 pm temperature is a decent guess for today's 3 pm and a bad guess for tonight. Trust dips and returns each day. |
| **transform noise** | Each step that handles the fact loses a little: retyped, summarized by an AI, converted. Each step multiplies the trust down. |
| **fidelity** | All of the above combined into one number from 0 to 1: how likely the fact is still true right now. |
| **floor** | The trust a decision needs. Sending a rescue boat: 0.9. A weekly report: 0.5. This is policy, not maths, and you choose it. |
| **SIGNAL / NOISE / UNKNOWN** | Above the floor: act. Below it: re-check first. Source unknown or content tampered with: can't say. |
| **TTL / expiry** | The moment trust falls below the floor. For a moving ship at floor 0.9, about 20 seconds; for a moored ship, hours. |

**How we know these numbers aren't made up**
- **Ground truth:** real records where the truth became known later, such as the next ship position, the next weather report, or who actually became CEO.
- **Held out:** we learn the numbers from half the airports (or ships, or companies), then test them on the other half, which the fitting never saw. It's like studying from half the past exam papers and grading yourself on the rest. It stops us fooling ourselves.
- **Brier score:** how wrong a stated confidence was, on average.
  - Say "90% sure" and be right: a tiny penalty (0.01).
  - Say "90% sure" and be wrong: a big penalty (0.81).
  - Always saying 50/50 scores 0.25. Lower is better.
  - The echo model's 0.1599 → 0.1572 means its confidence was slightly less wrong on data it had never seen.
- **Largest gap:** the worst disagreement between the model's curve and the real one. At 24 hours the old model said 22% of temperature readings are still right, reality was 43%, and the echo model says 39%.

## What fid does today

| Command / function | Plain meaning |
|---|---|
| `fid stamp` | Attach a trust label to a piece of data: where it came from, when it was measured, how fast it goes stale, and a fingerprint that detects tampering |
| `fid check` | Read the label and answer now: SIGNAL / NOISE / UNKNOWN against your floor. Exit codes 0 / 1 / 2, so scripts can act on the answer automatically. |
| `fid chain` | Show the history (source, each processing step, time passed) and how much trust each one cost |
| `fid ttl` | When the data stops being trustworthy enough, e.g. "expires at 14:32" or "never" |
| `fid calibrate` | Learn the half-life and baseline from your own log of re-checked facts. It refuses to guess when there's too little data. |
| registries (YAML) | Lookup tables: half-life per fact type and state, and noise per processing step |
| Python package | The same functions from Python: `pip install fidmeter` |
| conformance vectors | A test sheet, so anyone re-implementing the formula gets identical answers |
| calibration study | The evidence: CEOs, weather, 7M flights, 6,600 ships |

## The load-bearing parts, and how solid each is

Every answer fid gives rests on five things:

| Part | How solid today |
|---|---|
| Half-life and baseline | **Measured.** `fid calibrate` learns them from real re-checks, and the study shows they hold on held-out data. |
| Source fidelity | **Guessed.** It's hard-coded per class in `src/envelope.rs`: instrument 0.97, database 0.90, AI 0.55, and so on. `calibrate` already measures "correct when fresh", but that number never reaches the label. **This is the biggest gap.** |
| Transform noise | **Measured for flight legs and AI steps.** AI extraction loses almost nothing (0 errors in 4,800 checks); an AI summary loses 1.5% of facts (Opus 5.5) to 7% (Haiku 5.5). Deterministic steps (copy, join, aggregate…) are still guessed. |
| Floor | Your policy. It's right that fid doesn't choose it. |
| The re-check log | Only as good as what's fed in. `docs/LEDGER.md` covers the bias and the format. |

## What's missing for industrial and reliable, in order

1. **Measured source fidelity.** Carry the "correct when fresh" number that `calibrate` already measures into the registry and the label, replacing the class constants.
2. **Uncertainty.** fid gives one number today. With only 60 re-checks, the true half-life might be anywhere from 3 days to 9. fid should give a range, and refuse to answer when the range is too wide. This can be done by re-running the fit on random subsets of the data, with no new dependencies.
3. **Drift check.** Decay rates change: a company's turnover shifts, a season starts. A command would score past predictions against new re-checks and say "your calibration is out of date".
4. **AI transform noise.** Measure how often a fact survives AI extraction or summarization, using data where the true answer is known.
5. **The echo model in fid (phase 2).** It passed the test, but it's the smallest gain of the five.

## More kinds of data to test

Airport weather is good evidence, but only for one kind of world: physical facts. One dataset of each missing kind would show whether fid holds generally. I haven't yet checked that each source is reachable.

| Kind of fact | Candidate | Why it matters |
|---|---|---|
| Daily and weekly human rhythm | US grid electricity demand, hourly (EIA) | The fair test for echoes. People keep a working week, so a weekly cycle should appear where weather had none. |
| Business records (high stakes: fraud checks, compliance) | UK Companies House directors and addresses over time | "Company X's director is Y" decays like the CEO fact, at far larger scale |
| Software facts (what AI agents rely on) | PyPI release history: "the latest version of X is Y" | **Done (19 Oct).** 977 packages, 2015–2024: half-life 29 days just after a release, 80 days for a quiet package, 230 days for a dormant one. In `examples/half-lives.yaml` as `package_latest_version` with three states |
| Medical (highest stakes) | openFDA drug label versions | "This drug's warning says Z": changes are rare but critical |

## Next steps, for the cloud session

1. Fix the untested air-mass sentence in `docs/FINDINGS.md`.
2. Measured source fidelity, carried from `calibrate` into the registry and the label (gap 1).
3. Uncertainty ranges in `fid calibrate` (gap 2).
4. One new dataset: EIA electricity demand. It tests echoes, the weekly cycle and a human-driven fact all at once.
5. Then the drift check, AI noise, and phase 2 (the echo model in fid), in that order.

Each step is either measured on held-out data or proven by a test, and nothing goes into the docs without a number behind it.

## Progress, 18 October

| Step | State | Evidence |
|---|---|---|
| 1. Air-mass sentence | done | FINDINGS now states only the measured gap; the cause is an open question |
| 2. Measured source fidelity | done | `source_fidelity` in registry and envelope (0.3); written only from ≥ 20 checks in fresh bins. The public studies give s0 = 1 by construction (a source checked against itself), so none is claimed for them |
| 3. Uncertainty | done | 95% ranges from 200 entity resamples; refuses when the half-life range spans more than 2.5×. Ranges held the truth in 38 of 40 synthetic ledgers |
| 4. EIA electricity data | ready to start | api.eia.gov, api.fda.gov and the Companies House API are now allowed (19 Oct); `FID_EIA_API_KEY` is set for new sessions |
| 5a. Drift check | done | `fid drift`: likelihood ratio of registry against recent checks. False alarms 3.5% (1% with repeated checks per entity); a halved half-life caught 29 of 30 |
| 5b. AI transform noise | done (19 Oct) | 200 documents with known facts, standard and hard corpus, Opus 5.5 and Haiku 5.5. Extraction: 0 errors in 4,800 checks. Summary: 1.5% (Opus), 1.9% (Opus, low effort), 7.0% (Haiku); rewrite ≤ 0.2%. Every error was a fact left out, none a wrong value. In `examples/noise-model.yaml` as `llm_extract` 0.001 (was guessed 0.20), `llm_summarize` 0.015 (0.028 when facts are buried), `llm_rewrite` 0.002 |
| 5c. Echo in fid | done | `echo` in registries and envelopes, matching the study to 1e-12; `fid calibrate --period` keeps an echo only if it wins held-out (0 false keeps in 40) |
