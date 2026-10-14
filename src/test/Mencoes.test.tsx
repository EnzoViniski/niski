import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { Composer, mencaoNoCursor, textoDaMencao } from "../mobile/Composer";

describe("menção de arquivo (@)", () => {
  it("acha o @ que está sendo digitado", () => {
    expect(mencaoNoCursor("veja @src/Ap", 12)).toEqual({ inicio: 5, busca: "src/Ap" });
    expect(mencaoNoCursor("@", 1)).toEqual({ inicio: 0, busca: "" });
    expect(mencaoNoCursor("email@dominio", 13)).toBeNull();
    expect(mencaoNoCursor("veja @src/App.tsx e mais", 24)).toBeNull();
    expect(textoDaMencao("docs/meu arquivo.md")).toBe('@"docs/meu arquivo.md" ');
  });

  it("digitar @ lista os arquivos, setas escolhem e Enter insere sem enviar", async () => {
    const onEnviar = vi.fn();
    const buscar = vi.fn(async (busca: string) => ["src/App.tsx", "src/app/page.tsx"].filter((c) => c.toLowerCase().includes(busca.toLowerCase())));
    function Teste() {
      const [texto, setTexto] = useState("");
      return <Composer texto={texto} onTexto={setTexto} anexos={[]} onAdicionar={vi.fn()} onRemover={vi.fn()}
        onEnviar={onEnviar} podeEnviar ocupado={false} placeholder="msg" buscarArquivos={buscar} />;
    }
    render(<Teste />);
    const campo = screen.getByLabelText("Sua resposta") as HTMLTextAreaElement;
    await act(async () => { await userEvent.type(campo, "abra @app"); });
    expect(await screen.findByRole("option", { name: /page\.tsx/ })).toBeInTheDocument();
    expect(screen.getAllByRole("option")).toHaveLength(2);
    await act(async () => { await userEvent.keyboard("{ArrowDown}"); });
    await act(async () => { await userEvent.keyboard("{Enter}"); });
    expect(campo.value).toBe("abra @src/app/page.tsx ");
    expect(onEnviar).not.toHaveBeenCalled();
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });
});
