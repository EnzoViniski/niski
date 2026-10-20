import { createElement } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AvisoDeAtualizacao } from "../components/AvisoDeAtualizacao";
import {
  CHAVE_DO_REINICIO,
  INTERVALO_DA_CHECAGEM_MS,
  consumirReinicioDeAtualizacao,
  estadoDaAtualizacaoRemota,
  iniciarChecagemAutomatica,
  redefinirAtualizacaoRemota,
} from "../lib/atualizacaoRemota";

/*
 * Fork (atualização automática): "Atualizar agora" na barra. Updater e `invoke` falsos; a ordem das
 * chamadas é o que importa: cópia do estado → download/instalação → reiniciar com o engine novo.
 */

const check = vi.fn();
const relaunch = vi.fn();
vi.mock("../lib/identidade", async (original) => ({ ...(await original<typeof import("../lib/identidade")>()), ATUALIZACAO_AUTOMATICA: true }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: () => check() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: () => relaunch() }));

const ordem: string[] = [];

function pacote(version = "0.9.0") {
  return {
    version,
    body: "Novidades",
    downloadAndInstall: vi.fn(async (aoProgredir?: (evento: unknown) => void) => {
      ordem.push("downloadAndInstall");
      aoProgredir?.({ event: "Started", data: { contentLength: 100 } });
      aoProgredir?.({ event: "Progress", data: { chunkLength: 100 } });
      aoProgredir?.({ event: "Finished" });
    }),
  };
}

beforeEach(() => {
  redefinirAtualizacaoRemota();
  localStorage.clear();
  ordem.length = 0;
  check.mockReset();
  relaunch.mockReset();
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (comando: string) => {
    ordem.push(comando);
    if (comando === "agentes_trabalhando") return 2;
    if (comando === "backup_antes_de_atualizar") return { pasta: "/tmp/x", arquivos: 3 };
    return null;
  });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("Atualizar agora", () => {
  it("sem versão nova: nada aparece", async () => {
    check.mockResolvedValue(null);
    const { container } = render(createElement(AvisoDeAtualizacao));
    await waitFor(() => expect(check).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it("versão nova: o botão aparece com a versão", async () => {
    check.mockResolvedValue(pacote());
    render(createElement(AvisoDeAtualizacao));
    expect(await screen.findByRole("button", { name: /Atualizar agora · v0\.9\.0/ })).toBeInTheDocument();
  });

  it("clique: confirma, faz a cópia antes do download e reabre trocando o engine", async () => {
    const novo = pacote();
    check.mockResolvedValue(novo);
    render(createElement(AvisoDeAtualizacao));
    fireEvent.click(await screen.findByRole("button", { name: /Atualizar agora/ }));

    const dialogo = await screen.findByRole("dialog", { name: "Confirmar atualização" });
    expect(dialogo).toHaveTextContent("As conversas e abas voltam como estavam");
    expect(dialogo).toHaveTextContent("2 agentes trabalhando agora serão interrompidos e voltam com --resume");
    expect(novo.downloadAndInstall).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Atualizar e reabrir" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("reiniciar_para_atualizar", { trocarEngine: true }));

    expect(ordem.filter((passo) => passo !== "agentes_trabalhando")).toEqual([
      "backup_antes_de_atualizar",
      "downloadAndInstall",
      "reiniciar_para_atualizar",
    ]);
    expect(invoke).toHaveBeenCalledWith("backup_antes_de_atualizar", { interface: expect.any(Object) });
    expect(relaunch).not.toHaveBeenCalled();
    expect(localStorage.getItem(CHAVE_DO_REINICIO)).not.toBeNull();
    expect(screen.getByRole("button", { name: "Reiniciando…" })).toBeDisabled();
  });

  it("a cópia vai com o workspace do localStorage, nunca com chaves fora da lista", async () => {
    localStorage.setItem("omni-agents-workspaces", '{"workspaces":[]}');
    localStorage.setItem("outra-coisa", "nao");
    check.mockResolvedValue(pacote());
    render(createElement(AvisoDeAtualizacao));
    fireEvent.click(await screen.findByRole("button", { name: /Atualizar agora/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Atualizar e reabrir" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("backup_antes_de_atualizar", { interface: { "omni-agents-workspaces": '{"workspaces":[]}' } }));
  });

  it("download que falha: mostra o erro, não reinicia e apaga o sinalizador", async () => {
    const novo = pacote();
    novo.downloadAndInstall.mockRejectedValue(new Error("assinatura inválida"));
    check.mockResolvedValue(novo);
    render(createElement(AvisoDeAtualizacao));
    fireEvent.click(await screen.findByRole("button", { name: /Atualizar agora/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Atualizar e reabrir" }));
    const tentar = await screen.findByRole("button", { name: /Tentar de novo · v0\.9\.0/ });
    expect(tentar).toHaveAttribute("title", "A atualização falhou: assinatura inválida");
    expect(invoke).not.toHaveBeenCalledWith("reiniciar_para_atualizar", expect.anything());
    expect(localStorage.getItem(CHAVE_DO_REINICIO)).toBeNull();
  });

  it("erro de rede na checagem automática é silencioso", async () => {
    check.mockRejectedValue(new Error("sem rede"));
    const { container } = render(createElement(AvisoDeAtualizacao));
    await waitFor(() => expect(check).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
    expect(estadoDaAtualizacaoRemota()).toMatchObject({ fase: "ocioso", erro: null });
  });

  it("checa ao abrir e de novo a cada 30 min, uma vez só para todos os que olham", async () => {
    vi.useFakeTimers();
    check.mockResolvedValue(null);
    const pararBarra = iniciarChecagemAutomatica();
    const pararSino = iniciarChecagemAutomatica();
    await act(async () => { await Promise.resolve(); });
    expect(check).toHaveBeenCalledTimes(1);
    await act(async () => { vi.advanceTimersByTime(INTERVALO_DA_CHECAGEM_MS); });
    expect(check).toHaveBeenCalledTimes(2);
    pararBarra();
    pararSino();
    await act(async () => { vi.advanceTimersByTime(INTERVALO_DA_CHECAGEM_MS); });
    expect(check).toHaveBeenCalledTimes(2);
  });
});

describe("Sinalizador do reinício de atualização", () => {
  it("vale uma vez só e ignora sobra antiga", () => {
    expect(consumirReinicioDeAtualizacao()).toBe(false);
    localStorage.setItem(CHAVE_DO_REINICIO, JSON.stringify({ em: 1_000 }));
    expect(consumirReinicioDeAtualizacao(2_000)).toBe(true);
    expect(consumirReinicioDeAtualizacao(2_000)).toBe(false);
    localStorage.setItem(CHAVE_DO_REINICIO, JSON.stringify({ em: 1_000 }));
    expect(consumirReinicioDeAtualizacao(1_000 + 60 * 60 * 1000)).toBe(false);
    expect(localStorage.getItem(CHAVE_DO_REINICIO)).toBeNull();
  });
});
