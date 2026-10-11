### AIS 1 km, all vessels: vessel V is at position P (±1 km)

- Still correct means: next report within 1 km
- 1,448 entities, 437,736 observations
- Empirical half-life: **—** (never fell to 50% in the observed range)
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **11h**, shape error 0.242 — poor fit
- Fitted with baseline (decay to b): b = 0.70, half-life = **3.4m**, shape error 0.065
- Hand-picked guess (shipping_status: 4h): 4.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1896 |
| constant base rate | 0.1950 |
| fitted half life | 0.2197 |
| guessed half life | 0.2540 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 28,662 |
| 5.0m | 0.780 | 28,369 |
| 10m | 0.754 | 28,149 |
| 15m | 0.746 | 28,041 |
| 20m | 0.738 | 27,999 |
| 30m | 0.728 | 27,974 |
| 45m | 0.723 | 27,825 |
| 1.0h | 0.720 | 27,771 |
| 1.5h | 0.717 | 27,600 |
| 2.0h | 0.714 | 27,441 |
| 3.0h | 0.710 | 27,146 |
| 4.0h | 0.704 | 26,872 |
| 6.0h | 0.689 | 26,433 |
| 8.0h | 0.673 | 26,067 |
| 10h | 0.656 | 25,784 |
| 12h | 0.635 | 25,603 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.4–0.5 | 0.470 | 0.632 | 13,156 |
| 0.5–0.6 | 0.533 | 0.654 | 13,210 |
| 0.6–0.7 | 0.646 | 0.681 | 26,829 |
| 0.7–0.8 | 0.778 | 0.707 | 13,697 |
| 0.8–0.9 | 0.855 | 0.717 | 27,800 |
| 0.9–1.0 | 0.969 | 0.771 | 128,523 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.7–0.8 | 0.704 | 0.711 | 194,177 |
| 0.8–0.9 | 0.803 | 0.783 | 14,451 |
| 0.9–1.0 | 1.000 | 1.000 | 14,587 |

### AIS 1 km, underway: vessel V is at position P (±1 km)

- Still correct means: next report within 1 km
- 732 entities, 113,975 observations
- Empirical half-life: **3.2m**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **2.6m**, shape error 0.068
- Fitted with baseline (decay to b): b = 0.03, half-life = **2.3m**, shape error 0.046
- Hand-picked guess (shipping_status: 4h): 4.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.0414 |
| fitted half life | 0.0420 |
| constant base rate | 0.1038 |
| guessed half life | 0.4931 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 8,167 |
| 5.0m | 0.223 | 7,989 |
| 10m | 0.119 | 7,832 |
| 15m | 0.087 | 7,764 |
| 20m | 0.063 | 7,725 |
| 30m | 0.029 | 7,669 |
| 45m | 0.024 | 7,556 |
| 1.0h | 0.024 | 7,491 |
| 1.5h | 0.023 | 7,324 |
| 2.0h | 0.024 | 7,172 |
| 3.0h | 0.024 | 6,895 |
| 4.0h | 0.024 | 6,661 |
| 6.0h | 0.024 | 6,293 |
| 8.0h | 0.025 | 5,985 |
| 10h | 0.024 | 5,775 |
| 12h | 0.025 | 5,677 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.008 | 0.036 | 49,028 |
| 0.2–0.3 | 0.275 | 0.215 | 3,994 |
| 0.9–1.0 | 1.000 | 1.000 | 4,080 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.045 | 0.036 | 49,028 |
| 0.2–0.3 | 0.252 | 0.215 | 3,994 |
| 0.9–1.0 | 1.000 | 1.000 | 4,080 |

### AIS 1 km, moored or at anchor: vessel V is at position P (±1 km)

- Still correct means: next report within 1 km
- 1,018 entities, 323,761 observations
- Empirical half-life: **—** (never fell to 50% in the observed range)
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **1.6d**, shape error 0.006
- Fitted with baseline (decay to b): b = 0.51, half-life = **17h**, shape error 0.004
- Hand-picked guess (shipping_status: 4h): 4.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.0467 |
| fitted half life | 0.0467 |
| constant base rate | 0.0504 |
| guessed half life | 0.1718 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 20,495 |
| 5.0m | 0.999 | 20,380 |
| 10m | 0.998 | 20,317 |
| 15m | 0.998 | 20,277 |
| 20m | 0.995 | 20,274 |
| 30m | 0.992 | 20,305 |
| 45m | 0.984 | 20,269 |
| 1.0h | 0.977 | 20,280 |
| 1.5h | 0.967 | 20,276 |
| 2.0h | 0.959 | 20,269 |
| 3.0h | 0.943 | 20,251 |
| 4.0h | 0.928 | 20,211 |
| 6.0h | 0.897 | 20,140 |
| 8.0h | 0.866 | 20,082 |
| 10h | 0.838 | 20,009 |
| 12h | 0.809 | 19,926 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.8–0.9 | 0.853 | 0.850 | 41,161 |
| 0.9–1.0 | 0.980 | 0.979 | 124,952 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.8–0.9 | 0.854 | 0.850 | 41,161 |
| 0.9–1.0 | 0.979 | 0.979 | 124,952 |

### AIS 10 km, all vessels: vessel V is at position P (±10 km)

