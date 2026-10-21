import { useState } from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "./interacoesTelas";
import { invoke } from "@tauri-apps/api/core";
import { SettingsView } from "../components/SettingsView";
import { DEFAULT_SETTINGS, type AppSettings } from "../types/settings";
import { setShortcutOverrides, shortcutFor } from "../lib/shortcuts";

// O menu pertence a SettingsView; as telas delegadas têm suas próprias suítes.
vi.mock("../features/terminal/AgentConnections", () => ({ AgentConnections: () => <p>Contas instaladas</p> }));
vi.mock("../features/antigravity/PermissoesDoAntigravity", () => ({ PermissoesDoAntigravity: () => <p>Permissões do Antigravity</p> }));
vi.mock("../features/agentes/ModelosDeAgentes", () => ({ ModelosDeAgentes: ({ projectPath }: { projectPath: string }) => <p>Modelos em {projectPath}</p> }));
vi.mock("../features/extensoes/ExtensoesSettings", () => ({ ExtensoesSettings: () => <p>Extensões por conta</p> }));
vi.mock("../features/skills/SkillsManager", () => ({ SkillsManager: () => <p>Skills instaladas</p> }));
vi.mock("../features/mobile/MobileSettings", () => ({ MobileSettings: () => <p>Pareamento remoto</p> }));
vi.mock("../features/relatorios/RelatoriosIa", () => ({ RelatoriosIa: ({ projectPaths }: { projectPaths: string[] }) => <p>Relatórios de {projectPaths.join(", ")}</p> }));
// Fork (Sobre mim): a tela tem suíte própria (SobreMim.test.tsx).
vi.mock("../features/sobreMim/SobreMimDoDesktop", () => ({ SobreMimDoDesktop: () => <p>Áreas do Sobre mim</p> }));
vi.mock("../components/UpdateCard", () => ({ UpdateCard: () => <p>Atualização do app</p> }));
vi.mock("../features/terminal/useAttentionNotifier", () => ({ testarAviso: vi.fn(async () => null) }));

const limite = { limite: 32, padrao: 32, minimo: 1, maximo: 128 };
function montar(initialSection?: Parameters<typeof SettingsView>[0]["initialSection"], inicial: AppSettings = DEFAULT_SETTINGS) {
  const onChange = vi.fn();
  const onClose = vi.fn();
  const onReset = vi.fn();
  function Configuracoes() {
    const [settings, setSettings] = useState(inicial);
    setShortcutOverrides(settings.shortcuts);
    return <SettingsView settings={settings} initialSection={initialSection} projectPath="/fake/projeto" projectPaths={["/fake/projeto", "/fake/outro"]} onClose={onClose} onReset={onReset}
      onChange={(updates) => { onChange(updates); setSettings((s) => ({ ...s, ...updates })); }} />;
  }
  render(<Configuracoes />);
  return { onChange, onClose, onReset };
}
beforeEach(() => {
  setShortcutOverrides({});
  vi.mocked(invoke).mockReset().mockImplementation(async (cmd, args) => {
    if (cmd === "limite_de_sessoes") return limite;
    if (cmd === "definir_limite_de_sessoes") return { ...limite, limite: (args as { valor: number }).valor };
    return null;
  });
});
afterEach(() => setShortcutOverrides({}));

