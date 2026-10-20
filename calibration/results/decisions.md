# Decision benchmark: re-check by fid, or on a fixed schedule?

Re-checks needed per fact to keep held-out staleness at or below each target, and the ratio fid ÷ best fixed interval (95% range over 200 resamples of held-out entities; below 1 means fid needs fewer re-checks).

## ship_position

500 entities (205 train / 225 held-out groups).

- `fid calibrate` on the training half (fid): underway: half-life 2.0m, baseline 0.02, s0 0.998, moored: half-life 1.4d, baseline 0.00, s0 0.997
- `fid calibrate` on the training half (fid no state): all: half-life 2.9m, baseline 0.62, s0 0.995

| Staleness target | Best fixed interval | fid | fid ÷ fixed | fid, no state | no state ÷ fixed |
|---|---|---|---|---|---|
| 2% | 490.3/day | 508.6/day | 1.04 (0.92–1.14) | 490.3/day | 1.00 (1.00–1.00) |
| 5% | 392.8/day | 472.0/day | 1.20 (1.10–1.31) | 392.8/day | 1.00 (1.00–1.00) |
| 10% | 299.0/day | 425.9/day | 1.42 (1.31–1.54) | 294.2/day | 0.98 (0.98–0.99) |
| 20% | 160.4/day | 162.2/day | 1.01 (0.63–1.58) | 119.6/day | 0.75 (0.64–0.90) |

The floor as a knob (held-out): staleness fid predicts against what happened.

| Floor | Re-checks | Predicted staleness | Actual |
|---|---|---|---|
| 0.30 | 90.4/day | 0.241 | 0.238 |
| 0.40 | 119.9/day | 0.218 | 0.210 |
| 0.50 | 179.0/day | 0.190 | 0.197 |
| 0.60 | 179.0/day | 0.190 | 0.197 |
| 0.70 | 398.1/day | 0.099 | 0.137 |
| 0.75 | 409.9/day | 0.085 | 0.119 |
| 0.80 | 428.2/day | 0.069 | 0.097 |
| 0.85 | 445.9/day | 0.053 | 0.076 |
| 0.90 | 467.2/day | 0.035 | 0.055 |
| 0.93 | 487.1/day | 0.025 | 0.036 |
| 0.95 | 500.6/day | 0.018 | 0.026 |
| 0.97 | 516.4/day | 0.011 | 0.014 |
| 0.98 | 530.9/day | 0.008 | 0.007 |
| 0.99 | 566.5/day | 0.005 | 0.001 |

Mean |predicted − actual| for floors 0.5–0.95: **0.019**
Leaving out floors whose interval is shorter than the data's time step: 0.017 (3 floors)


Diagnostics, added after the first run (not part of the rule): staleness in the worst-served state, and fid with source fidelity 1 (a check reads the truth itself).

| Target | Fixed, average | fid s0=1, average | ÷ fixed | Fixed, worst state | fid, worst state | ÷ fixed | fid s0=1, worst state | ÷ fixed |
|---|---|---|---|---|---|---|---|---|
| 2% | 490.3/day | 507.0/day | 1.03 (0.92–1.13) | 659.6/day | 527.1/day | 0.80 (0.72–0.89) | 526.0/day | 0.80 (0.72–0.89) |
| 5% | 392.8/day | 472.4/day | 1.20 (1.10–1.32) | 509.1/day | 502.5/day | 0.99 (0.87–1.12) | 502.6/day | 0.99 (0.87–1.12) |
| 10% | 299.0/day | 426.2/day | 1.43 (1.31–1.56) | 430.8/day | 472.9/day | 1.10 (0.98–1.23) | 472.8/day | 1.10 (0.98–1.23) |
| 20% | 160.4/day | 162.2/day | 1.01 (0.63–1.57) | 341.2/day | 418.4/day | 1.23 (1.05–1.37) | 418.5/day | 1.23 (1.07–1.37) |

## airport_temperature

10 entities (6 train / 4 held-out groups).

- `fid calibrate` on the training half (fid): all: half-life 2.0h, baseline 0.28, s0 1.000

| Staleness target | Best fixed interval | fid | fid ÷ fixed |
|---|---|---|---|
| 2% | 21.2/day | 21.2/day | 1.00 (1.00–1.00) |
| 5% | 17.5/day | 17.5/day | 1.00 (1.00–1.00) |
| 10% | 12.8/day | 12.8/day | 1.00 (1.00–1.00) |
| 20% | 8.8/day | 8.8/day | 1.00 (1.00–1.01) |

