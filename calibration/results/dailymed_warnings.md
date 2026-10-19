### DailyMed warnings — all snapshots: the warnings of drug label X say Z

- Still correct means: the label's warnings text (boxed warning, warnings, warnings and precautions) is unchanged
- 173 entities, 113,410 observations
- Empirical half-life: **3.2y**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **3.2y**, shape error 0.043
- Fitted with baseline (decay to b): b = 0.23, half-life = **2.1y**, shape error 0.003

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1511 |
| fitted half life | 0.1515 |
| constant base rate | 0.1951 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 10,310 |
| 30d | 0.980 | 10,310 |
| 91d | 0.940 | 10,310 |
| 182d | 0.884 | 10,310 |
| 274d | 0.831 | 10,310 |
| 365d | 0.781 | 10,310 |
| 1.5y | 0.697 | 10,310 |
| 2.0y | 0.629 | 10,310 |
| 3.0y | 0.517 | 10,310 |
| 4.0y | 0.432 | 10,310 |
| 5.0y | 0.380 | 10,310 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.3–0.4 | 0.336 | 0.384 | 4,392 |
| 0.4–0.5 | 0.418 | 0.431 | 4,392 |
| 0.5–0.6 | 0.520 | 0.515 | 4,392 |
| 0.6–0.7 | 0.647 | 0.630 | 4,392 |
| 0.7–0.8 | 0.721 | 0.699 | 4,392 |
| 0.8–0.9 | 0.850 | 0.833 | 13,176 |
| 0.9–1.0 | 0.976 | 0.974 | 13,176 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.3–0.4 | 0.377 | 0.384 | 4,392 |
| 0.4–0.5 | 0.434 | 0.431 | 4,392 |
| 0.5–0.6 | 0.515 | 0.515 | 4,392 |
| 0.6–0.7 | 0.663 | 0.664 | 8,784 |
| 0.7–0.8 | 0.783 | 0.782 | 4,392 |
| 0.8–0.9 | 0.857 | 0.858 | 8,784 |
| 0.9–1.0 | 0.973 | 0.974 | 13,176 |

### DailyMed warnings — warnings changed in the year before: the warnings of drug label X say Z

- Still correct means: the label's warnings text (boxed warning, warnings, warnings and precautions) is unchanged
- 173 entities, 40,447 observations
- Empirical half-life: **2.9y**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **2.8y**, shape error 0.025
- Fitted with baseline (decay to b): b = 0.15, half-life = **2.2y**, shape error 0.013

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life | 0.1527 |
| fitted half life with baseline | 0.1527 |
| constant base rate | 0.2076 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 3,677 |
| 30d | 0.979 | 3,677 |
| 91d | 0.935 | 3,677 |
| 182d | 0.877 | 3,677 |
| 274d | 0.819 | 3,677 |
| 365d | 0.760 | 3,677 |
| 1.5y | 0.666 | 3,677 |
| 2.0y | 0.602 | 3,677 |
| 3.0y | 0.488 | 3,677 |
| 4.0y | 0.385 | 3,677 |
| 5.0y | 0.316 | 3,677 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.3–0.4 | 0.342 | 0.329 | 3,188 |
| 0.4–0.5 | 0.487 | 0.482 | 1,594 |
| 0.6–0.7 | 0.658 | 0.633 | 3,188 |
| 0.7–0.8 | 0.787 | 0.758 | 1,594 |
| 0.8–0.9 | 0.861 | 0.845 | 3,188 |
| 0.9–1.0 | 0.974 | 0.971 | 4,782 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.3–0.4 | 0.369 | 0.329 | 3,188 |
| 0.4–0.5 | 0.483 | 0.482 | 1,594 |
| 0.6–0.7 | 0.639 | 0.633 | 3,188 |
| 0.7–0.8 | 0.767 | 0.758 | 1,594 |
| 0.8–0.9 | 0.846 | 0.845 | 3,188 |
| 0.9–1.0 | 0.971 | 0.971 | 4,782 |

### DailyMed warnings — warnings unchanged for 1 to 5 years: the warnings of drug label X say Z

- Still correct means: the label's warnings text (boxed warning, warnings, warnings and precautions) is unchanged
- 145 entities, 60,456 observations
- Empirical half-life: **2.9y**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **3.0y**, shape error 0.048
- Fitted with baseline (decay to b): b = 0.21, half-life = **2.0y**, shape error 0.008

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1542 |
| fitted half life | 0.1547 |
| constant base rate | 0.2020 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 5,496 |
| 30d | 0.979 | 5,496 |
| 91d | 0.936 | 5,496 |
| 182d | 0.877 | 5,496 |
| 274d | 0.822 | 5,496 |
| 365d | 0.775 | 5,496 |
| 1.5y | 0.688 | 5,496 |
| 2.0y | 0.610 | 5,496 |
| 3.0y | 0.486 | 5,496 |
| 4.0y | 0.404 | 5,496 |
| 5.0y | 0.360 | 5,496 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.3–0.4 | 0.356 | 0.389 | 4,650 |
| 0.5–0.6 | 0.500 | 0.471 | 2,325 |
| 0.6–0.7 | 0.630 | 0.600 | 2,325 |
| 0.7–0.8 | 0.750 | 0.726 | 4,650 |
| 0.8–0.9 | 0.866 | 0.848 | 4,650 |
| 0.9–1.0 | 0.975 | 0.972 | 6,975 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.3–0.4 | 0.348 | 0.377 | 2,325 |
| 0.4–0.5 | 0.454 | 0.435 | 4,650 |
| 0.6–0.7 | 0.652 | 0.641 | 4,650 |
| 0.7–0.8 | 0.777 | 0.770 | 2,325 |
| 0.8–0.9 | 0.853 | 0.848 | 4,650 |
| 0.9–1.0 | 0.972 | 0.972 | 6,975 |

### DailyMed warnings — warnings unchanged for over 5 years: the warnings of drug label X say Z

- Still correct means: the label's warnings text (boxed warning, warnings, warnings and precautions) is unchanged
- 36 entities, 12,507 observations
- Empirical half-life: **—** (never fell to 50% in the observed range)
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **7.8y**, shape error 0.036
- Fitted with baseline (decay to b): b = 0.61, half-life = **2.1y**, shape error 0.008

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life | 0.0863 |
| fitted half life with baseline | 0.0868 |
| constant base rate | 0.0916 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 1,137 |
| 30d | 0.990 | 1,137 |
| 91d | 0.970 | 1,137 |
| 182d | 0.943 | 1,137 |
| 274d | 0.914 | 1,137 |
| 365d | 0.882 | 1,137 |
| 1.5y | 0.842 | 1,137 |
| 2.0y | 0.807 | 1,137 |
| 3.0y | 0.761 | 1,137 |
| 4.0y | 0.718 | 1,137 |
| 5.0y | 0.678 | 1,137 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.5–0.6 | 0.578 | 0.736 | 473 |
| 0.6–0.7 | 0.645 | 0.786 | 473 |
| 0.7–0.8 | 0.720 | 0.837 | 473 |
| 0.8–0.9 | 0.849 | 0.901 | 1,419 |
| 0.9–1.0 | 0.966 | 0.976 | 2,365 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.6–0.7 | 0.651 | 0.761 | 946 |
| 0.7–0.8 | 0.734 | 0.856 | 946 |
| 0.8–0.9 | 0.850 | 0.923 | 1,419 |
| 0.9–1.0 | 0.966 | 0.984 | 1,892 |

Sample: 400 prescription labels; 375 with warnings text, 2,277 versions read, 410 changes to the warnings text, 1 could not be read.
