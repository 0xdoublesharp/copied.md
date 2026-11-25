export const Platform = {
  Facebook: 'facebook',
  Twitter: 'twitter',
  Slack: 'slack',
  Discord: 'discord',
  Telegram: 'telegram',
  SMS: 'sms',
  Reddit: 'reddit',
  HTML: 'html',
} as const;

export type Platform = (typeof Platform)[keyof typeof Platform];

export interface ClipboardPayload {
  'text/plain': string;
  'text/html'?: string;
  'text/rtf'?: string;
  'text/signal'?: string;
}

export interface ConversionResult {
  payload: ClipboardPayload;
  truncated: boolean;
  originalLength: number;
}

export interface PlatformConverter {
  readonly platform: Platform;
  readonly label: string;
  readonly maxLength?: number;
  convert(markdown: string): ConversionResult;
}

export interface Diagnostic {
  code:
    | 'unsupported-url'
    | 'relative-url'
    | 'formatting-omitted'
    | 'length-exceeded'
    | 'spoiler-omitted';
  severity: 'warning' | 'info';
  message: string;
}
