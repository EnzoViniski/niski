import { describe, expect, it } from "vitest";
import { pareceCaminho } from "../mobile/CaminhoClicavel";

// Fork: caminho citado na Conversa vira link (`mobile/CaminhoClicavel.tsx`).
describe("pareceCaminho", () => {
  it.each(["~/omni/PROMPT-ORQUESTRADOR.md", "/Users/x/a.pdf", "./scripts/build.mjs", "src/App.tsx:12", "OMNI-AGENTS/docs/fork/CLAUDE.md", "roadmaps/fork/"])("%s é caminho", (t) => {
    expect(pareceCaminho(t)).toBe(true);
  });
  it.each(["npm run dev", "https://claude.ai/x", "useState", "a / b", "/", "PLACE_SESSION", "--resume"])("%s não é", (t) => {
    expect(pareceCaminho(t)).toBe(false);
  });
});
