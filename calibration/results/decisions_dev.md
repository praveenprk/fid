# Decision benchmark v2 (dev set): fid schedule against fixed intervals

Re-checks per fact needed to keep held-out staleness at or below each target; ratios are fid's re-checks ÷ the other policy's (95% range over 200 resamples of held-out entities; below 1: fid needs fewer).

## ship_position

500 entities (205 train / 225 scored groups); states: yes.

`fid calibrate` refused: fid calibrate: state 'at_anchor': half-life too uncertain: 95% range 1.77d to 5.92d (×inf; limit ×2.5). Re-check more entities, over a wider range of ages, or raise --max-spread

## airport_temperature

10 entities (6 train / 4 scored groups); states: no.

`fid calibrate` (training half and the scored half's history):

```yaml
# fid calibrate: 113773 observations. Fitted on half the entities, scored on the other half.
airport_temperature:
  half_life: 1.91h  # 95% range 1.72h to 2.08h
  baseline: 0.27  # 95% range 0.22 to 0.31
  source_fidelity: 1  # 95% range 1 to 1
```

Entity baselines written: 10.

| Staleness | Best fixed interval | fid schedule | ÷ fixed | schedule + floor 0.5 | ÷ fixed | fid floor | ÷ fixed |
|---|---|---|---|---|---|---|---|
| 2% | 21.2/day | 20.5/day | 0.97 (0.94–1.00) | 20.5/day | 0.97 (0.94–1.00) | 20.3/day | 0.96 (0.93–1.00) |
| 5% | 17.5/day | 16.1/day | 0.92 (0.88–1.00) | 16.1/day | 0.92 (0.88–1.00) | 17.2/day | 0.98 (0.94–1.03) |
| 10% | 12.8/day | 12.6/day | 0.98 (0.96–1.00) | 12.6/day | 0.98 (0.96–1.00) | 13.0/day | 1.02 (0.98–1.02) |
| 20% | 8.8/day | 8.7/day | 0.99 (0.97–1.00) | 8.8/day | 0.99 (0.97–1.01) | 8.8/day | 1.00 (0.99–1.01) |

Worst-served state (diagnostic): | Staleness | Fixed | fid schedule ÷ fixed |
|---|---|---|
| 2% | 21.2/day | 0.97 (0.94–1.00) |
| 5% | 17.5/day | 0.92 (0.88–1.00) |
| 10% | 12.8/day | 0.98 (0.96–1.00) |
| 20% | 8.8/day | 0.99 (0.97–1.00) |

Schedule's predicted staleness against actual, mean gap over prices with staleness 1–30%: **0.016** (3 prices)

| Price | Re-checks | Predicted | Actual |
|---|---|---|---|
| 1.0e-02 | 15.0/day | 0.086 | 0.059 |
| 1.6e-02 | 12.0/day | 0.119 | 0.111 |
| 2.5e-02 | 7.5/day | 0.228 | 0.242 |
| 4.0e-02 | 5.7/day | 0.302 | 0.331 |
| 6.3e-02 | 3.5/day | 0.431 | 0.479 |

## airport_flight_category

10 entities (6 train / 4 scored groups); states: yes.

`fid calibrate` (training half and the scored half's history):

```yaml
# fid calibrate: 113798 observations. Fitted on half the entities, scored on the other half.
airport_flight_category:
  states:
    IFR:
      # fast share kept: held-out Brier 0.1403 vs one clock 0.1414
      half_life: 4.86h
      baseline: 0.07
      source_fidelity: 1
      fast: { share: 0.501, half_life: 35.28m }
    LIFR:
      # fast share kept: held-out Brier 0.1574 vs one clock 0.1582
      half_life: 18.96h
      baseline: 0.03
      source_fidelity: 0.981
      fast: { share: 0.664, half_life: 2.04h }
    MVFR:
      # fast share kept: held-out Brier 0.1783 vs one clock 0.1794
      half_life: 5.29h
      baseline: 0.18
      source_fidelity: 0.999
      fast: { share: 0.427, half_life: 32.39m }
    VFR:
      half_life: 4.96h  # 95% range 3.3h to 7.1h
      baseline: 0.84  # 95% range 0.81 to 0.86
      source_fidelity: 0.985  # 95% range 0.983 to 0.988
```

Entity baselines written: 40.

| Staleness | Best fixed interval | fid schedule | ÷ fixed | schedule + floor 0.5 | ÷ fixed | fid floor | ÷ fixed |
|---|---|---|---|---|---|---|---|
| 2% | 16.7/day | 11.6/day | 0.70 (0.53–0.81) | 11.6/day | 0.70 (0.53–0.81) | 11.4/day | 0.69 (0.54–0.81) |
| 5% | 9.8/day | 6.2/day | 0.63 (0.44–0.82) | 6.2/day | 0.63 (0.42–0.82) | 6.1/day | 0.62 (0.41–0.81) |
| 10% | 4.4/day | 2.6/day | 0.58 (0.00–0.79) | 2.5/day | 0.56 (0.00–0.79) | 1.8/day | 0.41 (0.00–0.94) |
| 20% | every 1.4d | every 161d | 0.01 (0.00–0.72) | every 100d | 0.01 (0.00–0.75) | every 157d | 0.01 (0.00–0.17) |

Worst-served state (diagnostic): | Staleness | Fixed | fid schedule ÷ fixed |
|---|---|---|
| 2% | 22.2/day | 0.99 (0.89–1.02) |
| 5% | 19.7/day | 0.97 (0.83–1.04) |
| 10% | 16.1/day | 0.89 (0.81–1.07) |
| 20% | 10.9/day | 0.85 (0.72–1.04) |

Schedule's predicted staleness against actual, mean gap over prices with staleness 1–30%: **0.007** (11 prices)

| Price | Re-checks | Predicted | Actual |
|---|---|---|---|
| 1.0e-03 | 16.5/day | 0.019 | 0.011 |
| 1.6e-03 | 13.9/day | 0.022 | 0.014 |
| 2.5e-03 | 11.2/day | 0.028 | 0.021 |
| 4.0e-03 | 9.8/day | 0.032 | 0.025 |
| 6.3e-03 | 7.8/day | 0.041 | 0.038 |
| 1.0e-02 | 5.3/day | 0.065 | 0.058 |
| 1.6e-02 | 3.7/day | 0.087 | 0.079 |
| 2.5e-02 | 2.4/day | 0.116 | 0.103 |
| 4.0e-02 | 1.1/day | 0.148 | 0.139 |
| 6.3e-02 | every 157d | 0.171 | 0.168 |
| 1.0e-01 | every 137d | 0.172 | 0.169 |
| 1.6e-01 | every 183d | 0.355 | 0.373 |

## grid_demand

14 entities (7 train / 7 scored groups); states: no.

`fid calibrate` (training half and the scored half's history):

```yaml
# fid calibrate: 142082 observations. Fitted on half the entities, scored on the other half.
grid_demand:
  half_life: 1.03h  # 95% range 52.83m to 1.2h
  baseline: 0.3  # 95% range 0.27 to 0.34
  source_fidelity: 1  # 95% range 1 to 1
```

Entity baselines written: 14.

| Staleness | Best fixed interval | fid schedule | ÷ fixed | schedule + floor 0.5 | ÷ fixed | fid floor | ÷ fixed |
|---|---|---|---|---|---|---|---|
| 2% | 20.5/day | 19.9/day | 0.97 (0.92–1.01) | 19.9/day | 0.97 (0.92–1.01) | 19.9/day | 0.97 (0.93–1.02) |
| 5% | 16.1/day | 15.7/day | 0.98 (0.95–1.01) | 15.7/day | 0.98 (0.95–1.01) | 16.9/day | 1.05 (0.98–1.12) |
| 10% | 11.5/day | 11.5/day | 1.00 (1.00–1.01) | 11.6/day | 1.01 (1.00–1.02) | 13.4/day | 1.16 (1.04–1.21) |
| 20% | 8.5/day | 8.6/day | 1.02 (1.00–1.03) | 9.8/day | 1.16 (1.04–1.30) | 9.0/day | 1.06 (1.02–1.12) |

Worst-served state (diagnostic): | Staleness | Fixed | fid schedule ÷ fixed |
|---|---|---|
| 2% | 20.5/day | 0.97 (0.92–1.01) |
| 5% | 16.1/day | 0.98 (0.95–1.01) |
| 10% | 11.5/day | 1.00 (1.00–1.01) |
| 20% | 8.5/day | 1.02 (1.00–1.03) |

Schedule's predicted staleness against actual, mean gap over prices with staleness 1–30%: **0.041** (3 prices)

| Price | Re-checks | Predicted | Actual |
|---|---|---|---|
| 1.0e-02 | 18.9/day | 0.058 | 0.026 |
| 1.6e-02 | 12.0/day | 0.155 | 0.087 |
| 2.5e-02 | 7.1/day | 0.286 | 0.264 |

## package_latest_version

923 entities (455 train / 468 scored groups); states: yes.

`fid calibrate` (training half and the scored half's history):

```yaml
# fid calibrate: 599824 observations. Fitted on half the entities, scored on the other half.
package_latest_version:
  states:
    dormant:
      half_life: 258d  # 95% range 225.99d to 293.6d
      baseline: 0.16  # 95% range 0.08 to 0.24
      source_fidelity: 0.997  # 95% range 0.995 to 0.998
    just_released:
      # fast share kept: held-out Brier 0.1227 vs one clock 0.1239
      half_life: 68.15d
      baseline: 0.03
      source_fidelity: 0.984
      fast: { share: 0.41, half_life: 9.62d }
    quiet:
      # fast share kept: held-out Brier 0.1174 vs one clock 0.1176
      half_life: 131.01d
      baseline: 0.06
      source_fidelity: 0.999
      fast: { share: 0.3, half_life: 30.86d }
```

Entity baselines written: 1549.

| Staleness | Best fixed interval | fid schedule | ÷ fixed | schedule + floor 0.5 | ÷ fixed | fid floor | ÷ fixed |
|---|---|---|---|---|---|---|---|
| 2% | every 2.0d | 14.4/day | 28.76 | 14.4/day | 28.76 | 19.4/day | 38.86 (4.25–121.99) |
| 5% | every 6.0d | every 4.1d | 1.44 (1.04–2.51) | every 4.1d | 1.44 (1.04–2.51) | every 2.5d | 2.37 (1.46–7.31) |
| 10% | every 14d | every 13d | 1.13 (0.97–1.33) | every 13d | 1.13 (0.97–1.33) | every 9.9d | 1.45 (1.20–1.85) |
| 20% | every 38d | every 36d | 1.05 (0.98–1.14) | every 36d | 1.05 (0.98–1.15) | every 28d | 1.35 (1.22–1.48) |

Worst-served state (diagnostic): | Staleness | Fixed | fid schedule ÷ fixed |
|---|---|---|
| 2% | every 1.5d | — |
| 5% | every 4.6d | 1.38 (0.97–2.61) |
| 10% | every 11d | 1.10 (0.96–1.32) |
| 20% | every 27d | 1.02 (0.96–1.10) |

Schedule's predicted staleness against actual, mean gap over prices with staleness 1–30%: **0.017** (46 prices)

| Price | Re-checks | Predicted | Actual |
|---|---|---|---|
| 1.0e-08 | 789.5/day | 0.006 | 0.019 |
| 1.6e-08 | 627.1/day | 0.006 | 0.019 |
| 2.5e-08 | 498.1/day | 0.006 | 0.019 |
| 4.0e-08 | 395.7/day | 0.006 | 0.019 |
| 6.3e-08 | 314.3/day | 0.006 | 0.019 |
| 1.0e-07 | 249.6/day | 0.006 | 0.019 |
| 1.6e-07 | 198.3/day | 0.006 | 0.019 |
| 2.5e-07 | 157.5/day | 0.006 | 0.019 |
| 4.0e-07 | 125.1/day | 0.006 | 0.019 |
| 6.3e-07 | 99.4/day | 0.006 | 0.019 |
| 1.0e-06 | 78.9/day | 0.006 | 0.020 |
| 1.6e-06 | 62.7/day | 0.006 | 0.020 |
| 2.5e-06 | 49.8/day | 0.006 | 0.020 |
| 4.0e-06 | 39.6/day | 0.006 | 0.020 |
| 6.3e-06 | 31.4/day | 0.006 | 0.020 |
| 1.0e-05 | 25.0/day | 0.006 | 0.020 |
| 1.6e-05 | 19.8/day | 0.006 | 0.020 |
| 2.5e-05 | 15.7/day | 0.006 | 0.020 |
| 4.0e-05 | 12.5/day | 0.006 | 0.020 |
| 6.3e-05 | 9.9/day | 0.006 | 0.020 |
| 1.0e-04 | 7.9/day | 0.006 | 0.020 |
| 1.6e-04 | 6.3/day | 0.007 | 0.021 |
| 2.5e-04 | 5.0/day | 0.007 | 0.021 |
| 4.0e-04 | 3.9/day | 0.007 | 0.021 |
| 6.3e-04 | 3.1/day | 0.008 | 0.022 |
| 1.0e-03 | 2.5/day | 0.008 | 0.023 |
| 1.6e-03 | 2.0/day | 0.009 | 0.023 |
| 2.5e-03 | 1.6/day | 0.009 | 0.025 |
| 4.0e-03 | 1.2/day | 0.010 | 0.026 |
| 6.3e-03 | every 1.0d | 0.012 | 0.028 |
| 1.0e-02 | every 1.3d | 0.013 | 0.030 |
| 1.6e-02 | every 1.6d | 0.015 | 0.032 |
| 2.5e-02 | every 2.0d | 0.018 | 0.036 |
| 4.0e-02 | every 2.6d | 0.021 | 0.040 |
| 6.3e-02 | every 3.3d | 0.025 | 0.044 |
| 1.0e-01 | every 4.2d | 0.030 | 0.050 |
| 1.6e-01 | every 5.3d | 0.037 | 0.057 |
| 2.5e-01 | every 6.8d | 0.045 | 0.067 |
| 4.0e-01 | every 8.7d | 0.055 | 0.078 |
| 6.3e-01 | every 11d | 0.067 | 0.092 |
| 1.0e+00 | every 14d | 0.083 | 0.108 |
| 1.6e+00 | every 19d | 0.103 | 0.129 |
| 2.5e+00 | every 25d | 0.127 | 0.155 |
| 4.0e+00 | every 33d | 0.159 | 0.189 |
| 6.3e+00 | every 45d | 0.198 | 0.229 |
| 1.0e+01 | every 62d | 0.248 | 0.280 |

## drug_label_warnings

376 entities (194 train / 182 scored groups); states: no.

`fid calibrate` (training half and the scored half's history):

```yaml
# fid calibrate: 140363 observations. Fitted on half the entities, scored on the other half.
drug_label_warnings:
  half_life: 3.65y  # 95% range 2.91y to 4.52y
  source_fidelity: 0.996  # 95% range 0.987 to 1
```

Entity baselines written: 0.

| Staleness | Best fixed interval | fid schedule | ÷ fixed | schedule + floor 0.5 | ÷ fixed | fid floor | ÷ fixed |
|---|---|---|---|---|---|---|---|
| 2% | every 82d | every 92d | 0.89 (0.81–1.02) | every 92d | 0.89 (0.81–1.02) | every 89d | 0.93 (0.87–1.08) |
| 5% | every 223d | every 190d | 1.18 (1.03–1.39) | every 190d | 1.18 (1.03–1.39) | every 224d | 1.00 (0.92–1.09) |
| 10% | every 1.2y | every 190d | 2.34 (1.88–2.81) | every 190d | 2.34 (1.88–2.81) | every 1.2y | 1.01 (0.87–1.10) |
| 20% | every 2.5y | every 190d | 4.81 (3.80–5.57) | every 190d | 4.81 (3.80–5.57) | every 2.5y | 0.99 (0.93–1.04) |

Worst-served state (diagnostic): | Staleness | Fixed | fid schedule ÷ fixed |
|---|---|---|
| 2% | every 82d | 0.89 (0.81–1.02) |
| 5% | every 223d | 1.18 (1.03–1.39) |
| 10% | every 1.2y | 2.34 (1.88–2.81) |
| 20% | every 2.5y | 4.81 (3.80–5.57) |

Schedule's predicted staleness against actual, mean gap over prices with staleness 1–30%: **0.009** (7 prices)

| Price | Re-checks | Predicted | Actual |
|---|---|---|---|
| 1.6e-01 | every 25d | 0.010 | 0.006 |
| 2.5e-01 | every 31d | 0.012 | 0.007 |
| 4.0e-01 | every 39d | 0.014 | 0.009 |
| 6.3e-01 | every 49d | 0.017 | 0.012 |
| 1.0e+00 | every 61d | 0.020 | 0.013 |
| 1.6e+00 | every 77d | 0.024 | 0.017 |
| 2.5e+00 | every 97d | 0.029 | 0.021 |
| 4.0e+00 | every 122d | 0.036 | 0.026 |
| 6.3e+00 | every 153d | 0.044 | 0.033 |
| 1.0e+01 | every 190d | 0.054 | 0.040 |

## All kinds together

One fact of each kind; re-checks and staleness averaged across kinds.

| Staleness | one TTL | tuned TTL | fid floor | fid schedule | schedule + floor 0.5 |
|---|---|---|---|---|---|
| 2% | 147.5/day | 58.5/day | — | — | — |
| 5% | 37.5/day | 21.9/day | — | — | — |
| 10% | 11.1/day | 7.8/day | — | — | — |
| 20% | 5.4/day | 3.3/day | — | — | — |

| Staleness | one TTL ÷ tuned | fid floor ÷ tuned | fid schedule ÷ tuned | schedule + floor 0.5 ÷ tuned |
|---|---|---|---|---|
| 2% | 2.52 (2.45–2.59) | — | — | — |
| 5% | 1.71 (1.37–2.01) | — | — | — |
| 10% | 1.43 (1.38–1.68) | — | — | — |
| 20% | 1.61 (1.44–1.76) | — | — | — |

## The rule

1. All kinds, fid schedule against the tuned TTL: 5% **not reached**, 10% **not reached**, 20% **not reached**
2. ship_position, against the best fixed interval: refused (fid calibrate: state 'at_anchor': half-life too uncertain: 95% range 1.77d to 5.92d (×inf; limit ×2.5). Re-check more entities, over a wider range of ages, or raise --max-spread)
2. airport_flight_category, against the best fixed interval: 5% **fid better**, 10% **fid better**, 20% **fid better**
2. package_latest_version, against the best fixed interval: 5% **fid worse**, 10% **tie**, 20% **tie**
3. airport_temperature, no loss (upper end ≤ 1.05): 5% **no loss**, 10% **no loss**, 20% **no loss**
3. grid_demand, no loss (upper end ≤ 1.05): 5% **no loss**, 10% **no loss**, 20% **no loss**
3. drug_label_warnings, no loss (upper end ≤ 1.05): 5% **loss**, 10% **loss**, 20% **loss**
4. Honest schedule (gap ≤ 0.02): ship_position **no**, airport_temperature **yes**, airport_flight_category **yes**, grid_demand **no**, package_latest_version **yes**, drug_label_warnings **yes**

**Rule passed: no**
