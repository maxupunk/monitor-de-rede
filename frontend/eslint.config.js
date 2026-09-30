import pluginVue from 'eslint-plugin-vue'
import tseslint from 'typescript-eslint'
import vueParser from 'vue-eslint-parser'
import eslintConfigPrettier from 'eslint-config-prettier/flat'

export default tseslint.config(
  ...pluginVue.configs['flat/recommended'],
  ...tseslint.configs.recommended,
  {
    files: ['src/**/*.{js,ts,vue}'],
    languageOptions: {
      parser: vueParser,
      parserOptions: {
        parser: tseslint.parser,
        extraFileExtensions: ['.vue'],
        sourceType: 'module',
      },
    },
    rules: {
      'vue/multi-word-component-names': 'off',
      'vue/no-v-html': 'off',
      'vue/require-default-prop': 'off',
      'vue/valid-v-slot': 'off',
      'vue/no-template-shadow': 'warn',
      '@typescript-eslint/no-explicit-any': 'off',
      '@typescript-eslint/no-unused-vars': ['warn', { argsIgnorePattern: '^_' }],
    },
  },
  // Sempre por último: desliga TODA regra de estilo (indentação, quebras,
  // espaços, aspas) do ESLint e dos plugins. Formatação é só do Prettier; o
  // ESLint cuida de erro de código. Assim `lint --fix` e `format` nunca
  // brigam pelo mesmo trecho.
  eslintConfigPrettier
)

