import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "./interacoesTelas";
import { invoke } from "@tauri-apps/api/core";
import { RelatoriosIa } from "../features/relatorios/RelatoriosIa";

const relatorio = {
  uso: {
    dias: [{ data: "2026-10-19", tokens: 1200, saida: 200, cache_lido: 1200, prompts: 4, respostas: 3, conversas: 2 }],
    por_conta: [{ nome: "Conta sintética", provider: "claude", tokens: 1200, prompts: 4 }],
    por_modelo: [{ nome: "modelo-teste", tokens: 1200, prompts: 4 }],
    por_projeto: [{ nome: "Projeto sintético", tokens: 1200, prompts: 4 }],
    por_hora: Array.from({ length: 24 }, (_, h) => h === 9 ? 4 : 0),
  },
  commits: [{ caminho: "/fake/projeto", commits: [
    { em_ms: new Date("2026-10-19T15:00:00Z").getTime(), ia: true, linhas: 20, removidas: 5 },
    { em_ms: new Date("2026-10-19T16:00:00Z").getTime(), ia: false, linhas: 2, removidas: 0 },
  ] }],
};
beforeEach(() => { localStorage.clear(); vi.mocked(invoke).mockReset().mockResolvedValue(relatorio); });
it("mostra os totais, contas, modelos, projetos e contribuição da IA", async () => {
  render(<RelatoriosIa projectPaths={["/fake/projeto"]} />);
  const resumo = await screen.findByRole("region", { name: "Resumo" });
  expect(within(resumo).getAllByText("1.200")[0]).toBeInTheDocument();
  expect(within(resumo).getByText(/50% do contexto veio do cache/)).toBeInTheDocument();
  expect(screen.getByText("Conta sintética")).toBeInTheDocument();
  expect(screen.getByText("modelo-teste")).toBeInTheDocument();
  expect(screen.getByText("Projeto sintético")).toBeInTheDocument();
  expect(screen.getByText("1 de 2 commits pela IA")).toBeInTheDocument();
  expect(screen.getByText("Pico às 9h")).toBeInTheDocument();
  expect(screen.getByRole("img", { name: "Tokens por dia nos últimos 7 dias" })).toBeInTheDocument();
});
it("troca para 30 dias e relê os projetos abertos", async () => {
  render(<RelatoriosIa projectPaths={["/fake/projeto"]} />);
  await screen.findByRole("region", { name: "Resumo" });
  await userEvent.click(screen.getByRole("button", { name: "Últimos 30 dias" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("relatorio_ia", expect.objectContaining({ dias: 30, projetos: ["/fake/projeto"] })));
  expect(await screen.findByRole("img", { name: "Tokens por dia nos últimos 30 dias" })).toBeInTheDocument();
});
it("atualiza sob demanda", async () => {
  render(<RelatoriosIa projectPaths={[]} />);
  await screen.findByRole("region", { name: "Resumo" });
  await userEvent.click(screen.getByRole("button", { name: "Atualizar" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
  expect(await screen.findByText(/atualizado agora/)).toBeInTheDocument();
});
it("mostra falha da primeira leitura e permite recuperar", async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error("Relatório indisponível"));
  render(<RelatoriosIa projectPaths={[]} />);
  expect(await screen.findByRole("alert")).toHaveTextContent("Relatório indisponível");
  expect(screen.getByText("Sem dados.")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Atualizar" }));
  expect(await screen.findByRole("region", { name: "Resumo" })).toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});
it("preserva o relatório guardado se a atualização falhar", async () => {
  localStorage.setItem("omni-relatorio-v1-7", JSON.stringify({ em: Date.now(), dados: relatorio }));
  vi.mocked(invoke).mockRejectedValue(new Error("Sem conexão com o engine"));
  render(<RelatoriosIa projectPaths={[]} />);
  expect(screen.getByText("Conta sintética")).toBeInTheDocument();
  expect(await screen.findByRole("alert")).toHaveTextContent("Sem conexão com o engine");
  expect(screen.getByRole("region", { name: "Resumo" })).toBeInTheDocument();
});
it("explica listas vazias sem mostrar números inválidos", async () => {
  vi.mocked(invoke).mockResolvedValue({ uso: { dias: [], por_conta: [], por_modelo: [], por_projeto: [], por_hora: Array(24).fill(0) }, commits: [] });
  render(<RelatoriosIa projectPaths={[]} />);
  await screen.findByRole("region", { name: "Resumo" });
  expect(screen.getAllByText("Nenhum uso no período.")).toHaveLength(3);
  expect(screen.getByText("Nenhum commit nos projetos abertos no período.")).toBeInTheDocument();
  expect(screen.queryByText(/NaN|Infinity/)).not.toBeInTheDocument();
});
