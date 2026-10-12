### AIS 1 km, all vessels: vessel V is at position P (±1 km)

- Still correct means: next report within 1 km
- 6,146 entities, 2,945,357 observations
- Empirical half-life: **—** (never fell to 50% in the observed range)
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **7.4h**, shape error 0.314 — poor fit
- Fitted with baseline (decay to b): b = 0.62, half-life = **2.7m**, shape error 0.067
- Hand-picked guess (shipping_status: 4h): 4.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.2168 |
| constant base rate | 0.2256 |
| fitted half life | 0.2681 |
| guessed half life | 0.2803 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 193,938 |
| 5.0m | 0.700 | 190,254 |
| 10m | 0.671 | 188,926 |
| 15m | 0.664 | 188,511 |
| 20m | 0.659 | 187,621 |
| 30m | 0.652 | 187,302 |
| 45m | 0.645 | 186,040 |
| 1.0h | 0.640 | 186,199 |
| 1.5h | 0.635 | 185,090 |
| 2.0h | 0.630 | 184,346 |
| 3.0h | 0.622 | 182,783 |
| 4.0h | 0.614 | 181,337 |
| 6.0h | 0.600 | 179,188 |
| 8.0h | 0.585 | 176,666 |
| 10h | 0.570 | 174,459 |
| 12h | 0.553 | 172,697 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.3–0.4 | 0.357 | 0.562 | 177,720 |
| 0.4–0.5 | 0.471 | 0.586 | 90,356 |
| 0.5–0.6 | 0.569 | 0.601 | 91,667 |
| 0.6–0.7 | 0.686 | 0.616 | 92,783 |
| 0.7–0.8 | 0.754 | 0.623 | 93,540 |
| 0.8–0.9 | 0.848 | 0.634 | 189,069 |
| 0.9–1.0 | 0.965 | 0.707 | 772,366 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.6–0.7 | 0.623 | 0.627 | 1,310,782 |
| 0.7–0.8 | 0.718 | 0.702 | 97,402 |
| 0.9–1.0 | 1.000 | 1.000 | 99,317 |

### AIS 1 km, underway: vessel V is at position P (±1 km)

- Still correct means: next report within 1 km
- 3,885 entities, 1,001,246 observations
- Empirical half-life: **3.0m**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **2.0m**, shape error 0.046
- Fitted with baseline (decay to b): b = 0.02, half-life = **1.9m**, shape error 0.029
- Hand-picked guess (shipping_status: 4h): 4.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.0299 |
| fitted half life | 0.0303 |
| constant base rate | 0.0919 |
| guessed half life | 0.4956 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 69,876 |
| 5.0m | 0.163 | 68,167 |
| 10m | 0.072 | 66,885 |
| 15m | 0.052 | 66,442 |
| 20m | 0.039 | 65,934 |
| 30m | 0.022 | 65,552 |
| 45m | 0.017 | 64,726 |
| 1.0h | 0.016 | 64,569 |
| 1.5h | 0.016 | 63,611 |
| 2.0h | 0.015 | 62,898 |
| 3.0h | 0.015 | 61,512 |
| 4.0h | 0.015 | 60,208 |
| 6.0h | 0.015 | 58,048 |
| 8.0h | 0.015 | 55,858 |
| 10h | 0.015 | 54,106 |
| 12h | 0.015 | 52,854 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.003 | 0.025 | 438,436 |
| 0.1–0.2 | 0.181 | 0.164 | 34,676 |
| 0.9–1.0 | 1.000 | 1.000 | 35,588 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.022 | 0.025 | 438,436 |
| 0.1–0.2 | 0.173 | 0.164 | 34,676 |
| 0.9–1.0 | 1.000 | 1.000 | 35,588 |

### AIS 1 km, moored or at anchor: vessel V is at position P (±1 km)

