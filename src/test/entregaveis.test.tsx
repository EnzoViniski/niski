import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MessageBubble } from "../mobile/Chat";
import { modeloDaLista } from "../mobile/AjustesDaConversa";

// Arquivos gerados pelo agente na conversa e modelos do Claude com versão.
describe("arquivos gerados pelo agente", () => {
  it("viram cartões com nome e pasta na resposta", () => {
    render(<MessageBubble lado="agent" provider="codex" texto="Pronto, gerei os dois."
      arquivos={{ conversa: "c1", n: 4, caminhos: ["/Users/eu/projeto/saida/relatorio.pdf", "/Users/eu/projeto/planilha.xlsx"] }} />);
    const lista = screen.getByRole("list", { name: "Arquivos gerados pelo agente" });
    expect(lista.querySelectorAll("li")).toHaveLength(2);
    expect(screen.getByText("relatorio.pdf")).toBeTruthy();
    expect(screen.getByText("…/projeto/saida")).toBeTruthy();
    expect(screen.getByTitle("Baixar planilha.xlsx")).toBeTruthy();
  });

  it("imagem gerada sem texto ainda mostra o balão com a imagem", () => {
    render(<MessageBubble lado="agent" provider="codex" texto="" imagens={{ conversa: "c1", n: 2, total: 1 }} />);
    expect(screen.getByRole("list", { name: "Imagens da mensagem" })).toBeTruthy();
  });
});

describe("modelo do transcript no seletor", () => {
  const models = ["opus", "claude-opus-4-8", "claude-opus-4-5", "sonnet"].map((id) => ({ id, label: id, efforts: [] }));
  it("prefere a versão exata ou a mais específica ao apelido", () => {
    expect(modeloDaLista({ model: "claude-opus-4-8", models })).toBe("claude-opus-4-8");
    expect(modeloDaLista({ model: "claude-opus-4-5-20251101", models })).toBe("claude-opus-4-5");
    expect(modeloDaLista({ model: "claude-opus-5-5", models })).toBe("opus");
    expect(modeloDaLista({ model: null, models })).toBe("");
  });
});

describe("imagem no markdown da resposta", () => {
  it("caminho local não vira imagem quebrada; imagem da web continua", () => {
    const { container } = render(<MessageBubble lado="agent" provider="codex"
      texto={"Aqui:\n\n![logo](/Users/eu/.codex/generated_images/t/x.png)\n\n![web](https://exemplo.com/a.png)"} />);
    const imgs = container.querySelectorAll(".bubble-markdown img");
    expect(imgs).toHaveLength(1);
    expect(imgs[0].getAttribute("src")).toBe("https://exemplo.com/a.png");
  });
});
