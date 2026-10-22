import { beforeEach, describe, expect, it } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TarefasView } from "./TarefasView";
import { CHAVE_DOS_AJUSTES, recarregar, reiniciarParaTeste, salvar, transporteEmMemoria, usarTransporteDasTarefas, type TransporteDasTarefas } from "./loja";
import { criarTarefa, diaDe } from "./dominio";

let pasta: TransporteDasTarefas;

beforeEach(() => {
  localStorage.clear();
  pasta = transporteEmMemoria();
  usarTransporteDasTarefas(pasta);
  reiniciarParaTeste();
});

function comNome(nome = "Enzo") {
  localStorage.setItem(CHAVE_DOS_AJUSTES, JSON.stringify({ eu: { id: "enzo", nome, cor: "#5b8def", atualizadaEm: "" } }));
  reiniciarParaTeste();
}

describe("Tarefas", () => {
  it("primeiro uso pede o nome e publica a pessoa na pasta", async () => {
    render(<TarefasView onClose={() => {}} onAbrirAjustes={() => {}} />);
    await userEvent.type(screen.getByLabelText(/como você se chama/i), "Enzo{Enter}");
    expect(await screen.findByRole("heading", { name: "Tarefas" })).toBeInTheDocument();
    await act(async () => { await Promise.resolve(); });
    expect((await pasta.listar(null, "pessoas")).map((p) => JSON.parse(p).nome)).toEqual(["Enzo"]);
  });

  it("cria pela frase, aparece em Hoje, conclui e grava no disco", async () => {
    comNome();
    render(<TarefasView onClose={() => {}} onAbrirAjustes={() => {}} />);
    await screen.findByText(/nenhuma tarefa aberta para hoje/i);
    await userEvent.type(screen.getByLabelText("Nova tarefa"), "Tomar remédio hoje 23:59 #saude !alta");
    expect(screen.getByText("Vai criar:")).toBeInTheDocument();
    await userEvent.keyboard("{Enter}");
    const cartao = (await screen.findByText("Tomar remédio")).closest("[data-tarefa]") as HTMLElement;
    expect(within(cartao).getByText(/Alta/)).toBeInTheDocument();
    await userEvent.click(within(cartao).getByRole("button", { name: /concluir "Tomar remédio"/i }));
    await act(async () => { await Promise.resolve(); });
    const noDisco = (await pasta.listar(null, "tarefas")).map((t) => JSON.parse(t));
    expect(noDisco).toHaveLength(1);
    expect(noDisco[0]).toMatchObject({ titulo: "Tomar remédio", status: "feito", categoria: "saude", data: diaDe(new Date()) });
  });

  it("abre a gaveta, adiciona passo no checklist e comentário", async () => {
    comNome();
    await salvar(criarTarefa({ titulo: "Matrícula da faculdade", data: diaDe(new Date()) }, "enzo"));
    render(<TarefasView onClose={() => {}} onAbrirAjustes={() => {}} />);
    await userEvent.click(await screen.findByText("Matrícula da faculdade"));
    const gaveta = screen.getByRole("complementary", { name: /detalhes da tarefa/i });
    await userEvent.type(within(gaveta).getByPlaceholderText(/adicionar passo/i), "Levar RG{Enter}");
    await userEvent.type(within(gaveta).getByPlaceholderText(/escreva um comentário/i), "Prazo até sexta");
    await userEvent.click(within(gaveta).getByRole("button", { name: "Comentar" }));
    const [t] = (await pasta.listar(null, "tarefas")).map((x) => JSON.parse(x));
    expect(t.checklist.map((i: { texto: string }) => i.texto)).toEqual(["Levar RG"]);
    expect(t.comentarios[0]).toMatchObject({ autor: "enzo", texto: "Prazo até sexta" });
  });

  it("tarefa criada pela outra pessoa na pasta aparece ao reler, com quem é o responsável", async () => {
    comNome();
    render(<TarefasView onClose={() => {}} onAbrirAjustes={() => {}} />);
    await screen.findByText(/nenhuma tarefa aberta para hoje/i);
    await pasta.gravar(null, "pessoas", "primo", JSON.stringify({ id: "primo", nome: "Primo", cor: "#e0679a" }));
    await pasta.gravar(null, "tarefas", "t-1", JSON.stringify(criarTarefa({ id: "t-1", titulo: "Revisar contrato", data: diaDe(new Date()), responsavel: "enzo" }, "primo")));
    await act(async () => { await recarregar(); });
    expect(await screen.findByText("Revisar contrato")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /primo/i })).toBeInTheDocument(); // filtro por pessoa
  });

  it("quadro mostra as quatro colunas", async () => {
    comNome();
    localStorage.setItem(CHAVE_DOS_AJUSTES, JSON.stringify({ eu: { id: "enzo", nome: "Enzo", cor: "#000" }, visao: "quadro" }));
    reiniciarParaTeste();
    render(<TarefasView onClose={() => {}} onAbrirAjustes={() => {}} />);
    for (const coluna of ["A fazer", "Fazendo", "Esperando", "Feito"]) expect(await screen.findByRole("region", { name: coluna })).toBeInTheDocument();
  });
});
