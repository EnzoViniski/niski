// Fork (trocar agente): "Continuar com…" com o engine simulado (nenhuma CLI real).
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { TrocarAgente } from "../mobile/conversa/TrocarAgente";
import { destinosDaTroca, type OpcoesDeTroca } from "../mobile/conversa/trocaDeAgente";
import { LinhaDeAviso } from "../mobile/Chat";
import { request } from "../mobile/api";

vi.mock("../mobile/api", async (original) => ({ ...(await original<typeof import("../mobile/api")>()), request: vi.fn() }));

const opcoes: OpcoesDeTroca = {
  provider: "claude", profile_id: "pessoal", pode_trocar: true, motivo: null, live: true,
  agentes: [
    { id: "claude", label: "Claude", conversa: true, profiles: [{ id: "pessoal", name: "Pessoal" }, { id: "empresa", name: "Empresa" }] },
    { id: "codex", label: "Codex", conversa: true, profiles: [{ id: "trabalho", name: "Trabalho" }] },
    { id: "cursor", label: "Cursor", conversa: false, profiles: [] },
  ],
};

function engine(resposta: unknown = { modo: "passagem", provider: "codex", profile_id: "trabalho", passagem: "/p/.omni-agents/passagens/c-2.md", mensagem_enviada: true, conversa_no_chat: true }) {
  vi.mocked(request).mockImplementation(async (_caminho: string, init?: RequestInit) => (init?.method === "POST" ? resposta : opcoes) as never);
}
beforeEach(() => vi.mocked(request).mockReset());

it("destinos: outras contas do agente atual e cada conta dos outros agentes", () => {
  expect(destinosDaTroca(opcoes).map((d) => `${d.provider}:${d.profileId}`)).toEqual(["claude:empresa", "codex:trabalho", "cursor:null"]);
  // Agente atual com uma conta só não aparece (não há para onde ir dentro dele).
  expect(destinosDaTroca({ ...opcoes, provider: "codex", profile_id: "trabalho" }).map((d) => d.provider)).toEqual(["claude", "claude", "cursor"]);
});

it("escolher o Codex pede confirmação, troca levando o que foi escrito e avisa", async () => {
  engine();
  const onTrocado = vi.fn();
  render(<TrocarAgente conversaId="c/1" provider="claude" profileId="pessoal" pronto mensagem="agora mede no celular" onTrocado={onTrocado} />);
  fireEvent.click(screen.getByRole("button", { name: /Continuar com…/ }));
  fireEvent.click(await screen.findByRole("menuitem", { name: /Codex.*Trabalho/ }));
  expect(screen.getByText(/recebe um resumo completo/)).toBeInTheDocument();
  expect(screen.getByText(/O que você já escreveu vai junto/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Continuar" }));
  await waitFor(() => expect(onTrocado).toHaveBeenCalledWith(expect.objectContaining({ modo: "passagem", mensagem_enviada: true })));
  const post = vi.mocked(request).mock.calls.find(([, init]) => init?.method === "POST")!;
  expect(post[0]).toBe("/conversas/c%2F1/trocar-agente");
  expect(JSON.parse(String(post[1]!.body))).toEqual({ provider: "codex", profile_id: "trabalho", mensagem: "agora mede no celular" });
  expect(screen.getByRole("status")).toHaveTextContent("Continuando com o Codex · Trabalho");
});

it("outra conta do mesmo agente diz que é continuação literal", async () => {
  engine({ modo: "conta", provider: "claude", profile_id: "empresa", passagem: null, mensagem_enviada: false });
  render(<TrocarAgente conversaId="c" provider="claude" profileId="pessoal" pronto />);
  fireEvent.click(screen.getByRole("button", { name: /Continuar com…/ }));
  fireEvent.click(await screen.findByRole("menuitem", { name: /Claude.*Empresa/ }));
  expect(screen.getByText(/Mesma IA, outra conta/)).toBeInTheDocument();
});

it("agente trabalhando: opções desligadas com o motivo", async () => {
  vi.mocked(request).mockResolvedValue({ ...opcoes, pode_trocar:false, motivo:"Espere o agente terminar para trocar" });
  render(<TrocarAgente conversaId="c" provider="claude" pronto={false} />);
  fireEvent.click(screen.getByRole("button", { name: /Continuar com…/ }));
  expect(await screen.findByText("Espere o agente terminar para trocar")).toBeInTheDocument();
  for (const item of screen.getAllByRole("menuitem")) expect(item).toBeDisabled();
});

it("CLI travada em outro app: aceita a autorização atual do engine para passar o bastão", async () => {
  engine();
  render(<TrocarAgente conversaId="c" provider="claude" pronto={false} />);
  fireEvent.click(screen.getByRole("button", { name: /Continuar com…/ }));
  expect(await screen.findByRole("menuitem", { name: /Codex.*Trabalho/ })).toBeEnabled();
});

it("erro do engine aparece claro e o painel continua aberto", async () => {
  vi.mocked(request).mockImplementation(async (_c: string, init?: RequestInit) => {
    if (init?.method === "POST") throw new Error("O agente está trabalhando; espere terminar (ou pare) antes de trocar");
    return opcoes as never;
  });
  render(<TrocarAgente conversaId="c" provider="claude" profileId="pessoal" pronto />);
  fireEvent.click(screen.getByRole("button", { name: /Continuar com…/ }));
  fireEvent.click(await screen.findByRole("menuitem", { name: /Codex/ }));
  fireEvent.click(screen.getByRole("button", { name: "Continuar" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("O agente está trabalhando");
  expect(screen.getByRole("dialog")).toBeInTheDocument();
});

it("o separador da troca na conversa leva o ícone do agente novo", () => {
  const { container } = render(<LinhaDeAviso texto="Continuou com o Codex · conta Trabalho" />);
  expect(container.querySelector(".aviso-continuou")).toHaveTextContent("Continuou com o Codex · conta Trabalho");
});
