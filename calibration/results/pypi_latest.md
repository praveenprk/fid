### PyPI latest version — all snapshots: the latest version of package X is Y

- Still correct means: Y is still the highest final, unyanked release on PyPI
- 915 entities, 1,016,717 observations
- Empirical half-life: **83d**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **99d**, shape error 0.102 — poor fit
- Fitted with baseline (decay to b): b = 0.15, half-life = **63d**, shape error 0.056

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1343 |
| fitted half life | 0.1381 |
| constant base rate | 0.2469 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 78,209 |
| 1.0d | 0.980 | 78,209 |
| 3.0d | 0.949 | 78,209 |
| 7.0d | 0.898 | 78,209 |
| 14d | 0.828 | 78,209 |
| 30d | 0.715 | 78,209 |
| 61d | 0.569 | 78,209 |
| 91d | 0.476 | 78,209 |
| 182d | 0.315 | 78,209 |
| 274d | 0.229 | 78,209 |
| 365d | 0.179 | 78,209 |
| 1.5y | 0.124 | 78,209 |
| 2.0y | 0.094 | 78,209 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.041 | 0.123 | 115,707 |
| 0.1–0.2 | 0.163 | 0.217 | 38,569 |
| 0.2–0.3 | 0.300 | 0.302 | 38,569 |
| 0.5–0.6 | 0.547 | 0.465 | 38,569 |
| 0.6–0.7 | 0.668 | 0.558 | 38,569 |
| 0.8–0.9 | 0.820 | 0.704 | 38,569 |
| 0.9–1.0 | 0.968 | 0.927 | 192,845 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.1–0.2 | 0.167 | 0.147 | 154,276 |
| 0.2–0.3 | 0.276 | 0.302 | 38,569 |
| 0.4–0.5 | 0.478 | 0.465 | 38,569 |
| 0.5–0.6 | 0.599 | 0.558 | 38,569 |
| 0.7–0.8 | 0.771 | 0.704 | 38,569 |
| 0.8–0.9 | 0.884 | 0.819 | 38,569 |
| 0.9–1.0 | 0.976 | 0.953 | 154,276 |

### PyPI latest version — latest released in the 30 days before: the latest version of package X is Y

- Still correct means: Y is still the highest final, unyanked release on PyPI
- 914 entities, 294,736 observations
- Empirical half-life: **30d**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **35d**, shape error 0.097
- Fitted with baseline (decay to b): b = 0.07, half-life = **29d**, shape error 0.070

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1151 |
| fitted half life | 0.1167 |
| constant base rate | 0.2450 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 22,672 |
| 1.0d | 0.952 | 22,672 |
| 3.0d | 0.885 | 22,672 |
| 7.0d | 0.786 | 22,672 |
| 14d | 0.667 | 22,672 |
| 30d | 0.502 | 22,672 |
| 61d | 0.330 | 22,672 |
| 91d | 0.242 | 22,672 |
| 182d | 0.123 | 22,672 |
| 274d | 0.074 | 22,672 |
| 365d | 0.048 | 22,672 |
| 1.5y | 0.026 | 22,672 |
| 2.0y | 0.017 | 22,672 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.008 | 0.055 | 58,000 |
| 0.1–0.2 | 0.177 | 0.237 | 11,600 |
| 0.3–0.4 | 0.313 | 0.322 | 11,600 |
| 0.5–0.6 | 0.565 | 0.488 | 11,600 |
| 0.7–0.8 | 0.766 | 0.652 | 11,600 |
| 0.8–0.9 | 0.875 | 0.771 | 11,600 |
| 0.9–1.0 | 0.975 | 0.941 | 34,800 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.073 | 0.055 | 58,000 |
| 0.1–0.2 | 0.185 | 0.237 | 11,600 |
| 0.2–0.3 | 0.299 | 0.322 | 11,600 |
| 0.5–0.6 | 0.537 | 0.488 | 11,600 |
| 0.7–0.8 | 0.744 | 0.652 | 11,600 |
| 0.8–0.9 | 0.862 | 0.771 | 11,600 |
| 0.9–1.0 | 0.972 | 0.941 | 34,800 |

