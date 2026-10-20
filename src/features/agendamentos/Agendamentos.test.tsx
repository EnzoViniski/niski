import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { TelaDeAgendamentos } from "./TelaDeAgendamentos";
import { CentralDeAgendamentos } from "./CentralDeAgendamentos";
import { EditorDeAgendamento } from "./EditorDeAgendamento";
import { AcoesDaPaleta, useListaDeAcoes } from "../paleta/acoes";
import * as api from "./agendamentosService";
vi.mock("./agendamentosService", async () => ({ ...await vi.importActual<typeof api>("./agendamentosService"), listarAgendamentos: vi.fn(), opcoesAgendamentos: vi.fn(), conversasAgendaveis: vi.fn(), salvarAgendamento: vi.fn(), agirNoAgendamento: vi.fn(), resolverDataLocal: vi.fn() }));
const opcoes: api.Opcoes = { projetos: [{ id: "p", name: "Loja", path: "/tmp/loja" }], contas: [{ id: "trabalho", name: "Trabalho", provider: "codex" }], plataformas: [{ id: "claude", label: "Claude" }, { id: "codex", label: "Codex" }], fuso: "-03:00" };
const conversa = { id: "c", title: "Conversa da loja", project_id: "p", provider: "claude", profile_id: null, state: null, capabilities: null };
const a: api.Agendamento = { id: "agenda-1", titulo: "Revisão diária", prompt: "Revise o trabalho", destino: { tipo: "conversa", conversa: "c" }, regra: { tipo: "diario", horario: "09:00" }, pausado: false, proxima_ms: 1_900_000_000_000, fuso: "local_do_engine", historico: [{ em_ms: 1000, estado: "enviado", conversa: "c" }], pendente: null };
beforeEach(() => {
  vi.clearAllMocks(); vi.mocked(api.listarAgendamentos).mockResolvedValue([a]); vi.mocked(api.opcoesAgendamentos).mockResolvedValue(opcoes);
  vi.mocked(api.conversasAgendaveis).mockResolvedValue([conversa]); vi.mocked(api.salvarAgendamento).mockResolvedValue({}); vi.mocked(api.agirNoAgendamento).mockResolvedValue({}); vi.mocked(api.resolverDataLocal).mockResolvedValue({ em_ms: 1_900_000_000_000 });
});
it("lista próxima execução, histórico e ações de pausar, retomar e rodar", async () => {
  render(<TelaDeAgendamentos onFechar={vi.fn()} />);
  expect(await screen.findByText("Revisão diária")).toBeInTheDocument();
  expect(screen.getByText(/Próxima:/)).toBeInTheDocument(); expect(screen.getByText(/engine precisa estar ligado/)).toBeInTheDocument();
  fireEvent.click(screen.getByText("Últimas execuções (1)")); expect(screen.getByText(/Prompt enviado à CLI/)).toBeInTheDocument();
  fireEvent.click(screen.getByText("Pausar")); await waitFor(() => expect(api.agirNoAgendamento).toHaveBeenCalledWith("agenda-1", "pausar"));
  await waitFor(() => expect(screen.getByText("Rodar agora")).toBeEnabled()); fireEvent.click(screen.getByText("Rodar agora"));
  await waitFor(() => expect(api.agirNoAgendamento).toHaveBeenCalledWith("agenda-1", "rodar"));

});
it("exclusão exige o segundo clique e não dispara ao apenas abrir a confirmação", async () => {
  render(<TelaDeAgendamentos onFechar={vi.fn()} />); fireEvent.click(await screen.findByText("Apagar"));
  expect(api.agirNoAgendamento).not.toHaveBeenCalled(); fireEvent.click(screen.getByText("Confirmar exclusão"));
  await waitFor(() => expect(api.agirNoAgendamento).toHaveBeenCalledWith("agenda-1", "apagar"));
});
it("cria para esta conversa com prompt do compositor e data convertida no engine", async () => {
  render(<TelaDeAgendamentos inicial={{ conversa: "c", titulo: "Conversa da loja", prompt: "Verifique o deploy" }} onFechar={vi.fn()} />);
  const campo = await screen.findByLabelText("Data e hora"); fireEvent.change(campo, { target: { value: "2030-03-10T09:30" } });
  expect(screen.getByLabelText("Prompt")).toHaveValue("Verifique o deploy"); fireEvent.click(screen.getByText("Salvar agendamento"));
  await waitFor(() => expect(api.salvarAgendamento).toHaveBeenCalledWith(expect.objectContaining({ destino: { tipo: "conversa", conversa: "c" }, regra: { tipo: "uma_vez", em_ms: 1_900_000_000_000 }, prompt: "Verifique o deploy" })));
  expect(api.resolverDataLocal).toHaveBeenCalledWith("2030-03-10T09:30");
});
it("cria agente com plataforma, conta, modelo, pasta e worktree e intervalo em horas", async () => {
  render(<EditorDeAgendamento opcoes={opcoes} conversas={[conversa]} onSalvar={vi.fn()} onCancelar={vi.fn()} />);
  fireEvent.change(screen.getByLabelText("Título"), { target: { value: "Rotina" } }); fireEvent.change(screen.getByLabelText("Prompt"), { target: { value: "Confira a branch" } });
  fireEvent.change(screen.getByLabelText("Plataforma"), { target: { value: "codex" } }); fireEvent.change(screen.getByLabelText("Conta"), { target: { value: "trabalho" } });
  fireEvent.change(screen.getByLabelText("Modelo (opcional)"), { target: { value: "modelo-sintetico" } }); fireEvent.click(screen.getByLabelText("Isolar em worktree"));
  fireEvent.change(screen.getByLabelText("Frequência"), { target: { value: "intervalo" } }); fireEvent.change(screen.getByLabelText("A cada"), { target: { value: "2" } }); fireEvent.change(screen.getByLabelText("Unidade"), { target: { value: "60" } });
  fireEvent.click(screen.getByText("Salvar agendamento"));
  await waitFor(() => expect(api.salvarAgendamento).toHaveBeenCalledWith(expect.objectContaining({ regra: { tipo: "intervalo", minutos: 120 }, destino: { tipo: "novo_agente", projeto: "p", pasta: "/tmp/loja", plataforma: "codex", conta: "trabalho", modelo: "modelo-sintetico", worktree: true } })));
  expect(screen.getByText("Automática (em breve)")).toBeDisabled();
});
it("edita semanal e mantém o id, com dias selecionáveis", async () => {
  render(<EditorDeAgendamento existente={a} opcoes={opcoes} conversas={[conversa]} onSalvar={vi.fn()} onCancelar={vi.fn()} />);
  fireEvent.change(screen.getByLabelText("Frequência"), { target: { value: "semanal" } });
  const dias = within(screen.getByRole("group", { name: "Dias da semana" })); fireEvent.click(dias.getByText("Sáb"));
  fireEvent.change(screen.getByLabelText("Horário"), { target: { value: "10:15" } }); fireEvent.click(screen.getByText("Salvar agendamento"));
  await waitFor(() => expect(api.salvarAgendamento).toHaveBeenCalledWith(expect.objectContaining({ id: a.id, regra: { tipo: "semanal", horario: "10:15", dias: [0, 1, 2, 3, 4, 5] } })));
});
it("mantém o formulário e apresenta falha de gravação", async () => {
  vi.mocked(api.salvarAgendamento).mockRejectedValueOnce(new Error("Engine indisponível"));
  render(<EditorDeAgendamento existente={a} opcoes={opcoes} conversas={[conversa]} onSalvar={vi.fn()} onCancelar={vi.fn()} />);
  fireEvent.click(screen.getByText("Salvar agendamento")); expect(await screen.findByRole("alert")).toHaveTextContent("Engine indisponível"); expect(screen.getByLabelText("Prompt")).toHaveValue(a.prompt);
});
it("registra entrada na paleta, abre pelo evento e fecha com Escape", async () => {
  function Acoes() { const acoes = useListaDeAcoes(); return <>{acoes.map(a => <button key={a.id} onClick={() => a.executar()}>{a.titulo}</button>)}</>; }
  render(<AcoesDaPaleta><CentralDeAgendamentos /><Acoes /></AcoesDaPaleta>);
  fireEvent.click(await screen.findByText("Abrir Agendamentos")); expect(await screen.findByRole("dialog", { name: "Agendamentos" })).toBeInTheDocument();
  fireEvent.keyDown(window, { key: "Escape" }); expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  fireEvent(window, new Event("medza-abrir-agendamentos")); expect(await screen.findByRole("dialog")).toBeInTheDocument();
});
it("mostra erro de leitura e bloqueia editar durante execução", async () => {
  vi.mocked(api.listarAgendamentos).mockResolvedValue([{ ...a, pendente: {} }]);
  render(<TelaDeAgendamentos onFechar={vi.fn()} />); expect(await screen.findByText("Editar")).toBeDisabled(); expect(screen.getByText("Rodar agora")).toBeDisabled();
});

it("retoma um agendamento pausado", async () => {
  vi.mocked(api.listarAgendamentos).mockResolvedValue([{ ...a, pausado: true }]);
  render(<TelaDeAgendamentos onFechar={vi.fn()} />);
  fireEvent.click(await screen.findByText("Retomar"));
  await waitFor(() => expect(api.agirNoAgendamento).toHaveBeenCalledWith("agenda-1", "retomar"));
});

it("menu existente deixa agendar durante turno sem liberar comandos da CLI", async () => {
  const { ControlesDaConversa } = await import("../../mobile/conversa/ControlesDaConversa");
  const agendar = vi.fn();
  render(<ControlesDaConversa conversaId="c" provider="codex" pronto={false} onModo={vi.fn()} onComando={vi.fn()} onAgendar={agendar} />);
  fireEvent.click(screen.getByRole("button", { name: "Mais controles" }));
  expect(screen.getByText("Compactar")).toBeDisabled(); fireEvent.click(screen.getByText("Agendar esta mensagem"));
  expect(agendar).toHaveBeenCalledOnce();
});
