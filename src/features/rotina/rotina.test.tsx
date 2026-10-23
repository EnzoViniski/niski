import { render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import userEvent from "../../test/interacoesTelas";
import { AvisosDaRotina } from "./AvisosDaRotina";
import { RotinaView } from "./RotinaView";
import { Treino } from "./Treino";
import { ouvirPedidosDaRotina } from "./abrirRotina";
import * as servico from "./rotinaService";
import type { Dia, Treino as DadosDoTreino } from "./tipos";

vi.mock("./rotinaService", () => ({
  lerDia: vi.fn(), lerSemana: vi.fn(), lerTreino: vi.fn(), lerAlimentacao: vi.fn(), lerLembretes: vi.fn(), lerAjustes: vi.fn(),
  lerAvisos: vi.fn(), operar: vi.fn(), conversaAtual: vi.fn(), abrirConversa: vi.fn(), telegramDaRotina: vi.fn(),
}));
// A conversa de verdade (TerminalPane) precisa do engine; aqui só importa qual sessão ela recebe.
vi.mock("./ConversaDaRotina", () => ({
  ConversaDaRotina: ({ sessao, abrindo }: { sessao: string | null; abrindo: boolean }) =>
    <div data-testid="conversa">{abrindo ? "abrindo" : `sessao:${sessao ?? "-"}`}</div>,
  ehConversaDaRotina: (p: string) => p === "rotina",
}));

const dia: Dia = {
  data: "2026-10-27", dia_da_semana: "terça",
  itens: [
    { id: "bloco:b1", tipo: "bloco", titulo: "Medza", detalhe: "", inicio: "09:00", fim: "12:00", area: "medza", cor: "#0ea5e9", estado: "", alvo: "b1", categoria: "trabalho" },
    { id: "tarefa:t1", tipo: "tarefa", titulo: "Call Aircook", detalhe: "", inicio: "14:00", fim: "14:30", area: "aircook", cor: "#f97316", estado: "", alvo: "t1", categoria: "trabalho" },
  ],
  sem_hora: [], cancelados: [], agua: { ml: 500, meta_ml: 2500, copo_ml: 250 },
  alimentacao: { kcal: 0, proteina_g: 0, refeicoes: 0, agua_ml: 500 },
  treino: { ficha: null, sessao: null }, agora: "bloco:b1", proximo: "tarefa:t1", foco: "Entregar a Aircook", metas: [],
};

const treino: DadosDoTreino = {
  hoje: "2026-10-27",
  fichas: [{ id: "fa", nome: "A", titulo: "Peito", dias: [], exercicios: [{ nome: "Supino reto", series: 4, repeticoes: "8-10", carga_kg: null, descanso_s: 90, obs: "" }] }],
  meta_semanal: 4, proxima_ficha: "fa", sessao_de_hoje: null,
  sessoes: [{ id: "s0", data: "2026-10-23", ficha: "fa", inicio: "07:00", fim: "", obs: "", concluida: true, volume: 600,
    series: [{ exercicio: "Supino reto", carga_kg: 60, repeticoes: 10, obs: "", em: "07:10" }] }],
  frequencia: [{ semana: "2026-10-19", treinos: 1 }, { semana: "2026-10-26", treinos: 0 }],
  exercicios: [{ nome: "Supino reto", sessoes: 1, ultima_data: "2026-10-23", carga_max: 60, um_rm: 80 }],
};

beforeEach(() => {
  localStorage.clear();
  vi.mocked(servico.lerDia).mockReset().mockResolvedValue(dia);
  vi.mocked(servico.operar).mockReset().mockResolvedValue({ ok: true });
  vi.mocked(servico.conversaAtual).mockReset().mockResolvedValue({ conversa: null, sessao: null });
  vi.mocked(servico.abrirConversa).mockReset().mockResolvedValue({ conversa: "conv-1", sessao: "s-9", nova: true });
  vi.mocked(servico.lerTreino).mockReset().mockResolvedValue(treino);
  vi.mocked(servico.lerAvisos).mockReset().mockResolvedValue({ nao_vistos: 0, avisos: [] });
});

it("Hoje mostra a linha do tempo com agora e a seguir e marca feito num toque", async () => {
  render(<RotinaView agentView="conversa" onClose={() => undefined} />);
  const medza = (await screen.findByText("Medza")).closest("li")!;
  expect(within(medza).getByText("Agora")).toBeInTheDocument();
  expect(within(screen.getByText("Call Aircook").closest("li")!).getByText("A seguir")).toBeInTheDocument();
  expect(screen.getByText("Entregar a Aircook")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Marcar Medza como feito" }));
  expect(servico.operar).toHaveBeenCalledWith("marcar", { data: "2026-10-27", item: "bloco:b1", estado: "feito" });
  await userEvent.click(screen.getByRole("button", { name: /\+1 copo/ }));
  expect(servico.operar).toHaveBeenCalledWith("registrar_agua", { data: "2026-10-27" });
  await waitFor(() => expect(servico.lerDia).toHaveBeenCalledTimes(3), { timeout: 2000 });
});

it("Planejar a semana abre a conversa do agente ao lado com o padrão do copiloto", async () => {
  localStorage.setItem("medza-copiloto", JSON.stringify({ provider: "codex" }));
  render(<RotinaView agentView="conversa" onClose={() => undefined} />);
  await screen.findByText("Medza");
  await userEvent.click(screen.getByRole("button", { name: "Planejar a semana" }));
  expect(servico.abrirConversa).toHaveBeenCalledWith("planejar", { plataforma: "codex" });
  expect(await screen.findByText("sessao:s-9")).toBeInTheDocument();
  // A coluna aberta fica guardada (volta aberta ao reabrir a tela).
  expect(JSON.parse(localStorage.getItem("medza-rotina")!).conversaAberta).toBe(true);
});

it("treino: + Série grava com a carga e as repetições da última vez, e os botões ajustam", async () => {
  vi.mocked(servico.operar).mockResolvedValue({ recorde: false, serie_do_exercicio: 1 });
  render(<Treino fichaEscolhida="" onFicha={() => undefined} onErro={() => undefined} />);
  await screen.findByRole("button", { name: "+ Série" });
  expect(screen.getByText(/Última vez 1× 60 kg × 10/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "+ Série" }));
  expect(servico.operar).toHaveBeenCalledWith("registrar_serie", { exercicio: "Supino reto", carga_kg: 60, repeticoes: 10, ficha: "fa" });
  await userEvent.click(screen.getByRole("button", { name: "Mais 2,5 kg" }));
  await userEvent.click(screen.getByRole("button", { name: "Menos uma repetição" }));
  await userEvent.click(screen.getByRole("button", { name: "+ Série" }));
  expect(servico.operar).toHaveBeenLastCalledWith("registrar_serie", { exercicio: "Supino reto", carga_kg: 62.5, repeticoes: 9, ficha: "fa" });
  expect(await screen.findByText("Supino reto: série 1 registrada.")).toBeInTheDocument();
});

it("treino: recorde aparece no aviso", async () => {
  vi.mocked(servico.operar).mockResolvedValue({ recorde: true, serie_do_exercicio: 2 });
  render(<Treino fichaEscolhida="fa" onFicha={() => undefined} onErro={() => undefined} />);
  await screen.findByRole("button", { name: "+ Série" });
  await userEvent.click(screen.getByRole("button", { name: "+ Série" }));
  expect(await screen.findByText("Recorde em Supino reto: 60 kg!")).toBeInTheDocument();
});

it("aviso discreto responde Feito e Planejar abre a Rotina", async () => {
  const agora = Date.now();
  vi.mocked(servico.lerAvisos).mockResolvedValue({ nao_vistos: 2, avisos: [
    { chave: "agua:x", tipo: "agua", titulo: "Beba água", texto: "0,5 L de 2,5 L hoje", em_ms: agora, alvo: "agua", data: "", botoes: ["copo", "adiar", "pular"], enviado_ms: agora, visto: false, resposta: "" },
    { chave: "planejar:2026-11-02", tipo: "planejar", titulo: "Hora de planejar a semana", texto: "", em_ms: agora, alvo: "", data: "", botoes: ["planejar", "adiar"], enviado_ms: agora, visto: false, resposta: "" },
    { chave: "velho", tipo: "agua", titulo: "Velho", texto: "", em_ms: 0, alvo: "", data: "", botoes: [], enviado_ms: 1, visto: false, resposta: "" },
  ] });
  const pedidos: unknown[] = [];
  const parar = ouvirPedidosDaRotina((p) => pedidos.push(p));
  render(<AvisosDaRotina intervaloMs={60_000} />);
  expect(await screen.findByText("Beba água")).toBeInTheDocument();
  expect(screen.queryByText("Velho")).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "+1 copo" }));
  expect(servico.operar).toHaveBeenCalledWith("responder_aviso", { chave: "agua:x", acao: "copo", minutos: 15 });
  await userEvent.click(screen.getByRole("button", { name: "Planejar agora" }));
  expect(pedidos).toEqual([{ aba: "semana", conversa: "planejar" }]);
  parar();
});

it("engine sem a Rotina: sem avisos e sem erro", async () => {
  vi.mocked(servico.lerAvisos).mockRejectedValue(new Error("404"));
  const { container } = render(<AvisosDaRotina intervaloMs={60_000} />);
  await waitFor(() => expect(servico.lerAvisos).toHaveBeenCalled());
  expect(container).toBeEmptyDOMElement();
});
