import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ControlesDaConversa } from "../mobile/conversa/ControlesDaConversa";
import { request } from "../mobile/api";

vi.mock("../mobile/api", () => ({ request: vi.fn() }));
vi.mock("../web/TerminalEspelho", () => ({ default: ({ saida, teclas }: { saida: string; teclas: string }) => <div data-testid="terminal" data-saida={saida} data-teclas={teclas} /> }));
const props = () => ({ conversaId: "c/1", provider: "claude", modo: "plan", pronto: true, onModo: vi.fn(async () => {}), onComando: vi.fn(async () => {}) });
beforeEach(() => { vi.mocked(request).mockReset(); vi.mocked(request).mockResolvedValue({ fast_supported: false }); });

it("oferece quatro modos e mantém o valor confirmado enquanto aguarda a CLI", async () => {
  const p = props(); let concluir!: () => void;
  p.onModo.mockImplementation(() => new Promise<void>((r) => { concluir = r; }));
  const { rerender } = render(<ControlesDaConversa {...p} />);
  expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["Perguntar", "Aceitar edições", "Plano", "Auto"]);
  fireEvent.change(screen.getByLabelText("Modo do agente"), { target: { value: "acceptEdits" } });
  expect(p.onModo).toHaveBeenCalledWith("acceptEdits");
  expect(screen.getByLabelText("Modo do agente")).toHaveValue("plan");
  expect(screen.getByLabelText("Modo do agente")).toBeDisabled();
  concluir();
  await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
  rerender(<ControlesDaConversa {...p} modo="acceptEdits" />);
  expect(screen.getByLabelText("Modo do agente")).toHaveValue("acceptEdits");
});

it("bloqueia controles durante turno e não presume modo desconhecido", () => {
  render(<ControlesDaConversa {...props()} modo={null} pronto={false} />);
  expect(screen.getByLabelText("Modo do agente")).toHaveValue("");
  expect(screen.getByLabelText("Modo do agente")).toBeDisabled();
  expect(screen.getByRole("button", { name: "Mais controles" })).toBeDisabled();
});

it("fast depende do suporte do modelo e manda on/off sem confirmar estado otimista", async () => {
  vi.mocked(request).mockResolvedValue({ fast_supported: true });
  const p = props(); const { rerender } = render(<ControlesDaConversa {...p} fast={false} />);
  fireEvent.click(await screen.findByRole("button", { name: "Ligar fast" }));
  await waitFor(() => expect(p.onComando).toHaveBeenCalledWith("fast", "on"));
  expect(screen.getByRole("button", { name: "Ligar fast" })).toHaveAttribute("aria-pressed", "false");
  await waitFor(() => expect(screen.getByRole("button", { name: "Ligar fast" })).toBeEnabled());
  rerender(<ControlesDaConversa {...p} fast />);
  fireEvent.click(screen.getByRole("button", { name: "Desligar fast" }));
  await waitFor(() => expect(p.onComando).toHaveBeenCalledWith("fast", "off"));
});

it("oculta fast quando o modelo não confirma suporte", async () => {
  render(<ControlesDaConversa {...props()} />);
  await waitFor(() => expect(request).toHaveBeenCalled());
  expect(screen.queryByRole("button", { name: /fast/ })).not.toBeInTheDocument();
});

it.each(["rewind", "branch"] as const)("%s abre o terminal oficial e fechar só esconde o painel", async (tipo) => {
  const p = props(); render(<ControlesDaConversa {...p} />);
  fireEvent.click(screen.getByRole("button", { name: "Mais controles" }));
  fireEvent.click(screen.getByRole("button", { name: tipo === "rewind" ? "Voltar a um ponto" : "Ramificar aqui" }));
  const painel = await screen.findByRole("dialog");
  expect(p.onComando).toHaveBeenCalledWith(tipo);
  const terminal = await screen.findByTestId("terminal");
  expect(terminal).toHaveAttribute("data-saida", "/conversas/c%2F1/terminal");
  expect(terminal).toHaveAttribute("data-teclas", "/conversas/c%2F1/teclas");
  if (tipo === "rewind") expect(screen.getByText(/Não desfaz comandos de Bash nem edições de subagentes/)).toBeInTheDocument();
  fireEvent.keyDown(painel, { key: "Escape" });
  expect(painel).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Fechar terminal" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(p.onComando).toHaveBeenCalledTimes(1);
});

it("mostra falha da CLI e permite repetir sem abrir terminal", async () => {
  const p = props(); p.onComando.mockRejectedValue(new Error("Contexto mudou"));
  render(<ControlesDaConversa {...p} />);
  fireEvent.click(screen.getByRole("button", { name: "Mais controles" }));
  fireEvent.click(screen.getByRole("button", { name: "Voltar a um ponto" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Contexto mudou");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

it("Codex só recebe Compactar; outras plataformas não ganham controles Claude", async () => {
  const p = props(); const { rerender } = render(<ControlesDaConversa {...p} provider="codex" />);
  expect(screen.queryByLabelText("Modo do agente")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Mais controles" }));
  expect(screen.queryByRole("button", { name: "Ramificar aqui" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Compactar" }));
  await waitFor(() => expect(p.onComando).toHaveBeenCalledWith("compact"));
  rerender(<ControlesDaConversa {...p} provider="antigravity" />);
  expect(screen.queryByLabelText("Controles da conversa")).not.toBeInTheDocument();
});

// Fork (modo do Codex na Conversa): o pedido do Enzo era escolher Perguntar/Sem perguntar no Codex.
it("Codex tem o seletor dele: Perguntar, Sem perguntar, Acesso total e Somente leitura", () => {
  const p = props(); const { rerender } = render(<ControlesDaConversa {...p} provider="codex" modo={null} />);
  const seletor = screen.getByLabelText("Modo do Codex");
  expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["Modo da conta", "Perguntar", "Sem perguntar", "Acesso total", "Somente leitura"]);
  fireEvent.change(seletor, { target: { value: "semPerguntar" } });
  expect(p.onModo).toHaveBeenCalledWith("semPerguntar");
  rerender(<ControlesDaConversa {...p} provider="codex" modo="semPerguntar" />);
  expect(screen.getByLabelText("Modo do Codex")).toHaveValue("semPerguntar");
  rerender(<ControlesDaConversa {...p} provider="codex" modo="plan" />);
  expect(screen.getByLabelText("Modo do Codex")).toHaveValue("");
  expect(screen.getAllByRole("option")[0]).toHaveTextContent("Plano");
});
