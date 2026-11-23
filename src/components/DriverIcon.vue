<script setup lang="ts">
import type { Driver } from "../lib/types";
import mysqlIcon from "../assets/drivers/mysql.svg";
import postgresIcon from "../assets/drivers/postgresql.svg";
import redisIcon from "../assets/drivers/redis.svg";
import sqliteIcon from "../assets/drivers/sqlite.svg";

defineProps<{
  driver: Driver;
  size?: number;
}>();

const ICONS: Record<Driver, string> = {
  mysql: mysqlIcon,
  postgres: postgresIcon,
  redis: redisIcon,
  sqlite: sqliteIcon,
};
</script>

<template>
  <img
    class="driver-icon"
    :src="ICONS[driver]"
    :width="size ?? 18"
    :height="size ?? 18"
    alt=""
    aria-hidden="true"
    draggable="false"
  />
</template>