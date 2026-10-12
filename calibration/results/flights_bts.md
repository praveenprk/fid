### Flight legs: aircraft's k-th leg of the day arrives on schedule

- Still correct means: arrived within 15 min, not cancelled or diverted; lag = legs flown before it
- 6,112 entities, 7,046,561 observations
- Empirical half-life: **—** (never fell to 50% in the observed range)
- Fitted fid model (decay to 0): s0 = 0.855, half-life = **14.9 legs**, shape error 0.068
- Fitted with baseline (decay to b): b = 0.62, half-life = **3.1 legs**, shape error 0.021

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1682 |
| fitted half life | 0.1683 |
| constant base rate | 0.1712 |

| Lag | Still correct | n |
|---|---|---|
| 0 legs | 0.855 | 1,728,380 |
| 1 legs | 0.805 | 1,602,779 |
| 2 legs | 0.772 | 1,375,240 |
| 3 legs | 0.736 | 1,103,916 |
| 4 legs | 0.707 | 676,325 |
| 5 legs | 0.701 | 400,271 |
| 6 legs | 0.659 | 113,832 |
| 7 legs | 0.686 | 45,818 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.6–0.7 | 0.668 | 0.689 | 274,119 |
| 0.7–0.8 | 0.753 | 0.745 | 1,542,423 |
| 0.8–0.9 | 0.836 | 0.830 | 1,633,357 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.6–0.7 | 0.689 | 0.689 | 274,119 |
| 0.7–0.8 | 0.746 | 0.745 | 1,542,423 |
| 0.8–0.9 | 0.832 | 0.830 | 1,633,357 |

### Flight schedules: source fidelity

- 7,079,061 flights, 2024
- The schedule is true (on time, ±15 min) for **77.9%** of flights

| Month | On time | n |
|---|---|---|
| 2024-01 | 0.729 | 547,271 |
| 2024-02 | 0.834 | 519,221 |
| 2024-03 | 0.783 | 591,767 |
| 2024-04 | 0.802 | 582,185 |
| 2024-05 | 0.723 | 609,743 |
| 2024-06 | 0.739 | 611,132 |
| 2024-07 | 0.681 | 634,613 |
| 2024-08 | 0.747 | 619,025 |
| 2024-09 | 0.839 | 582,622 |
| 2024-10 | 0.858 | 615,497 |
| 2024-11 | 0.848 | 575,404 |
| 2024-12 | 0.781 | 590,581 |

| Airline | On time | n |
|---|---|---|
| 9E | 0.821 | 200,094 |
| AA | 0.725 | 984,306 |
| AS | 0.762 | 245,819 |
| B6 | 0.731 | 240,282 |
| DL | 0.818 | 1,009,174 |
| F9 | 0.695 | 208,624 |
| G4 | 0.768 | 117,210 |
| HA | 0.836 | 78,530 |
| MQ | 0.778 | 279,955 |
| NK | 0.745 | 261,103 |
| OH | 0.768 | 227,971 |
| OO | 0.798 | 744,658 |
| UA | 0.786 | 760,451 |
| WN | 0.786 | 1,419,419 |
| YX | 0.842 | 301,465 |

| Scheduled departure | On time | n |
|---|---|---|
| 00:00 | 0.785 | 12,331 |
| 01:00 | 0.773 | 4,077 |
| 05:00 | 0.896 | 200,157 |
| 06:00 | 0.891 | 495,017 |
| 07:00 | 0.865 | 493,915 |
| 08:00 | 0.850 | 482,402 |
| 09:00 | 0.836 | 395,739 |
| 10:00 | 0.824 | 439,299 |
| 11:00 | 0.812 | 442,008 |
| 12:00 | 0.797 | 419,882 |
| 13:00 | 0.776 | 423,120 |
| 14:00 | 0.754 | 406,552 |
| 15:00 | 0.738 | 407,710 |
| 16:00 | 0.717 | 409,197 |
| 17:00 | 0.709 | 443,706 |
| 18:00 | 0.692 | 435,133 |
| 19:00 | 0.686 | 387,893 |
| 20:00 | 0.681 | 317,595 |
| 21:00 | 0.702 | 235,114 |
| 22:00 | 0.712 | 166,351 |
| 23:00 | 0.771 | 58,692 |

### Flight legs: independence of transform noise

- Fitted per-leg noise (fid model): **0.045**
- On time after an on-time previous leg: **0.859** (n = 4,285,796)
- On time after a late previous leg: **0.338** (n = 1,032,385)
- On time, any leg after the first: **0.758**
- Conditional noise after a late leg: 1 − 0.338 / 0.758 = **0.55**
- If legs were independent, as multiplying per-step noise assumes, these two would match.
