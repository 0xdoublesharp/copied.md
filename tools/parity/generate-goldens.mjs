import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '../..');
const require = createRequire(resolve(repoRoot, 'apps/cli/package.json'));
const esbuild = require('esbuild');

const outBundle = resolve(
  '/Volumes/Scratch/copied-md/tmp',
  'copied-md-oracle.cjs',
);
mkdirSync(dirname(outBundle), { recursive: true });
await esbuild.build({
  entryPoints: [resolve(repoRoot, 'packages/converters/src/index.ts')],
  bundle: true,
  platform: 'node',
  format: 'cjs',
  outfile: outBundle,
});

const { convertMarkdown, targets } = require(outBundle);

const fidelityFixture = readFileSync(
  resolve(repoRoot, 'packages/converters/src/__tests__/fixtures/clipboard-fidelity.md'),
  'utf8',
);

const cases = [
  { name: 'empty', markdown: '' },
  { name: 'whitespace-only', markdown: '   \n\t\uFEFF  ' },
  { name: 'clipboard-fidelity', markdown: fidelityFixture },
  {
    name: 'headings-and-styles',
    markdown:
      '# Heading 1\n\n## Heading 2\n\n#### Heading 4\n\n**Bold**, *italic*, ***bold italic***, ~~strike~~, `inline <code &>`.\n\n# **Bold inside heading** and *italic*',
  },
  {
    name: 'links-and-urls',
    markdown:
      '[Safe](https://example.org/docs?a=1&b=2#top) [Rel](./notes/page) [Frag](#intro) [Mail](mailto:hi@example.org) [Bad](javascript:alert(1)) [Data](data:text/html,hi) [Ref][r]\n\n[r]: https://example.org/ref "Title"',
  },
  {
    name: 'images-and-linked-images',
    markdown:
      '![Icon](https://example.org/icon.png)\n\n[![Asset](./asset.png)](./manual)\n\n<a href="https://example.org/manual"><img src="./asset.png" alt="Manual"></a>',
  },
  {
    name: 'lists-and-tasks',
    markdown:
      '3. First item\n   - Nested bullet\n   - [x] Done task\n   - [ ] Open task\n4. Second item\n\n   Continuation paragraph.',
  },
  {
    name: 'footnotes-and-hr',
    markdown:
      'Alpha[^b] and beta[^a] and again[^b].\n\n---\n\n[^a]: First defined\n[^b]: First referenced',
  },
  {
    name: 'tables-gfm-and-html',
    markdown:
      '| Name | Value | Link |\n| --- | --- | --- |\n| A\\|B | `x < y` | [Docs](https://example.org/docs) |\n| Café | **100%** | Plain |\n\n<table><caption>Raw Caption</caption><tr><th>H1</th><th>H2</th></tr><tr><td>A<br>B</td><td><a href="javascript:alert(1)">Bad</a> <a href="https://example.org/ok">OK</a></td></tr></table>',
  },
  {
    name: 'code-blocks-and-backticks',
    markdown:
      'Inline ``a`b`` and `plain`.\n\n```rust\nfn main() {\n    println!("<hello> & \t0");\n}\n```\n\n~~~\nFence with ``` inside\n~~~',
  },
  {
    name: 'blockquotes-nested',
    markdown: '> Outer quote\n>\n> > Inner quote\n>\n> After inner',
  },
  {
    name: 'signal-spoilers-valid',
    markdown:
      'Before <span data-spoiler="true">**secret** `code`</span> after.\n\n<div>Block <span data-spoiler="true">hidden\nline</span> public.</div>\n\nLiteral `<span data-spoiler="true">not spoiler</span>`.',
  },
  {
    name: 'signal-spoilers-malformed',
    markdown: 'Before **<span data-spoiler="true">private** text</span>',
  },
  {
    name: 'telegram-rtf-unicode-and-tabs',
    markdown:
      '# Café 🚀\n\n**Café ☕** `{x} \\b` [safe](<https://example.com/café?q="x"&y=z>)\n\n```\n  <script>\n\t0\n```',
  },
  {
    name: 'x-weighted-length',
    markdown: '漢'.repeat(140) + ' https://example.com/very/long/path/here 🎉 **bold**',
  },
  {
    name: 'raw-html-sanitization',
    markdown:
      '<script>alert(1)</script><style>body{}</style><iframe src="https://evil.test"></iframe><div onclick="bad()"><b>Clean</b> <a href="vbscript:x">Unsafe</a></div>',
  },
];

const optionCombos = [
  { name: 'default', options: {} },
  { name: 'with-source-url', options: { sourceUrl: 'https://example.org/docs/guide' } },
  { name: 'decorative-unicode', options: { decorativeUnicode: true } },
  {
    name: 'source-url-and-unicode',
    options: { sourceUrl: 'https://example.org/docs/guide', decorativeUnicode: true },
  },
];

const output = {
  oracleTag: 'ts-oracle-v1',
  targets,
  cases: cases.map(({ name, markdown }) => ({
    name,
    markdown,
    runs: optionCombos.map(({ name: optName, options }) => ({
      optionSet: optName,
      options,
      results: Object.fromEntries(
        targets.map(({ id }) => [id, convertMarkdown(markdown, id, options)]),
      ),
    })),
  })),
};

const outPath = resolve(repoRoot, 'crates/copied-core/tests/golden/corpus.json');
mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, JSON.stringify(output, null, 2) + '\n', 'utf8');
console.log(
  `Wrote ${cases.length} cases x ${optionCombos.length} option sets x ${targets.length} targets = ${cases.length * optionCombos.length * targets.length} golden outputs to ${outPath}`,
);