The floor as a knob (held-out): staleness fid predicts against what happened.

| Floor | Re-checks | Predicted staleness | Actual |
|---|---|---|---|
| 0.30 | 2.4/day | 0.484 | 0.586 |
| 0.40 | 4.8/day | 0.318 | 0.390 |
| 0.50 | 8.0/day | 0.192 | 0.227 |
| 0.60 | 12.0/day | 0.107 | 0.110 |
| 0.70 | 12.0/day | 0.107 | 0.110 |
| 0.75 | 24.0/day | 0.000 | 0.000 |
| 0.80 | 24.0/day | 0.000 | 0.000 |
| 0.85 | 24.0/day | 0.000 | 0.000 |
| 0.90 | 24.0/day | 0.000 | 0.000 |
| 0.93 | 24.0/day | 0.000 | 0.000 |
| 0.95 | 24.0/day | 0.000 | 0.000 |
| 0.97 | 24.0/day | 0.000 | 0.000 |
| 0.98 | 24.0/day | 0.000 | 0.000 |
| 0.99 | 24.0/day | 0.000 | 0.000 |

Mean |predicted − actual| for floors 0.5–0.95: **0.005**
Leaving out floors whose interval is shorter than the data's time step: 0.010 (4 floors)


Diagnostics, added after the first run (not part of the rule): staleness in the worst-served state, and fid with source fidelity 1 (a check reads the truth itself).

| Target | Fixed, average | fid s0=1, average | ÷ fixed | Fixed, worst state | fid, worst state | ÷ fixed | fid s0=1, worst state | ÷ fixed |
|---|---|---|---|---|---|---|---|---|
| 2% | 21.2/day | 21.2/day | 1.00 (1.00–1.00) | 21.2/day | 21.2/day | 1.00 (1.00–1.00) | 21.2/day | 1.00 (1.00–1.00) |
| 5% | 17.5/day | 17.5/day | 1.00 (1.00–1.00) | 17.5/day | 17.5/day | 1.00 (1.00–1.00) | 17.5/day | 1.00 (1.00–1.00) |
| 10% | 12.8/day | 12.8/day | 1.00 (1.00–1.00) | 12.8/day | 12.8/day | 1.00 (1.00–1.00) | 12.8/day | 1.00 (1.00–1.00) |
| 20% | 8.8/day | 8.8/day | 1.00 (1.00–1.01) | 8.8/day | 8.8/day | 1.00 (1.00–1.01) | 8.8/day | 1.00 (1.00–1.01) |

## airport_flight_category

10 entities (6 train / 4 held-out groups).

- `fid calibrate` on the training half (fid): all: half-life 3.3h, baseline 0.72, s0 0.974

| Staleness target | Best fixed interval | fid | fid ÷ fixed |
|---|---|---|---|
| 2% | 17.0/day | 17.0/day | 1.00 (1.00–1.00) |
| 5% | 10.2/day | 10.2/day | 1.00 (1.00–1.02) |
| 10% | 4.7/day | 4.8/day | 1.01 (0.95–1.03) |
| 20% | every 1.0y | every 1.0y | 1.00 (0.01–1.00) |

The floor as a knob (held-out): staleness fid predicts against what happened.

| Floor | Re-checks | Predicted staleness | Actual |
|---|---|---|---|
| 0.30 | every 1.0y | 0.280 | 0.176 |
| 0.40 | every 1.0y | 0.280 | 0.176 |
| 0.50 | every 1.0y | 0.280 | 0.176 |
| 0.60 | every 1.0y | 0.280 | 0.176 |
| 0.70 | every 1.0y | 0.280 | 0.176 |
| 0.75 | 2.4/day | 0.162 | 0.148 |
| 0.80 | 4.0/day | 0.119 | 0.112 |
| 0.85 | 8.0/day | 0.071 | 0.065 |
| 0.90 | 12.0/day | 0.050 | 0.040 |
| 0.93 | 24.0/day | 0.026 | 0.000 |
| 0.95 | 24.0/day | 0.026 | 0.000 |
| 0.97 | 24.0/day | 0.026 | 0.000 |
| 0.98 | 24.0/day | 0.026 | 0.000 |
| 0.99 | 24.0/day | 0.026 | 0.000 |

Mean |predicted − actual| for floors 0.5–0.95: **0.045**
Leaving out floors whose interval is shorter than the data's time step: 0.050 (7 floors)


