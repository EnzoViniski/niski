import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ListaDeAtividade } from "./ListaDeAtividade";
import { ConfiguracaoDeAvisos } from "./ConfiguracaoDeAvisos";
import { CaixaDeAtividade } from "./CaixaDeAtividade";
import { Atividade } from "../../mobile/telas/Atividade";
import { DEFAULT_SETTINGS } from "../../types/settings";
import { preferenciasDeAvisos, type ItemDeAtividade } from "./tipos";
import { AcoesDaPaleta, useListaDeAcoes } from "../paleta/acoes";
import { setShortcutOverrides } from "../../lib/shortcuts";
import { useAberturasDaAtividade } from "./navegacao";
import { lerRota, urlDaRota } from "../../mobile/rota";
const feed = vi.hoisted(() => ({ itens: [] as ItemDeAtividade[], carregado: true, erro: null as string | null, marcar: vi.fn(), atualizar: vi.fn() }));
vi.mock("./atividadeService", () => ({ useAtividade: () => feed }));
function item(tipo: ItemDeAtividade["tipo"], id = tipo): ItemDeAtividade {
  return { id, conversa: id, sessao: id, projeto_id: "p", projeto: "Loja", titulo: `Conversa ${id}`, plataforma: "codex",
    conta: "Equipe", conta_id: "conta", tipo, desde_ms: Date.now()-120_000, visto: false, resumo: tipo, orquestrador: null };
}
beforeEach(() => { vi.clearAllMocks(); feed.itens = [item("precisa_de_voce"), item("terminou"), item("erro"), item("subagente_terminou"), item("limite")]; feed.marcar.mockResolvedValue(undefined); setShortcutOverrides({}); });
describe("Caixa Atividade", () => {
  it("mostra projeto, agente, conta e tempo e filtra os tipos", async () => {
    render(<ListaDeAtividade {...feed} abrir={vi.fn()} />);
    expect(screen.getAllByText(/Loja · Codex · Equipe/)).toHaveLength(5);
    expect(screen.getAllByText("há 2 min")).toHaveLength(5);
    await userEvent.click(screen.getByRole("button", { name: "Terminou" }));
    expect(screen.getByText("Conversa subagente_terminou")).toBeInTheDocument(); expect(screen.queryByText("Conversa erro")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Erro" }));
    expect(screen.getByText("Conversa limite")).toBeInTheDocument();
  });
  it("marca tudo, inclusive fora do filtro, e clique abre a conversa depois de guardar visto", async () => {
    const abrir = vi.fn(); render(<ListaDeAtividade {...feed} abrir={abrir} />);
    await userEvent.click(screen.getByRole("button", { name: "Precisa de você" }));
    await userEvent.click(screen.getByRole("button", { name: "Marcar tudo como lido" }));
    expect(feed.marcar).toHaveBeenCalledWith(feed.itens.map(i => i.id));
    await userEvent.click(screen.getByRole("button", { name: /Conversa precisa_de_voce/ }));
    expect(abrir).toHaveBeenCalledWith(feed.itens[0]);
  });
  it("falha ao guardar leitura é visível e não abre silenciosamente", async () => {
    feed.marcar.mockRejectedValue(new Error("disco")); const abrir = vi.fn();
    render(<ListaDeAtividade {...feed} abrir={abrir} />);
    await userEvent.click(screen.getByRole("button", { name: /Conversa precisa_de_voce/ }));
    expect(screen.getByRole("alert")).toHaveTextContent(/Não foi possível marcar/); expect(abrir).not.toHaveBeenCalled();
  });
  it("configura foco, desligado e som separadamente e preserva os outros tipos", async () => {
    const onChange = vi.fn(); render(<ConfiguracaoDeAvisos settings={DEFAULT_SETTINGS} onChange={onChange} />);
    await userEvent.selectOptions(screen.getByLabelText("Precisa de você"), "sempre");
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ avisos: expect.objectContaining({ precisa_de_voce: { quando: "sempre", som: "Glass" }, erro: DEFAULT_SETTINGS.avisos.erro }) }));
    await userEvent.selectOptions(screen.getByLabelText("Som: Erro"), "");
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ avisos: expect.objectContaining({ erro: { quando: "fora_de_foco", som: null } }) }));
    await userEvent.selectOptions(screen.getByLabelText("Terminou o turno"), "nunca");
    expect(onChange.mock.lastCall?.[0].avisos.terminou.quando).toBe("nunca");
    expect(preferenciasDeAvisos({ terminou: { quando: "nunca", som: null } }).erro.som).toBe("Basso");
  });
  it("preferência antiga desligada só religa o tipo escolhido", async () => {
    const onChange = vi.fn(); render(<ConfiguracaoDeAvisos settings={{ ...DEFAULT_SETTINGS, notifyAttention: false }} onChange={onChange} />);
    await userEvent.selectOptions(screen.getByLabelText("Precisa de você"), "sempre");
    expect(onChange.mock.lastCall?.[0]).toEqual(expect.objectContaining({ notifyAttention: true,
      avisos: expect.objectContaining({ precisa_de_voce: { quando: "sempre", som: "Glass" }, terminou: { quando: "nunca", som: "Ping" } }) }));
  });
  it("contador, paleta, atalho reconfigurado, foco e abertura por todos os projetos", async () => {
    const abrir = vi.fn();
    function Harness() {
      useAberturasDaAtividade(abrir); const acoes = useListaDeAcoes();
      return <><CaixaDeAtividade />{acoes.map(a => <button key={a.id} onClick={() => a.executar()}>Paleta: {a.titulo}</button>)}</>;
    }
    render(<AcoesDaPaleta><Harness /></AcoesDaPaleta>);
    expect(screen.getByRole("button", { name: "Atividade (5 não lidos)" })).toBeInTheDocument();
    await userEvent.click(await screen.findByRole("button", { name: "Paleta: Abrir Atividade" }));
    const painel = screen.getByRole("dialog", { name: "Atividade" }); expect(within(painel).getByRole("button", { name: "Fechar atividade" })).toHaveFocus();
    await userEvent.click(within(painel).getByRole("button", { name: /Conversa erro/ }));
    await waitFor(() => expect(abrir).toHaveBeenCalledWith(expect.objectContaining({ id: "erro", project_id: "p" })));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    setShortcutOverrides({ atividade: "Ctrl+Shift+Y" });
    act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Y", code: "KeyY", ctrlKey: true, shiftKey: true })));
    expect(screen.getByRole("dialog")).toBeInTheDocument(); await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
  it("celular usa a mesma lista e abre pelo id da conversa; a rota sobrevive ao recarregar", async () => {
    const abrirConversa = vi.fn(); render(<Atividade abrirConversa={abrirConversa} />);
    await userEvent.click(screen.getByRole("button", { name: /Conversa erro/ })); expect(abrirConversa).toHaveBeenCalledWith("erro");
    expect(lerRota("#/atividade")).toEqual({ tela: "atividade" }); expect(urlDaRota({ tela: "atividade" })).toMatch(/#\/atividade$/);
  });
  it("captura o atalho antes do terminal e não deixa a tecla chegar à PTY", () => {
    const pty = vi.fn(); render(<><textarea aria-label="Terminal" onKeyDown={pty} /><CaixaDeAtividade /></>);
    act(() => { screen.getByLabelText("Terminal").dispatchEvent(new KeyboardEvent("keydown", { key: "u", code: "KeyU", ctrlKey: true, altKey: true, bubbles: true, cancelable: true })); });
    expect(screen.getByRole("dialog", { name: "Atividade" })).toBeInTheDocument(); expect(pty).not.toHaveBeenCalled();
  });
});
