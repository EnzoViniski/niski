import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "./interacoesTelas";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { SkillsManager } from "../features/skills/SkillsManager";
import { installSkill, listSkills, removeSkill, setSkillEnabled, type SkillInfo } from "../features/skills/skillsService";
import { perfis } from "./fixturesTelas";

vi.mock("../features/skills/skillsService", async (importOriginal) => ({
  ...await importOriginal<typeof import("../features/skills/skillsService")>(),
  listSkills: vi.fn(), installSkill: vi.fn(), removeSkill: vi.fn(), setSkillEnabled: vi.fn(),
}));
const skill: SkillInfo = { id: "revisar", name: "Revisão de telas", description: "Confere o foco", path: "/fake/pessoal/skills/revisar", enabled: true, symlink: false };
let skills: SkillInfo[];
beforeEach(() => {
  vi.mocked(invoke).mockReset().mockResolvedValue(perfis);
  vi.mocked(open).mockReset().mockResolvedValue(null);
  skills = [{ ...skill }];
  vi.mocked(listSkills).mockReset().mockImplementation(async () => [...skills]);
  vi.mocked(setSkillEnabled).mockReset().mockImplementation(async (_scope, alvo, enabled) => { skills = skills.map((s) => s.id === alvo.id ? { ...s, enabled } : s); });
  vi.mocked(removeSkill).mockReset().mockImplementation(async () => { skills = []; });
  vi.mocked(installSkill).mockReset().mockResolvedValue(undefined);
});
it("lista skills e alterna o escopo da conta para o projeto", async () => {
  render(<SkillsManager projectPath="/fake/projeto" />);
  expect(await screen.findByText("Revisão de telas")).toBeInTheDocument();
  expect(screen.getByText("1 ativa de 1.")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("combobox", { name: "Escopo" }));
  await userEvent.click(screen.getByRole("option", { name: /Projeto · projeto/ }));
  await waitFor(() => expect(listSkills).toHaveBeenLastCalledWith(expect.objectContaining({ id: "project", root: "/fake/projeto/.claude" })));
  expect(screen.getByText("/fake/projeto/.claude/skills")).toBeInTheDocument();
});
it("desativa e reativa uma skill e relê a lista", async () => {
  render(<SkillsManager projectPath={null} />);
  await userEvent.click(await screen.findByRole("button", { name: "Desativar" }));
  expect(await screen.findByText("desativada")).toBeInTheDocument();
  expect(screen.getByText("0 ativas de 1.")).toBeInTheDocument();
  expect(setSkillEnabled).toHaveBeenCalledWith(expect.objectContaining({ id: "profile:pessoal" }), skill, false);
  await userEvent.click(screen.getByRole("button", { name: "Ativar" }));
  expect(await screen.findByRole("button", { name: "Desativar" })).toBeInTheDocument();
  expect(screen.queryByText("desativada")).not.toBeInTheDocument();
});
it("pede confirmação para remover e permite cancelar", async () => {
  render(<SkillsManager projectPath={null} />);
  await userEvent.click(await screen.findByRole("button", { name: "Remover" }));
  expect(removeSkill).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(screen.queryByRole("button", { name: "Confirmar" })).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Remover" }));
  await userEvent.click(screen.getByRole("button", { name: "Confirmar" }));
  await waitFor(() => expect(screen.queryByText("Revisão de telas")).not.toBeInTheDocument());
  expect(removeSkill).toHaveBeenCalledWith(skill);
  expect(screen.getByText(/Nenhuma skill neste escopo/)).toBeInTheDocument();
});
it("instala a pasta escolhida no escopo atual", async () => {
  vi.mocked(open).mockResolvedValue("/fake/nova-skill");
  render(<SkillsManager projectPath={null} />);
  await screen.findByText("Revisão de telas");
  await userEvent.click(screen.getByRole("button", { name: "Instalar skill…" }));
  await waitFor(() => expect(installSkill).toHaveBeenCalledWith(expect.objectContaining({ id: "profile:pessoal" }), "/fake/nova-skill"));
  expect(open).toHaveBeenCalledWith(expect.objectContaining({ directory: true, multiple: false }));
});
it("cancelar o seletor de pasta não instala nada", async () => {
  render(<SkillsManager projectPath={null} />);
  await screen.findByText("Revisão de telas");
  await userEvent.click(screen.getByRole("button", { name: "Instalar skill…" }));
  expect(open).toHaveBeenCalledOnce();
  expect(installSkill).not.toHaveBeenCalled();
});
it("exibe falha da ação sem retirar a skill da lista", async () => {
  vi.mocked(setSkillEnabled).mockRejectedValue(new Error("Pasta sem permissão"));
  render(<SkillsManager projectPath={null} />);
  await userEvent.click(await screen.findByRole("button", { name: "Desativar" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Pasta sem permissão");
  expect(screen.getByText("Revisão de telas")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Desativar" })).toBeEnabled();
});
it("explica como começar quando não existem contas nem projeto", async () => {
  vi.mocked(invoke).mockResolvedValue([]);
  render(<SkillsManager projectPath={null} />);
  expect(await screen.findByText(/Nenhuma conta de Claude ou Codex cadastrada/)).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Instalar skill…" })).not.toBeInTheDocument();
});
it("limpa o aviso de falha ao recuperar pelo botão Atualizar", async () => {
  vi.mocked(listSkills).mockRejectedValueOnce(new Error("Não foi possível listar"));
  render(<SkillsManager projectPath={null} />);
  expect(await screen.findByRole("alert")).toHaveTextContent("Não foi possível listar");
  await userEvent.click(screen.getByRole("button", { name: "Atualizar" }));
  expect(await screen.findByText("Revisão de telas")).toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});
