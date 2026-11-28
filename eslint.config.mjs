// JavaScript linter for the inline <script> blocks of the results site
// (eng-principles ENG-STD-004 §6.1, ENG-DEC-015; V-ENG-004-02).
// Versions are pinned in .github/workflows/ci.yml, so the recommended rule
// set changes only when the pin moves.
import js from "@eslint/js";
import html from "eslint-plugin-html";
import globals from "globals";

export default [
  {
    files: ["site/**/*.html"],
    plugins: { html },
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "script",
      globals: {
        ...globals.browser,
        // d3 from its CDN script; dash, set on window by shell.html for page.html
        d3: "readonly",
        dash: "readonly",
        satellite: "readonly",
      },
    },
    rules: { ...js.configs.recommended.rules },
  },
  {
    // The site's formula test (site/formula.test.mjs), run by node in CI.
    files: ["site/**/*.mjs"],
    languageOptions: { ecmaVersion: 2022, sourceType: "module", globals: globals.node },
    rules: { ...js.configs.recommended.rules },
  },
];
