import twitterText from 'twitter-text';
import { EXIT, visit } from 'unist-util-visit';
import { readMarkdown } from './document';
import { renderMarkdown } from './render-markdown';
import { renderText } from './render-text';
import { renderRichHtml } from './rich-html';
import type { ClipboardPayload, Diagnostic } from './types';

export const targets = [
  { id: 'rich-text', label: 'Rich Text', rich: true },
  { id: 'plain-text', label: 'Plain Text', rich: false },
  { id: 'html-source', label: 'HTML Source', rich: false },
  { id: 'slack-paste', label: 'Slack', rich: true },
  { id: 'discord', label: 'Discord', rich: false },
  { id: 'telegram', label: 'Telegram', rich: true },
  { id: 'reddit', label: 'Reddit', rich: false },
  { id: 'facebook', label: 'Facebook', rich: false },
  { id: 'twitter', label: 'X', rich: false },
  { id: 'sms', label: 'SMS', rich: false },
  { id: 'slack-api', label: 'Slack API (mrkdwn)', rich: false },
] as const;

export type CopyTarget = (typeof targets)[number]['id'];

export interface ConversionOptions {
  sourceUrl?: string;
  decorativeUnicode?: boolean;
}

export interface MarkdownConversion {
  schemaVersion: 1;
  target: CopyTarget;
  representations: ClipboardPayload;
  metrics: {
    sourceCodeUnits: number;
    outputLength: number;
    unit: 'code-points' | 'x-weighted-characters';
    limit?: number;
  };
  diagnostics: Diagnostic[];
}

export function resolveTarget(value: string): CopyTarget | undefined {
  if (value === 'html') return 'rich-text';
  if (value === 'slack') return 'slack-paste';
  return targets.find((item) => item.id === value)?.id;
}

export function convertMarkdown(
  markdown: string,
  target: CopyTarget,
  options: ConversionOptions = {},
): MarkdownConversion {
  const profile = targets.find((item) => item.id === target);
  if (!profile) throw new RangeError('Unknown copy target: ' + target);
  const { tree, diagnostics } = readMarkdown(markdown, options.sourceUrl);
  const markdownTarget = target === 'discord' || target === 'reddit';
  let hasImages = false;
  visit(tree, 'image', () => {
    hasImages = true;
    return EXIT;
  });
  const plain = markdownTarget
    ? renderMarkdown(tree, target)
    : renderText(tree, {
        slackApi: target === 'slack-api',
        decorativeUnicode:
          options.decorativeUnicode &&
          (target === 'facebook' || target === 'twitter' || target === 'telegram'),
      });
  const representations: ClipboardPayload = { 'text/plain': plain };
  if (profile.rich || target === 'html-source') {
    const html = renderRichHtml(tree, options.sourceUrl);
    if (target === 'html-source') representations['text/plain'] = html;
    else representations['text/html'] = html;
    if (hasImages)
      diagnostics.push({
        code: 'formatting-omitted',
        severity: 'info',
        message: 'Images are represented as links.',
      });
  }

  const output = representations['text/plain'];
  const weighted = target === 'twitter' ? twitterText.parseTweet(output).weightedLength : undefined;
  if (weighted !== undefined && weighted > 280) {
    diagnostics.push({
      code: 'length-exceeded',
      severity: 'warning',
      message: 'This exceeds the standard X post limit of 280 weighted characters.',
    });
  }

  return {
    schemaVersion: 1,
    target,
    representations,
    metrics: {
      sourceCodeUnits: markdown.length,
      outputLength: weighted ?? Array.from(output).length,
      unit: weighted === undefined ? 'code-points' : 'x-weighted-characters',
      ...(weighted !== undefined ? { limit: 280 } : {}),
    },
    diagnostics,
  };
}