- Still correct means: next report within 10 km
- 1,448 entities, 437,736 observations
- Empirical half-life: **—** (never fell to 50% in the observed range)
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **14h**, shape error 0.173 — poor fit
- Fitted with baseline (decay to b): b = 0.71, half-life = **43m**, shape error 0.062
- Hand-picked guess (shipping_status: 4h): 4.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1252 |
| fitted half life | 0.1334 |
| constant base rate | 0.1380 |
| guessed half life | 0.1865 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 28,662 |
| 5.0m | 0.999 | 28,369 |
| 10m | 0.999 | 28,149 |
| 15m | 0.999 | 28,041 |
| 20m | 0.981 | 27,999 |
| 30m | 0.859 | 27,974 |
| 45m | 0.792 | 27,825 |
| 1.0h | 0.780 | 27,771 |
| 1.5h | 0.771 | 27,600 |
| 2.0h | 0.766 | 27,441 |
| 3.0h | 0.757 | 27,146 |
| 4.0h | 0.746 | 26,872 |
| 6.0h | 0.728 | 26,433 |
| 8.0h | 0.713 | 26,067 |
| 10h | 0.699 | 25,784 |
| 12h | 0.679 | 25,603 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.5–0.6 | 0.555 | 0.677 | 13,156 |
| 0.6–0.7 | 0.644 | 0.707 | 26,542 |
| 0.7–0.8 | 0.745 | 0.730 | 13,497 |
| 0.8–0.9 | 0.842 | 0.757 | 27,530 |
| 0.9–1.0 | 0.969 | 0.898 | 142,490 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.7–0.8 | 0.725 | 0.736 | 108,738 |
| 0.8–0.9 | 0.848 | 0.815 | 42,557 |
| 0.9–1.0 | 0.957 | 0.996 | 71,920 |

### AIS 10 km, underway: vessel V is at position P (±10 km)

- Still correct means: next report within 10 km
- 732 entities, 113,975 observations
- Empirical half-life: **30m**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **39m**, shape error 0.237 — poor fit
- Fitted with baseline (decay to b): b = 0.08, half-life = **33m**, shape error 0.249 — poor fit
- Hand-picked guess (shipping_status: 4h): 4.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1057 |
| fitted half life | 0.1075 |
| guessed half life | 0.2261 |
| constant base rate | 0.2480 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 8,167 |
| 5.0m | 1.000 | 7,989 |
| 10m | 1.000 | 7,832 |
| 15m | 0.998 | 7,764 |
| 20m | 0.934 | 7,725 |
| 30m | 0.486 | 7,669 |
| 45m | 0.237 | 7,556 |
| 1.0h | 0.193 | 7,491 |
| 1.5h | 0.166 | 7,324 |
| 2.0h | 0.153 | 7,172 |
| 3.0h | 0.134 | 6,895 |
| 4.0h | 0.112 | 6,661 |
| 6.0h | 0.095 | 6,293 |
| 8.0h | 0.093 | 5,985 |
| 10h | 0.093 | 5,775 |
| 12h | 0.087 | 5,677 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.010 | 0.100 | 18,812 |
| 0.1–0.2 | 0.155 | 0.160 | 7,224 |
| 0.3–0.4 | 0.337 | 0.195 | 3,738 |
| 0.4–0.5 | 0.442 | 0.242 | 3,776 |
| 0.5–0.6 | 0.580 | 0.490 | 3,828 |
| 0.6–0.7 | 0.696 | 0.935 | 3,857 |
| 0.7–0.8 | 0.762 | 0.997 | 3,874 |
| 0.8–0.9 | 0.834 | 0.999 | 3,919 |
| 0.9–1.0 | 0.957 | 1.000 | 8,074 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.081 | 0.091 | 15,367 |
| 0.1–0.2 | 0.127 | 0.146 | 7,020 |
| 0.2–0.3 | 0.218 | 0.169 | 3,649 |
| 0.3–0.4 | 0.340 | 0.195 | 3,738 |
| 0.4–0.5 | 0.436 | 0.242 | 3,776 |
| 0.5–0.6 | 0.569 | 0.490 | 3,828 |
| 0.6–0.7 | 0.684 | 0.935 | 3,857 |
| 0.7–0.8 | 0.751 | 0.997 | 3,874 |
| 0.8–0.9 | 0.825 | 0.999 | 3,919 |
| 0.9–1.0 | 0.954 | 1.000 | 8,074 |

### AIS 10 km, moored or at anchor: vessel V is at position P (±10 km)

- Still correct means: next report within 10 km
- 1,018 entities, 323,761 observations
- Empirical half-life: **—** (never fell to 50% in the observed range)
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **2.2d**, shape error 0.009
- Fitted with baseline (decay to b): b = 0.00, half-life = **2.2d**, shape error 0.009
- Hand-picked guess (shipping_status: 4h): 4.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life | 0.0315 |
| fitted half life with baseline | 0.0315 |
| constant base rate | 0.0338 |
| guessed half life | 0.1728 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 20,495 |
| 5.0m | 0.999 | 20,380 |
| 10m | 0.999 | 20,317 |
| 15m | 1.000 | 20,277 |
| 20m | 0.999 | 20,274 |
| 30m | 1.000 | 20,305 |
| 45m | 0.999 | 20,269 |
| 1.0h | 0.996 | 20,280 |
| 1.5h | 0.990 | 20,276 |
| 2.0h | 0.983 | 20,269 |
| 3.0h | 0.969 | 20,251 |
| 4.0h | 0.955 | 20,211 |
| 6.0h | 0.926 | 20,140 |
| 8.0h | 0.898 | 20,082 |
| 10h | 0.874 | 20,009 |
| 12h | 0.848 | 19,926 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.8–0.9 | 0.866 | 0.860 | 20,534 |
| 0.9–1.0 | 0.975 | 0.980 | 145,579 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.8–0.9 | 0.866 | 0.860 | 20,534 |
| 0.9–1.0 | 0.975 | 0.980 | 145,579 |