it.each([
  ["Geral", "Salvar edições"], ["Aparência", "Fundo do app"], ["Agentes", "Contas instaladas"],
  ["Modelos de agentes", "Modelos em /fake/projeto"], ["Extensões", "Extensões por conta"],
  ["Skills", "Skills instaladas"], ["Acesso remoto", "Pareamento remoto"],
  ["Relatórios", "Relatórios de /fake/projeto, /fake/outro"], ["Atalhos", "Atalhos do teclado"], ["Sobre", "Atualização do app"], ["Sobre mim", "Áreas do Sobre mim"],
])("abre a seção %s pelo menu", async (nome, conteudo) => {
  montar();
  const menu = screen.getByRole("navigation", { name: "Seções de configurações" });
  const botao = within(menu).getByRole("button", { name: new RegExp(`^${nome}(?! mim)`) }); // Fork: "Sobre" × "Sobre mim".
  await userEvent.click(botao);
  expect(botao).toHaveAttribute("aria-current", "page");
  expect(screen.getByText(conteudo)).toBeInTheDocument();
});
it("grava a combinação escolhida e passa a exibi-la", async () => {
  const { onChange } = montar("atalhos");
  const botao = screen.getByRole("button", { name: /^Atalho de Buscar arquivo:/ });
  await userEvent.click(botao);
  fireEvent.keyDown(botao, { key: "u", code: "KeyU", ctrlKey: true, shiftKey: true });
  expect(onChange).toHaveBeenCalledWith({ shortcuts: { quickOpen: "Ctrl+Shift+U" } });
  expect(botao).not.toHaveTextContent("Aperte as teclas");
  expect(botao).toHaveAccessibleName(/U/);
});
it("avisa do conflito sem salvar e permite cancelar com Escape", async () => {
  const { onChange } = montar("atalhos");
  const botao = screen.getByRole("button", { name: /^Atalho de Buscar arquivo:/ });
  const combo = shortcutFor("closePane");
  await userEvent.click(botao);
  fireEvent.keyDown(botao, { key: "w", code: "KeyW", ctrlKey: combo.includes("Ctrl"), metaKey: combo.includes("Meta") });
  expect(screen.getByRole("alert")).toHaveTextContent(/já é usado em "Fechar painel"/);
  expect(onChange).not.toHaveBeenCalled();
  fireEvent.keyDown(botao, { key: "Escape" });
  expect(botao).not.toHaveTextContent("Aperte as teclas");
});
it("restaura um atalho personalizado ao padrão", async () => {
  const { onChange } = montar("atalhos", { ...DEFAULT_SETTINGS, shortcuts: { quickOpen: "Ctrl+Shift+U" } });
  await userEvent.click(screen.getByRole("button", { name: "Padrão" }));
  expect(onChange).toHaveBeenCalledWith({ shortcuts: {} });
  expect(screen.queryByRole("button", { name: "Padrão" })).not.toBeInTheDocument();
});
it("carrega Sessões, impede limite fora do intervalo e salva um novo valor", async () => {
  montar();
  const campo = await screen.findByRole("spinbutton", { name: "Limite de sessões ativas neste PC" });
  await waitFor(() => expect(campo).toHaveValue(32));
  const salvar = screen.getByRole("button", { name: "Salvar" });
  expect(salvar).toBeDisabled();
  fireEvent.change(campo, { target: { value: "129" } });
  expect(salvar).toBeDisabled();
  fireEvent.change(campo, { target: { value: "0" } });
  expect(salvar).toBeDisabled();
  fireEvent.change(campo, { target: { value: "16" } });
  await userEvent.click(salvar);
  expect(await screen.findByRole("status")).toHaveTextContent("Limite salvo: 16 sessões. Vale na hora.");
  expect(invoke).toHaveBeenCalledWith("definir_limite_de_sessoes", { valor: 16 });
  expect(salvar).toBeDisabled();
});
it("mostra erro ao salvar Sessões sem perder o valor digitado", async () => {
  vi.mocked(invoke).mockImplementation(async (cmd) => {
    if (cmd === "limite_de_sessoes") return limite;
    throw new Error("Engine indisponível");
  });
  montar();
  const campo = screen.getByRole("spinbutton");
  await waitFor(() => expect(campo).toBeEnabled());
  fireEvent.change(campo, { target: { value: "24" } });
  fireEvent.keyDown(campo, { key: "Enter" });
  expect(await screen.findByRole("status")).toHaveTextContent("Engine indisponível");
  expect(campo).toHaveValue(24);
  expect(screen.getByRole("button", { name: "Salvar" })).toBeEnabled();
});
it("altera tema e fecha pelo botão de configurações", async () => {
  const { onChange, onClose } = montar("aparencia");
  await userEvent.click(screen.getByRole("button", { name: "Claro" }));
  expect(onChange).toHaveBeenCalledWith({ theme: "claro" });
  expect(screen.getByRole("button", { name: "Claro" })).toHaveAttribute("aria-pressed", "true");
  await userEvent.click(screen.getByRole("button", { name: "Fechar configurações" }));
  expect(onClose).toHaveBeenCalledOnce();
});
it("acompanha mudança de seção solicitada externamente", () => {
  const props = { settings: DEFAULT_SETTINGS, onChange: vi.fn(), onReset: vi.fn(), onClose: vi.fn(), projectPath: null };
  const { rerender } = render(<SettingsView {...props} initialSection="atalhos" />);
  expect(screen.getByText("Atalhos do teclado")).toBeInTheDocument();
  rerender(<SettingsView {...props} initialSection="sobre" />);
  expect(screen.getByText("Atualização do app")).toBeInTheDocument();
  expect(screen.queryByText("Atalhos do teclado")).not.toBeInTheDocument();
});

it("Aparência: escolhe o estilo da interface (Clássico por padrão)", () => {
  const { onChange } = montar("aparencia");
  expect(screen.getByRole("radio", { name: /Clássico/ })).toHaveAttribute("aria-checked", "true");
  fireEvent.click(screen.getByRole("radio", { name: /Minimalista/ }));
  expect(onChange).toHaveBeenCalledWith({ estilo: "minimalista" });
  expect(screen.getByRole("radio", { name: /Minimalista/ })).toHaveAttribute("aria-checked", "true");
  expect(screen.getByText(/No Minimalista e no Vidro o fundo fica liso/)).toBeInTheDocument();
});
