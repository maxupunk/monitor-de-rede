import 'vuetify/styles'
import '@mdi/font/css/materialdesignicons.css'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'
import { initialThemeName } from '@/utils/themePreference'

export default createVuetify({
  components,
  directives,
  /**
   * Superfícies — o padrão visual de todas as telas, num lugar só:
   * - card de página: `elevation="2"`, sem contorno;
   * - painel dentro de outro card ou diálogo: `border flat` (a borda do tema,
   *   discreta nos dois temas);
   * - card de escolha clicável: `<ChoiceCard>`;
   * - canto sempre `lg`. `variant="outlined"` sem `color` desenha a borda com a
   *   cor do texto (quase branca no tema escuro) e `xl` destoa do resto — os
   *   dois são barrados por `tests/conventions/surfaces.test.ts`.
   */
  defaults: {
    VCard: { rounded: 'lg' },
  },
  theme: {
    // A preferência do usuário (menu → Tema), lida antes de montar.
    defaultTheme: initialThemeName(),
    themes: {
      dark: {
        colors: {
          primary: '#1976D2',
          /**
           * Ações secundárias ("Escanear", "Escanear bloco", "Atualizar"...)
           * usam este token. Era `#424242` — um cinza-escuro que, no tema
           * escuro, ficava praticamente indistinguível de um botão desabilitado.
           * Cor de ação é sempre uma cor viva: cinza só para estado desligado.
           */
          secondary: '#00ACC1',
          accent: '#82B1FF',
          error: '#FF5252',
          info: '#2196F3',
          success: '#4CAF50',
          warning: '#FFC107',
          /** Uso alto (CPU/memória ≥ 70%): laranja, entre o aviso e o erro. */
          caution: '#FF9100',
          /**
           * Tudo o que vem de plugin (aba Plugins, Clientes Wi-Fi e qualquer
           * aba alimentada por plugin): cor própria, para o operador separar o
           * que é do equipamento do que é extensão. NÃO trocar por `primary`
           * nem reaproveitar em outra coisa — ver AGENTS.md §12.
           */
          plugin: '#1DE9B6',
        },
      },
      light: {
        colors: {
          caution: '#E65100',
          plugin: '#00796B',
        },
        /**
         * O cinza "médio" do Vuetify (60% do preto) deixava subtítulo de
         * lista e texto secundário desbotados sobre o branco — o §12 do
         * AGENTS.md pede contraste alto. 75% fica nítido sem igualar ao título.
         */
        variables: {
          'medium-emphasis-opacity': 0.75,
        },
      },
    },
  },
})
