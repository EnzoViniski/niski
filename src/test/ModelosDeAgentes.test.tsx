import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "./interacoesTelas";
import { invoke } from "@tauri-apps/api/core";
import { ModelosDeAgentes } from "../features/agentes/ModelosDeAgentes";
import type { AgenteSalvo } from "../features/agentes/agentesService";
import { agente, perfis } from "./fixturesTelas";

let salvos: AgenteSalvo[];
beforeEach(() => {
  salvos = [{ ...agente }, { ...agente, name: "dev-projeto", escopo: "projeto" }];
  vi.mocked(invoke).mockReset().mockImplementation(async (cmd, args) => {
    if (cmd === "agentes_listar") return [...salvos];
    if (cmd === "agentes_modelos") return {};
    if (cmd === "list_profiles") return perfis;
    if (cmd === "agentes_criar_com_ia") return { ...agente, name: "gerado" };
    if (cmd === "agentes_salvar") {
      const dados = args as { modelo: AgenteSalvo; escopo: "global" | "projeto"; nomeAnterior: string | null };
      salvos = salvos.filter((a) => !(a.name === dados.nomeAnterior && a.escopo === dados.escopo));
      salvos.push({ ...dados.modelo, escopo: dados.escopo, caminho: "/fake/agents/novo.md" });
      return "/fake/agents/novo.md";
    }
    if (cmd === "agentes_apagar") { salvos = salvos.filter((a) => a.name !== (args as { nome: string }).nome); return null; }
    if (cmd === "agentes_instalar_claude") return "/fake/projeto/.claude/agents/revisor.md";
    return null;
  });
});
async function cartao(nome = "revisor") {
  return (await screen.findByRole("heading", { name: nome, exact: true })).closest("article")!;
}
it("filtra modelos globais e do projeto", async () => {
  render(<ModelosDeAgentes projectPath="/fake/projeto" />);
  await cartao();
  await userEvent.click(screen.getByRole("tab", { name: /Do projeto/ }));
  expect(screen.queryByRole("heading", { name: "revisor" })).not.toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "dev-projeto" })).toBeInTheDocument();
  await userEvent.click(screen.getByRole("tab", { name: /Globais/ }));
  expect(screen.getByRole("heading", { name: "revisor" })).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "dev-projeto" })).not.toBeInTheDocument();
});
it("cria em branco, escolhe ferramentas e salva o modelo", async () => {
  render(<ModelosDeAgentes projectPath="/fake/projeto" />);
  await userEvent.click(screen.getByRole("button", { name: "Em branco" }));
  await userEvent.type(screen.getByRole("textbox", { name: "Nome" }), "novorevisor");
  await userEvent.type(screen.getByRole("textbox", { name: "Descrição" }), "Confere telas");
  await userEvent.type(screen.getByRole("textbox", { name: "Instruções" }), "Confira contraste.");
  await userEvent.click(screen.getByRole("button", { name: "Edit", exact: true }));
  expect(screen.getByRole("button", { name: "Edit", exact: true })).toHaveAttribute("aria-pressed", "true");
  await userEvent.click(screen.getByRole("button", { name: "Salvar", exact: true }));
  expect(await screen.findByRole("status")).toHaveTextContent('"novorevisor" salvo.');
  expect(await screen.findByRole("heading", { name: "novorevisor" })).toBeInTheDocument();
  expect(invoke).toHaveBeenCalledWith("agentes_salvar", expect.objectContaining({ escopo: "global", modelo: expect.objectContaining({ name: "novorevisor", ferramentas: expect.arrayContaining(["Edit"]), instrucoes: "Confira contraste." }) }));
});
it("abre a geração com IA para revisão antes de gravar", async () => {
  render(<ModelosDeAgentes projectPath="/fake/projeto" />);
  const criar = screen.getByRole("button", { name: "Criar com IA" });
  expect(criar).toBeDisabled();
  await userEvent.type(screen.getByRole("textbox", { name: "Descreva o agente" }), "Um revisor de telas acessíveis");
  await userEvent.click(criar);
  expect(await screen.findByRole("textbox", { name: "Nome" })).toHaveValue("gerado");
  expect(screen.getByRole("status")).toHaveTextContent("Revise e salve");
  expect(invoke).not.toHaveBeenCalledWith("agentes_salvar", expect.anything());
  await userEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(screen.queryByRole("textbox", { name: "Nome" })).not.toBeInTheDocument();
});
it("duplica sem sobrescrever o original", async () => {
  render(<ModelosDeAgentes projectPath="/fake/projeto" />);
  await userEvent.click(within(await cartao()).getByRole("button", { name: "Duplicar" }));
  expect(screen.getByRole("textbox", { name: "Nome" })).toHaveValue("revisor-copia");
  await userEvent.click(screen.getByRole("button", { name: "Salvar", exact: true }));
  expect(await screen.findByRole("heading", { name: "revisor-copia" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "revisor", exact: true })).toBeInTheDocument();
  expect(invoke).toHaveBeenCalledWith("agentes_salvar", expect.objectContaining({ nomeAnterior: null }));
});
it("edita preservando o nome anterior usado ao salvar", async () => {
  render(<ModelosDeAgentes projectPath="/fake/projeto" />);
  await userEvent.click(within(await cartao()).getByRole("button", { name: "Editar" }));
  const campo = screen.getByRole("textbox", { name: "Descrição" });
  await userEvent.clear(campo);
  await userEvent.type(campo, "Confere foco e contraste");
  await userEvent.click(screen.getByRole("button", { name: "Salvar", exact: true }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("agentes_salvar", expect.objectContaining({ nomeAnterior: "revisor", modelo: expect.objectContaining({ description: "Confere foco e contraste" }) })));
  expect(await screen.findByText("Confere foco e contraste")).toBeInTheDocument();
});
it("só apaga depois do segundo clique de confirmação", async () => {
  render(<ModelosDeAgentes projectPath="/fake/projeto" />);
  const card = within(await cartao());
  await userEvent.click(card.getByRole("button", { name: "Apagar" }));
  expect(invoke).not.toHaveBeenCalledWith("agentes_apagar", expect.anything());
  await userEvent.click(card.getByRole("button", { name: "Confirmar" }));
  await waitFor(() => expect(screen.queryByRole("heading", { name: "revisor", exact: true })).not.toBeInTheDocument());
});
it("instala o modelo como subagente do projeto", async () => {
  render(<ModelosDeAgentes projectPath="/fake/projeto" />);
  await userEvent.click(within(await cartao()).getByRole("button", { name: "No Claude Code" }));
  expect(await screen.findByRole("status")).toHaveTextContent("Instalado em /fake/projeto/.claude/agents/revisor.md");
  expect(invoke).toHaveBeenCalledWith("agentes_instalar_claude", { projectPath: "/fake/projeto", modelo: agente });
});
it("avisa que modelos do projeto precisam de um projeto aberto", async () => {
  salvos = [];
  render(<ModelosDeAgentes projectPath={null} />);
  await userEvent.click(screen.getByRole("tab", { name: /Do projeto/ }));
  expect(await screen.findByText("Abra um projeto para ver os modelos dele.")).toBeInTheDocument();
});
it("mantém o editor aberto quando salvar falha", async () => {
  render(<ModelosDeAgentes projectPath={null} />);
  await userEvent.click(screen.getByRole("button", { name: "Em branco" }));
  await userEvent.type(screen.getByRole("textbox", { name: "Nome" }), "novo");
  vi.mocked(invoke).mockRejectedValueOnce(new Error("Não foi possível salvar"));
  await userEvent.click(screen.getByRole("button", { name: "Salvar", exact: true }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Não foi possível salvar");
  expect(screen.getByRole("textbox", { name: "Nome" })).toHaveValue("novo");
});

it("permite digitar um nome de agente com hífen", async () => {
  render(<ModelosDeAgentes projectPath={null} />);
  await userEvent.click(screen.getByRole("button", { name: "Em branco" }));
  const nome = screen.getByRole("textbox", { name: "Nome" });
  await userEvent.type(nome, "revisor-telas");
  expect(nome).toHaveValue("revisor-telas");
});
