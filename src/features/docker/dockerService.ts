import { invoke } from "@tauri-apps/api/core";

export interface DockerContainer {
  id: string;
  name: string;
  image: string;
  /** `running`, `exited`, `paused`, `created`, `restarting`, `dead`. */
  state: string;
  status: string;
  ports: string;
  /** Projeto do Compose; vazio quando o container não veio de Compose. */
  compose_project: string;
}

export type DockerAction = "start" | "stop" | "restart" | "remove";

/**
 * Docker indisponível por um motivo que não é erro de verdade: o Docker Desktop está fechado (ou o
 * daemon parado) ou o docker nem está instalado. Nesses casos o painel mostra uma linha neutra e
 * não o stderr do docker — que é uma URL de pipe de 200 caracteres sem espaço, alarmante e que
 * esticava a lateral com rolagem horizontal.
 */
export function dockerIndisponivel(erro: string): "fechado" | "ausente" | null {
  const texto = erro.toLowerCase();
  if (texto.includes("não encontrado no path") || texto.includes("not recognized") || texto.includes("program not found")) {
    return "ausente";
  }
  if (
    texto.includes("error during connect")
    || texto.includes("cannot connect to the docker daemon")
    || texto.includes("is the docker daemon running")
    || texto.includes("dockerdesktoplinuxengine")
    || texto.includes("docker_engine")
    || texto.includes("pipe/docker")
    || texto.includes("docker desktop is not running")
    || texto.includes("docker.sock")
  ) {
    return "fechado";
  }
  return null;
}

export function dockerContainers(): Promise<DockerContainer[]> {
  return invoke<DockerContainer[]>("docker_containers");
}

export function dockerContainerAction(id: string, action: DockerAction): Promise<void> {
  return invoke("docker_container_action", { id, action });
}

/** Agrupa como a extensão Containers: um grupo por projeto do Compose, soltos por último. */
export function groupByCompose(containers: DockerContainer[]): { project: string; containers: DockerContainer[] }[] {
  const groups = new Map<string, DockerContainer[]>();
  // Item sem nome ou sem projeto (resposta estranha do docker) não pode derrubar a barra lateral.
  for (const container of [...containers].sort((a, b) => (a.name ?? "").localeCompare(b.name ?? ""))) {
    const projeto = container.compose_project ?? "";
    const list = groups.get(projeto) ?? [];
    list.push(container);
    groups.set(projeto, list);
  }
  return [...groups.entries()]
    .sort(([a], [b]) => (a === "" ? 1 : b === "" ? -1 : a.localeCompare(b)))
    .map(([project, list]) => ({ project, containers: list }));
}
