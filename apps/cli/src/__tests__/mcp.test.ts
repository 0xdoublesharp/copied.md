import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory.js';
import { expect, it, vi } from 'vitest';
import { serveMcp } from '../mcp';

const { stdio } = vi.hoisted(() => ({
  stdio: { transport: undefined as InMemoryTransport | undefined },
}));
vi.mock('@modelcontextprotocol/sdk/server/stdio.js', () => ({
  StdioServerTransport: vi.fn(function () {
    return stdio.transport;
  }),
}));

it('serves the target list, conversions and input validation over MCP', async () => {
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  stdio.transport = serverTransport;
  const client = new Client({ name: 'copied-md-test', version: '1.0.0' });
  await serveMcp();
  await client.connect(clientTransport);
  try {
    const { tools } = await client.listTools();
    expect(tools.map(({ name }) => name)).toEqual(['list_copy_targets', 'convert_markdown']);
    expect(tools.every(({ annotations }) => annotations?.readOnlyHint)).toBe(true);

    const list = await client.callTool({ name: 'list_copy_targets' });
    expect(list.structuredContent).toMatchObject({
      targets: expect.arrayContaining([expect.objectContaining({ id: 'telegram' })]),
    });
    expect(list.content).toEqual([{ type: 'text', text: JSON.stringify(list.structuredContent) }]);

    const conversion = await client.callTool({
      name: 'convert_markdown',
      arguments: {
        markdown: '[Notes](./notes)',
        target: 'telegram',
        sourceUrl: 'https://example.org/posts/a',
        decorativeUnicode: false,
      },
    });
    expect(conversion.structuredContent).toMatchObject({
      schemaVersion: 1,
      target: 'telegram',
      representations: {
        'text/plain': 'Notes (https://example.org/posts/notes)',
        'text/html': expect.stringContaining('href="https://example.org/posts/notes"'),
        'text/rtf': expect.stringContaining('HYPERLINK'),
      },
    });
    expect(conversion.content).toEqual([
      { type: 'text', text: JSON.stringify(conversion.structuredContent) },
    ]);

    for (const args of [
      { markdown: '# Hi', target: 'unknown' },
      { markdown: false, target: 'telegram' },
      { markdown: '# Hi', target: 'telegram', sourceUrl: '/relative' },
    ]) {
      const result = await client.callTool({ name: 'convert_markdown', arguments: args });
      expect(result.isError).toBe(true);
      expect(result.structuredContent).toBeUndefined();
    }
  } finally {
    await client.close();
    await serverTransport.close();
  }
});
