### AI transform noise: claude-opus-5-5 (effort default)

- 200 documents, 1200 field checks; 0 documents left out for refusals
- Noise as fid multiplies it: extract = 1 − accuracy; summarize and rewrite = 1 − accuracy(step → extract) ÷ accuracy(extract)

| Step | Noise | 95% range | Held-out: train → test |
|---|---|---|---|
| extract | 0.0% | 0.0–0.0% | 0.0% → 0.0% (inside) |
| summarize | 0.0% | 0.0–0.0% | 0.0% → 0.0% (inside) |
| rewrite | 0.0% | 0.0–0.0% | 0.0% → 0.0% (inside) |

| Field / condition | Extract | Summarize | Rewrite | Checks |
|---|---|---|---|---|
| ceo | 0.0% | 0.0% | 0.0% | 200 |
| founded | 0.0% | 0.0% | 0.0% | 200 |
| hq_city | 0.0% | 0.0% | 0.0% | 200 |
| employees | 0.0% | 0.0% | 0.0% | 200 |
| revenue_musd | 0.0% | 0.0% | 0.0% | 200 |
| fiscal_year_end | 0.0% | 0.0% | 0.0% | 200 |
| superseded=yes | 0.0% | 0.0% | 0.0% | 97 |
| superseded=no | 0.0% | 0.0% | 0.0% | 103 |
| old_figure=yes | 0.0% | 0.0% | 0.0% | 105 |
| old_figure=no | 0.0% | 0.0% | 0.0% | 95 |
| unit=yes | 0.0% | 0.0% | 0.0% | 133 |
| unit=no | 0.0% | 0.0% | 0.0% | 67 |
| competitor=yes | 0.0% | 0.0% | 0.0% | 300 |
| competitor=no | 0.0% | 0.0% | 0.0% | 900 |

```yaml
llm_extract:
  noise: 0.000    # 95% range 0.000 to 0.000
  conditions:
    superseded: 0.000
    old_figure: 0.000
    unit: 0.000
    competitor: 0.000
llm_summarize:
  noise: 0.000    # 95% range 0.000 to 0.000
  conditions:
    superseded: 0.000
    old_figure: 0.000
    unit: 0.000
    competitor: 0.000
llm_rewrite:
  noise: 0.000    # 95% range 0.000 to 0.000
  conditions:
    superseded: 0.000
    old_figure: 0.000
    unit: 0.000
    competitor: 0.000
```
