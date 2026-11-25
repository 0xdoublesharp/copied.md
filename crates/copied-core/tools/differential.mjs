// Builds a differential corpus: random Markdown (the invariants.test.ts
// generators plus fixtures from the TypeScript test suite) converted by the
// ts-oracle-v1 engine for every target and option set.
//
// Usage: node crates/copied-core/tools/differential.mjs <oracle.cjs> <out.json> [runs] [seed]
// Build oracle.cjs with esbuild from packages/converters/src/index.ts.
import { createRequire } from 'node:module';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const require = createRequire(import.meta.url);
const [oraclePath, outPath, runsArg, seedArg] = process.argv.slice(2);
const { convertMarkdown, targets } = require(resolve(oraclePath));
const fc = require(resolve('packages/converters/node_modules/fast-check'));

const fragments = [
  '# Release check', '> Quote\n>\n> Second paragraph', '- [x] Task\n- **Bold** and *italic*\n  3. Nested',
  '| A | B |\n|---|---|\n| a | b |', '~~~ts\nconst a = "<x>";\n~~~', '[Link][ref]\n\n[ref]: ./notes "Title"',
  '[^x] and again [^x]\n\n[^x]: footnote', '![Icon](https://example.org/icon.png)',
  '[![Image][asset]][manual]\n\n[asset]: ./asset.png\n[manual]: ./manual',
  '<a href="https://example.org/manual"><img src="./asset.png" alt="Image"></a>',
  '[Relative](./notes) [Fragment](#intro) [Mail](mailto:hi@example.org)', '[Unsafe](javascript:alert%281%29)',
  '<script>ignored()</script><b>HTML</b>', 'one  \ntwo ~~three~~', 'Before <span data-spoiler="true">covered</span> after.',
  'Unclosed <span data-spoiler="true">covered', '👩🏽‍💻 🇺🇦 é 𝄞 漢字 & < > \\ { }', '---',
  // Extra coverage beyond invariants.test.ts.
  '1. a\n\n   b\n2. c', '* x\n+ y\n- z', '`` a`b `` and ` `` `', 'Setext\n===', 'H2\n---\n\ntext',
  '<details><summary>S</summary>Body</details>', '<table><tr><td>a|b</td></tr></table>', 'a <br> b',
  'www.example.com and https://x.test/a_(b)', 'x@example.com', '***strong em*** _a_ __b__', '\\* \\_ \\[ \\]',
  '<span class="x">a</span> <SPAN DATA-SPOILER=\'TRUE\'>b</SPAN>', '<div>Before <span data-spoiler="true">hidden\ntext</span> after.</div>',
  'Before **<span data-spoiler="true">private** text</span>', '| a | b |\n|:-|-:|\n| `x|y` | **z** |\n| c |',
  '> - q\n>   ```\n>   code\n>   ```', '#### Deep\n\n###### Deeper', 'a&amp;b &copy; &#x26;', '<!-- comment -->text',
  '<textarea>**x**</textarea> after', '<p onclick="x()">p</p><style>s</style>', '[a](<b c> "t") ![i](x.png \'q\')',
  'line\\\nbreak', '- [ ] \n- [x]', '- a\n- b\n\n\n\nc', '- a\n\n\n- b', '- a\n  - n\n\n  b', '- a\n\n  ```\n  x\n\n  y\n  ```\n- c',
  '1. a\n   > q\n\n   r', '- a\n  <div>\n\n  b</div>', 'nul\u0000here', 'Term[^long-note]\n\n[^long-note]: Para one.\n\n    Para two.',
];
const markdown = fc
  .array(fc.oneof(fc.constantFrom(...fragments), fc.string({ unit: 'binary', maxLength: 60 }), fc.string({ unit: 'grapheme', maxLength: 40 })), { maxLength: 8 })
  .map((parts) => parts.join('\n\n'));
const samples = fc.sample(markdown, { numRuns: Number(runsArg ?? 300), seed: Number(seedArg ?? 17041) });
const inputs = [...fragments, ...samples];
const optionSets = [{}, { sourceUrl: 'https://example.org/docs/page', decorativeUnicode: true }];
const out = [];
for (const md of inputs) {
  for (const options of optionSets) {
    const results = {};
    for (const { id } of targets) results[id] = convertMarkdown(md, id, options);
    out.push({ markdown: md, options, results });
  }
}
writeFileSync(outPath, JSON.stringify(out));
console.error(`wrote ${out.length} runs to ${outPath}`);
