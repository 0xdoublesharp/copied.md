export enum Platform {
  Facebook = 'facebook',
  Twitter = 'twitter',
  Slack = 'slack',
  Discord = 'discord',
  Telegram = 'telegram',
  SMS = 'sms',
  Reddit = 'reddit',
  HTML = 'html',
}

export interface ClipboardPayload {
  'text/plain': string;
  'text/html'?: string;
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
