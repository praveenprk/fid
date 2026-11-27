<!-- Release record (eng-principles ENG-STD-005 §2.6; AGENTS.md, Releases).
     Draft it in the description of the pull request that promotes main to
     stable. Once PK approves, it becomes the signed tag's message and goes on
     the GitHub release page. -->
fidmeter X.Y.Z

Commit    : <full SHA on main being promoted>
Checks    : <each check the ruleset main-branch-integrity requires, and its
            result at that commit. On 2026-11-25 they are: lint, rust,
            python, msrv, python-min, supply-chain, spec, spec-ref,
            commit-check / check, commit-check / questions,
            GitGuardian Security Checks>
Violations: <the OPEN entries in docs/VIOLATIONS.md at that commit, by ID
            and severity, or "none">
Authorised: PK (praveenprk), <YYYY-MM-DD>
Closes    : <DECISION-NNN, V-ENG-NNN-NN, #N anchors this release closes>