### PyPI latest version — latest is 30 days to 1 year old: the latest version of package X is Y

- Still correct means: Y is still the highest final, unyanked release on PyPI
- 886 entities, 562,458 observations
- Empirical half-life: **93d**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **105d**, shape error 0.073
- Fitted with baseline (decay to b): b = 0.10, half-life = **80d**, shape error 0.031

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1184 |
| fitted half life | 0.1199 |
| constant base rate | 0.2448 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 43,266 |
| 1.0d | 0.989 | 43,266 |
| 3.0d | 0.969 | 43,266 |
| 7.0d | 0.932 | 43,266 |
| 14d | 0.873 | 43,266 |
| 30d | 0.765 | 43,266 |
| 61d | 0.609 | 43,266 |
| 91d | 0.504 | 43,266 |
| 182d | 0.313 | 43,266 |
| 274d | 0.212 | 43,266 |
| 365d | 0.156 | 43,266 |
| 1.5y | 0.099 | 43,266 |
| 2.0y | 0.070 | 43,266 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.045 | 0.103 | 63,789 |
| 0.1–0.2 | 0.172 | 0.203 | 21,263 |
| 0.3–0.4 | 0.310 | 0.303 | 21,263 |
| 0.5–0.6 | 0.557 | 0.497 | 21,263 |
| 0.6–0.7 | 0.676 | 0.604 | 21,263 |
| 0.8–0.9 | 0.825 | 0.762 | 21,263 |
| 0.9–1.0 | 0.969 | 0.953 | 106,315 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.1–0.2 | 0.142 | 0.128 | 85,052 |
| 0.2–0.3 | 0.294 | 0.303 | 21,263 |
| 0.5–0.6 | 0.514 | 0.497 | 21,263 |
| 0.6–0.7 | 0.634 | 0.604 | 21,263 |
| 0.7–0.8 | 0.796 | 0.762 | 21,263 |
| 0.8–0.9 | 0.898 | 0.872 | 21,263 |
| 0.9–1.0 | 0.979 | 0.973 | 85,052 |

### PyPI latest version — no new latest version for over a year: the latest version of package X is Y

- Still correct means: Y is still the highest final, unyanked release on PyPI
- 496 entities, 159,523 observations
- Empirical half-life: **1.0y**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **1.0y**, shape error 0.058
- Fitted with baseline (decay to b): b = 0.24, half-life = **230d**, shape error 0.007

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1208 |
| fitted half life | 0.1214 |
| constant base rate | 0.1795 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 12,271 |
| 1.0d | 0.998 | 12,271 |
| 3.0d | 0.993 | 12,271 |
| 7.0d | 0.983 | 12,271 |
| 14d | 0.965 | 12,271 |
| 30d | 0.930 | 12,271 |
| 61d | 0.866 | 12,271 |
| 91d | 0.811 | 12,271 |
| 182d | 0.678 | 12,271 |
| 274d | 0.576 | 12,271 |
| 365d | 0.500 | 12,271 |
| 1.5y | 0.390 | 12,271 |
| 2.0y | 0.318 | 12,271 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.2–0.3 | 0.269 | 0.309 | 5,706 |
| 0.3–0.4 | 0.373 | 0.380 | 5,706 |
| 0.5–0.6 | 0.519 | 0.491 | 5,706 |
| 0.6–0.7 | 0.611 | 0.569 | 5,706 |
| 0.7–0.8 | 0.721 | 0.672 | 5,706 |
| 0.8–0.9 | 0.873 | 0.835 | 11,412 |
| 0.9–1.0 | 0.984 | 0.977 | 34,236 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.3–0.4 | 0.364 | 0.344 | 11,412 |
| 0.5–0.6 | 0.540 | 0.530 | 11,412 |
| 0.6–0.7 | 0.684 | 0.672 | 5,706 |
| 0.8–0.9 | 0.848 | 0.835 | 11,412 |
| 0.9–1.0 | 0.980 | 0.977 | 34,236 |
