import boundaries from 'eslint-plugin-boundaries'
import functional from 'eslint-plugin-functional'
import oxlint from 'eslint-plugin-oxlint'
import preferArrowFunctions from 'eslint-plugin-prefer-arrow-functions'
import { defineConfig } from 'eslint/config'
import tseslint from 'typescript-eslint'

// ── Boundary mode ──────────────────────────────────────────────
// "feature" → src/orders/domain.ts, src/orders/use-cases.ts, ...
// "layer"   → src/domain/orders.ts, src/application/orders.ts, ...
// "none"    → no boundary enforcement
const BOUNDARY_MODE = 'feature'

// ── Boundary element definitions per mode ──────────────────────
const boundaryElements = {
  feature: [
    { type: 'domain', pattern: ['src/*/domain.ts', 'src/*/domain/**'] },
    { type: 'application', pattern: ['src/*/use-cases.ts', 'src/*/logic.ts', 'src/*/ports.ts'] },
    { type: 'infrastructure', pattern: ['src/*/repo.ts', 'src/*/io.ts', 'src/*/adapter.ts', 'src/cli/**'] },
    { type: 'shared', pattern: ['src/shared/**'] },
    { type: 'composition', pattern: ['src/main.ts'] },
    { type: 'test', pattern: ['src/**/*.spec.ts', 'src/test/**'] },
  ],
  layer: [
    { type: 'domain', pattern: ['src/domain/**'] },
    { type: 'application', pattern: ['src/application/**'] },
    { type: 'infrastructure', pattern: ['src/infrastructure/**'] },
    { type: 'shared', pattern: ['src/shared/**'] },
    { type: 'composition', pattern: ['src/main.ts'] },
    { type: 'test', pattern: ['src/**/*.spec.ts', 'src/test/**'] },
  ],
}

// Dependency inversion: inner layers cannot import outer layers
const boundaryRules = [
  { from: 'domain', allow: ['domain', 'shared'] },
  { from: 'application', allow: ['domain', 'application', 'shared'] },
  { from: 'infrastructure', allow: ['domain', 'application', 'infrastructure', 'shared'] },
  { from: 'composition', allow: ['domain', 'application', 'infrastructure', 'shared', 'composition'] },
  { from: 'test', allow: ['domain', 'application', 'infrastructure', 'shared', 'test'] },
]

// Domain file globs (used for purity + FP strictness overrides)
const domainFiles = BOUNDARY_MODE === 'feature' ?
  ['src/*/domain.ts', 'src/*/domain/**/*.ts'] :
  ['src/domain/**/*.ts']

// Infra file globs (relaxed FP rules)
const infraFiles = BOUNDARY_MODE === 'feature' ?
  ['src/*/repo.ts', 'src/*/io.ts', 'src/*/adapter.ts', 'src/cli/**/*.ts'] :
  ['src/infrastructure/**/*.ts']

// ── Functional rules (global defaults) ─────────────────────────
// These are the core FP rules that oxlint cannot replicate.
// ESLint owns all functional/* enforcement.
const functionalRules = {
  'functional/no-loop-statements': 'error',
  'functional/no-try-statements': 'error',
  'functional/no-throw-statements': 'error',
  'functional/no-let': ['error', { allowInForLoopInit: false }],
  'functional/no-classes': 'error',
  'functional/no-this-expressions': 'error',
  'functional/immutable-data': ['warn', { ignoreImmediateMutation: true, ignoreClasses: true }],
  'functional/prefer-readonly-type': 'warn',
  'functional/no-expression-statements': ['error', {
    ignoreVoid: true,
    ignoreCodePattern: ['^expect', '^assert'],
  }],
  'functional/no-return-void': 'error',
}

