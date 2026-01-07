<template>
    <main class="container">
        <h1 class="title">Modbus TCP Slave Simulator</h1>

        <section class="card">
            <header class="card-header">
                <h2 class="card-title">
                    Параметры подключения (ModbusConnectionProfile)
                </h2>
                <p class="card-subtitle">
                    Укажи адрес сервера, порт и Unit ID (адрес устройства). Эти
                    данные будут сохранены локально и восстановятся при
                    следующем запуске.
                </p>
            </header>

            <form class="form-grid" @submit.prevent="onSaveProfile">
                <div class="form-row">
                    <label for="profile-name">Название профиля</label>
                    <input
                        id="profile-name"
                        v-model="editableProfile.name"
                        type="text"
                        placeholder="Например: Локальный сервер"
                    />
                </div>

                <div class="form-row">
                    <label for="profile-host">IP / хост</label>
                    <input
                        id="profile-host"
                        v-model="editableProfile.host"
                        type="text"
                        placeholder="127.0.0.1"
                    />
                </div>

                <div class="form-row">
                    <label for="profile-port">Порт</label>
                    <input
                        id="profile-port"
                        v-model.number="editableProfile.port"
                        type="number"
                        min="1"
                        max="65535"
                        placeholder="502"
                    />
                </div>

                <div class="form-row">
                    <label for="profile-unit-id">Unit ID (Slave ID)</label>
                    <input
                        id="profile-unit-id"
                        v-model.number="editableProfile.unitId"
                        type="number"
                        min="0"
                        max="247"
                        placeholder="1"
                    />
                </div>

                <div class="form-row checkbox-row">
                    <label class="checkbox-label" for="profile-auto-reconnect">
                        <input
                            id="profile-auto-reconnect"
                            v-model="editableProfile.autoReconnect"
                            type="checkbox"
                        />
                        Автоматически переподключаться
                    </label>
                </div>

                <div v-if="validationError" class="validation-error">
                    {{ validationError }}
                </div>

                <div class="form-actions">
                    <button class="btn primary" type="submit">
                        Сохранить профиль
                    </button>
                    <button
                        class="btn secondary"
                        type="button"
                        @click="onResetToDefaults"
                    >
                        Сбросить по умолчанию
                    </button>
                </div>

                <p v-if="lastSavedAt" class="save-info">
                    Последнее сохранение: {{ lastSavedAt }}
                </p>
            </form>
        </section>
    </main>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";

/**
 * Типы данных для профиля подключения.
 * Вариант A+1:
 * - один текущий профиль;
 * - один проект (позже добавим переменные).
 */

type ModbusConnectionProfileId = string;

interface ModbusConnectionProfile {
    id: ModbusConnectionProfileId;
    name: string;
    host: string;
    port: number;
    unitId: number;
    autoReconnect: boolean;
}

interface ModbusProject {
    profiles: ModbusConnectionProfile[];
    currentProfileId: ModbusConnectionProfileId | null;
    // variables: ModbusVariable[]; // добавим позже
}

const STORAGE_KEY = "modbus_project_v1";

/**
 * Значения по умолчанию для проекта и профиля
 */
function createDefaultProfile(): ModbusConnectionProfile {
    return {
        id: "default",
        name: "Локальный сервер",
        host: "127.0.0.1",
        port: 502,
        unitId: 1,
        autoReconnect: true,
    };
}

function createDefaultProject(): ModbusProject {
    const defaultProfile = createDefaultProfile();
    return {
        profiles: [defaultProfile],
        currentProfileId: defaultProfile.id,
    };
}

/**
 * Локальное состояние проекта
 */
const project = ref<ModbusProject>(createDefaultProject());

/**
 * Редактируемая копия текущего профиля для формы.
 */
const editableProfile = reactive<ModbusConnectionProfile>(
    createDefaultProfile(),
);

const validationError = ref<string | null>(null);
const lastSavedAt = ref<string | null>(null);

/**
 * Текущий выбранный профиль
 */
const currentProfile = computed<ModbusConnectionProfile | null>(() => {
    const p = project.value;
    if (!p.currentProfileId) return null;
    return p.profiles.find((pr) => pr.id === p.currentProfileId) ?? null;
});

/**
 * Загрузка проекта из localStorage при старте
 */
onMounted(() => {
    try {
        const raw = window.localStorage.getItem(STORAGE_KEY);
        if (!raw) {
            project.value = createDefaultProject();
            applyProfileToEditable(createDefaultProfile());
            return;
        }

        const parsed = JSON.parse(raw) as ModbusProject;

        if (
            !parsed ||
            !Array.isArray(parsed.profiles) ||
            parsed.profiles.length === 0
        ) {
            project.value = createDefaultProject();
            applyProfileToEditable(createDefaultProfile());
            return;
        }

        project.value = parsed;

        const current = currentProfile.value ?? parsed.profiles[0];
        project.value.currentProfileId = current.id;
        applyProfileToEditable(current);
    } catch (e) {
        project.value = createDefaultProject();
        applyProfileToEditable(createDefaultProfile());
    }
});

/**
 * Копирование данных из профиля в редактируемую форму
 */
function applyProfileToEditable(profile: ModbusConnectionProfile) {
    editableProfile.id = profile.id;
    editableProfile.name = profile.name;
    editableProfile.host = profile.host;
    editableProfile.port = profile.port;
    editableProfile.unitId = profile.unitId;
    editableProfile.autoReconnect = profile.autoReconnect;
}

