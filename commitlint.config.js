export default {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'type-enum': [
      2,
      'always',
      [
        'build',
        'chore',
        'ci',
        'docs',
        'feat',
        'fix',
        'perf',
        'refactor',
        'revert',
        'style',
        'test'
      ]
    ],
    // The subject can contain emojis, but we need to ensure the whole message
    // starts with an alphanumeric character (the type).
    'header-pattern': [
      2,
      'always',
      /^[a-zA-Z]+(\([^)]+\))?!?: .+/
    ]
  },
  plugins: [
    {
      rules: {
        'header-pattern': (parsed) => {
          // Commitlint's default parser might fail or misinterpret emojis at the start.
          // The safest way is to check the raw header.
          const { header } = parsed;
          const regex = /^[a-zA-Z]+(\([^)]+\))?!?: .+/;
          if (regex.test(header)) {
            return [true];
          }
          return [false, `header must match pattern /^[a-zA-Z]+(\\([^)]+\\))?!?: .+/, found: "${header}"`];
        }
      }
    }
  ]
};
