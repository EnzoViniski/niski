// Fork (Editor de vídeo): id de projeto que o engine usa para a conversa de um vídeo. Arquivo sem
// dependências: a recuperação do workspace importa daqui sem carregar o editor.
const PREFIXO = "video:";

export const idDoProjetoDoVideo = (id: string) => `${PREFIXO}${id}`;
export const ehConversaDoVideo = (projectId: string | null | undefined) => !!projectId && projectId.startsWith(PREFIXO);
