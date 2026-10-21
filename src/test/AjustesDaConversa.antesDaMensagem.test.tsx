// Fork (chat-estavel): Codex recém-aberto troca modelo/esforço antes da primeira mensagem, e reler
// as opções quando o agente volta a ficar pronto não desfaz o que acabou de ser aplicado.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AjustesDaConversa } from "../mobile/AjustesDaConversa";

const CODEX = {
  provider: "codex", profile_id: "codex-padrao", model: "gpt-x", effort: "medium", can_resume: false, can_adjust: true,
  profiles: [{ id: "codex-padrao", name: "Padrão" }],
  models: [{ id: "gpt-x", label: "GPT X", efforts: ["low", "medium", "high"] }, { id: "gpt-y", label: "GPT Y", efforts: ["low", "high"] }],
};

let resposta: Record<string, unknown> = CODEX;
const fetchFalso = vi.fn(async () => new Response(JSON.stringify(resposta)));
beforeEach(() => { resposta = CODEX; fetchFalso.mockClear(); vi.stubGlobal("fetch", fetchFalso); });

// Fork (seletor no estilo do Codex): o botão abre o painel; o nome do modelo leva à lista.
const gatilho = () => document.querySelector<HTMLButtonElement>(".seletor-gatilho")!;
async function escolherModelo(opcao: string) {
  await act(async () => { await userEvent.click(gatilho()); });
  await act(async () => { await userEvent.click(document.querySelector<HTMLButtonElement>(".seletor-modelo-link")!); });
  await act(async () => { await userEvent.click(screen.getByRole("option", { name: opcao })); });
}

describe("AjustesDaConversa antes da primeira mensagem", () => {
  it("Codex sem mensagem libera a troca quando o engine diz can_adjust", async () => {
    const onAplicar = vi.fn().mockResolvedValue(undefined);
    render(<AjustesDaConversa conversaId="c" pronto onAplicar={onAplicar} />);
    expect(await screen.findByText("GPT X")).toBeInTheDocument();
    expect(gatilho()).not.toBeDisabled();
    await escolherModelo("GPT Y");
    await act(async () => { await userEvent.click(screen.getByRole("button", { name: /Aplicar/ })); });
    expect(onAplicar).toHaveBeenCalledWith({ profile_id: "codex-padrao", model: "gpt-y", effort: "medium" });
  });

  it("engine antigo (sem can_adjust) continua pedindo a primeira mensagem", async () => {
    resposta = { ...CODEX, can_adjust: undefined };
    render(<AjustesDaConversa conversaId="c" pronto onAplicar={vi.fn()} />);
    await screen.findByText("GPT X");
    expect(gatilho()).toBeDisabled();
  });

  it("relê ao ficar pronto e mantém o aplicado até o engine mostrar outra leitura", async () => {
    const onAplicar = vi.fn().mockResolvedValue(undefined);
    const { rerender } = render(<AjustesDaConversa conversaId="c" pronto onAplicar={onAplicar} />);
    await screen.findByText("GPT X");
    await escolherModelo("GPT Y");
    await act(async () => { await userEvent.click(screen.getByRole("button", { name: /Aplicar/ })); });
    // Reabrindo: some o pronto e volta; o engine ainda lê o modelo antigo.
    rerender(<AjustesDaConversa conversaId="c" pronto={false} onAplicar={onAplicar} />);
    rerender(<AjustesDaConversa conversaId="c" pronto onAplicar={onAplicar} />);
    await act(async () => { await Promise.resolve(); });
    expect(fetchFalso.mock.calls.length).toBeGreaterThanOrEqual(3);
    expect(screen.getByText("GPT Y")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Aplicar/ })).not.toBeInTheDocument();
    // A leitura muda (a primeira resposta com o modelo novo): vale o engine.
    resposta = { ...CODEX, model: "gpt-y", effort: "high" };
    rerender(<AjustesDaConversa conversaId="c" pronto={false} onAplicar={onAplicar} />);
    await act(async () => { await Promise.resolve(); });
    expect(screen.getByText("Alto")).toBeInTheDocument();
  });
});