- Still correct means: next report within 1 km
- 4,541 entities, 1,944,111 observations
- Empirical half-life: **—** (never fell to 50% in the observed range)
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **1.4d**, shape error 0.012
- Fitted with baseline (decay to b): b = 0.64, half-life = **9.8h**, shape error 0.005
- Hand-picked guess (shipping_status: 4h): 4.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.0532 |
| fitted half life | 0.0532 |
| constant base rate | 0.0578 |
| guessed half life | 0.1707 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 124,062 |
| 5.0m | 0.999 | 122,087 |
| 10m | 0.999 | 122,041 |
| 15m | 0.998 | 122,069 |
| 20m | 0.995 | 121,687 |
| 30m | 0.991 | 121,750 |
| 45m | 0.981 | 121,314 |
| 1.0h | 0.972 | 121,630 |
| 1.5h | 0.959 | 121,479 |
| 2.0h | 0.948 | 121,448 |
| 3.0h | 0.929 | 121,271 |
| 4.0h | 0.912 | 121,129 |
| 6.0h | 0.880 | 121,140 |
| 8.0h | 0.848 | 120,808 |
| 10h | 0.819 | 120,353 |
| 12h | 0.791 | 119,843 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.7–0.8 | 0.784 | 0.789 | 61,636 |
| 0.8–0.9 | 0.851 | 0.847 | 186,207 |
| 0.9–1.0 | 0.978 | 0.973 | 750,958 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.7–0.8 | 0.797 | 0.789 | 61,636 |
| 0.8–0.9 | 0.848 | 0.847 | 186,207 |
| 0.9–1.0 | 0.974 | 0.973 | 750,958 |

### AIS 10 km, all vessels: vessel V is at position P (±10 km)

- Still correct means: next report within 10 km
- 6,146 entities, 2,945,357 observations
- Empirical half-life: **—** (never fell to 50% in the observed range)
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **9.5h**, shape error 0.243 — poor fit
- Fitted with baseline (decay to b): b = 0.63, half-life = **35m**, shape error 0.093
- Hand-picked guess (shipping_status: 4h): 4.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1529 |
| fitted half life | 0.1695 |
| constant base rate | 0.1750 |
| guessed half life | 0.1954 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 193,938 |
| 5.0m | 1.000 | 190,254 |
| 10m | 0.999 | 188,926 |
| 15m | 0.998 | 188,511 |
| 20m | 0.962 | 187,621 |
| 30m | 0.784 | 187,302 |
| 45m | 0.704 | 186,040 |
| 1.0h | 0.692 | 186,199 |
| 1.5h | 0.683 | 185,090 |
| 2.0h | 0.677 | 184,346 |
| 3.0h | 0.667 | 182,783 |
| 4.0h | 0.657 | 181,337 |
| 6.0h | 0.640 | 179,188 |
| 8.0h | 0.625 | 176,666 |
| 10h | 0.611 | 174,459 |
| 12h | 0.595 | 172,697 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.4–0.5 | 0.446 | 0.604 | 177,720 |
| 0.5–0.6 | 0.555 | 0.627 | 90,356 |
| 0.6–0.7 | 0.643 | 0.642 | 91,667 |
| 0.7–0.8 | 0.745 | 0.660 | 92,783 |
| 0.8–0.9 | 0.854 | 0.679 | 282,609 |
| 0.9–1.0 | 0.973 | 0.894 | 772,366 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.6–0.7 | 0.635 | 0.647 | 735,135 |
| 0.7–0.8 | 0.759 | 0.701 | 190,539 |
| 0.8–0.9 | 0.855 | 0.873 | 191,872 |
| 0.9–1.0 | 0.951 | 0.999 | 389,955 |

### AIS 10 km, underway: vessel V is at position P (±10 km)

