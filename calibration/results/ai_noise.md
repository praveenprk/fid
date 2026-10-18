### AI transform noise: claude-opus-5-5 (effort default)

- 50 documents, 300 field checks; 0 documents left out for refusals
- Noise as fid multiplies it: extract = 1 − accuracy; summarize and rewrite = 1 − accuracy(step → extract) ÷ accuracy(extract)

| Step | Noise | 95% range | Held-out: train → test |
|---|---|---|---|
| extract | 0.0% | 0.0–0.0% | 0.0% → 0.0% (inside) |
| summarize | 0.0% | 0.0–0.0% | 0.0% → 0.0% (inside) |
| rewrite | 0.0% | 0.0–0.0% | 0.0% → 0.0% (inside) |

| Field / condition | Extract | Summarize | Rewrite | Checks |
|---|---|---|---|---|
| ceo | 0.0% | 0.0% | 0.0% | 50 |
| founded | 0.0% | 0.0% | 0.0% | 50 |
| hq_city | 0.0% | 0.0% | 0.0% | 50 |
| employees | 0.0% | 0.0% | 0.0% | 50 |
| revenue_musd | 0.0% | 0.0% | 0.0% | 50 |
| fiscal_year_end | 0.0% | 0.0% | 0.0% | 50 |
| superseded=yes | 0.0% | 0.0% | 0.0% | 29 |
| superseded=no | 0.0% | 0.0% | 0.0% | 21 |
| old_figure=yes | 0.0% | 0.0% | 0.0% | 29 |
| old_figure=no | 0.0% | 0.0% | 0.0% | 21 |
| unit=yes | 0.0% | 0.0% | 0.0% | 35 |
| unit=no | 0.0% | 0.0% | 0.0% | 15 |
| competitor=yes | 0.0% | 0.0% | 0.0% | 72 |
| competitor=no | 0.0% | 0.0% | 0.0% | 228 |

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
