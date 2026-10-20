import { describe, expect, it } from "vitest";
import { comandoCodexLocal } from "../features/terminal/modeloLocal";

describe("modelo local pela CLI oficial", () => {
  it("usa o provedor local e preserva o identificador instalado", () => {
    expect(comandoCodexLocal("codex", "ollama", " empresa/modelo:local ")).toBe("codex --oss --local-provider ollama --model empresa/modelo:local");
    expect(comandoCodexLocal("codex", "lmstudio", "modelo-gguf")).toContain("--local-provider lmstudio");
    expect(comandoCodexLocal("codex", "conta", "")).toBe("codex");
    expect(comandoCodexLocal("codex resume abc", "ollama", "modelo-local")).toBe("codex resume abc --oss --local-provider ollama --model modelo-local");
  });
  it.each(["", "a; rm -rf x", "$(comando)", "`comando`", "a\nb", "--help", "a b"])("recusa argumento perigoso ou vazio: %s", (modelo) => {
    expect(() => comandoCodexLocal("codex", "ollama", modelo)).toThrow();
  });
});