- Still correct means: next report within 10 km
- 3,885 entities, 1,001,246 observations
- Empirical half-life: **28m**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **32m**, shape error 0.273 — poor fit
- Fitted with baseline (decay to b): b = 0.04, half-life = **30m**, shape error 0.278 — poor fit
- Hand-picked guess (shipping_status: 4h): 4.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.0897 |
| fitted half life | 0.0903 |
| constant base rate | 0.2416 |
| guessed half life | 0.2437 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 69,876 |
| 5.0m | 1.000 | 68,167 |
| 10m | 0.999 | 66,885 |
| 15m | 0.993 | 66,442 |
| 20m | 0.892 | 65,934 |
| 30m | 0.382 | 65,552 |
| 45m | 0.152 | 64,726 |
| 1.0h | 0.119 | 64,569 |
| 1.5h | 0.101 | 63,611 |
| 2.0h | 0.093 | 62,898 |
| 3.0h | 0.081 | 61,512 |
| 4.0h | 0.072 | 60,208 |
| 6.0h | 0.063 | 58,048 |
| 8.0h | 0.061 | 55,858 |
| 10h | 0.061 | 54,106 |
| 12h | 0.059 | 52,854 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.015 | 0.073 | 205,654 |
| 0.1–0.2 | 0.140 | 0.104 | 32,319 |
| 0.2–0.3 | 0.269 | 0.122 | 32,844 |
| 0.3–0.4 | 0.374 | 0.156 | 32,915 |
| 0.5–0.6 | 0.519 | 0.380 | 33,338 |
| 0.6–0.7 | 0.646 | 0.889 | 33,529 |
| 0.7–0.8 | 0.720 | 0.995 | 33,804 |
| 0.8–0.9 | 0.850 | 0.999 | 68,709 |
| 0.9–1.0 | 1.000 | 1.000 | 35,588 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.0–0.1 | 0.042 | 0.073 | 205,654 |
| 0.1–0.2 | 0.152 | 0.104 | 32,319 |
| 0.2–0.3 | 0.273 | 0.122 | 32,844 |
| 0.3–0.4 | 0.374 | 0.156 | 32,915 |
| 0.5–0.6 | 0.516 | 0.380 | 33,338 |
| 0.6–0.7 | 0.642 | 0.889 | 33,529 |
| 0.7–0.8 | 0.716 | 0.995 | 33,804 |
| 0.8–0.9 | 0.848 | 0.999 | 68,709 |
| 0.9–1.0 | 1.000 | 1.000 | 35,588 |

### AIS 10 km, moored or at anchor: vessel V is at position P (±10 km)

- Still correct means: next report within 10 km
- 4,541 entities, 1,944,111 observations
- Empirical half-life: **—** (never fell to 50% in the observed range)
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **1.9d**, shape error 0.011
- Fitted with baseline (decay to b): b = 0.00, half-life = **1.9d**, shape error 0.011
- Hand-picked guess (shipping_status: 4h): 4.0h

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life | 0.0353 |
| fitted half life with baseline | 0.0353 |
| constant base rate | 0.0384 |
| guessed half life | 0.1708 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 124,062 |
| 5.0m | 1.000 | 122,087 |
| 10m | 0.999 | 122,041 |
| 15m | 1.000 | 122,069 |
| 20m | 1.000 | 121,687 |
| 30m | 1.000 | 121,750 |
| 45m | 0.999 | 121,314 |
| 1.0h | 0.996 | 121,630 |
| 1.5h | 0.988 | 121,479 |
| 2.0h | 0.980 | 121,448 |
| 3.0h | 0.964 | 121,271 |
| 4.0h | 0.947 | 121,129 |
| 6.0h | 0.916 | 121,140 |
| 8.0h | 0.885 | 120,808 |
| 10h | 0.858 | 120,353 |
| 12h | 0.831 | 119,843 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.8–0.9 | 0.864 | 0.856 | 185,583 |
| 0.9–1.0 | 0.979 | 0.984 | 813,218 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.8–0.9 | 0.864 | 0.856 | 185,583 |
| 0.9–1.0 | 0.979 | 0.984 | 813,218 |
