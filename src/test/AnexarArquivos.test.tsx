import { beforeEach, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { ConversationDetail } from "../mobile/MobileApp";
import { EVENTO_ANEXAR, separarAnexos, tipoPelaExtensao } from "../mobile/anexos";
import type { Conversation } from "../mobile/api";

const CONVERSA: Conversation = {
  id: "c", title: "Conversa", project_id: "p", provider: "claude", profile_id: "p", state: "answered", mode: "auto", live: true,
  capabilities: { prompt: true, approve: false, revision: '"r"', approval_text: null, reason: null },
};
const TIMELINE = { timeline: { messages: [], next_cursor: null, prev_cursor: null, unavailable_segments: [] }, actions: [], conversation: CONVERSA };

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify(TIMELINE))));
  window.HTMLElement.prototype.scrollTo = vi.fn() as never;
  // jsdom não tem as URLs de prévia de imagem.
  URL.createObjectURL = vi.fn(() => "blob:previa");
  URL.revokeObjectURL = vi.fn();
});

const arquivo = (nome: string, tipo: string) => new File(["conteudo"], nome, { type: tipo });

function abrir() {
  render(<ConversationDetail conversation={CONVERSA} refresh={async () => undefined} />);
  return document.querySelector(".chat-tela") as HTMLElement;
}

it("arrastar PDF, planilha e imagem para a conversa vira anexo", async () => {
  const tela = abrir();
  const arquivos = [arquivo("contrato.pdf", "application/pdf"), arquivo("vendas.xlsx", ""), arquivo("print.png", "image/png")];
  await act(async () => {
    fireEvent.dragEnter(tela, { dataTransfer: { types: ["Files"], files: arquivos } });
  });
  expect(screen.getByText(/Solte para anexar/)).toBeInTheDocument();
  await act(async () => {
    fireEvent.drop(tela, { dataTransfer: { types: ["Files"], files: arquivos } });
  });
  for (const nome of ["contrato.pdf", "vendas.xlsx", "print.png"]) expect(screen.getByText(nome)).toBeInTheDocument();
  expect(screen.queryByText(/Solte para anexar/)).not.toBeInTheDocument();
});

it("o app do PC entrega arquivos arrastados pelo evento da conversa", async () => {
  const tela = abrir();
  await act(async () => {
    tela.dispatchEvent(new CustomEvent(EVENTO_ANEXAR, { detail: [arquivo("notas.md", "text/markdown")] }));
  });
  expect(screen.getByText("notas.md")).toBeInTheDocument();
});

it("colar um arquivo com o foco fora do campo também anexa (uma vez só)", async () => {
  const tela = abrir();
  await act(async () => {
    fireEvent.paste(tela, { clipboardData: { files: [arquivo("dados.csv", "text/csv")] } });
  });
  expect(screen.getAllByText("dados.csv")).toHaveLength(1);
});

it("tipo pela extensão e marcador antigo continuam funcionando", () => {
  expect(tipoPelaExtensao("Relatório.PDF")).toBe("application/pdf");
  expect(tipoPelaExtensao("planilha.xlsx")).toContain("spreadsheetml");
  expect(separarAnexos("oi\n\n[Anexos enviados pelo celular]\n- C:/p/a.png").anexos).toEqual(["C:/p/a.png"]);
});
