// Lista do `/` no campo da Conversa (sem engine: a lista vem de uma função falsa).
import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { Composer } from "../mobile/Composer";
import { barraNoCursor, filtrarItens, interpretarEnvio, type ItemDaBarra } from "../mobile/conversa/comandosDaBarra";

const ITENS: ItemDaBarra[] = [
  { nome: "compact", descricao: "Resume a conversa", tipo: "cli", origem: "cli", insere: "/compact" },
  { nome: "medza-code-review", descricao: "Revisar diffs", tipo: "skill", origem: "conta", insere: "/medza-code-review " },
  { nome: "revisar-front", descricao: "Comando do projeto", tipo: "comando", origem: "projeto", insere: "/revisar-front " },
];

describe("comandos do / (regras)", () => {
  it("só abre no começo da mensagem e antes do espaço", () => {
    expect(barraNoCursor("/", 1)).toEqual({ busca: "" });
    expect(barraNoCursor("/rev", 4)).toEqual({ busca: "rev" });
    expect(barraNoCursor("veja /rev", 9)).toBeNull();
    expect(barraNoCursor("/revisar agora", 14)).toBeNull();
    expect(barraNoCursor("/src/app", 8)).toBeNull();
  });

  it("filtra pelo começo do nome antes do resto", () => {
    expect(filtrarItens(ITENS, "rev").map((i) => i.nome)).toEqual(["revisar-front", "medza-code-review"]);
    expect(filtrarItens(ITENS, "diffs").map((i) => i.nome)).toEqual(["medza-code-review"]);
    expect(filtrarItens(ITENS, "")).toHaveLength(3);
  });

  it("separa comando da CLI de skill no envio", () => {
    expect(interpretarEnvio("/compact")).toEqual({ tipo: "oficial", comando: "compact" });
    expect(interpretarEnvio(" /fast on ")).toEqual({ tipo: "oficial", comando: "fast", argumento: "on" });
    expect(interpretarEnvio("/medza-code-review o diff")).toEqual({ tipo: "barra", texto: "/medza-code-review o diff" });
    expect(interpretarEnvio("olá /compact")).toBeNull();
  });
});

describe("lista do / no Composer", () => {
  function montar(onEnviar = vi.fn()) {
    const buscar = vi.fn(async () => ITENS);
    function Teste() {
      const [texto, setTexto] = useState("");
      return <Composer texto={texto} onTexto={setTexto} anexos={[]} onAdicionar={vi.fn()} onRemover={vi.fn()}
        onEnviar={onEnviar} podeEnviar={texto.trim().length > 0} ocupado={false} placeholder="msg" buscarComandos={buscar} />;
    }
    render(<Teste />);
    return { campo: screen.getByLabelText("Sua resposta") as HTMLTextAreaElement, buscar, onEnviar };
  }

  it("digitar / mostra comandos e skills; Enter completa sem enviar", async () => {
    const { campo, onEnviar } = montar();
    await act(async () => { await userEvent.type(campo, "/"); });
    expect(await screen.findByRole("option", { name: /medza-code-review/ })).toBeInTheDocument();
    expect(screen.getAllByRole("option")).toHaveLength(3);
    await act(async () => { await userEvent.type(campo, "med"); });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    await act(async () => { await userEvent.keyboard("{Enter}"); });
    expect(campo.value).toBe("/medza-code-review ");
    expect(onEnviar).not.toHaveBeenCalled();
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("comando já completo: Enter envia; Esc fecha a lista", async () => {
    const { campo, onEnviar } = montar();
    await act(async () => { await userEvent.type(campo, "/compact"); });
    expect(await screen.findByRole("option", { name: /compact/ })).toBeInTheDocument();
    await act(async () => { await userEvent.keyboard("{Enter}"); });
    expect(onEnviar).toHaveBeenCalledTimes(1);

    await act(async () => { await userEvent.clear(campo); await userEvent.type(campo, "/"); });
    expect(await screen.findByRole("listbox")).toBeInTheDocument();
    await act(async () => { await userEvent.keyboard("{Escape}"); });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("sem buscarComandos, / é só texto", async () => {
    function Teste() {
      const [texto, setTexto] = useState("");
      return <Composer texto={texto} onTexto={setTexto} anexos={[]} onAdicionar={vi.fn()} onRemover={vi.fn()}
        onEnviar={vi.fn()} podeEnviar ocupado={false} placeholder="msg" />;
    }
    render(<Teste />);
    await act(async () => { await userEvent.type(screen.getByLabelText("Sua resposta"), "/"); });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });
});