Diagnostics, added after the first run (not part of the rule): staleness in the worst-served state, and fid with source fidelity 1 (a check reads the truth itself).

| Target | Fixed, average | fid s0=1, average | ÷ fixed | Fixed, worst state | fid, worst state | ÷ fixed | fid s0=1, worst state | ÷ fixed |
|---|---|---|---|---|---|---|---|---|
| 2% | 17.0/day | 17.0/day | 1.00 (1.00–1.00) | 17.0/day | 17.0/day | 1.00 (1.00–1.00) | 17.0/day | 1.00 (1.00–1.00) |
| 5% | 10.2/day | 10.2/day | 1.00 (0.99–1.02) | 10.2/day | 10.2/day | 1.00 (1.00–1.02) | 10.2/day | 1.00 (0.99–1.02) |
| 10% | 4.7/day | 4.7/day | 1.00 (0.96–1.02) | 4.7/day | 4.8/day | 1.01 (0.95–1.03) | 4.7/day | 1.00 (0.96–1.02) |
| 20% | every 1.0y | every 1.0y | 1.00 (0.02–1.00) | every 1.0y | every 1.0y | 1.00 (0.01–1.00) | every 1.0y | 1.00 (0.02–1.00) |

## grid_demand

14 entities (7 train / 7 held-out groups).

- `fid calibrate` on the training half (fid): all: half-life 57m, baseline 0.28, s0 1.000

| Staleness target | Best fixed interval | fid | fid ÷ fixed |
|---|---|---|---|
| 2% | 20.1/day | 20.1/day | 1.00 (1.00–1.00) |
| 5% | 15.4/day | 15.4/day | 1.00 (1.00–1.00) |
| 10% | 11.2/day | 11.2/day | 1.00 (1.00–1.01) |
| 20% | 8.2/day | 8.3/day | 1.01 (1.00–1.03) |

The floor as a knob (held-out): staleness fid predicts against what happened.

| Floor | Re-checks | Predicted staleness | Actual |
|---|---|---|---|
| 0.30 | 4.8/day | 0.448 | 0.384 |
| 0.40 | 12.0/day | 0.186 | 0.078 |
| 0.50 | 12.0/day | 0.186 | 0.078 |
| 0.60 | 24.0/day | 0.000 | 0.000 |
| 0.70 | 24.0/day | 0.000 | 0.000 |
| 0.75 | 24.0/day | 0.000 | 0.000 |
| 0.80 | 24.0/day | 0.000 | 0.000 |
| 0.85 | 24.0/day | 0.000 | 0.000 |
| 0.90 | 24.0/day | 0.000 | 0.000 |
| 0.93 | 24.0/day | 0.000 | 0.000 |
| 0.95 | 24.0/day | 0.000 | 0.000 |
| 0.97 | 24.0/day | 0.000 | 0.000 |
| 0.98 | 24.0/day | 0.000 | 0.000 |
| 0.99 | 24.0/day | 0.000 | 0.000 |

Mean |predicted − actual| for floors 0.5–0.95: **0.012**
Leaving out floors whose interval is shorter than the data's time step: 0.054 (2 floors)


Diagnostics, added after the first run (not part of the rule): staleness in the worst-served state, and fid with source fidelity 1 (a check reads the truth itself).

| Target | Fixed, average | fid s0=1, average | ÷ fixed | Fixed, worst state | fid, worst state | ÷ fixed | fid s0=1, worst state | ÷ fixed |
|---|---|---|---|---|---|---|---|---|
| 2% | 20.1/day | 20.1/day | 1.00 (1.00–1.00) | 20.1/day | 20.1/day | 1.00 (1.00–1.00) | 20.1/day | 1.00 (1.00–1.00) |
| 5% | 15.4/day | 15.4/day | 1.00 (1.00–1.00) | 15.4/day | 15.4/day | 1.00 (1.00–1.00) | 15.4/day | 1.00 (1.00–1.00) |
| 10% | 11.2/day | 11.2/day | 1.00 (1.00–1.01) | 11.2/day | 11.2/day | 1.00 (1.00–1.01) | 11.2/day | 1.00 (1.00–1.01) |
| 20% | 8.2/day | 8.3/day | 1.01 (1.00–1.03) | 8.2/day | 8.3/day | 1.01 (1.00–1.03) | 8.3/day | 1.01 (1.00–1.03) |

## package_latest_version

923 entities (455 train / 468 held-out groups).

