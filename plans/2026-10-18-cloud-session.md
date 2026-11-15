# fid: where we are, in plain language, and what comes next

## The goal in one sentence

fid tells a system how much it can still trust a stored fact right now, using
how fast that kind of fact really goes stale (measured, not guessed), so that
stale facts get re-checked before anyone acts on them.

## About "the guess"

You're right. In docs/FINDINGS.md I wrote "the first day back carries the same
air mass", and nothing tested it. That sentence is already committed. First
fix: change it to the measured fact only (the first echo is stronger than the
model predicts: 0.43 seen vs 0.39 predicted). The air-mass idea is then either
dropped or written as an untested question. It can be tested later: compare
days when pressure changed a lot with days it didn't.

## One real example: a coast guard rescue

At 10:00 a ship reports its GPS position. At 10:20 a rescue boat is about to
be sent there. Should it trust the 10:00 position?

| Word | What it means here |
|---|---|
| **fact** | "Ship X is at position P", stored at 10:00 |
| **source fidelity** (s0) | How often a fresh report is right. GPS: about 97 out of 100. A rumor on the radio: about 17. |
| **half-life** (h) | The time until half of such facts are wrong. A moving ship: **3 minutes**. A ship tied up in port: **1.4 days**. Same ships, same GPS. What matters is what the thing is doing (its *state*). |
| **decay** | Trust drops as time passes. A 20-minute-old position of a moving ship is almost worthless. The same report about a moored ship is still nearly certain. |
| **baseline** (b) | How often a stale fact is right by luck. "The weather at this airport is clear" is still right 71% of the time days later, because most days are clear. Trust falls toward 71%, not toward 0. |
| **echo** | Some facts come back. Yesterday's 3 pm temperature is a decent guess for today's 3 pm, and a bad guess for tonight. Trust dips and returns each day. |
| **transform noise** | Each step that handles the fact loses a little: retyped, summarized by an AI, converted. Each step multiplies the trust down. |
| **fidelity** | All of the above combined: one number from 0 to 1, "how likely this fact is still true right now". |
| **floor** | The trust a decision needs. Sending a rescue boat: 0.9. A weekly report: 0.5. This is policy, not maths: you choose it. |
| **SIGNAL / NOISE / UNKNOWN** | Above the floor: act. Below: re-check first. Source unknown or content tampered with: can't say. |
| **TTL / expiry** | The moment trust falls below the floor. Moving ship with floor 0.9: about 20 seconds. Moored ship: hours. |

**How we know these numbers aren't made up**
- **Ground truth.** Real records where we later learned the truth: the next ship
  position, the next weather report, who the CEO actually was.
- **Held out.** We learn the numbers from half the airports (or ships, or
  companies), then test them on the other half, which the fitting never saw.
  It's like studying from half the past exam papers and grading yourself on
  the rest. It stops us from fooling ourselves.
- **Brier score.** How wrong a confidence was, on average. Say "90% sure" and
  be right: a tiny penalty (0.01). Say "90% sure" and be wrong: a big one
  (0.81). Always saying 50/50 scores 0.25. Lower is better. When the echo model
  went from 0.1599 to 0.1572, its confidence was slightly less wrong on data it
  had never seen.
- **Largest gap.** The worst disagreement between the model's curve and the
  real curve. At 24 hours the old model said 22% of temperature readings are
  still right, the real figure is 43%, and the echo model says 39%.

## What fid does today

| Command / function | Plain meaning |
|---|---|
| `fid stamp` | Attach a "trust label" (envelope) to a piece of data: where it came from, when it was measured, how fast it goes stale, a tamper-check fingerprint |
| `fid check` | Read the label and answer now: SIGNAL / NOISE / UNKNOWN against your floor. Exit code 0 / 1 / 2, so scripts can gate on it. |
| `fid chain` | Show the history: source, then each processing step, then time, and how much trust each one cost |
| `fid ttl` | When this data stops being trustworthy enough ("expires at 14:32" or "never") |
| `fid calibrate` | Learn the half-life and baseline from your own log of re-checked facts, and refuse to guess if there's too little data |
| registries (YAML) | Lookup tables: half-life per fact type and state, noise per processing step |
| Python package | The same functions from Python: `pip install fidmeter` |
| conformance vectors | A test sheet so anyone re-implementing the formula gets identical answers |
| calibration study | The evidence: CEOs, weather, 7M flights, 6,600 ships |

## The load-bearing parts, and how solid each is

Every answer fid gives rests on five numbers:

| Part | How solid today |
|---|---|
| Half-life and baseline | **Measured.** `fid calibrate` learns them from real re-checks, and the study shows they work on held-out data. |
| Source fidelity | **Guessed.** `src/envelope.rs` hard-codes it per class: instrument 0.97, database 0.90, AI 0.55… `calibrate` already measures "correct when fresh", but that number never reaches the label. **Biggest gap.** |
| Transform noise | **Mostly guessed.** Measured only for flight legs. Steps that summarize or extract with AI are unmeasured, and they're the most common case today. |
| Floor | Policy, set by the user. Correct that fid doesn't choose it. |
| The re-check log | As good as the data fed in. The bias and format guidance is in docs/LEDGER.md. |

## What's missing for industrial and reliable, in order

1. **Measured source fidelity.** Carry the measured "correct when fresh" from
   calibrate into the registry and the label, instead of class constants.
2. **Uncertainty.** Today fid gives one number. With 60 re-checks the true
   half-life might be 3 days or 9. Give a range, and refuse when it's too wide.
   (Resample the entities: plain counting, no new dependencies.)
3. **Drift check.** Decay rates change: a company's churn rate shifts, a
   season starts. A command that scores past predictions against new
   re-checks and says "your calibration is out of date".
4. **AI transform noise.** Measure how often a fact survives AI extraction or
   summarization, against data where the true answer is known.
5. **Echo in fid (phase 2).** It passed, but it's the smallest gain of the five.

## More kinds of data to test

Airport weather is good evidence for physical facts, but it's one kind of
world. To trust fid generally, add one dataset of each missing kind. Whether
each source is reachable still needs checking.

| Kind of fact | Candidate | Why it matters |
|---|---|---|
| Daily and weekly human rhythm | US grid electricity demand, hourly (EIA) | The fair test for echoes: people work weekdays, so a weekly cycle should appear where weather had none |
| Business records (high stakes: fraud, compliance checks) | UK Companies House: officers and addresses over time | "Company X's director is Y" decays like the CEO fact, at a much larger scale |
| Software facts (what AI agents rely on) | PyPI release history: "the latest version of X is Y" | Very fast, real decay. Exactly the stale knowledge AI coding tools act on. |
| Medical (highest stakes) | openFDA drug label versions | "This drug's warning says Z": rare but critical changes |

## Next steps, for the cloud session

1. Fix the untested air-mass sentence in docs/FINDINGS.md.
2. Measured source fidelity, from calibrate into the registry and label (gap 1).
3. Uncertainty ranges in `fid calibrate` (gap 2).
4. One new dataset: EIA electricity demand. It tests echoes, the weekly cycle,
   and a human-driven fact in one go.
5. Then the drift check, AI noise, and phase 2 (echo in fid), in that order.

Each step: measured on held-out data, or a test proves it works. No claim
goes in the docs without a number behind it.
