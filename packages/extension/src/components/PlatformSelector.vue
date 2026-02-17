<script setup lang="ts">
import { Platform } from '@copied-md/converters';
import { useUiStore } from '@/stores/ui.store';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

const ui = useUiStore();

const platforms = [
  { id: Platform.Facebook, label: 'Facebook', icon: '📘' },
  { id: Platform.Twitter, label: 'Twitter/X', icon: '𝕏' },
  { id: Platform.Slack, label: 'Slack', icon: '💬' },
  { id: Platform.Discord, label: 'Discord', icon: '🎮' },
  { id: Platform.Telegram, label: 'Telegram', icon: '✈' },
  { id: Platform.SMS, label: 'SMS', icon: '💭' },
  { id: Platform.Reddit, label: 'Reddit', icon: '🔴' },
  { id: Platform.HTML, label: 'HTML', icon: '</>' },
];
</script>

<template>
  <div class="grid grid-cols-4 gap-2">
    <Button
      v-for="platform in platforms"
      :key="platform.id"
      :variant="ui.selectedPlatform === platform.id ? 'default' : 'outline'"
      size="sm"
      :class="
        cn(
          'flex flex-col items-center gap-1 h-auto py-2 text-xs',
          ui.selectedPlatform === platform.id && 'ring-2 ring-ring',
        )
      "
      @click="ui.selectPlatform(platform.id)"
    >
      <span class="text-base">{{ platform.icon }}</span>
      <span>{{ platform.label }}</span>
    </Button>
  </div>
</template>
