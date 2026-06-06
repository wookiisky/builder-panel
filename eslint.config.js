import js from "@eslint/js";
import tseslint from "typescript-eslint";

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    ignores: [
      "dist/**",
      "node_modules/**",
      "open-vibe-island/**",
      "src-tauri/target/**",
      "src-tauri/gen/**",
    ],
  },
  {
    files: ["src/**/*.{ts,tsx}", "vite.config.ts", "scripts/**/*.mjs"],
    languageOptions: {
      globals: {
        console: "readonly",
        process: "readonly",
      },
      parserOptions: {
        projectService: {
          allowDefaultProject: ["scripts/*.mjs"],
        },
      },
    },
    rules: {
      "@typescript-eslint/consistent-type-imports": "error",
      "@typescript-eslint/no-import-type-side-effects": "error",
      "@typescript-eslint/no-explicit-any": "error",
    },
  },
);
