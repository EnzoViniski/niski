import identidade from "../../identidade.json";

/** Nome do produto, de `identidade.json` na raiz — o mesmo arquivo que dá nome, bundle id e portas
 *  ao engine e ao app. Um fork troca o arquivo, não as telas. */
export const NOME_DO_PRODUTO: string = identidade.nome;
export const NOME_CURTO: string = identidade.nomeCurto;
/** Porta padrão do servidor do celular. */
export const PORTA_DO_CELULAR: number = identidade.portaDoCelular;
/** Atualização automática só com chave de assinatura própria configurada. */
export const ATUALIZACAO_AUTOMATICA = Boolean(identidade.updater.pubkey && identidade.updater.endpoint);
export const PAGINA_DE_DOWNLOADS = `https://github.com/${identidade.repositorio}/releases`;
