### AI transform noise: claude-opus-5-5 (effort default) — hard corpus

- 200 documents, 1200 field checks; 0 documents left out for refusals
- Noise as fid multiplies it: extract = 1 − accuracy; summarize and rewrite = 1 − accuracy(step → extract) ÷ accuracy(extract)

| Step | Noise | 95% range | Held-out: train → test |
|---|---|---|---|
| extract | 0.0% | 0.0–0.0% | 0.0% → 0.0% (inside) |
| summarize | 1.5% | 0.9–2.2% | 1.6% → 1.4% (inside) |
| rewrite | 0.2% | 0.0–0.4% | 0.2% → 0.2% (inside) |

| Field / condition | Extract | Summarize | Rewrite | Checks |
|---|---|---|---|---|
| ceo | 0.0% | 0.0% | 0.0% | 200 |
| founded | 0.0% | 5.0% | 0.0% | 200 |
| hq_city | 0.0% | 1.0% | 0.0% | 200 |
| employees | 0.0% | 0.0% | 0.0% | 200 |
| revenue_musd | 0.0% | 0.0% | 1.0% | 200 |
| fiscal_year_end | 0.0% | 3.0% | 0.0% | 200 |
| superseded=yes | 0.0% | 0.0% | 0.0% | 97 |
| superseded=no | 0.0% | 0.0% | 0.0% | 103 |
| old_figure=yes | 0.0% | 0.0% | 0.0% | 105 |
| old_figure=no | 0.0% | 0.0% | 0.0% | 95 |
| unit=yes | 0.0% | 0.0% | 0.8% | 133 |
| unit=no | 0.0% | 0.0% | 1.5% | 67 |
| competitor=yes | 0.0% | 0.7% | 0.0% | 300 |
| competitor=no | 0.0% | 1.8% | 0.2% | 900 |
| buried=yes | 0.0% | 2.8% | 0.2% | 564 |
| buried=no | 0.0% | 0.3% | 0.2% | 636 |
| correction=yes | 0.0% | 0.0% | 1.8% | 111 |
| correction=no | 0.0% | 0.0% | 0.0% | 89 |
| forecast=yes | 0.0% | 0.0% | 1.1% | 174 |
| forecast=no | 0.0% | 0.0% | 0.0% | 226 |
| subsidiary=yes | 0.0% | 0.0% | 0.0% | 309 |
| subsidiary=no | 0.0% | 0.7% | 0.0% | 291 |
| relocated=yes | 0.0% | 0.0% | 0.0% | 104 |
| relocated=no | 0.0% | 2.1% | 0.0% | 96 |
| renamed=yes | 0.0% | 3.9% | 0.0% | 103 |
| renamed=no | 0.0% | 6.2% | 0.0% | 97 |

```yaml
llm_extract:
  noise: 0.000    # 95% range 0.000 to 0.000
  conditions:
    superseded: 0.000
    old_figure: 0.000
    unit: 0.000
    competitor: 0.000
    buried: 0.000
    correction: 0.000
    forecast: 0.000
    subsidiary: 0.000
    relocated: 0.000
    renamed: 0.000
llm_summarize:
  noise: 0.015    # 95% range 0.009 to 0.022
  conditions:
    superseded: 0.000
    old_figure: 0.000
    unit: 0.000
    competitor: 0.007
    buried: 0.028
    correction: 0.000
    forecast: 0.000
    subsidiary: 0.000
    relocated: 0.000
    renamed: 0.039
llm_rewrite:
  noise: 0.002    # 95% range 0.000 to 0.004
  conditions:
    superseded: 0.000
    old_figure: 0.000
    unit: 0.008
    competitor: 0.000
    buried: 0.002
    correction: 0.018
    forecast: 0.011
    subsidiary: 0.000
    relocated: 0.000
    renamed: 0.000
```
