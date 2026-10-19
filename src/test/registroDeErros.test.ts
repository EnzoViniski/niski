import { describe, expect, it } from "vitest";
import { detalheDoErro } from "../lib/registroDeErros";

describe("registro de erros da interface", () => {
  it("leva tipo, mensagem e origem, cortado", () => {
    expect(detalheDoErro(new TypeError("x is undefined"), "App.tsx:10")).toBe("TypeError: x is undefined @ App.tsx:10");
    expect(detalheDoErro(42)).toBe("erro sem mensagem");
    expect(detalheDoErro("a".repeat(1000)).length).toBe(300);
  });
});
