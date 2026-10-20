import { describe, expect, it, vi, beforeEach } from "vitest";
import {
  disabledSkillsDir,
  installSkill,
  installSkillEverywhere,
  listSkills,
  removeSkill,
  setSkillEnabled,
  skillScopes,
  skillsDir,
  type SkillInfo,
  type SkillScope,
} from "../features/skills/skillsService";
import type { Profile } from "../features/terminal/terminalService";

const fs = await import("@tauri-apps/plugin-fs");
const mockExists = vi.mocked(fs.exists);
const mockReadDir = vi.mocked(fs.readDir);
const mockReadTextFile = vi.mocked(fs.readTextFile);
const mockRename = vi.mocked(fs.rename);
const mockRemove = vi.mocked(fs.remove);
const mockMkdir = vi.mocked(fs.mkdir);
const mockStat = vi.mocked(fs.stat);
const mockCopyFile = vi.mocked(fs.copyFile);

beforeEach(() => {
  vi.clearAllMocks();
});

const escopo: SkillScope = {
  id: "profile:claude-p",
  label: "Claude · Padrão",
  provider: "claude",
  kind: "global",
  root: "C:\\Users\\eu\\.claude",
};

const perfil = (over: Partial<Profile>): Profile => ({
  id: "p", provider: "claude", name: "Padrão", config_dir: "C:\\Users\\eu\\.claude",
  builtin: true, created_at_ms: 0, last_used_at_ms: null, authenticated: true, ...over,
});

const skill = (over: Partial<SkillInfo>): SkillInfo => ({
  id: "minha-skill", name: "Minha skill", description: "", enabled: true, symlink: false,
  path: "C:\\Users\\eu\\.claude\\skills\\minha-skill", ...over,
});

describe("skillScopes", () => {
  it("dá um escopo por conta de Claude/Codex e um para o projeto aberto", () => {
    const scopes = skillScopes(
      [perfil({ id: "c1", name: "Pessoal" }), perfil({ id: "x1", provider: "codex", name: "Trabalho", config_dir: "C:\\codex" })],
      "D:\\dev\\omni"
    );
    expect(scopes.map((s) => s.label)).toEqual(["Claude · Pessoal", "Codex · Trabalho", "Projeto · omni · Claude", "Projeto · omni · Codex"]);
    expect(scopes[1].root).toBe("C:\\codex");
    expect(scopes[2].root).toBe("D:\\dev\\omni\\.claude");
    expect(scopes[3].root).toBe("D:\\dev\\omni\\.agents");
  });

  it("expõe skills compartilhadas do Codex no diretório .agents do usuário", () => {
    const scopes = skillScopes([perfil({ provider: "codex", config_dir: "/Users/enzo/.codex" })], null);
    expect(scopes.find((s) => s.id === "user:agents")?.root).toBe("/Users/enzo/.agents");
  });

  it("ignora provider sem pasta de skills e projeto fechado", () => {
    // O CLI do Cursor (`agent`) não lê `skills/`; oferecer o escopo dele só criaria pasta vazia.
    const scopes = skillScopes([perfil({ id: "cur", provider: "cursor", name: "Padrão" })], null);
    expect(scopes).toEqual([]);
  });
});

describe("listSkills", () => {
  it("inclui skill instalada como link e marca as desativadas", async () => {
    // `readDir` não segue link: a skill ligada chega com isDirectory: false, isSymlink: true.
    mockExists.mockImplementation(async (path) => !String(path).includes("nao-skill"));
    mockReadDir
      .mockResolvedValueOnce([
        { name: "ligada", isDirectory: false, isFile: false, isSymlink: true },
        { name: "propria", isDirectory: true, isFile: false, isSymlink: false },
        { name: "nao-skill", isDirectory: true, isFile: false, isSymlink: false },
      ])
      .mockResolvedValueOnce([{ name: "guardada", isDirectory: true, isFile: false, isSymlink: false }]);
    // Nome vindo do frontmatter de cada pasta, para a ordenação por nome ser de verdade.
    mockReadTextFile.mockImplementation(async (path) => {
      const pasta = String(path).split("\\").at(-2);
      return `---\nname: ${pasta}\ndescription: d\n---`;
    });

    const skills = await listSkills(escopo);

    expect(skills.map((s) => [s.id, s.enabled, s.symlink])).toEqual([
      ["guardada", false, false],
      ["ligada", true, true],
      ["propria", true, false],
    ]);
  });

  it("escopo sem pasta nenhuma devolve lista vazia", async () => {
    mockExists.mockResolvedValue(false);
    expect(await listSkills(escopo)).toEqual([]);
    expect(mockReadDir).not.toHaveBeenCalled();
  });
});

