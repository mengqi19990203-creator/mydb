<script setup lang="ts">
defineProps<{ size?: number }>();
</script>

<template>
  <svg
    :width="size ?? 24"
    :height="size ?? 24"
    viewBox="0 0 24 24"
    fill="none"
    aria-hidden="true"
  >
    <defs>
      <linearGradient id="mydb-mark-bg" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="#0A1120" />
        <stop offset="1" stop-color="#182744" />
      </linearGradient>
      <linearGradient id="mydb-mark-cylinder" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="#3D7DFF" />
        <stop offset="1" stop-color="#8AB6FF" />
      </linearGradient>
    </defs>
    <rect
      x="1"
      y="1"
      width="22"
      height="22"
      rx="6"
      fill="url(#mydb-mark-bg)"
      stroke="rgba(255,255,255,0.12)"
    />
    <path
      d="M4.2 7.2v9.1c0 1.2 1.9 2.2 4.3 2.2s4.3-1 4.3-2.2V7.2"
      fill="url(#mydb-mark-cylinder)"
    />
    <ellipse cx="8.5" cy="7.2" rx="4.3" ry="1.6" fill="#B3D0FF" />
    <path
      d="M4.5 10.7c.6.9 2.2 1.6 4 1.6s3.4-.7 4-1.6M4.5 13.8c.6.9 2.2 1.6 4 1.6s3.4-.7 4-1.6"
      stroke="#122647"
      stroke-opacity="0.6"
      stroke-width="1"
      stroke-linecap="round"
    />
    <path
      d="M17.2 8.2h2.4M18.4 8.2v7.6M17.2 15.8h2.4"
      stroke="#3FB950"
      stroke-width="1.6"
      stroke-linecap="round"
    />
  </svg>
</template>