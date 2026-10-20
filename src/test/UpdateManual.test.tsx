import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { UpdateCard } from "../components/UpdateCard";
import { NotificationBell } from "../components/NotificationBell";
import { redefinirAtualizacaoRemota } from "../lib/atualizacaoRemota";

const check = vi.fn();
const relaunch = vi.fn();
// Fork (atualização automática): o updater liga ou desliga por teste.
const chave = vi.hoisted(() => ({ ligada: true }));
vi.mock("../lib/identidade", async (original) => {
  const real = await original<typeof import("../lib/identidade")>();
  return { ...real, get ATUALIZACAO_AUTOMATICA() { return chave.ligada; } };
});
vi.mock("@tauri-apps/plugin-updater", () => ({ check: () => check() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: () => relaunch() }));

beforeEach(() => {
  redefinirAtualizacaoRemota();
  chave.ligada = true;
  check.mockReset();
  relaunch.mockReset();
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockResolvedValue(null);
});

it("a distribuição sem chave própria abre os downloads do repositório de releases e não consulta o updater", async () => {
  chave.ligada = false;
  render(<><UpdateCard /><NotificationBell onOpenSettings={vi.fn()} /></>);
  await userEvent.click(screen.getByRole("button", { name: "Ver versões para baixar" }));
  expect(openUrl).toHaveBeenCalledWith("https://github.com/Medza-Primos-Organizacao/medza-agentes-releases/releases");
  expect(check).not.toHaveBeenCalled();
});

it("com o updater ligado, Configurações atualiza e reabre trocando o engine (sem relaunch)", async () => {
  const downloadAndInstall = vi.fn().mockResolvedValue(undefined);
  check.mockResolvedValue({ version: "0.9.0", body: "Novidades", downloadAndInstall });
  render(<UpdateCard />);

  expect(await screen.findByText("Versão 0.9.0 disponível")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Atualizar agora" }));
  expect(screen.getByText(/As conversas e abas voltam como estavam/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Atualizar e reabrir" }));

  await waitFor(() => expect(invoke).toHaveBeenCalledWith("reiniciar_para_atualizar", { trocarEngine: true }));
  expect(invoke).toHaveBeenCalledWith("backup_antes_de_atualizar", expect.anything());
  expect(downloadAndInstall).toHaveBeenCalledTimes(1);
  expect(relaunch).not.toHaveBeenCalled();
});

it("procurar sem versão nova diz que está na mais recente", async () => {
  check.mockResolvedValue(null);
  render(<UpdateCard />);
  await userEvent.click(screen.getByRole("button", { name: "Procurar atualizações" }));
  expect(await screen.findByText(/versão mais recente/)).toBeInTheDocument();
});
