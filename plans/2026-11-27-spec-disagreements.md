# Spec/code disagreements found while closing #16

Date: 2026-11-27. Both were found by the nightshift runner (opencode, Kimi
K3.1 Pro) while closing #16. In each case it stopped and reported instead
of writing vectors around the behaviour. engineering-fa verified both
against `main`, as did the Claude session that fixed them. Both are in
0.2.0 and 0.2.1. The 0.2.1 release record lists A as a known issue.

| | A. Required members accepted when missing | B. Echo time-to-floor search bound |
|---|---|---|
| Draft | Fields: `half_life`, `fid_at_stamp`, `derived_from` are required (Req. yes). Each may be null, but not absent. | Time to floor: "With an echo, it is never if no crossing occurs within 10,000 periods." Security Considerations: searches are bounded. |
| Code | `src/envelope.rs` read them as `Option`, so a missing member read as `None`. | `src/compute.rs` (~390, ~747) searched to `start + 10,000·period`, where `start` is the crossing without the echo. |
| Found | While writing #16 envelope-fields vectors. | While writing #16 security vectors. |
| Probe | Remove each member from a stamped artifact and run `fid check`. Missing `half_life` gave UNKNOWN (exit 2); missing `fid_at_stamp` or `derived_from` scored normally. | f = 1, b = 0, F = 0.5; echo period 0.1 s, width 1e-6 s, strength 0.1, no fade. h = 999.55 s gave After(999.55), which is correct. h = 1000.55 s gave After(1000.55); the draft requires Never. |
| Verified | Yes: engineering-fa, and the fixing session, which reproduced it in tests. | Yes: engineering-fa, and the fixing session; the regression test fails on the old bound. |
| Decision | Change the code. Missing and null are different statements, and the draft fails closed. Every envelope fid has written carries all three members, so nothing fid wrote is affected. | Change the code. The limit bounds scoring work, and the reference implementation (`conformance/generate.py`) already counts from t = 0. |
| Fix | #67 (`security`): a presence check makes a missing member malformed (exit 3, ValueError). Null keeps its meaning. | #68 (`security`): both searches end at 10,000·period from measurement. |
| Effect on 0.2.x | 0.2.0 and 0.2.1 accept such envelopes. Fixed from the next release (CHANGELOG, Fixed). | 0.2.0 and 0.2.1 can return a crossing past the limit. Only crossings after 10,000 echo periods are affected (more than 27 years for a 24 h echo). Fixed from the next release. |
| Then | nightshift re-runs #16 envelope-fields on top of #67. | nightshift re-runs #16 security on top of #68. |
