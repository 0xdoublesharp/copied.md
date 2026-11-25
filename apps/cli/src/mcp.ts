import { convertMarkdown, targets, type CopyTarget } from '@0xdoublesharp/copied-md-converters';
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';

const annotations = {
  readOnlyHint: true,
  destructiveHint: false,
  idempotentHint: true,
  openWorldHint: false,
};

export async function serveMcp() {
  const server = new McpServer({ name: 'copied-md', version: '2.0.0' });
  server.registerTool(
    'list_copy_targets',
    {
      description: 'List the available Markdown conversion targets.',
      annotations,
    },
    () => ({
      content: [{ type: 'text', text: JSON.stringify({ targets }) }],
      structuredContent: { targets },
    }),
  );
  server.registerTool(
    'convert_markdown',
    {
      description:
        'Convert supplied Markdown into clipboard representations for a chosen destination.',
      inputSchema: {
        markdown: z.string(),
        target: z.enum(targets.map(({ id }) => id)),
        sourceUrl: z.url().optional(),
        decorativeUnicode: z.boolean().optional(),
      },
      annotations,
    },
    ({ markdown, target, sourceUrl, decorativeUnicode }) => {
      const converted = convertMarkdown(markdown, target as CopyTarget, {
        sourceUrl,
        decorativeUnicode,
      });
      return {
        content: [{ type: 'text', text: JSON.stringify(converted) }],
        structuredContent: { ...converted },
      };
    },
  );
  await server.connect(new StdioServerTransport());
}
