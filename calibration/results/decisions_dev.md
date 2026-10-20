# Decision benchmark v2 (dev set): fid schedule against fixed intervals

Re-checks per fact needed to keep held-out staleness at or below each target; ratios are fid's re-checks ÷ the other policy's (95% range over 200 resamples of held-out entities; below 1: fid needs fewer).

## ship_position

500 entities (205 train / 225 scored groups); states: yes.

`fid calibrate` (training half and the scored half's history):

```yaml
# fid calibrate: 231144 observations. Fitted on half the entities, scored on the other half.
ship_position:
  states:
    stationary:
      half_life: 1.31d  # 95% range 1.09d to 1.68d
      source_fidelity: 1  # 95% range 1 to 1
    underway:
      half_life: 1.87m  # 95% range 1.68m to 2.15m
      baseline: 0.03  # 95% range 0.02 to 0.04
      source_fidelity: 1  # 95% range 1 to 1
```

Entity baselines written: 0.

| Staleness | Best fixed interval | fid schedule | ÷ fixed | schedule + floor 0.5 | ÷ fixed | fid floor | ÷ fixed |
|---|---|---|---|---|---|---|---|
| 2% | 480.0/day | 218.0/day | 0.45 (0.36–0.53) | 241.5/day | 0.50 (0.42–0.57) | 463.7/day | 0.97 (0.80–1.10) |
| 5% | 383.6/day | 152.2/day | 0.40 (0.34–0.45) | 219.9/day | 0.57 (0.50–0.65) | 431.4/day | 1.12 (0.95–1.27) |
| 10% | 284.9/day | 112.2/day | 0.39 (0.33–0.45) | 198.9/day | 0.70 (0.64–0.79) | 394.7/day | 1.39 (0.45–1.55) |
| 20% | 134.4/day | 59.0/day | 0.44 (0.41–0.49) | 198.6/day | 1.48 (1.22–2.07) | 133.7/day | 1.00 (0.82–1.39) |

Worst-served state (diagnostic): | Staleness | Fixed | fid schedule ÷ fixed |
|---|---|---|
| 2% | 670.7/day | 0.48 (0.37–0.58) |
| 5% | 514.9/day | 0.43 (0.35–0.51) |
| 10% | 433.4/day | 0.38 (0.33–0.44) |
| 20% | 343.3/day | 0.39 (0.33–0.44) |

Schedule's predicted staleness against actual, mean gap over prices with staleness 1–30%: **0.040** (4 prices)

| Price | Re-checks | Predicted | Actual |
|---|---|---|---|
| 2.5e-04 | 256.5/day | 0.054 | 0.010 |
| 4.0e-04 | 250.7/day | 0.055 | 0.013 |
| 6.3e-04 | 168.7/day | 0.094 | 0.033 |
| 1.0e-03 | 105.0/day | 0.150 | 0.111 |
| 1.6e-03 | 56.9/day | 0.222 | 0.205 |
| 2.5e-03 | 8.3/day | 0.422 | 0.425 |
| 4.0e-03 | 7.2/day | 0.425 | 0.424 |
| 6.3e-03 | 6.2/day | 0.425 | 0.426 |
| 1.0e-02 | 5.4/day | 0.414 | 0.417 |
| 1.6e-02 | 4.8/day | 0.416 | 0.421 |
| 2.5e-02 | 4.3/day | 0.417 | 0.421 |
| 4.0e-02 | 3.8/day | 0.421 | 0.424 |
| 6.3e-02 | 3.6/day | 0.428 | 0.429 |
| 1.0e-01 | 3.3/day | 0.433 | 0.429 |
| 1.6e-01 | 3.1/day | 0.434 | 0.429 |
| 2.5e-01 | 3.1/day | 0.434 | 0.429 |
| 4.0e-01 | 3.1/day | 0.434 | 0.429 |
| 6.3e-01 | 3.1/day | 0.434 | 0.429 |
| 1.0e+00 | 3.1/day | 0.434 | 0.429 |
| 1.6e+00 | 3.1/day | 0.434 | 0.429 |
| 2.5e+00 | 3.1/day | 0.434 | 0.429 |
| 4.0e+00 | 3.1/day | 0.434 | 0.429 |
| 6.3e+00 | 3.1/day | 0.434 | 0.429 |
| 1.0e+01 | 3.1/day | 0.434 | 0.429 |
| 1.6e+01 | 3.1/day | 0.434 | 0.429 |
| 2.5e+01 | 3.1/day | 0.434 | 0.429 |
| 4.0e+01 | 3.1/day | 0.434 | 0.429 |
| 6.3e+01 | 3.1/day | 0.434 | 0.429 |
| 1.0e+02 | 3.1/day | 0.434 | 0.429 |
| 1.6e+02 | 3.1/day | 0.434 | 0.429 |
| 2.5e+02 | 3.1/day | 0.434 | 0.429 |
| 4.0e+02 | 3.1/day | 0.434 | 0.429 |
| 6.3e+02 | 3.1/day | 0.434 | 0.429 |
| 1.0e+03 | 3.1/day | 0.434 | 0.429 |
| 1.6e+03 | 3.1/day | 0.434 | 0.429 |
| 2.5e+03 | 3.1/day | 0.434 | 0.429 |
| 4.0e+03 | 3.1/day | 0.434 | 0.429 |
| 6.3e+03 | 3.1/day | 0.434 | 0.429 |
| 1.0e+04 | 3.1/day | 0.434 | 0.429 |
| 1.6e+04 | 3.1/day | 0.434 | 0.429 |
| 2.5e+04 | 3.1/day | 0.434 | 0.429 |
| 4.0e+04 | 3.1/day | 0.434 | 0.429 |
| 6.3e+04 | 3.1/day | 0.434 | 0.429 |
| 1.0e+05 | 3.1/day | 0.434 | 0.429 |

## airport_temperature

10 entities (6 train / 4 scored groups); states: no.

`fid calibrate` (training half and the scored half's history):

```yaml
# fid calibrate: 113906 observations. Fitted on half the entities, scored on the other half.
airport_temperature:
  # echo kept: held-out Brier 0.1857 vs decay 0.1897
  half_life: 2.72h
  baseline: 0.09
  source_fidelity: 1
  echo: { period: 1d, strength: 0.44, width: 5.17h, fade: 2.73d }
```

Entity baselines written: 0.

| Staleness | Best fixed interval | fid schedule | ÷ fixed | schedule + floor 0.5 | ÷ fixed | fid floor | ÷ fixed |
|---|---|---|---|---|---|---|---|
| 2% | 21.2/day | 21.2/day | 1.00 (1.00–1.00) | 21.2/day | 1.00 (1.00–1.00) | 21.2/day | 1.00 (1.00–1.00) |
| 5% | 17.6/day | 17.6/day | 1.00 (1.00–1.00) | 17.6/day | 1.00 (1.00–1.00) | 17.6/day | 1.00 (1.00–1.00) |
| 10% | 12.9/day | 12.9/day | 1.00 (1.00–1.00) | 12.9/day | 1.00 (1.00–1.00) | 12.9/day | 1.00 (1.00–1.00) |
| 20% | 8.9/day | 8.9/day | 1.00 (1.00–1.00) | 8.9/day | 1.00 (1.00–1.11) | 8.9/day | 1.00 (1.00–1.02) |

Worst-served state (diagnostic): | Staleness | Fixed | fid schedule ÷ fixed |
|---|---|---|
| 2% | 21.2/day | 1.00 (1.00–1.00) |
| 5% | 17.6/day | 1.00 (1.00–1.00) |
| 10% | 12.9/day | 1.00 (1.00–1.00) |
| 20% | 8.9/day | 1.00 (1.00–1.00) |

Schedule's predicted staleness against actual, mean gap over prices with staleness 1–30%: **0.028** (3 prices)

| Price | Re-checks | Predicted | Actual |
|---|---|---|---|
| 1.0e-02 | 12.0/day | 0.095 | 0.111 |
| 1.6e-02 | 12.0/day | 0.095 | 0.111 |
| 2.5e-02 | 8.0/day | 0.179 | 0.230 |
| 4.0e-02 | 6.0/day | 0.251 | 0.314 |
| 6.3e-02 | 4.0/day | 0.366 | 0.447 |

## airport_flight_category

10 entities (6 train / 4 scored groups); states: yes.

`fid calibrate` (training half and the scored half's history):

```yaml
# fid calibrate: 113932 observations. Fitted on half the entities, scored on the other half.
airport_flight_category:
  states:
    IFR:
      half_life: 1.75h  # 95% range 1.33h to 2.09h
      baseline: 0.11  # 95% range 0.09 to 0.13
      source_fidelity: 1  # 95% range 1 to 1
    LIFR:
      half_life: 3.05h  # 95% range 2.06h to 4.01h
      baseline: 0.13  # 95% range 0.09 to 0.17
      source_fidelity: 1  # 95% range 1 to 1
    MVFR:
      # fast share kept: held-out Brier 0.1785 vs one clock 0.1799
      half_life: 5.32h  # 95% range 4.1h to 6.34h
      baseline: 0.18  # 95% range 0.15 to 0.2
      source_fidelity: 1  # 95% range 1 to 1
      fast: { share: 0.429, half_life: 32.74m }
    VFR:
      half_life: 4.02h  # 95% range 2.87h to 5.7h
      baseline: 0.84  # 95% range 0.81 to 0.87
      source_fidelity: 1  # 95% range 1 to 1
```

Entity baselines written: 40.

| Staleness | Best fixed interval | fid schedule | ÷ fixed | schedule + floor 0.5 | ÷ fixed | fid floor | ÷ fixed |
|---|---|---|---|---|---|---|---|
| 2% | 16.5/day | 11.3/day | 0.68 (0.54–0.81) | 11.3/day | 0.68 (0.54–0.81) | 11.1/day | 0.67 (0.52–0.81) |
| 5% | 9.6/day | 6.2/day | 0.64 (0.45–0.81) | 6.2/day | 0.64 (0.37–0.81) | 6.1/day | 0.64 (0.41–0.83) |
| 10% | 4.4/day | 2.4/day | 0.55 (0.07–1.00) | 2.3/day | 0.51 (0.02–1.00) | 1.0/day | 0.23 (0.01–1.00) |
| 20% | every 274d | every 274d | 1.00 (0.00–1.00) | every 274d | 1.00 (0.00–1.00) | every 274d | 1.00 (0.00–1.00) |

Worst-served state (diagnostic): | Staleness | Fixed | fid schedule ÷ fixed |
|---|---|---|
| 2% | 22.2/day | 0.90 (0.80–1.00) |
| 5% | 19.7/day | 0.89 (0.80–0.99) |
| 10% | 16.1/day | 0.88 (0.74–0.98) |
| 20% | 10.9/day | 0.87 (0.76–0.99) |

Schedule's predicted staleness against actual, mean gap over prices with staleness 1–30%: **0.002** (40 prices)

| Price | Re-checks | Predicted | Actual |
|---|---|---|---|
| 1.6e-03 | 13.9/day | 0.011 | 0.014 |
| 2.5e-03 | 12.9/day | 0.014 | 0.015 |
| 4.0e-03 | 10.3/day | 0.022 | 0.024 |
| 6.3e-03 | 8.1/day | 0.033 | 0.034 |
| 1.0e-02 | 5.9/day | 0.056 | 0.053 |
| 1.6e-02 | 4.0/day | 0.081 | 0.074 |
| 2.5e-02 | 2.6/day | 0.111 | 0.098 |
| 4.0e-02 | every 1.0d | 0.150 | 0.136 |
| 6.3e-02 | every 274d | 0.169 | 0.168 |
| 1.0e-01 | every 274d | 0.169 | 0.168 |
| 1.6e-01 | every 274d | 0.169 | 0.168 |
| 2.5e-01 | every 274d | 0.169 | 0.168 |
| 4.0e-01 | every 274d | 0.169 | 0.168 |
| 6.3e-01 | every 274d | 0.169 | 0.168 |
| 1.0e+00 | every 274d | 0.169 | 0.168 |
| 1.6e+00 | every 274d | 0.169 | 0.168 |
| 2.5e+00 | every 274d | 0.169 | 0.168 |
| 4.0e+00 | every 274d | 0.169 | 0.168 |
| 6.3e+00 | every 274d | 0.169 | 0.168 |
| 1.0e+01 | every 274d | 0.169 | 0.168 |
| 1.6e+01 | every 274d | 0.169 | 0.168 |
| 2.5e+01 | every 274d | 0.169 | 0.168 |
| 4.0e+01 | every 274d | 0.169 | 0.168 |
| 6.3e+01 | every 274d | 0.169 | 0.168 |
| 1.0e+02 | every 274d | 0.169 | 0.168 |
| 1.6e+02 | every 274d | 0.169 | 0.168 |
| 2.5e+02 | every 274d | 0.169 | 0.168 |
| 4.0e+02 | every 274d | 0.169 | 0.168 |
| 6.3e+02 | every 274d | 0.169 | 0.168 |
| 1.0e+03 | every 274d | 0.169 | 0.168 |
| 1.6e+03 | every 274d | 0.169 | 0.168 |
| 2.5e+03 | every 274d | 0.169 | 0.168 |
| 4.0e+03 | every 274d | 0.169 | 0.168 |
| 6.3e+03 | every 274d | 0.169 | 0.168 |
| 1.0e+04 | every 274d | 0.169 | 0.168 |
| 1.6e+04 | every 274d | 0.169 | 0.168 |
| 2.5e+04 | every 274d | 0.169 | 0.168 |
| 4.0e+04 | every 274d | 0.169 | 0.168 |
| 6.3e+04 | every 274d | 0.169 | 0.168 |
| 1.0e+05 | every 274d | 0.169 | 0.168 |

## grid_demand

14 entities (7 train / 7 scored groups); states: no.

`fid calibrate` (training half and the scored half's history):

```yaml
# fid calibrate: 142318 observations. Fitted on half the entities, scored on the other half.
grid_demand:
  # echo kept: held-out Brier 0.1985 vs decay 0.2105
  half_life: 1.45h
  baseline: 0.17
  source_fidelity: 1
  echo: { period: 1d, strength: 0.76, width: 1.74h, fade: 1.83d }
```

Entity baselines written: 14.

| Staleness | Best fixed interval | fid schedule | ÷ fixed | schedule + floor 0.5 | ÷ fixed | fid floor | ÷ fixed |
|---|---|---|---|---|---|---|---|
| 2% | 20.3/day | 19.5/day | 0.96 (0.93–1.00) | 19.5/day | 0.96 (0.93–1.00) | 20.0/day | 0.99 (0.95–1.01) |
| 5% | 15.7/day | 15.3/day | 0.98 (0.95–1.00) | 15.3/day | 0.98 (0.95–1.00) | 15.4/day | 0.98 (0.94–1.05) |
| 10% | 11.4/day | 11.4/day | 1.00 (1.00–1.00) | 11.4/day | 1.00 (1.00–1.01) | 12.2/day | 1.08 (1.00–1.11) |
| 20% | 8.5/day | 8.5/day | 1.00 (1.00–1.00) | 10.3/day | 1.21 (1.10–1.32) | 8.8/day | 1.03 (1.01–1.05) |

Worst-served state (diagnostic): | Staleness | Fixed | fid schedule ÷ fixed |
|---|---|---|
| 2% | 20.3/day | 0.96 (0.93–1.00) |
| 5% | 15.7/day | 0.98 (0.95–1.00) |
| 10% | 11.4/day | 1.00 (1.00–1.00) |
| 20% | 8.5/day | 1.00 (1.00–1.00) |

Schedule's predicted staleness against actual, mean gap over prices with staleness 1–30%: **0.035** (3 prices)

| Price | Re-checks | Predicted | Actual |
|---|---|---|---|
| 1.0e-02 | 18.9/day | 0.052 | 0.023 |
| 1.6e-02 | 12.0/day | 0.138 | 0.082 |
| 2.5e-02 | 8.0/day | 0.241 | 0.221 |
| 4.0e-02 | 3.8/day | 0.424 | 0.478 |

## package_latest_version

923 entities (455 train / 468 scored groups); states: yes.

`fid calibrate` (training half and the scored half's history):

```yaml
# fid calibrate: 599824 observations. Fitted on half the entities, scored on the other half.
package_latest_version:
  states:
    dormant:
      half_life: 252.58d  # 95% range 222.42d to 287.22d
      baseline: 0.16  # 95% range 0.09 to 0.24
      source_fidelity: 1  # 95% range 1 to 1
    just_released:
      # fast share kept: held-out Brier 0.1227 vs one clock 0.1249
      half_life: 59.76d  # 95% range 51.56d to 71.18d
      baseline: 0.04  # 95% range 0.03 to 0.04
      source_fidelity: 1  # 95% range 1 to 1
      fast: { share: 0.354, half_life: 7.23d }
    quiet:
      # fast share kept: held-out Brier 0.1174 vs one clock 0.1177
      half_life: 129.68d  # 95% range 117.6d to 146.07d
      baseline: 0.06  # 95% range 0.05 to 0.07
      source_fidelity: 1  # 95% range 1 to 1
      fast: { share: 0.292, half_life: 29.96d }
```

Entity baselines written: 0.

| Staleness | Best fixed interval | fid schedule | ÷ fixed | schedule + floor 0.5 | ÷ fixed | fid floor | ÷ fixed |
|---|---|---|---|---|---|---|---|
| 2% | every 2.0d | every 2.5d | 0.81 (0.76–0.86) | every 2.5d | 0.81 (0.76–0.86) | every 2.1d | 0.96 (0.82–1.09) |
| 5% | every 6.0d | every 6.9d | 0.86 (0.83–0.89) | every 6.9d | 0.86 (0.83–0.89) | every 5.7d | 1.05 (0.97–1.12) |
| 10% | every 14d | every 16d | 0.90 (0.87–0.92) | every 16d | 0.90 (0.87–0.92) | every 13d | 1.10 (1.05–1.16) |
| 20% | every 38d | every 40d | 0.95 (0.93–0.97) | every 40d | 0.94 (0.92–0.97) | every 33d | 1.16 (1.12–1.21) |

Worst-served state (diagnostic): | Staleness | Fixed | fid schedule ÷ fixed |
|---|---|---|
| 2% | every 1.5d | 0.79 (0.75–0.85) |
| 5% | every 4.6d | 0.85 (0.82–0.88) |
| 10% | every 11d | 0.88 (0.86–0.91) |
| 20% | every 27d | 0.93 (0.91–0.95) |

Schedule's predicted staleness against actual, mean gap over prices with staleness 1–30%: **0.005** (16 prices)

| Price | Re-checks | Predicted | Actual |
|---|---|---|---|
| 2.5e-03 | 1.7/day | 0.004 | 0.005 |
| 4.0e-03 | 1.3/day | 0.005 | 0.006 |
| 6.3e-03 | 1.1/day | 0.007 | 0.008 |
| 1.0e-02 | every 1.2d | 0.008 | 0.010 |
| 1.6e-02 | every 1.5d | 0.011 | 0.013 |
| 2.5e-02 | every 1.9d | 0.013 | 0.016 |
| 4.0e-02 | every 2.4d | 0.017 | 0.020 |
| 6.3e-02 | every 3.1d | 0.021 | 0.024 |
| 1.0e-01 | every 3.9d | 0.027 | 0.030 |
| 1.6e-01 | every 5.0d | 0.033 | 0.037 |
| 2.5e-01 | every 6.4d | 0.042 | 0.047 |
| 4.0e-01 | every 8.2d | 0.053 | 0.058 |
| 6.3e-01 | every 11d | 0.066 | 0.071 |
| 1.0e+00 | every 14d | 0.083 | 0.088 |
| 1.6e+00 | every 18d | 0.104 | 0.110 |
| 2.5e+00 | every 24d | 0.130 | 0.139 |
| 4.0e+00 | every 32d | 0.164 | 0.170 |
| 6.3e+00 | every 44d | 0.204 | 0.214 |
| 1.0e+01 | every 61d | 0.255 | 0.264 |
| 1.6e+01 | every 85d | 0.315 | 0.322 |
| 2.5e+01 | every 121d | 0.384 | 0.394 |
| 4.0e+01 | every 179d | 0.473 | 0.476 |

## drug_label_warnings

376 entities (194 train / 182 scored groups); states: no.

`fid calibrate` (training half and the scored half's history):

```yaml
# fid calibrate: 140363 observations. Fitted on half the entities, scored on the other half.
drug_label_warnings:
  half_life: 3.61y  # 95% range 2.91y to 4.47y
  source_fidelity: 1  # 95% range 1 to 1
```

Entity baselines written: 0.

| Staleness | Best fixed interval | fid schedule | ÷ fixed | schedule + floor 0.5 | ÷ fixed | fid floor | ÷ fixed |
|---|---|---|---|---|---|---|---|
| 2% | every 82d | every 91d | 0.91 (0.79–1.01) | every 91d | 0.91 (0.79–1.01) | every 93d | 0.89 (0.83–1.02) |
| 5% | every 223d | every 221d | 1.01 (0.91–1.11) | every 221d | 1.01 (0.91–1.11) | every 223d | 1.00 (0.95–1.08) |
| 10% | every 1.2y | every 1.2y | 1.01 (0.90–1.11) | every 1.2y | 1.01 (0.90–1.11) | every 1.2y | 1.02 (0.87–1.11) |
| 20% | every 2.5y | every 2.4y | 1.03 (0.93–1.12) | every 2.5y | 1.01 (0.93–1.18) | every 2.5y | 1.00 (0.95–1.04) |

Worst-served state (diagnostic): | Staleness | Fixed | fid schedule ÷ fixed |
|---|---|---|
| 2% | every 82d | 0.91 (0.79–1.01) |
| 5% | every 223d | 1.01 (0.91–1.11) |
| 10% | every 1.2y | 1.01 (0.90–1.11) |
| 20% | every 2.5y | 1.03 (0.93–1.12) |

Schedule's predicted staleness against actual, mean gap over prices with staleness 1–30%: **0.016** (16 prices)

| Price | Re-checks | Predicted | Actual |
|---|---|---|---|
| 1.6e-01 | every 24d | 0.006 | 0.005 |
| 2.5e-01 | every 31d | 0.008 | 0.007 |
| 4.0e-01 | every 39d | 0.010 | 0.008 |
| 6.3e-01 | every 49d | 0.013 | 0.011 |
| 1.0e+00 | every 61d | 0.016 | 0.013 |
| 1.6e+00 | every 77d | 0.020 | 0.016 |
| 2.5e+00 | every 96d | 0.025 | 0.021 |
| 4.0e+00 | every 121d | 0.032 | 0.024 |
| 6.3e+00 | every 152d | 0.040 | 0.034 |
| 1.0e+01 | every 188d | 0.050 | 0.040 |
| 1.6e+01 | every 236d | 0.063 | 0.054 |
| 2.5e+01 | every 296d | 0.078 | 0.067 |
| 4.0e+01 | every 1.0y | 0.098 | 0.082 |
| 6.3e+01 | every 1.2y | 0.121 | 0.103 |
| 1.0e+02 | every 1.5y | 0.151 | 0.115 |
| 1.6e+02 | every 1.9y | 0.189 | 0.163 |
| 2.5e+02 | every 2.4y | 0.227 | 0.198 |
| 4.0e+02 | every 2.9y | 0.273 | 0.233 |
| 6.3e+02 | every 3.3y | 0.328 | 0.299 |
| 1.0e+03 | every 3.8y | 0.370 | 0.336 |
| 1.6e+03 | every 4.2y | 0.404 | 0.364 |
| 2.5e+03 | every 4.2y | 0.404 | 0.364 |
| 4.0e+03 | every 4.2y | 0.404 | 0.364 |
| 6.3e+03 | every 4.2y | 0.404 | 0.364 |
| 1.0e+04 | every 4.2y | 0.404 | 0.364 |
| 1.6e+04 | every 4.2y | 0.404 | 0.364 |
| 2.5e+04 | every 4.2y | 0.404 | 0.364 |
| 4.0e+04 | every 4.2y | 0.404 | 0.364 |
| 6.3e+04 | every 4.2y | 0.404 | 0.364 |
| 1.0e+05 | every 4.2y | 0.404 | 0.364 |

## All kinds together

One fact of each kind; re-checks and staleness averaged across kinds.

| Staleness | one TTL | tuned TTL | fid floor | fid schedule | schedule + floor 0.5 |
|---|---|---|---|---|---|
| 2% | 135.4/day | 55.0/day | 84.1/day | 29.1/day | 46.0/day |
| 5% | 25.6/day | 17.8/day | 77.4/day | 16.8/day | 41.6/day |
| 10% | 10.9/day | 7.2/day | 73.1/day | 7.5/day | 37.8/day |
| 20% | 5.3/day | 3.1/day | 35.4/day | 3.1/day | 36.2/day |

| Staleness | one TTL ÷ tuned | fid floor ÷ tuned | fid schedule ÷ tuned | schedule + floor 0.5 ÷ tuned |
|---|---|---|---|---|
| 2% | 2.46 (2.38–2.55) | 1.53 (1.44–1.64) | 0.53 (0.50–0.56) | 0.84 (0.79–0.89) |
| 5% | 1.43 (1.30–1.79) | 4.34 (3.64–5.01) | 0.94 (0.80–1.10) | 2.33 (1.95–2.68) |
| 10% | 1.50 (1.35–1.77) | 10.13 (8.04–12.05) | 1.04 (0.92–1.13) | 5.23 (4.25–6.14) |
| 20% | 1.70 (1.53–1.85) | 11.35 (9.89–13.35) | 0.98 (0.93–1.02) | 11.59 (9.36–13.94) |

## The rule

1. All kinds, fid schedule against the tuned TTL: 5% **tie**, 10% **tie**, 20% **tie**
2. ship_position, against the best fixed interval: 5% **fid better**, 10% **fid better**, 20% **fid better**
2. airport_flight_category, against the best fixed interval: 5% **fid better**, 10% **tie**, 20% **tie**
2. package_latest_version, against the best fixed interval: 5% **fid better**, 10% **fid better**, 20% **fid better**
3. airport_temperature, no loss (lower end ≤ 1, ratio ≤ 1.05): 5% **no loss**, 10% **no loss**, 20% **no loss**
3. grid_demand, no loss (lower end ≤ 1, ratio ≤ 1.05): 5% **no loss**, 10% **no loss**, 20% **no loss**
3. drug_label_warnings, no loss (lower end ≤ 1, ratio ≤ 1.05): 5% **no loss**, 10% **no loss**, 20% **no loss**
4. Honest schedule (gap ≤ 0.02): ship_position **no**, airport_temperature **no**, airport_flight_category **yes**, grid_demand **no**, package_latest_version **yes**, drug_label_warnings **yes**

**Rule passed: no**
