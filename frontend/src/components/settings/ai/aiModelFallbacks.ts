import type { OpenCodeModelItem } from '@/bindings/OpenCodeModelItem'
import type { OpenRouterModelItem } from '@/bindings/OpenRouterModelItem'

/**
 * Sugestões exibidas no seletor de modelo enquanto o catálogo do provedor
 * não respondeu (sem chave, fora do ar). Só dados: quem as transforma em
 * opção da tela é `aiModelCatalog`.
 */

export const FALLBACK_OPENROUTER_MODELS: OpenRouterModelItem[] = [
  {
    id: 'openrouter/free',
    name: 'Free Models Router (Automático Gratuito)',
    isFree: true,
    description: 'Roteia para o melhor modelo gratuito',
    supportsTools: true,
  },
  {
    id: 'google/gemma-4-31b-it:free',
    name: 'Google: Gemma 4 31B (Gratuito)',
    isFree: true,
    description: 'Excelente raciocínio e suporte gratuito',
    supportsTools: true,
  },
  {
    id: 'qwen/qwen3.8-27b:free',
    name: 'Qwen: Qwen 3.8 27B (Gratuito)',
    isFree: true,
    description: 'Alta performance em código e raciocínio técnico',
    supportsTools: true,
  },
  {
    id: 'nvidia/nemotron-3.5-lightning:free',
    name: 'NVIDIA: Nemotron 3.5 Lightning (Gratuito)',
    isFree: true,
    description: 'Velocidade e precisão para diagnósticos de rede',
    supportsTools: true,
  },
  {
    id: 'meta-llama/llama-3.3-70b-instruct',
    name: 'Meta: Llama 3.3 70B Instruct (Padrão / Créditos)',
    isFree: false,
    description: 'Modelo recomendado da Meta para alta complexidade',
    supportsTools: true,
  },
  {
    id: 'openai/gpt-4o-mini',
    name: 'OpenAI: GPT-4o Mini',
    isFree: false,
    description: 'Rápido, econômico e altamente capaz',
    supportsTools: true,
  },
  {
    id: 'anthropic/claude-3.5-sonnet',
    name: 'Anthropic: Claude 3.5 Sonnet',
    isFree: false,
    description: 'Estado da arte em raciocínio e engenharia',
    supportsTools: true,
  },
  {
    id: 'deepseek/deepseek-chat',
    name: 'DeepSeek: DeepSeek Chat (V3)',
    isFree: false,
    description: 'Excelente custo-benefício em análise de sistemas',
    supportsTools: true,
  },
]

export const FALLBACK_OPENCODE_MODELS: OpenCodeModelItem[] = [
  {
    id: 'muse-spark-1.3-contributor-free',
    name: 'Muse Spark 1.3 Contributor (Gratuito)',
    isFree: true,
    description: 'Modelo recomendado gratuito com excelente raciocínio',
    supportsTools: true,
  },
  {
    id: 'mimo-v2.6-flash-free',
    name: 'Mimo v2.6 Flash (Gratuito)',
    isFree: true,
    description: 'Modelo ultra-rápido gratuito otimizado para chamadas e código',
    supportsTools: true,
  },
  {
    id: 'jev-1.13-free',
    name: 'Jev 1.13 (Gratuito)',
    isFree: true,
    description: 'Modelo gratuito de uso geral para diagnósticos',
    supportsTools: true,
  },
  {
    id: 'deepseek-v4.1-flash',
    name: 'DeepSeek v4.1 Flash',
    isFree: false,
    description: 'Alta performance em análise de rede e scripts',
    supportsTools: true,
  },
  {
    id: 'gemini-3.8-flash',
    name: 'Gemini 3.8 Flash',
    isFree: false,
    description: 'Latência reduzida e raciocínio avançado',
    supportsTools: true,
  },
  {
    id: 'glm-5.3-flash',
    name: 'GLM 5.3 Flash',
    isFree: false,
    description: 'Excelente para tarefas de diagnóstico e suporte',
    supportsTools: true,
  },
  {
    id: 'gpt-6-astra',
    name: 'GPT-6 Astra',
    isFree: false,
    description: 'Modelo de ponta para análise profunda e playbooks',
    supportsTools: true,
  },
  {
    id: 'qwen3.8-flash',
    name: 'Qwen 3.8 Flash',
    isFree: false,
    description: 'Modelo rápido para consultas e suporte operacional',
    supportsTools: true,
  },
]