// ── AST selector bans (oxlint cannot do ESQuery selectors) ─────
const restrictedSyntax = ['error', {
  selector: ":function > Identifier.params[typeAnnotation.typeAnnotation.type='TSBooleanKeyword']",
  message: 'Boolean params are banned. Split into separate functions.',
}, {
  selector: "CallExpression[callee.property.name='_unsafeUnwrap']",
  message: 'Use .map(), .andThen(), or .match() instead.',
}, {
  selector: "CallExpression[callee.property.name='_unsafeUnwrapErr']",
  message: 'Use .mapErr(), .andThen(), or .match() instead.',
}]

// ── Domain purity: ban IO imports in domain files ──────────────
const domainImportBans = {
  patterns: [{
    group: ['fs', 'fs/*', 'path', 'http', 'https', 'net', 'child_process', 'crypto'],
    message: 'Domain must be pure. No Node.js IO.',
  }, {
    group: ['express', 'fastify', 'koa', 'hono', 'elysia'],
    message: 'Domain must be pure. No HTTP frameworks.',
  }, {
    group: ['pg', 'mysql*', 'redis', 'ioredis', 'mongoose', '@prisma/*', 'kysely'],
    message: 'Domain must be pure. No database drivers.',
  }, {
    group: ['@aws-sdk/*', '@azure/*', '@google-cloud/*'],
    message: 'Domain must be pure. No cloud SDKs.',
  }],
}

// ── Build config array ─────────────────────────────────────────
const config = [
  { ignores: ['node_modules/**', 'dist/**', 'eslint.config.mjs'] },

  // Global: all source files
  {
    files: ['src/**/*.ts'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    plugins: {
      functional,
      'prefer-arrow-functions': preferArrowFunctions,
      ...(BOUNDARY_MODE !== 'none' && { boundaries }),
    },
    ...(BOUNDARY_MODE !== 'none' && { settings: { 'boundaries/elements': boundaryElements[BOUNDARY_MODE] } }),
    rules: {
      ...functionalRules,
      'no-restricted-syntax': restrictedSyntax,

      // Arrow function enforcement: ESLint owns the *semantic* rule
      // (must use arrows). dprint owns arrow *formatting* (parens, wrapping).
      'prefer-arrow-functions/prefer-arrow-functions': ['error', {
        allowNamedFunctions: false,
        classPropertiesAllowed: false,
        disallowPrototype: true,
        returnStyle: 'unchanged',
        singleReturnOnly: false,
      }],

      // Boundaries (only when enabled)
      ...(BOUNDARY_MODE !== 'none' &&
        { 'boundaries/element-types': ['error', { default: 'disallow', rules: boundaryRules }] }),
    },
  },

  // Domain purity: ban IO imports
  {
    files: domainFiles,
    rules: {
      'no-restricted-imports': ['error', domainImportBans],
      'functional/prefer-readonly-type': 'error',
    },
  },

  // Tests: fully relax FP rules
  {
    files: ['src/**/*.spec.ts', 'src/test/**/*.ts'],
    rules: {
      'functional/no-let': 'off',
      'functional/no-loop-statements': 'off',
      'functional/no-try-statements': 'off',
      'functional/no-throw-statements': 'off',
      'functional/no-expression-statements': 'off',
      'functional/no-return-void': 'off',
      'functional/immutable-data': 'off',
      'functional/no-classes': 'off',
      'functional/no-this-expressions': 'off',
      'no-restricted-syntax': 'off',
    },
  },

  // Infrastructure: imperative shell allowances
  {
    files: infraFiles,
    rules: {
      'functional/no-let': 'warn',
      'functional/no-loop-statements': 'warn',
      'functional/no-try-statements': 'off',
      'functional/no-throw-statements': 'off',
      'functional/no-expression-statements': 'off',
      'functional/no-return-void': 'off',
      'functional/immutable-data': 'off',
      'functional/no-classes': ['error', {
        ignoreIdentifierPattern: '^.*(Controller|Adapter|Module|Client|Provider|Gateway)$',
      }],
      'functional/no-this-expressions': 'off',
    },
  },

  // Deduplicate: turn off anything oxlint already handles
  ...oxlint.buildFromOxlintConfigFile('./.oxlintrc.json'),
]

// eslint-disable no-default-export
export default defineConfig(...config)