/**
 * Простая валидация полей профиля
 */
function validateProfile(profile: ModbusConnectionProfile): string | null {
    if (!profile.name.trim()) {
        return "Укажите название профиля.";
    }
    if (!profile.host.trim()) {
        return "Укажите IP или хост сервера.";
    }

    if (
        !Number.isFinite(profile.port) ||
        profile.port <= 0 ||
        profile.port > 65535
    ) {
        return "Порт должен быть числом от 1 до 65535.";
    }

    if (
        !Number.isFinite(profile.unitId) ||
        profile.unitId < 0 ||
        profile.unitId > 247
    ) {
        return "Unit ID (Slave ID) должен быть в диапазоне 0–247.";
    }

    return null;
}

/**
 * Сохранение профиля в проект и в localStorage
 */
function onSaveProfile() {
    const err = validateProfile(editableProfile);
    validationError.value = err;
    if (err) return;

    const p = project.value;
    const existingIndex = p.profiles.findIndex(
        (prof) => prof.id === editableProfile.id,
    );

    const toSave: ModbusConnectionProfile = {
        id: editableProfile.id || "default",
        name: editableProfile.name.trim(),
        host: editableProfile.host.trim(),
        port: Number(editableProfile.port),
        unitId: Number(editableProfile.unitId),
        autoReconnect: !!editableProfile.autoReconnect,
    };

    if (existingIndex >= 0) {
        // Обновляем существующий профиль
        p.profiles.splice(existingIndex, 1, toSave);
    } else {
        // Добавляем новый профиль
        p.profiles.push(toSave);
    }

    p.currentProfileId = toSave.id;
    project.value = { ...p };

    persistProject();
}

/**
 * Сброс профиля к значениям по умолчанию
 */
function onResetToDefaults() {
    const def = createDefaultProfile();
    project.value.currentProfileId = def.id;
    project.value.profiles = [def];
    applyProfileToEditable(def);
    validationError.value = null;
    persistProject();
}

/**
 * Сохранение проекта в localStorage
 */
function persistProject() {
    try {
        const payload = JSON.stringify(project.value);
        window.localStorage.setItem(STORAGE_KEY, payload);
        const now = new Date();
        lastSavedAt.value = now.toLocaleString();
    } catch (e) {
        // Здесь можно будет добавить отображение ошибки сохранения, если нужно
    }
}

/**
 * Вотчер на проект (пока автосейв выключен)
 */
watch(
    () => project.value,
    () => {
        // Для автосохранения можно раскомментировать:
        // persistProject();
    },
    { deep: true },
);
</script>

<style scoped>
.container {
    margin: 0;
    min-height: 100vh;
    padding: 3vh 4vw;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    background-color: #f6f6f6;
    color: #0f0f0f;
    box-sizing: border-box;
    font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
}

.title {
    font-size: 2.2rem;
    margin-bottom: 1.5rem;
}

.card {
    background: #ffffff;
    border-radius: 10px;
    padding: 1.5rem 1.75rem;
    box-shadow: 0 2px 8px rgba(15, 15, 15, 0.08);
    max-width: 720px;
}

.card-header {
    margin-bottom: 1rem;
}

.card-title {
    font-size: 1.4rem;
    margin: 0 0 0.4rem;
}

.card-subtitle {
    margin: 0;
    color: #555;
    font-size: 0.95rem;
}

.form-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 0.9rem;
    margin-top: 0.75rem;
}

.form-row {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
}

.form-row label {
    font-size: 0.9rem;
    font-weight: 500;
}

.form-row input[type="text"],
.form-row input[type="number"] {
    border-radius: 6px;
    border: 1px solid #d0d0d0;
    padding: 0.45rem 0.6rem;
    font-size: 0.95rem;
    font-family: inherit;
    outline: none;
    transition:
        border-color 0.15s,
        box-shadow 0.15s;
}

.form-row input[type="text"]:focus,
.form-row input[type="number"]:focus {
    border-color: #396cd8;
    box-shadow: 0 0 0 1px rgba(57, 108, 216, 0.25);
}

.checkbox-row {
    margin-top: 0.4rem;
}

.checkbox-label {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.95rem;
    user-select: none;
}

.checkbox-label input[type="checkbox"] {
    width: 16px;
    height: 16px;
}

.validation-error {
    margin-top: 0.2rem;
    padding: 0.4rem 0.6rem;
    border-radius: 6px;
    background-color: #ffecec;
    color: #b00020;
    font-size: 0.9rem;
}

.form-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.6rem;
    margin-top: 0.4rem;
}

.btn {
    border-radius: 6px;
    border: 1px solid transparent;
    padding: 0.45rem 0.9rem;
    font-size: 0.95rem;
    font-weight: 500;
    font-family: inherit;
    cursor: pointer;
    transition:
        background-color 0.15s,
        border-color 0.15s,
        box-shadow 0.15s;
}

.btn.primary {
    background-color: #396cd8;
    color: #ffffff;
}

.btn.primary:hover {
    background-color: #305bc0;
}

.btn.secondary {
    background-color: #ffffff;
    border-color: #d0d0d0;
    color: #333333;
}

.btn.secondary:hover {
    border-color: #396cd8;
}

.save-info {
    margin: 0.3rem 0 0;
    font-size: 0.85rem;
    color: #666666;
}

@media (max-width: 768px) {
    .container {
        padding: 2vh 3vw;
    }

    .card {
        padding: 1.25rem 1.3rem;
    }
}
</style>
