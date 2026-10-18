// The same rule set as acme-web's oxlint.config.mts, minus its repo-specific plugins.

// The active half of typescript-eslint's `recommendedTypeChecked`, listed rather than inherited:
// oxlint has no equivalent preset, and these sit across several categories, so a category alone
// would silently drop most of them.
const recommendedTypeChecked = {
  'no-case-declarations': 'error',
  'no-empty': 'error',
  'no-prototype-builtins': 'error',
  'no-regex-spaces': 'error',
  'no-unexpected-multiline': 'error',
  'no-useless-assignment': 'error',
  'preserve-caught-error': 'error',
  'no-var': 'error',
  'prefer-const': 'error',
  'prefer-rest-params': 'error',
  'prefer-spread': 'error',
  'typescript/await-thenable': 'error',
  'typescript/ban-ts-comment': 'error',
  'typescript/no-array-constructor': 'error',
  'typescript/no-array-delete': 'error',
  'typescript/no-base-to-string': 'error',
  'typescript/no-duplicate-enum-values': 'error',
  'typescript/no-duplicate-type-constituents': 'error',
  'typescript/no-empty-object-type': 'error',
  'typescript/no-explicit-any': 'error',
  'typescript/no-extra-non-null-assertion': 'error',
  'typescript/no-floating-promises': 'error',
  'typescript/no-for-in-array': 'error',
  'typescript/no-implied-eval': 'error',
  'typescript/no-misused-new': 'error',
  'typescript/no-misused-promises': 'error',
  'typescript/no-namespace': 'error',
  'typescript/no-non-null-asserted-optional-chain': 'error',
  'typescript/no-redundant-type-constituents': 'error',
  'typescript/no-require-imports': 'error',
  'typescript/no-this-alias': 'error',
  'typescript/no-unnecessary-type-assertion': 'error',
  'typescript/no-unnecessary-type-constraint': 'error',
  'typescript/no-unsafe-argument': 'error',
  'typescript/no-unsafe-assignment': 'error',
  'typescript/no-unsafe-call': 'error',
  'typescript/no-unsafe-declaration-merging': 'error',
  'typescript/no-unsafe-enum-comparison': 'error',
  'typescript/no-unsafe-function-type': 'error',
  'typescript/no-unsafe-member-access': 'error',
  'typescript/no-unsafe-return': 'error',
  'typescript/no-unsafe-unary-minus': 'error',
  'typescript/no-unused-expressions': 'error',
  'typescript/no-unused-vars': 'error',
  'typescript/no-wrapper-object-types': 'error',
  'typescript/only-throw-error': 'error',
  'typescript/prefer-as-const': 'error',
  'typescript/prefer-namespace-keyword': 'error',
  'typescript/prefer-promise-reject-errors': 'error',
  'typescript/require-await': 'error',
  'typescript/restrict-plus-operands': 'error',
  'typescript/restrict-template-expressions': 'error',
  'typescript/triple-slash-reference': 'error',
  'typescript/unbound-method': 'error',
};

// eslint-plugin-react-hooks v7's `flat.recommended`, minus `config` and `gating`, which oxlint does
// not implement and which only validate React Compiler configuration.
const reactHooksRecommended = {
  'react-hooks/rules-of-hooks': 'error',
  'react-hooks/exhaustive-deps': 'warn',
  'react-hooks/static-components': 'error',
  'react-hooks/use-memo': 'error',
  'react-hooks/preserve-manual-memoization': 'error',
  'react-hooks/incompatible-library': 'warn',
  'react-hooks/immutability': 'error',
  'react-hooks/globals': 'error',
  'react-hooks/refs': 'error',
  'react-hooks/set-state-in-effect': 'error',
  'react-hooks/error-boundaries': 'error',
  'react-hooks/purity': 'error',
  'react-hooks/set-state-in-render': 'error',
  'react-hooks/unsupported-syntax': 'warn',
};

// Type-aware rules that ride in on the `correctness` category, listed so the JS override below can
// switch them off with the rest.
const correctnessTypeAware = [
  'typescript/no-meaningless-void-operator',
  'typescript/no-misused-spread',
  'typescript/no-unnecessary-parameter-property-assignment',
  'typescript/no-useless-default-assignment',
  'typescript/no-useless-empty-export',
  'typescript/require-array-sort-compare',
];

// Plain JS has no tsconfig program, so type-aware rules read every value as `any`.
const typeAwareOff = Object.fromEntries(
  Object.keys(recommendedTypeChecked)
    .filter((rule) => rule.startsWith('typescript/'))
    .concat(correctnessTypeAware, [
      'typescript/consistent-type-imports',
      'func-style',
      'prefer-arrow-callback',
    ])
    .map((rule) => [rule, 'off']),
);

export default {
  plugins: ['typescript', 'react', 'unicorn', 'oxc', 'import'],

  ignorePatterns: ['**/out/**', '**/dist/**', '**/node_modules/**', 'spikes/logs/**'],

  // Only the bug-catching slice of each plugin; the style categories are noise.
  categories: { correctness: 'error' },

  rules: {
    ...recommendedTypeChecked,
    'func-style': ['error', 'expression', { allowArrowFunctions: true }],
    'prefer-arrow-callback': 'error',
    'typescript/consistent-type-imports': [
      'error',
      { prefer: 'type-imports', fixStyle: 'inline-type-imports' },
    ],
    'import/no-duplicates': 'error',
    'unicorn/prefer-at': 'error',
  },

  overrides: [
    {
      files: ['**/*.{js,mjs,cjs}'],
      rules: typeAwareOff,
    },
    {
      files: ['src/renderer/**/*.{ts,tsx}'],
      rules: reactHooksRecommended,
    },
  ],
};