- `fid calibrate` on the training half (fid): just_released: half-life 33d, baseline 0.06, s0 0.945, quiet: half-life 84d, baseline 0.10, s0 0.984, dormant: half-life 268d, baseline 0.15, s0 0.997
- `fid calibrate` on the training half (fid no state): all: half-life 73d, baseline 0.14, s0 0.965

| Staleness target | Best fixed interval | fid | fid ÷ fixed | fid, no state | no state ÷ fixed |
|---|---|---|---|---|---|
| 2% | every 1.9d | 184.7/day | 353.15 (258.68–499.70) | every 1.9d | 1.00 (0.99–3.26) |
| 5% | every 5.7d | every 4.2d | 1.34 (1.27–1.42) | every 5.5d | 1.04 (1.01–1.05) |
| 10% | every 13d | every 13d | 1.07 (1.03–1.12) | every 13d | 1.00 (0.98–1.02) |
| 20% | every 36d | every 33d | 1.09 (1.06–1.11) | every 36d | 1.00 (0.99–1.01) |

The floor as a knob (held-out): staleness fid predicts against what happened.

| Floor | Re-checks | Predicted staleness | Actual |
|---|---|---|---|
| 0.30 | every 150d | 0.437 | 0.513 |
| 0.40 | every 109d | 0.358 | 0.431 |
| 0.50 | every 80d | 0.290 | 0.361 |
| 0.60 | every 56d | 0.227 | 0.288 |
| 0.70 | every 36d | 0.169 | 0.215 |
| 0.75 | every 28d | 0.141 | 0.178 |
| 0.80 | every 20d | 0.115 | 0.141 |
| 0.85 | every 13d | 0.088 | 0.101 |
| 0.90 | every 6.4d | 0.063 | 0.061 |
| 0.93 | every 2.4d | 0.048 | 0.035 |
| 0.95 | 419.1/day | 0.039 | 0.018 |
| 0.97 | 433.5/day | 0.032 | 0.008 |
| 0.98 | 441.4/day | 0.028 | 0.003 |
| 0.99 | 1,232.2/day | 0.027 | 0.001 |

Mean |predicted − actual| for floors 0.5–0.95: **0.032**


Diagnostics, added after the first run (not part of the rule): staleness in the worst-served state, and fid with source fidelity 1 (a check reads the truth itself).

| Target | Fixed, average | fid s0=1, average | ÷ fixed | Fixed, worst state | fid, worst state | ÷ fixed | fid s0=1, worst state | ÷ fixed |
|---|---|---|---|---|---|---|---|---|
| 2% | every 1.9d | every 2.2d | 0.86 (0.80–0.93) | every 1.6d | 420.6/day | 663.16 (537.69–796.30) | every 1.9d | 0.85 (0.81–0.91) |
| 5% | every 5.7d | every 6.2d | 0.92 (0.88–0.96) | every 4.6d | every 3.2d | 1.47 (1.36–1.59) | every 5.2d | 0.90 (0.86–0.95) |
| 10% | every 13d | every 14d | 0.98 (0.95–1.02) | every 11d | every 9.6d | 1.13 (1.09–1.17) | every 11d | 0.96 (0.93–1.00) |
| 20% | every 36d | every 34d | 1.04 (1.02–1.06) | every 28d | every 26d | 1.09 (1.06–1.12) | every 27d | 1.04 (1.01–1.06) |

## drug_label_warnings

376 entities (194 train / 182 held-out groups).

- `fid calibrate` on the training half (fid): all: half-life 3.7y, baseline 0.00, s0 0.996

| Staleness target | Best fixed interval | fid | fid ÷ fixed |
|---|---|---|---|
| 2% | every 81d | every 82d | 0.98 (0.89–1.12) |
| 5% | every 212d | every 216d | 0.98 (0.90–1.08) |
| 10% | every 1.2y | every 1.1y | 1.05 (0.93–1.23) |
| 20% | every 2.7y | every 2.5y | 1.06 (0.99–1.10) |

The floor as a knob (held-out): staleness fid predicts against what happened.

| Floor | Re-checks | Predicted staleness | Actual |
|---|---|---|---|
| 0.30 | every 3.9y | 0.347 | 0.279 |
| 0.40 | every 3.3y | 0.300 | 0.257 |
| 0.50 | every 2.8y | 0.254 | 0.222 |
| 0.60 | every 2.1y | 0.205 | 0.164 |
| 0.70 | every 1.6y | 0.152 | 0.139 |
| 0.75 | every 1.3y | 0.128 | 0.114 |
| 0.80 | every 1.1y | 0.102 | 0.094 |
| 0.85 | every 289d | 0.077 | 0.064 |
| 0.90 | every 189d | 0.052 | 0.044 |
| 0.93 | every 129d | 0.037 | 0.029 |
| 0.95 | every 90d | 0.027 | 0.021 |
| 0.97 | every 51d | 0.017 | 0.013 |
| 0.98 | every 31d | 0.012 | 0.008 |
| 0.99 | every 12d | 0.007 | 0.003 |

