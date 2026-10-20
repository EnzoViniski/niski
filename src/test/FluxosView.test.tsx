import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "./interacoesTelas";
import { invoke } from "@tauri-apps/api/core";
import { FluxosView } from "../features/fluxos/FluxosView";
import type { Execucao, FluxoSalvo, EstadoDoNo } from "../features/fluxos/fluxosService";
import { agente } from "./fixturesTelas";

// React Flow real: jsdom não desenha o canvas, mas os formulários, diálogos e painéis são reais.
const fluxo: FluxoSalvo = { id: "revisar", nome: "Revisar telas", descricao: "Revisão com aprovação", escopo: "global", caminho: "/fake/fluxos/revisar.json", nos: [
  { id: "entrada", tipo: "entrada", titulo: "Tarefa", config: {}, x: 0, y: 0 },
  { id: "aprovar", tipo: "aprovacao", titulo: "Revisão humana", config: { pergunta: "Publicar a revisão?" }, x: 200, y: 0 },
  { id: "fim", tipo: "fim", titulo: "Finalizar", config: {}, x: 400, y: 0 },
], conexoes: [{ de: "entrada", para: "aprovar" }, { de: "aprovar", para: "fim", saida: "sim" }] };
const estado: EstadoDoNo = { status: "aguardando", voltas: 1, inicio_ms: 1000, fim_ms: 0, saida: "Revisão pronta", session_id: "sessao", custo_usd: 0, erro: null, cli: "claude", conta: null, caminho: null };
let execucao: Execucao;
let salvos: FluxoSalvo[];
let listarExecucoes: boolean;
function montar(temProjeto = true) {
  const props = { projectId: temProjeto ? "proj" : null, projectPath: temProjeto ? "/fake/projeto" : null, onAbrirSessao: vi.fn(), onClose: vi.fn() };
  render(<FluxosView {...props} />);
  return props;
}
async function abrirSalvo() { await userEvent.click(await screen.findByRole("button", { name: /Revisar telas Global/ })); }
beforeEach(() => {
  salvos = [structuredClone(fluxo)];
  listarExecucoes = false;
  execucao = { id: "exec", fluxo, projeto: "/fake/projeto", projeto_id: "proj", entrada: "Revisar o formulário", pasta: "/fake/worktree", branch: "fluxo/revisar", base: "main", status: "aguardando", erro: null, criado_ms: 1000, atualizado_ms: 2000, nos: { aprovar: { ...estado } }, pr_url: null, custo_usd: 0 };
  vi.mocked(invoke).mockReset().mockImplementation(async (cmd, args) => {
    if (cmd === "fluxos_listar") return [...salvos];
    if (cmd === "agentes_listar") return [agente];
    if (cmd === "agentes_modelos") return {};
    if (cmd === "fluxos_salvar") return "/fake/fluxos/novo.json";
    if (cmd === "fluxos_apagar") { salvos = []; return null; }
    if (cmd === "engine_api") {
      const { path, method } = args as { path: string; method: string };
      let body: unknown = { ok: true };
      if (path === "/fluxos/execucoes") body = method === "POST" ? execucao : listarExecucoes ? [{ ...execucao, fluxo: fluxo.nome, fluxo_id: fluxo.id }] : [];
      else if (path.endsWith("/historico")) body = [{ kind: "text", role: "assistant", text: "Foco e contraste conferidos" }];
      else if (path.endsWith("/conversa")) body = { session_id: "sessao", name: "Revisor" };
      else if (path === "/fluxos/execucoes/exec") body = execucao;
      return { type: "api", status: 200, body: JSON.stringify(body) };
    }
    return null;
  });
});
it("carrega os fluxos salvos, mostra o começo e fecha a tela", async () => {
  const { onClose } = montar();
  expect(await screen.findByRole("button", { name: /Revisar telas Global/ })).toBeInTheDocument();
  expect(screen.getByText(/Escolha um fluxo à esquerda/)).toBeInTheDocument();
  expect(screen.getByText("Nenhuma ainda.")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Fechar fluxos" }));
  expect(onClose).toHaveBeenCalledOnce();
});
it("cria um fluxo, altera nome e descrição e salva", async () => {
  montar();
  await userEvent.click(screen.getByRole("button", { name: "+ Novo fluxo" }));
  const campo = screen.getByRole("textbox", { name: "Nome do fluxo" });
  await userEvent.clear(campo);
  await userEvent.type(campo, "Meu processo");
  await userEvent.type(screen.getByRole("textbox", { name: "Descrição" }), "Revisão local");
  await userEvent.click(screen.getByRole("button", { name: "Salvar" }));
  expect(await screen.findByRole("status")).toHaveTextContent("Fluxo salvo.");
  expect(screen.queryByText("não salvo")).not.toBeInTheDocument();
  expect(invoke).toHaveBeenCalledWith("fluxos_salvar", expect.objectContaining({ fluxo: expect.objectContaining({ id: "meu-processo", nome: "Meu processo", descricao: "Revisão local" }) }));
});
it.each([
  ["Agente", "Prompt desta etapa"], ["Condição", "Limite de voltas"], ["Aprovação", "Pergunta"], ["Time", "Objetivo da reunião"], ["Pull request", "Título do PR"], ["Fim", "Título"],
])("adiciona e configura uma etapa %s", async (tipo, campo) => {
  montar();
  await userEvent.click(screen.getByRole("button", { name: "+ Novo" }));
  await userEvent.click(within(screen.getByRole("toolbar", { name: "Adicionar etapa" })).getByRole("button", { name: tipo, exact: true }));
  expect(screen.getByLabelText(campo)).toBeInTheDocument();
  const titulo = screen.getByRole("textbox", { name: "Título", exact: true });
  await userEvent.clear(titulo);
  await userEvent.type(titulo, "Etapa configurada");
  expect(titulo).toHaveValue("Etapa configurada");
});
it("mostra a validação que impede salvar sem descartar o rascunho", async () => {
  montar();
  await abrirSalvo();
  vi.mocked(invoke).mockRejectedValueOnce(new Error("Falta conectar a etapa"));
  await userEvent.click(screen.getByRole("button", { name: "Salvar" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Falta conectar a etapa");
  expect(screen.getByRole("textbox", { name: "Nome do fluxo" })).toHaveValue("Revisar telas");
});
it("remove um fluxo salvo da lista", async () => {
  montar();
  await abrirSalvo();
  await userEvent.click(screen.getByRole("button", { name: "Apagar fluxo" }));
  await waitFor(() => expect(screen.queryByRole("button", { name: /Revisar telas Global/ })).not.toBeInTheDocument());
  expect(invoke).toHaveBeenCalledWith("fluxos_apagar", { escopo: "global", projectPath: "/fake/projeto", id: "revisar" });
});
it("impede executar quando nenhum projeto está aberto", async () => {
  montar(false);
  await abrirSalvo();
  expect(screen.getByRole("button", { name: "Executar" })).toBeDisabled();
});
it("confere as etapas e só inicia depois de descrever a tarefa", async () => {
  montar();
  await abrirSalvo();
  await userEvent.click(screen.getByRole("button", { name: "Executar" }));
  const dialogo = within(screen.getByRole("dialog", { name: "Executar Revisar telas" }));
  expect(dialogo.getByText("Revisão humana")).toBeInTheDocument();
  expect(dialogo.getByRole("button", { name: "Iniciar" })).toBeDisabled();
  await userEvent.type(dialogo.getByRole("textbox", { name: "Tarefa" }), "Revisar o formulário");
  await userEvent.click(dialogo.getByRole("button", { name: "Iniciar" }));
  expect(await screen.findByRole("button", { name: "Editor" })).toBeInTheDocument();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(invoke).toHaveBeenCalledWith("engine_api", expect.objectContaining({ method: "POST", path: "/fluxos/execucoes", body: expect.stringContaining('"entrada":"Revisar o formulário"') }));
});
it.each([["Aprovar", true], ["Recusar", false]] as const)("abre o histórico do nó e permite %s", async (nome, sim) => {
  listarExecucoes = true;
  const { onAbrirSessao } = montar();
  await userEvent.click(await screen.findByRole("button", { name: /Revisar telas Esperando você/ }));
  await userEvent.click(await screen.findByRole("button", { name: /Clique para aprovar ou recusar/ }));
  expect(await screen.findByText("Foco e contraste conferidos")).toBeInTheDocument();
  expect(screen.getByText("Publicar a revisão?")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: nome, exact: true }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("engine_api", expect.objectContaining({ path: "/fluxos/execucoes/exec/nos/aprovar/aprovar", body: JSON.stringify({ sim }) })));
  await userEvent.click(screen.getByRole("button", { name: "Abrir numa conversa (ajustar ou reverter)" }));
  await waitFor(() => expect(onAbrirSessao).toHaveBeenCalledWith("sessao", "Revisor"));
});
it.each([["rodando", "Pausar", "pausar"], ["pausado", "Retomar", "retomar"], ["aguardando", "Cancelar", "cancelar"]] as const)("controla execução %s usando %s", async (status, nome, acao) => {
  listarExecucoes = true;
  execucao.status = status;
  montar();
  await userEvent.click(await screen.findByRole("button", { name: new RegExp(`Revisar telas ${status === "rodando" ? "Rodando" : status === "pausado" ? "Pausado" : "Esperando você"}`) }));
  await userEvent.click(await screen.findByRole("button", { name: nome, exact: true }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("engine_api", expect.objectContaining({ path: `/fluxos/execucoes/exec/${acao}`, method: "POST" })));
});
it("importa JSON para revisar no editor antes de salvar", async () => {
  montar();
  await userEvent.click(screen.getByRole("button", { name: "+ Novo" }));
  await userEvent.click(screen.getByRole("button", { name: "Importar" }));
  fireEvent.change(screen.getByRole("textbox", { name: "JSON do fluxo" }), { target: { value: JSON.stringify(fluxo) } });
  await userEvent.click(screen.getByRole("button", { name: "Conferir" }));
  await userEvent.click(await screen.findByRole("button", { name: "Abrir no editor" }));
  expect(screen.getByRole("textbox", { name: "Nome do fluxo" })).toHaveValue("Revisar telas");
  expect(screen.getByText("não salvo")).toBeInTheDocument();
});
it("apresenta erro de JSON inválido dentro do diálogo", async () => {
  montar();
  await userEvent.click(screen.getByRole("button", { name: "+ Novo" }));
  await userEvent.click(screen.getByRole("button", { name: "Importar" }));
  fireEvent.change(screen.getByRole("textbox", { name: "JSON do fluxo" }), { target: { value: "{}" } });
  await userEvent.click(screen.getByRole("button", { name: "Conferir" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("O JSON não tem nos e conexoes");
});

it.each([["Objetivo /goal", "Condição de conclusão"], ["Repetir", "Máximo de repetições"]])("edita e salva o bloco %s", async (tipo, campo) => {
  montar();
  await userEvent.click(screen.getByRole("button", { name: "+ Novo" }));
  await userEvent.click(within(screen.getByRole("toolbar", { name: "Adicionar etapa" })).getByRole("button", { name: tipo, exact:true }));
  expect(screen.getByLabelText(campo)).toBeInTheDocument();
  if (tipo === "Repetir") {
    fireEvent.change(screen.getByLabelText(campo), { target:{ value:"5" } });
    fireEvent.change(screen.getByLabelText("Intervalo em minutos"), { target:{ value:"2" } });
    await userEvent.click(screen.getByLabelText("Parar quando o ramo devolver aprovado"));
  } else {
    await userEvent.clear(screen.getByLabelText(campo));
    await userEvent.type(screen.getByLabelText(campo),"Todos os testes passam");
    expect(screen.getByText(/Codex: \/goal em exec ainda não documentado/)).toBeInTheDocument();
  }
  await userEvent.click(screen.getByRole("button", { name:"Salvar" }));
  expect(invoke).toHaveBeenCalledWith("fluxos_salvar",expect.objectContaining({ fluxo:expect.objectContaining({ nos:expect.arrayContaining([expect.objectContaining({ tipo:tipo === "Repetir" ? "repetir" : "objetivo",config:expect.objectContaining(tipo === "Repetir" ? {vezes:5,intervalo_minutos:2,parar_quando_aprovado:true} : {objetivo:"Todos os testes passam",max_turnos:10}) })]) }) }));
});
