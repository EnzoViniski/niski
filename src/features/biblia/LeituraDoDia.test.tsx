import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { definirBibliaParaTestes } from "./arquivo";
import { lerBiblia } from "./biblia";
import { arquivoDeTeste } from "./bibliaDeTeste";
import { estadoDaLeitura, leituraDeAgora, recarregarDaBiblia } from "./leitura";
import { LeituraDoDia } from "./LeituraDoDia";

const biblia = lerBiblia(arquivoDeTeste({ capitulos: 5 }));

beforeEach(() => { localStorage.clear(); recarregarDaBiblia(); definirBibliaParaTestes(biblia); });

describe("Palavra do dia (tela)", () => {
  it("mostra uma passagem por trilha e 'Li' registra e avança", async () => {
    const antes = leituraDeAgora(biblia).map((l) => l.passagem.id);
    render(<LeituraDoDia onFechar={() => {}} onAjustes={() => {}} />);
    expect(screen.getByRole("heading", { name: "São Mateus 1, 1-6" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Salmo 1" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Li ✓" }));
    expect(await screen.findByText("Amém 🙏")).toBeInTheDocument();
    expect(estadoDaLeitura().hoje?.estado).toBe("lida");
    expect(estadoDaLeitura().historico[estadoDaLeitura().historico.length - 1]?.ids).toEqual(antes);
    expect(leituraDeAgora(biblia).map((l) => l.passagem.id).some((id) => antes.includes(id))).toBe(false);
  });

  it("'Deixar para amanhã' fecha sem avançar", async () => {
    const fechar = vi.fn();
    const antes = leituraDeAgora(biblia).map((l) => l.passagem.id);
    render(<LeituraDoDia onFechar={fechar} onAjustes={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: "Deixar para amanhã" }));
    expect(fechar).toHaveBeenCalled();
    expect(estadoDaLeitura().hoje?.estado).toBe("amanha");
    expect(leituraDeAgora(biblia).map((l) => l.passagem.id)).toEqual(antes);
  });

  it("'Ler mais tarde' e Esc guardam a leitura para depois", async () => {
    const fechar = vi.fn();
    render(<LeituraDoDia onFechar={fechar} onAjustes={() => {}} />);
    await userEvent.keyboard("{Escape}");
    expect(fechar).toHaveBeenCalled();
    expect(estadoDaLeitura().hoje?.estado).toBe("depois");
  });

  it("já lida hoje: mostra o que leu e deixa ler as próximas", async () => {
    render(<LeituraDoDia onFechar={() => {}} onAjustes={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: "Li ✓" }));
    await userEvent.click(screen.getByRole("button", { name: "Começar o dia" }));
    const tela = render(<LeituraDoDia onFechar={() => {}} onAjustes={() => {}} />);
    expect(tela.getByText("✓ Leitura de hoje feita")).toBeInTheDocument();
    expect(tela.getByRole("heading", { name: "São Mateus 1, 1-6" })).toBeInTheDocument();
    await userEvent.click(tela.getByRole("button", { name: "Ler as próximas" }));
    expect(tela.getByRole("heading", { name: "São Mateus 1, 7-12" })).toBeInTheDocument();
  });
});
