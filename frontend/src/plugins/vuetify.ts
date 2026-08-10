import 'vuetify/styles'
import '@mdi/font/css/materialdesignicons.css'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'

export default createVuetify({
  components,
  directives,
  theme: {
    defaultTheme: 'dark',
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
        },
      },
    },
  },
})
