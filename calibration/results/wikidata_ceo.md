### Wikidata CEOs: the CEO of company X is Y

- Still correct means: Y is still CEO (no end date yet, and no successor started)
- 283 entities, 91,799 observations
- Empirical half-life: **5.8y**
- Fitted fid model (decay to 0): s0 = 1.000, half-life = **5.8y**, shape error 0.035
- Fitted with baseline (decay to b): b = 0.00, half-life = **5.8y**, shape error 0.035
- Hand-picked guess (employee_role: 1y): 1.0y

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life | 0.2008 |
| fitted half life with baseline | 0.2008 |
| constant base rate | 0.2507 |
| guessed half life | 0.3745 |

| Lag | Still correct | n |
|---|---|---|
| 0s | 1.000 | 2,239 |
| 91d | 0.983 | 2,239 |
| 183d | 0.956 | 2,239 |
| 274d | 0.933 | 2,239 |
| 1.0y | 0.912 | 2,239 |
| 1.2y | 0.894 | 2,239 |
| 1.5y | 0.866 | 2,239 |
| 1.8y | 0.839 | 2,239 |
| 2.0y | 0.816 | 2,239 |
| 2.2y | 0.800 | 2,239 |
| 2.5y | 0.774 | 2,239 |
| 2.8y | 0.749 | 2,239 |
| 3.0y | 0.729 | 2,239 |
| 3.2y | 0.711 | 2,239 |
| 3.5y | 0.689 | 2,239 |
| 3.8y | 0.663 | 2,239 |
| 4.0y | 0.640 | 2,239 |
| 4.2y | 0.624 | 2,239 |
| 4.5y | 0.603 | 2,239 |
| 4.8y | 0.580 | 2,239 |
| 5.0y | 0.557 | 2,239 |
| 5.2y | 0.542 | 2,239 |
| 5.5y | 0.523 | 2,239 |
| 5.8y | 0.502 | 2,239 |
| 6.0y | 0.486 | 2,239 |
| 6.2y | 0.473 | 2,239 |
| 6.5y | 0.459 | 2,239 |
| 6.8y | 0.442 | 2,239 |
| 7.0y | 0.428 | 2,239 |
| 7.2y | 0.412 | 2,239 |
| 7.5y | 0.397 | 2,239 |
| 7.8y | 0.382 | 2,239 |
| 8.0y | 0.369 | 2,239 |
| 8.2y | 0.356 | 2,239 |
| 8.5y | 0.343 | 2,239 |
| 8.8y | 0.331 | 2,239 |
| 9.0y | 0.320 | 2,239 |
| 9.2y | 0.310 | 2,239 |
| 9.5y | 0.298 | 2,239 |
| 9.8y | 0.287 | 2,239 |
| 10y | 0.277 | 2,239 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.3–0.4 | 0.367 | 0.290 | 6,748 |
| 0.4–0.5 | 0.450 | 0.366 | 7,712 |
| 0.5–0.6 | 0.551 | 0.469 | 6,748 |
| 0.6–0.7 | 0.648 | 0.593 | 4,820 |
| 0.7–0.8 | 0.742 | 0.712 | 4,820 |
| 0.8–0.9 | 0.850 | 0.838 | 4,820 |
| 0.9–1.0 | 0.961 | 0.961 | 3,856 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.3–0.4 | 0.367 | 0.290 | 6,748 |
| 0.4–0.5 | 0.450 | 0.366 | 7,712 |
| 0.5–0.6 | 0.551 | 0.469 | 6,748 |
| 0.6–0.7 | 0.648 | 0.593 | 4,820 |
| 0.7–0.8 | 0.742 | 0.712 | 4,820 |
| 0.8–0.9 | 0.850 | 0.838 | 4,820 |
| 0.9–1.0 | 0.961 | 0.961 | 3,856 |