Mean |predicted − actual| for floors 0.5–0.95: **0.016**


Diagnostics, added after the first run (not part of the rule): staleness in the worst-served state, and fid with source fidelity 1 (a check reads the truth itself).

| Target | Fixed, average | fid s0=1, average | ÷ fixed | Fixed, worst state | fid, worst state | ÷ fixed | fid s0=1, worst state | ÷ fixed |
|---|---|---|---|---|---|---|---|---|
| 2% | every 81d | every 80d | 1.01 (0.89–1.15) | every 81d | every 82d | 0.98 (0.89–1.12) | every 80d | 1.01 (0.89–1.15) |
| 5% | every 212d | every 208d | 1.02 (0.96–1.10) | every 212d | every 216d | 0.98 (0.90–1.08) | every 208d | 1.02 (0.96–1.10) |
| 10% | every 1.2y | every 1.2y | 1.03 (0.95–1.17) | every 1.2y | every 1.1y | 1.05 (0.93–1.23) | every 1.2y | 1.03 (0.95–1.17) |
| 20% | every 2.7y | every 2.5y | 1.06 (0.98–1.11) | every 2.7y | every 2.5y | 1.06 (0.99–1.10) | every 2.5y | 1.06 (0.98–1.11) |

## All kinds together

One fact of each kind; re-checks and staleness averaged across kinds. The tuned TTL picks one interval per kind on the training half for each re-check budget.

| Staleness target | One TTL | Tuned TTL per kind | fid, no state | fid |
|---|---|---|---|---|
| 2% | 146.3/day | 58.1/day | 250.8/day | 92.1/day |
| 5% | 37.3/day | 21.9/day | 110.1/day | 83.9/day |
| 10% | 11.0/day | 7.9/day | 58.5/day | 75.9/day |
| 20% | 5.2/day | 2.8/day | 7.1/day | 33.6/day |

| Staleness target | One TTL ÷ tuned | fid, no state ÷ tuned | fid ÷ tuned |
|---|---|---|---|
| 2% | 2.52 (2.45–2.59) | 4.32 (4.02–4.67) | 1.59 (1.51–1.68) |
| 5% | 1.71 (1.40–2.01) | 5.03 (3.96–6.48) | 3.83 (3.42–4.42) |
| 10% | 1.38 (1.32–1.65) | 7.37 (6.06–8.94) | 9.56 (8.21–11.46) |
| 20% | 1.85 (1.71–1.93) | 2.53 (2.08–3.79) | 11.94 (10.01–13.89) |

Diagnostic, added after the first run: every kind and every state kept at or below the target, each kind at its own cheapest setting.

| Target | Fixed interval per kind | fid floor per kind | ÷ fixed | fid s0=1 floor per kind | ÷ fixed |
|---|---|---|---|---|---|
| 2% | 119.8/day | 167.7/day | 1.40 (1.31–1.51) | 97.5/day | 0.81 (0.74–0.90) |
| 5% | 92.1/day | 91.0/day | 0.99 (0.88–1.11) | 91.0/day | 0.99 (0.88–1.11) |
| 10% | 76.6/day | 83.6/day | 1.09 (0.98–1.21) | 83.6/day | 1.09 (0.98–1.21) |
| 20% | 59.7/day | 72.6/day | 1.22 (1.05–1.35) | 72.6/day | 1.22 (1.06–1.35) |

## The rule

- ship_position, fid against the best fixed interval: 5%: **fid worse**, 10%: **fid worse**, 20%: **tie**
- package_latest_version, fid against the best fixed interval: 5%: **fid worse**, 10%: **fid worse**, 20%: **fid worse**
- All kinds, fid against the tuned TTL: 5%: **fid worse**, 10%: **fid worse**, 20%: **fid worse**
- Honest knob (error ≤ 0.02): ship_position: **yes**, airport_temperature: **yes**, airport_flight_category: **no**, grid_demand: **yes**, package_latest_version: **no**, drug_label_warnings: **yes**

fid's TTL against the closed form: largest gap 1 s.
