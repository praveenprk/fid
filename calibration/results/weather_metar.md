### METAR temperature: the temperature at airport A is T

- Still correct means: within ±2 °F
- 10 entities, 1,575,505 observations
- Empirical half-life: **2.9h**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **4.2h**, shape error 0.415 — poor fit
- Fitted with baseline (decay to b): b = 0.25, half-life = **1.8h**, shape error 0.183 — poor fit
- Hand-picked guess (weather_forecast: 6h): 6.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1809 |
| fitted half life | 0.2145 |
| guessed half life | 0.2184 |
| constant base rate | 0.2263 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 87,785 |
| 1.0h | 0.830 | 87,727 |
| 2.0h | 0.621 | 87,711 |
| 3.0h | 0.482 | 87,700 |
| 4.0h | 0.392 | 87,690 |
| 6.0h | 0.286 | 87,670 |
| 8.0h | 0.230 | 87,650 |
| 10h | 0.204 | 87,630 |
| 12h | 0.194 | 87,610 |
| 15h | 0.200 | 87,580 |
| 18h | 0.242 | 87,550 |
| 21h | 0.333 | 87,520 |
| 1.0d | 0.433 | 87,490 |
| 1.2d | 0.236 | 87,430 |
| 1.5d | 0.171 | 87,370 |
| 2.0d | 0.342 | 87,250 |
| 2.5d | 0.169 | 87,131 |
| 3.0d | 0.320 | 87,011 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.030 | 0.259 | 279,484 |
| 0.1–0.2 | 0.142 | 0.159 | 35,030 |
| 0.2–0.3 | 0.242 | 0.155 | 70,092 |
| 0.3–0.4 | 0.354 | 0.180 | 35,058 |
| 0.4–0.5 | 0.459 | 0.227 | 35,066 |
| 0.5–0.6 | 0.595 | 0.323 | 35,074 |
| 0.6–0.7 | 0.677 | 0.408 | 35,078 |
| 0.7–0.8 | 0.771 | 0.549 | 35,083 |
| 0.8–0.9 | 0.878 | 0.776 | 35,091 |
| 0.9–1.0 | 1.000 | 1.000 | 35,113 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.2–0.3 | 0.274 | 0.231 | 384,606 |
| 0.3–0.4 | 0.338 | 0.204 | 70,124 |
| 0.4–0.5 | 0.452 | 0.323 | 35,074 |
| 0.5–0.6 | 0.527 | 0.408 | 35,078 |
| 0.6–0.7 | 0.634 | 0.549 | 35,083 |
| 0.7–0.8 | 0.786 | 0.776 | 35,091 |
| 0.9–1.0 | 1.000 | 1.000 | 35,113 |

### METAR flight category: airport A is VFR / MVFR / IFR / LIFR

- Still correct means: same flight category
- 10 entities, 1,575,714 observations
- Empirical half-life: **—** (never fell to 50% in the observed range)
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **3.3d**, shape error 0.184 — poor fit
- Fitted with baseline (decay to b): b = 0.73, half-life = **2.9h**, shape error 0.031
- Hand-picked guess (weather_forecast: 6h): 6.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1554 |
| constant base rate | 0.1603 |
| fitted half life | 0.1664 |
| guessed half life | 0.4408 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 87,791 |
| 1.0h | 0.912 | 87,738 |
| 2.0h | 0.876 | 87,723 |
| 3.0h | 0.853 | 87,712 |
| 4.0h | 0.834 | 87,702 |
| 6.0h | 0.805 | 87,682 |
| 8.0h | 0.784 | 87,662 |
| 10h | 0.769 | 87,642 |
| 12h | 0.758 | 87,622 |
| 15h | 0.750 | 87,592 |
| 18h | 0.750 | 87,562 |
| 21h | 0.754 | 87,532 |
| 1.0d | 0.753 | 87,502 |
| 1.2d | 0.728 | 87,442 |
| 1.5d | 0.715 | 87,382 |
| 2.0d | 0.726 | 87,262 |
| 2.5d | 0.704 | 87,143 |
| 3.0d | 0.718 | 87,023 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.5–0.6 | 0.546 | 0.725 | 69,680 |
| 0.6–0.7 | 0.644 | 0.744 | 34,912 |
| 0.7–0.8 | 0.739 | 0.734 | 69,944 |
| 0.8–0.9 | 0.848 | 0.771 | 175,160 |
| 0.9–1.0 | 0.962 | 0.861 | 280,717 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.7–0.8 | 0.733 | 0.761 | 454,912 |
| 0.8–0.9 | 0.862 | 0.863 | 105,277 |
| 0.9–1.0 | 0.971 | 0.960 | 70,224 |

### METAR adverse conditions: airport A is IFR or worse

- Still correct means: still IFR or LIFR
- 10 entities, 105,909 observations
- Empirical half-life: **5.1h**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **7.0h**, shape error 0.178 — poor fit
- Fitted with baseline (decay to b): b = 0.20, half-life = **3.5h**, shape error 0.078
- Hand-picked guess (weather_forecast: 6h): 6.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1990 |
| guessed half life | 0.2143 |
| fitted half life | 0.2152 |
| constant base rate | 0.2498 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 5,906 |
| 1.0h | 0.789 | 5,900 |
| 2.0h | 0.686 | 5,901 |
| 3.0h | 0.614 | 5,901 |
| 4.0h | 0.554 | 5,902 |
| 6.0h | 0.459 | 5,902 |
| 8.0h | 0.389 | 5,902 |
| 10h | 0.332 | 5,899 |
| 12h | 0.293 | 5,896 |
| 15h | 0.268 | 5,889 |
| 18h | 0.261 | 5,886 |
| 21h | 0.273 | 5,887 |
| 1.0d | 0.270 | 5,886 |
| 1.2d | 0.198 | 5,889 |
| 1.5d | 0.150 | 5,882 |
| 2.0d | 0.186 | 5,865 |
| 2.5d | 0.122 | 5,826 |
| 3.0d | 0.171 | 5,790 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.026 | 0.241 | 19,304 |
| 0.1–0.2 | 0.142 | 0.309 | 5,530 |
| 0.2–0.3 | 0.239 | 0.332 | 2,772 |
| 0.3–0.4 | 0.344 | 0.396 | 5,553 |
| 0.4–0.5 | 0.489 | 0.490 | 2,779 |
| 0.6–0.7 | 0.660 | 0.619 | 5,556 |
| 0.7–0.8 | 0.788 | 0.718 | 2,778 |
| 0.8–0.9 | 0.888 | 0.809 | 2,778 |
| 0.9–1.0 | 1.000 | 1.000 | 2,779 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.1–0.2 | 0.175 | 0.250 | 22,068 |
| 0.2–0.3 | 0.238 | 0.337 | 8,313 |
| 0.3–0.4 | 0.325 | 0.424 | 2,778 |
| 0.4–0.5 | 0.405 | 0.490 | 2,779 |
| 0.5–0.6 | 0.528 | 0.590 | 2,778 |
| 0.6–0.7 | 0.612 | 0.648 | 2,778 |
| 0.7–0.8 | 0.715 | 0.718 | 2,778 |
| 0.8–0.9 | 0.843 | 0.809 | 2,778 |
| 0.9–1.0 | 1.000 | 1.000 | 2,779 |
