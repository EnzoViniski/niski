import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AjustesDaConversa } from "../mobile/AjustesDaConversa";

const OPCOES = {
  provider: "claude", profile_id: "emailuber", model: "claude-opus-5-5", effort: "medium", can_resume: true,
  profiles: [{ id: "claude-padrao", name: "Padrão" }, { id: "emailuber", name: "EmailUber" }],
  models: ["fable", "opus", "sonnet", "haiku"].map((id) => ({ id, label: id[0].toUpperCase() + id.slice(1), efforts: ["low", "medium", "high", "xhigh", "max"] })),
};

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify(OPCOES))));
});

describe("AjustesDaConversa", () => {
  it("mostra conta, modelo e esforço atuais e aplica sozinho ao trocar a conta (sem botão Aplicar)", async () => {
    const onAplicar = vi.fn().mockResolvedValue(undefined);
    render(<AjustesDaConversa conversaId="c" pronto onAplicar={onAplicar} />);

    // O transcript grava `claude-opus-5-5`; a lista oferece o apelido `opus`.
    expect(await screen.findByText("Opus")).toBeInTheDocument();
    expect(screen.getByText("EmailUber")).toBeInTheDocument();
    expect(screen.getByText("Médio")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Aplicar/ })).not.toBeInTheDocument();

    const [conta] = screen.getAllByRole("combobox");
    await act(async () => { await userEvent.click(conta); });
    await act(async () => { await userEvent.click(screen.getByRole("option", { name: "Padrão" })); });
    expect(screen.queryByRole("button", { name: /Aplicar/ })).not.toBeInTheDocument();

    expect(onAplicar).toHaveBeenCalledWith({ profile_id: "claude-padrao", model: "opus", effort: "medium" });
  });

  it("com o agente trabalhando, nada se troca", async () => {
    render(<AjustesDaConversa conversaId="c" pronto={false} onAplicar={vi.fn()} />);
    await screen.findByText("Opus");
    for (const campo of screen.getAllByRole("combobox")) expect(campo).toBeDisabled();
  });

  /** O caso do relato: front novo (HMR), engine antigo ainda rodando — a faixa sumia sem dizer nada. */
  it("engine antigo sem a rota explica em vez de sumir", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response("Not Found", { status: 404 })));
    render(<AjustesDaConversa conversaId="c" pronto onAplicar={vi.fn()} />);
    expect(await screen.findByText(/reinicie o Medza/i)).toBeInTheDocument();
  });
});
