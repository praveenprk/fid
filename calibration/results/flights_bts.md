### Flight legs: aircraft's k-th leg of the day arrives on schedule

- Still correct means: arrived within 15 min, not cancelled or diverted; lag = legs flown before it
- 5,604 entities, 540,812 observations
- Empirical half-life: **—** (never fell to 50% in the observed range)
- Fitted fid model (decay to 0): s0 = 0.786, half-life = **19.9 legs**, shape error 0.028
- Fitted with baseline (decay to b): b = 0.62, half-life = **3.0 legs**, shape error 0.045

| Predictor (held-out half) | Brier score (lower is better) |
|---|---|
| fitted half life with baseline | 0.1925 |
| fitted half life | 0.1925 |
| constant base rate | 0.1939 |

| Lag | Still correct | n |
|---|---|---|
| 0 legs | 0.786 | 141,138 |
| 1 legs | 0.744 | 128,136 |
| 2 legs | 0.729 | 106,570 |
| 3 legs | 0.707 | 82,348 |
| 4 legs | 0.688 | 47,098 |
| 5 legs | 0.689 | 25,906 |
| 6 legs | 0.619 | 6,873 |
| 7 legs | 0.609 | 2,743 |

Reliability on the held-out half, decay to 0:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.6–0.7 | 0.672 | 0.678 | 40,344 |
| 0.7–0.8 | 0.752 | 0.747 | 225,656 |

Reliability on the held-out half, decay to baseline:

| Predicted bin | Mean predicted | Actual | n |
|---|---|---|---|
| 0.6–0.7 | 0.681 | 0.678 | 40,344 |
| 0.7–0.8 | 0.748 | 0.747 | 225,656 |

### Flight schedules: source fidelity

- 547,271 flights, January 2024
- The schedule is true (on time, ±15 min) for **72.9%** of flights

| Airline | On time | n |
|---|---|---|
| 9E | 0.733 | 16,972 |
| AA | 0.693 | 77,346 |
| AS | 0.591 | 17,775 |
| B6 | 0.695 | 19,580 |
| DL | 0.804 | 74,384 |
| F9 | 0.712 | 14,379 |
| G4 | 0.756 | 8,596 |
| HA | 0.720 | 6,576 |
| MQ | 0.688 | 20,750 |
| NK | 0.725 | 20,415 |
| OH | 0.703 | 16,526 |
| OO | 0.724 | 56,814 |
| UA | 0.719 | 58,855 |
| WN | 0.739 | 115,389 |
| YX | 0.790 | 22,914 |

| Scheduled departure | On time | n |
|---|---|---|
| 05:00 | 0.813 | 14,500 |
| 06:00 | 0.815 | 39,849 |
| 07:00 | 0.790 | 37,556 |
| 08:00 | 0.774 | 36,756 |
| 09:00 | 0.767 | 31,392 |
| 10:00 | 0.744 | 35,162 |
| 11:00 | 0.742 | 34,041 |
| 12:00 | 0.728 | 33,282 |
| 13:00 | 0.719 | 34,095 |
| 14:00 | 0.704 | 32,509 |
| 15:00 | 0.696 | 32,065 |
| 16:00 | 0.686 | 32,271 |
| 17:00 | 0.679 | 34,437 |
| 18:00 | 0.666 | 34,692 |
| 19:00 | 0.667 | 28,740 |
| 20:00 | 0.672 | 22,783 |
| 21:00 | 0.703 | 17,469 |
| 22:00 | 0.743 | 10,264 |
| 23:00 | 0.763 | 3,974 |

### Flight legs: independence of transform noise

- Fitted per-leg noise (fid model): **0.034**
- On time after an on-time previous leg: **0.846** (n = 301,737)
- On time after a late previous leg: **0.328** (n = 97,937)
- If legs were independent, as multiplying per-step noise assumes, these two would match.
