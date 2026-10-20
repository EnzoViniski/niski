import { render, screen } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { NoDoFluxoView, type DadosDoNo } from "../features/fluxos/NoDoFluxo";
import type { EstadoDoNo, TipoDeNo } from "../features/fluxos/fluxosService";

function montar(tipo: TipoDeNo, parte: Partial<DadosDoNo> = {}) {
  const data: DadosDoNo = { no: { id: "no", tipo, titulo: "Etapa de teste", config: {}, x: 0, y: 0 }, ...parte };
  render(<ReactFlowProvider><NoDoFluxoView id="no" type="omni" data={data} selected={false} dragging={false} draggable selectable deletable isConnectable zIndex={0} positionAbsoluteX={0} positionAbsoluteY={0} /></ReactFlowProvider>);
}
it.each([
  ["entrada", "Entrada", "a tarefa que você digita"], ["agente", "Agente", "escolha um agente"],
  ["condicao", "Condição", "lê o veredito · até 3 voltas"], ["aprovacao", "Aprovação", "espera você aprovar"],
  ["sala", "Time", null], ["pr", "Pull request", "commit, push e gh pr create"], ["fim", "Fim", null],
] as const)("identifica o nó %s e explica sua função", (tipo, nome, explicacao) => {
  montar(tipo);
  expect(screen.getByText(nome)).toBeInTheDocument();
  expect(screen.getByText("Etapa de teste")).toBeInTheDocument();
  if (explicacao) expect(screen.getByText(explicacao)).toBeInTheDocument();
});
it("mostra as três saídas visíveis da condição", () => {
  montar("condicao");
  for (const nome of ["sim", "não", "esgotou"]) expect(screen.getByText(nome)).toBeInTheDocument();
});
const estado: EstadoDoNo = { status: "rodando", voltas: 2, inicio_ms: 1000, fim_ms: 0, saida: "", session_id: null, custo_usd: 0, erro: null, cli: "claude", conta: null, caminho: null, atividade: "Conferindo o formulário" };
it("mostra agente, modelo, esforço e atividade enquanto trabalha", () => {
  montar("agente", { no: { id: "no", tipo: "agente", titulo: "Revisar", agente: "revisor", config: {}, x: 0, y: 0 }, cli: "claude", modelo: "modelo-teste", esforco: "high", estado });
  expect(screen.getByText("revisor · modelo-teste · alto")).toBeInTheDocument();
  expect(screen.getByText("rodando")).toBeInTheDocument();
  expect(screen.getByText("Conferindo o formulário")).toBeInTheDocument();
  expect(screen.getByText("2ª vez")).toBeInTheDocument();
});
it("mostra duração, custo e conclusão sem atividade antiga", () => {
  montar("agente", { estado: { ...estado, status: "concluido", fim_ms: 66000, custo_usd: 0.25 } });
  expect(screen.getByText("feito")).toBeInTheDocument();
  expect(screen.getByText("1min 5s")).toBeInTheDocument();
  expect(screen.getByText("US$ 0.25")).toBeInTheDocument();
  expect(screen.queryByText("Conferindo o formulário")).not.toBeInTheDocument();
});
it("avisa quando é a vez do usuário aprovar", () => {
  montar("aprovacao", { estado: { ...estado, status: "aguardando", voltas: 0 } });
  expect(screen.getByText("sua vez")).toBeInTheDocument();
  expect(screen.getByText("sim")).toBeInTheDocument();
  expect(screen.getByText("não")).toBeInTheDocument();
});

it("explica o objetivo nativo e mostra limite", () => {
  montar("objetivo");
  expect(screen.getByText("Objetivo /goal")).toBeInTheDocument();
  expect(screen.getByText("escolha um agente Claude · até 10 turnos")).toBeInTheDocument();
});
it("mostra corpo e fim da repetição com limite e intervalo", () => {
  montar("repetir", { no:{ id:"r",tipo:"repetir",titulo:"Rever",config:{vezes:4,intervalo_minutos:5},x:0,y:0 } });
  expect(screen.getByText("até 4 vezes · a cada 5 min")).toBeInTheDocument();
  expect(screen.getByText("corpo")).toBeInTheDocument();
  expect(screen.getByText("fim")).toBeInTheDocument();
});