describe("pacote de skills em várias contas", () => {
  it("instala o pacote nas contas e preserva as versões existentes ou desativadas", async () => {
    const contaCodex = { ...escopo, id: "codex", root: "/codex", provider: "codex" as const };
    mockExists.mockImplementation(async (path) => {
      const p = String(path).replace(/\\/g, "/");
      if (p === "/pacote/SKILL.md") return false;
      if (p.endsWith("/SKILL.md")) return true;
      return p.endsWith("/skills/a") || p.endsWith("/skills-disabled/b");
    });
    mockReadDir.mockResolvedValue([
      { name: "a", isDirectory: true, isFile: false, isSymlink: false },
      { name: "b", isDirectory: true, isFile: false, isSymlink: false },
    ]);
    const resultado = await installSkillEverywhere([escopo, contaCodex, contaCodex], "/pacote");
    expect(resultado).toEqual({ instaladas: 0, existentes: 4 });
    expect(mockCopyFile).not.toHaveBeenCalled();
  });
});

describe("setSkillEnabled", () => {
  it("desativar move para skills-disabled/ sem tocar no conteúdo", async () => {
    mockExists.mockResolvedValue(false);
    await setSkillEnabled(escopo, skill({}), false);
    expect(mockMkdir).toHaveBeenCalledWith(disabledSkillsDir(escopo), { recursive: true });
    expect(mockRename).toHaveBeenCalledWith(
      "C:\\Users\\eu\\.claude\\skills\\minha-skill",
      "C:\\Users\\eu\\.claude\\skills-disabled\\minha-skill"
    );
  });

  it("ativar traz de volta para skills/", async () => {
    mockExists.mockResolvedValue(false);
    const guardada = skill({ enabled: false, path: "C:\\Users\\eu\\.claude\\skills-disabled\\minha-skill" });
    await setSkillEnabled(escopo, guardada, true);
    expect(mockRename).toHaveBeenCalledWith(guardada.path, `${skillsDir(escopo)}\\minha-skill`);
  });

  it("recusa quando já existe uma com o mesmo nome do outro lado", async () => {
    // Sem isto o rename sobrescreveria a de destino e a skill guardada sumiria em silêncio.
    mockExists.mockResolvedValue(true);
    await expect(setSkillEnabled(escopo, skill({}), false)).rejects.toThrow(/Já existe/);
    expect(mockRename).not.toHaveBeenCalled();
  });
});

describe("removeSkill", () => {
  it("apaga a pasta inteira quando a skill é dela mesma", async () => {
    await removeSkill(skill({}));
    expect(mockRemove).toHaveBeenCalledWith(skill({}).path, { recursive: true });
  });

  it("em link apaga só o link — recursive levaria a pasta original junto", async () => {
    const ligada = skill({ symlink: true });
    await removeSkill(ligada);
    expect(mockRemove).toHaveBeenCalledWith(ligada.path, { recursive: false });
  });
});

describe("installSkill", () => {
  it("recusa pasta sem SKILL.md na raiz", async () => {
    mockExists.mockResolvedValue(false);
    await expect(installSkill(escopo, "D:\\baixados\\qualquer")).rejects.toThrow(/SKILL\.md/);
    expect(mockCopyFile).not.toHaveBeenCalled();
  });

  it("recusa nome que já existe no escopo", async () => {
    mockExists.mockResolvedValue(true);
    await expect(installSkill(escopo, "D:\\baixados\\minha-skill")).rejects.toThrow(/Já existe/);
    expect(mockCopyFile).not.toHaveBeenCalled();
  });

  it("copia a pasta recursivamente para dentro de skills/", async () => {
    mockExists.mockImplementation(async (path) => String(path).endsWith("SKILL.md"));
    mockStat.mockImplementation(async (path) =>
      ({ isDirectory: !String(path).endsWith(".md") }) as unknown as Awaited<ReturnType<typeof fs.stat>>
    );
    mockReadDir.mockResolvedValueOnce([{ name: "SKILL.md", isDirectory: false, isFile: true, isSymlink: false }]);

    const destino = await installSkill(escopo, "D:\\baixados\\minha-skill");

    expect(destino).toBe("C:\\Users\\eu\\.claude\\skills\\minha-skill");
    expect(mockCopyFile).toHaveBeenCalledWith(
      "D:\\baixados\\minha-skill\\SKILL.md",
      "C:\\Users\\eu\\.claude\\skills\\minha-skill\\SKILL.md"
    );
  });
});
