<template>
  <main class="container">
    <h1 class="title">Modbus TCP Client</h1>
    <p class="subtitle">
      Начнём с чистого экрана. Здесь будет интерфейс клиента Modbus TCP.
    </p>
  </main>
</template>

<script setup lang="ts">
// Пока без логики. Далее добавим поля ввода и управление соединением.
</script>

<style scoped>
.container {
  margin: 0;
  min-height: 100vh;
  padding: 5vh 5vw;
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  align-items: flex-start;
  background-color: #f6f6f6;
  color: #0f0f0f;
  box-sizing: border-box;
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
}

.title {
  font-size: 2rem;
  margin-bottom: 0.5rem;
}

.subtitle {
  margin-top: 0;
  color: #555;
  max-width: 600px;
}
</style>
