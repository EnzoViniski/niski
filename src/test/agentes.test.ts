import { describe, expect, it } from "vitest";
import { nomeDeArquivo } from "../features/agentes/agentesService";

describe("nomeDeArquivo", () => {
  it("vira nome de arquivo seguro: sem acento, minúsculas, hífen e até 40 caracteres", () => {
    expect(nomeDeArquivo("Revisor de Acessibilidade")).toBe("revisor-de-acessibilidade");
    expect(nomeDeArquivo("  QA / Front-end!! ")).toBe("qa-front-end");
    expect(nomeDeArquivo("../../etc")).toBe("etc");
    expect(nomeDeArquivo("a".repeat(60)).length).toBe(40);
  });
});
