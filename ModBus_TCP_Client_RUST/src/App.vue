<template>
    <main class="container">
        <h1 class="title">Modbus TCP Slave Simulator</h1>

        <!-- Управление сервером -->
        <section class="card server-control-card">
            <header class="card-header">
                <h2 class="card-title">Управление эмулятором</h2>
            </header>

            <div class="server-status-row">
                <div
                    class="status-indicator"
                    :class="serverStatus.running ? 'running' : 'stopped'"
                >
                    <span class="status-dot"></span>
                    <span class="status-text">
                        {{ serverStatus.running ? "Запущен" : "Остановлен" }}
                    </span>
                </div>

                <div v-if="serverStatus.running" class="status-details">
                    <span>{{ serverStatus.host }}:{{ serverStatus.port }}</span>
                    <span>Unit ID: {{ serverStatus.unitId }}</span>
                    <span
                        >Подключений: {{ serverStatus.connectionsCount }}</span
                    >
                </div>
            </div>

            <div v-if="serverStatus.error" class="server-error">
                {{ serverStatus.error }}
            </div>

            <div class="server-actions">
                <button
                    v-if="!serverStatus.running"
                    class="btn primary"
                    type="button"
                    :disabled="serverLoading"
                    @click="onStartServer"
                >
                    {{ serverLoading ? "Запуск..." : "▶ Запустить эмулятор" }}
                </button>
                <button
                    v-else
                    class="btn danger"
                    type="button"
                    :disabled="serverLoading"
                    @click="onStopServer"
                >
                    {{
                        serverLoading ? "Остановка..." : "■ Остановить эмулятор"
                    }}
                </button>
            </div>
        </section>

        <!-- Профиль подключения -->
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
                <div class="form-row form-row-grid">
                    <div class="form-field">
                        <label for="profile-name">Название профиля</label>
                        <input
                            id="profile-name"
                            v-model="editableProfile.name"
                            type="text"
                            placeholder="Например: Локальный сервер"
                        />
                    </div>

                    <div class="form-field">
                        <label for="profile-host">IP / хост</label>
                        <input
                            id="profile-host"
                            v-model="editableProfile.host"
                            type="text"
                            placeholder="127.0.0.1"
                        />
                    </div>
                </div>

                <div class="form-row form-row-grid">
                    <div class="form-field">
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

                    <div class="form-field">
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
                </div>

                <div class="form-row checkbox-row">
                    <label class="checkbox-label" for="profile-auto-reconnect">
                        <input
                            id="profile-auto-reconnect"
                            v-model="editableProfile.autoReconnect"
                            type="checkbox"
                        />
                        Автоматически запускать эмулятор при старте
                        (зарезервировано)
                    </label>
                </div>

                <div v-if="validationError" class="validation-error">
                    {{ validationError }}
                </div>

                <div class="form-actions">
                    <button
                        class="btn primary"
                        type="submit"
                        :disabled="!isProfileDirty"
                    >
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

        <!-- Таблица переменных Modbus -->
        <section class="card variables-card">
            <header class="card-header">
                <h2 class="card-title">Переменные Modbus (ModbusVariable)</h2>
                <p class="card-subtitle">
                    Определи список регистров и коилов, которые будет отдавать
                    эмулятор.
                </p>
            </header>

            <!-- Краткая сводка -->
            <div class="vars-summary">
                <span>Всего переменных: {{ project.variables.length }}</span>
            </div>

            <!-- Таблица переменных -->
            <div class="table-wrapper">
                <table class="vars-table">
                    <thead>
                        <tr>
                            <th>#</th>
                            <th>Имя</th>
                            <th>Область</th>

                            <th>Адрес</th>
                            <th>Тип</th>
                            <th>Значение</th>
                            <th>Примечание</th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr
                            v-for="(variable, index) in project.variables"
                            :key="variable.id"
                        >
                            <td>{{ index + 1 }}</td>
                            <td>
                                <input
                                    v-model="variable.name"
                                    type="text"
                                    class="cell-input"
                                    placeholder="Имя"
                                    @change="persistProject"
                                />
                            </td>
                            <td>
                                <select
                                    v-model="variable.area"
                                    class="cell-select"
                                    @change="persistProject"
                                >
                                    <option value="coil">Coil (0x)</option>
                                    <option value="discrete_input">
                                        Discrete Input (1x)
                                    </option>
                                    <option value="input_register">
                                        Input Register (3x)
                                    </option>
                                    <option value="holding_register">
                                        Holding Register (4x)
                                    </option>
                                </select>
                            </td>
                            <td>
                                <input
                                    v-model.number="variable.address"
                                    type="number"
                                    class="cell-input cell-input-number"
                                    min="0"
                                    max="65535"
                                    @change="onVariableAddressChange(variable)"
                                />
                            </td>
                            <td>
                                <select
                                    v-model="variable.dataType"
                                    class="cell-select"
                                    @change="persistProject"
                                >
                                    <option value="bool">bool</option>
                                    <option value="uint16">uint16</option>
                                    <option value="int16">int16</option>
                                    <option value="uint32">uint32</option>
                                    <option value="float32">float32</option>
                                </select>
                            </td>
                            <td
                                class="value-cell"
                                :class="{
                                    'value-changed': recentlyChangedIds.has(
                                        variable.id,
                                    ),
                                }"
                            >
                                <!-- Поле значения: интерпретация по dataType -->
                                <input
                                    v-if="variable.dataType === 'bool'"
                                    v-model="variable.value"
                                    type="checkbox"
                                    class="cell-checkbox"
                                    @change="onVariableValueChange(variable)"
                                />
                                <input
                                    v-else
                                    v-model.number="variable.value"
                                    type="number"
                                    class="cell-input cell-input-number"
                                    @change="onVariableValueChange(variable)"
                                />
                            </td>

                            <td>
                                <input
                                    v-model="variable.note"
                                    type="text"
                                    class="cell-input"
                                    placeholder="Примечание"
                                    @change="persistProject"
                                />
                            </td>
                            <td class="cell-actions">
                                <button
                                    class="btn small danger"
                                    type="button"
                                    @click="onRemoveVariable(variable.id)"
                                >
                                    ✕
                                </button>
                            </td>
                        </tr>

                        <tr v-if="project.variables.length === 0">
                            <td colspan="10" class="empty-row">
                                Пока нет ни одной переменной. Добавь первую с
                                помощью кнопки ниже.
                            </td>
                        </tr>
                    </tbody>
                </table>
            </div>

            <!-- Кнопка добавления переменной -->
            <div class="vars-actions">
                <button
                    class="btn primary"
                    type="button"
                    @click="onAddVariable"
                >
                    + Добавить переменную
                </button>
            </div>
        </section>

        <!-- Логирование запросов -->
        <section class="card logs-card">
            <header class="card-header logs-header">
                <div class="logs-title-row">
                    <h2 class="card-title">Логирование запросов</h2>
                    <span class="logs-count"
                        >{{ logEntries.length }} записей</span
                    >
                </div>
                <div class="logs-actions">
                    <button
                        class="btn small secondary"
                        type="button"
                        @click="onClearLogs"
                        :disabled="logEntries.length === 0"
                    >
                        🗑 Очистить
                    </button>
                </div>
            </header>

            <div class="logs-container" ref="logsContainerRef">
                <div
                    v-for="entry in logEntries"
                    :key="entry.id"
                    class="log-entry"
                    :class="getLogEntryClass(entry)"
                >
                    <div class="log-entry-header">
                        <span class="log-time">{{
                            formatLogTime(entry.timestamp)
                        }}</span>
                        <span class="log-type" :class="entry.entryType">
                            {{ getLogTypeLabel(entry.entryType) }}
                        </span>
                        <span v-if="entry.functionName" class="log-function">
                            {{ entry.functionName }}
                            <span class="log-function-code"
                                >(0x{{
                                    entry.functionCode
                                        ?.toString(16)
                                        .padStart(2, "0")
                                        .toUpperCase()
                                }})</span
                            >
                        </span>
                        <span class="log-client">{{ entry.clientAddr }}</span>
                        <span v-if="entry.durationUs" class="log-duration">
                            {{ (entry.durationUs / 1000).toFixed(2) }} ms
                        </span>
                    </div>
                    <div class="log-entry-summary">{{ entry.summary }}</div>
                    <div v-if="entry.rawData" class="log-entry-raw">
                        <span class="raw-label">HEX:</span>
                        <code>{{ entry.rawData }}</code>
                    </div>
                </div>

                <div v-if="logEntries.length === 0" class="logs-empty">
                    Логи пусты. Запустите эмулятор и подключите
                    мастер-устройство.
                </div>
            </div>
        </section>
    </main>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Значение переменной (зеркало Rust ModbusValue)
 */
type ModbusValue = boolean | number | null;

/**
 * Тип записи лога
 */
type LogEntryType = "request" | "response" | "error" | "info";

/**
 * Запись лога (зеркало Rust LogEntry)
 */
interface LogEntry {
    id: number;
    timestamp: string;
    entryType: LogEntryType;
    clientAddr: string;
    functionCode?: number;
    functionName?: string;
    summary: string;
    rawData?: string;
    durationUs?: number;
}

/**
 * Типы данных для профиля подключения.
 * Вариант A+1:
 * - один текущий профиль;
 * - один проект (сейчас добавляем переменные).
 */

type ModbusConnectionProfileId = string;
type ModbusVariableId = string;

type ModbusArea =
    | "coil"
    | "discrete_input"
    | "input_register"
    | "holding_register";

type ModbusDataType = "bool" | "uint16" | "int16" | "uint32" | "float32";

interface ModbusConnectionProfile {
    id: ModbusConnectionProfileId;
    name: string;
    host: string;
    port: number;
    unitId: number;
    autoReconnect: boolean;
}

interface ModbusVariable {
    id: ModbusVariableId;
    name: string;
    area: ModbusArea;
    /**
     * Адрес регистра/коила.
     * Договоримся использовать 0-based (как в большинстве библиотек Modbus).
     */
    address: number;
    dataType: ModbusDataType;
    /**
     * Текущее значение, которое эмулятор отдаст мастеру.
     * Для разных типов данных будем интерпретировать его по-разному.
     */
    value: number | boolean | null;
    /**
     * Бит внутри регистра (для bool в holding/input_register), опционально.
     */
    bit?: number | null;
    readonly?: boolean;

    /**

         * Текстовое примечание / комментарий к переменной.

         */

    note?: string | null;
}

interface ModbusProject {
    profiles: ModbusConnectionProfile[];
    currentProfileId: ModbusConnectionProfileId | null;
    variables: ModbusVariable[];
}

const STORAGE_KEY = "modbus_project_v1";

/**
 * Server status interface (mirrors Rust ServerStatus)
 */
interface ServerStatus {
    running: boolean;
    host: string;
    port: number;
    unitId: number;
    connectionsCount: number;
    error: string | null;
}

function createDefaultServerStatus(): ServerStatus {
    return {
        running: false,
        host: "0.0.0.0",
        port: 502,
        unitId: 1,
        connectionsCount: 0,
        error: null,
    };
}

/**
 * Значения по умолчанию для проекта, профиля и переменных
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

function createDefaultVariables(): ModbusVariable[] {
    return [];
}

function createDefaultProject(): ModbusProject {
    const defaultProfile = createDefaultProfile();
    return {
        profiles: [defaultProfile],
        currentProfileId: defaultProfile.id,
        variables: createDefaultVariables(),
    };
}

/**
 * Утилита для генерации простых ID.
 * Для локального UI достаточно timestamp + random.
 */
function generateId(prefix: string): string {
    return `${prefix}_${Date.now().toString(36)}_${Math.random()
        .toString(36)
        .slice(2, 6)}`;
}

/**
 * Локальное состояние проекта
 */
const project = reactive<ModbusProject>(createDefaultProject());

/**
 * Редактируемая копия текущего профиля для формы.
 */
const editableProfile = reactive<ModbusConnectionProfile>(
    createDefaultProfile(),
);

const validationError = ref<string | null>(null);
const lastSavedAt = ref<string | null>(null);

/**
 * Server state
 */
const serverStatus = reactive<ServerStatus>(createDefaultServerStatus());
const serverLoading = ref(false);
let statusPollInterval: number | null = null;
let variablesPollInterval: number | null = null;

/**
 * Набор ID переменных, значения которых недавно изменились (для подсветки).
 */
const recentlyChangedIds = reactive(new Set<string>());

/**
 * Записи логов
 */
const logEntries = reactive<LogEntry[]>([]);
const logsContainerRef = ref<HTMLElement | null>(null);
let logUnlisten: UnlistenFn | null = null;

/**
 * Максимальное количество записей в логе
 */
const MAX_LOG_ENTRIES = 500;

/**
 * Текущий выбранный профиль
 */
const currentProfile = computed<ModbusConnectionProfile | null>(() => {
    const p = project;
    if (!p.currentProfileId) return null;
    return p.profiles.find((pr) => pr.id === p.currentProfileId) ?? null;
});

/**
 * Признак того, что профиль в форме отличается от текущего сохранённого.
 * Кнопка "Сохранить профиль" активна только если есть изменения.
 */
const isProfileDirty = computed(() => {
    const current = currentProfile.value;
    if (!current) {
        // Если профиля нет, считаем что форма "грязная", чтобы можно было сохранить.
        return true;
    }

    return (
        editableProfile.id !== current.id ||
        editableProfile.name.trim() !== current.name ||
        editableProfile.host.trim() !== current.host ||
        Number(editableProfile.port) !== current.port ||
        Number(editableProfile.unitId) !== current.unitId ||
        Boolean(editableProfile.autoReconnect) !==
            Boolean(current.autoReconnect)
    );
});

/**
 * Загрузка проекта из localStorage при старте
 */
onMounted(async () => {
    // Подписываемся на события логирования от Rust
    try {
        logUnlisten = await listen<LogEntry>("modbus-log", (event) => {
            addLogEntry(event.payload);
        });
    } catch (e) {
        console.error("Failed to listen for log events:", e);
    }

    // Получить начальный статус сервера
    try {
        const status = await invoke<ServerStatus>("get_server_status");
        Object.assign(serverStatus, status);
    } catch (e) {
        console.error("Failed to get server status:", e);
    }

    // Запустить периодический опрос статуса
    statusPollInterval = window.setInterval(async () => {
        try {
            const status = await invoke<ServerStatus>("get_server_status");
            Object.assign(serverStatus, status);
        } catch (e) {
            // Ignore polling errors
        }
    }, 2000);

    // Запустить периодический опрос переменных (когда сервер запущен)
    variablesPollInterval = window.setInterval(async () => {
        if (!serverStatus.running) {
            return;
        }
        try {
            await syncVariablesFromBackend();
        } catch (e) {
            // Ignore polling errors
        }
    }, 1000);

    try {
        const raw = window.localStorage.getItem(STORAGE_KEY);
        if (!raw) {
            const defProject = createDefaultProject();
            assignProject(defProject);
            applyProfileToEditable(defProject.profiles[0]);
            return;
        }

        const parsed = JSON.parse(raw) as Partial<ModbusProject>;

        if (
            !parsed ||
            !Array.isArray(parsed.profiles) ||
            parsed.profiles.length === 0
        ) {
            const defProject = createDefaultProject();
            assignProject(defProject);
            applyProfileToEditable(defProject.profiles[0]);
            return;
        }

        // Нормализуем загруженный проект
        const loadedProject: ModbusProject = {
            profiles: parsed.profiles,
            currentProfileId:
                parsed.currentProfileId ?? parsed.profiles[0]?.id ?? "default",
            variables: Array.isArray(parsed.variables) ? parsed.variables : [],
        };

        assignProject(loadedProject);

        const current = currentProfile.value ?? loadedProject.profiles[0];
        project.currentProfileId = current.id;
        applyProfileToEditable(current);
    } catch (e) {
        const defProject = createDefaultProject();
        assignProject(defProject);
        applyProfileToEditable(defProject.profiles[0]);
    }
});

/**
 * Очистка при размонтировании
 */
onUnmounted(() => {
    if (statusPollInterval !== null) {
        clearInterval(statusPollInterval);
        statusPollInterval = null;
    }
    if (variablesPollInterval !== null) {
        clearInterval(variablesPollInterval);
        variablesPollInterval = null;
    }
    if (logUnlisten) {
        logUnlisten();
        logUnlisten = null;
    }
});

/**
 * Синхронизация значений переменных из бэкенда.
 * Обновляет только поле value, чтобы не мешать редактированию других полей.
 */
async function syncVariablesFromBackend() {
    const backendVars = await invoke<ModbusVariable[]>("get_variables");

    // Создаём карту id -> value из бэкенда
    const valueMap = new Map<string, ModbusValue>();
    for (const v of backendVars) {
        valueMap.set(v.id, v.value);
    }

    // Обновляем только значения в локальных переменных
    for (const localVar of project.variables) {
        const backendValue = valueMap.get(localVar.id);
        if (backendValue !== undefined) {
            // Обновляем значение только если оно отличается
            if (
                JSON.stringify(localVar.value) !== JSON.stringify(backendValue)
            ) {
                localVar.value = backendValue;

                // Подсветить изменённую переменную
                highlightChangedVariable(localVar.id);
            }
        }
    }
}

/**
 * Подсветить переменную как недавно изменённую.
 * Подсветка автоматически снимается через 1.5 секунды.
 */
function highlightChangedVariable(id: string) {
    recentlyChangedIds.add(id);
    setTimeout(() => {
        recentlyChangedIds.delete(id);
    }, 1500);
}

/**
 * Обработчик изменения значения переменной пользователем.
 * Сохраняет в localStorage и отправляет на бэкенд (если сервер запущен).
 */
async function onVariableValueChange(variable: ModbusVariable) {
    persistProject();

    // Если сервер запущен, отправляем обновлённое значение на бэкенд
    if (serverStatus.running) {
        try {
            await invoke("update_variable", {
                id: variable.id,
                value: variable.value,
            });
        } catch (e) {
            console.error("Failed to update variable on backend:", e);
        }
    }
}

/**
 * Logging functions
 */

/**
 * Добавить запись в лог
 */
function addLogEntry(entry: LogEntry) {
    logEntries.unshift(entry);

    // Ограничиваем размер лога
    if (logEntries.length > MAX_LOG_ENTRIES) {
        logEntries.pop();
    }
}

/**
 * Очистить все логи
 */
function onClearLogs() {
    logEntries.length = 0;
}

/**
 * Получить CSS-класс для записи лога
 */
function getLogEntryClass(entry: LogEntry): string {
    return `log-${entry.entryType}`;
}

/**
 * Получить человекочитаемую метку типа лога
 */
function getLogTypeLabel(type: LogEntryType): string {
    switch (type) {
        case "request":
            return "→ REQ";
        case "response":
            return "← RES";
        case "error":
            return "✕ ERR";
        case "info":
            return "ℹ INFO";
        default:
            return type;
    }
}

/**
 * Форматировать время лога
 */
function formatLogTime(timestamp: string): string {
    // timestamp приходит как "секунды.миллисекунды" с эпохи
    const parts = timestamp.split(".");
    const secs = parseInt(parts[0], 10);
    const millis = parts[1] || "000";

    const date = new Date(secs * 1000);
    const hours = date.getHours().toString().padStart(2, "0");
    const minutes = date.getMinutes().toString().padStart(2, "0");
    const seconds = date.getSeconds().toString().padStart(2, "0");

    return `${hours}:${minutes}:${seconds}.${millis}`;
}

/**
 * Помощник для замены содержимого reactive project
 */
function assignProject(src: ModbusProject) {
    project.profiles = src.profiles;
    project.currentProfileId = src.currentProfileId;
    project.variables = src.variables ?? [];
}

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

    const existingIndex = project.profiles.findIndex(
        (prof) => prof.id === editableProfile.id,
    );

    const id = editableProfile.id || "default";

    const toSave: ModbusConnectionProfile = {
        id,
        name: editableProfile.name.trim(),
        host: editableProfile.host.trim(),
        port: Number(editableProfile.port),
        unitId: Number(editableProfile.unitId),
        autoReconnect: !!editableProfile.autoReconnect,
    };

    if (existingIndex >= 0) {
        project.profiles.splice(existingIndex, 1, toSave);
    } else {
        project.profiles.push(toSave);
    }

    project.currentProfileId = toSave.id;

    persistProject();
}

/**
 * Сброс профиля к значениям по умолчанию
 */
function onResetToDefaults() {
    const defProject = createDefaultProject();
    assignProject(defProject);
    applyProfileToEditable(defProject.profiles[0]);
    validationError.value = null;
    persistProject();
}

/**
 * Сохранение проекта в localStorage
 */
function persistProject() {
    try {
        const payload = JSON.stringify(project);
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
    () => project,
    () => {
        // Для автосохранения можно раскомментировать:
        // persistProject();
    },
    { deep: true },
);

/**
 * Работа с переменными Modbus
 */

/**
 * Добавить новую переменную с дефолтными значениями.
 */
function onAddVariable() {
    const newVar: ModbusVariable = {
        id: generateId("var"),
        name: "NewVar",
        area: "holding_register",

        address: 0,

        dataType: "uint16",

        value: 0,

        note: "",
    };

    project.variables.push(newVar);
    persistProject();
}

/**
 * Удалить переменную по ID.
 */
function onRemoveVariable(id: ModbusVariableId) {
    const idx = project.variables.findIndex((v) => v.id === id);
    if (idx >= 0) {
        project.variables.splice(idx, 1);
        persistProject();
    }
}

/**
 * Корректировка адреса переменной.
 * Сейчас просто ограничиваем диапазон, без сложной валидации.
 */
function onVariableAddressChange(variable: ModbusVariable) {
    if (!Number.isFinite(variable.address)) {
        variable.address = 0;
    }
    if (variable.address < 0) {
        variable.address = 0;
    }
    if (variable.address > 65535) {
        variable.address = 65535;
    }
    persistProject();
}

/**
 * Server control functions
 */

/**
 * Запустить сервер эмулятора
 */
async function onStartServer() {
    serverLoading.value = true;
    serverStatus.error = null;

    try {
        // Получить текущий профиль
        const profile = currentProfile.value;
        if (!profile) {
            throw new Error("Профиль подключения не выбран");
        }

        // Запустить сервер с текущим профилем и переменными
        const status = await invoke<ServerStatus>("start_server", {
            profile: profile,
            variables: project.variables,
        });

        Object.assign(serverStatus, status);
    } catch (e) {
        serverStatus.error = String(e);
        console.error("Failed to start server:", e);
    } finally {
        serverLoading.value = false;
    }
}

/**
 * Остановить сервер эмулятора
 */
async function onStopServer() {
    serverLoading.value = true;
    serverStatus.error = null;

    try {
        const status = await invoke<ServerStatus>("stop_server");
        Object.assign(serverStatus, status);
    } catch (e) {
        serverStatus.error = String(e);
        console.error("Failed to stop server:", e);
    } finally {
        serverLoading.value = false;
    }
}
</script>

<style scoped>
.container {
    margin: 0;
    min-height: 100vh;
    padding: 3vh 4vw;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    align-items: stretch;
    background-color: #f6f6f6;
    color: #0f0f0f;
    box-sizing: border-box;
    font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
}

.title {
    font-size: 2.2rem;
    margin-bottom: 0.5rem;
}

.card {
    background: #ffffff;
    border-radius: 10px;
    padding: 1.5rem 1.75rem;
    box-shadow: 0 2px 8px rgba(15, 15, 15, 0.08);
    max-width: 100%;
}

.variables-card {
    margin-top: 0.25rem;
}

/* Server control styles */
.server-control-card {
    border-left: 4px solid #666;
}

.server-control-card.running {
    border-left-color: #4caf50;
}

.server-status-row {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    flex-wrap: wrap;
    margin-bottom: 0.75rem;
}

.status-indicator {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 600;
    font-size: 1rem;
}

.status-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background-color: #888;
}

.status-indicator.running .status-dot {
    background-color: #4caf50;
    box-shadow: 0 0 6px #4caf50;
}

.status-indicator.stopped .status-dot {
    background-color: #888;
}

.status-indicator.running .status-text {
    color: #4caf50;
}

.status-indicator.stopped .status-text {
    color: #666;
}

.status-details {
    display: flex;
    gap: 1rem;
    font-size: 0.9rem;
    color: #555;
}

.server-error {
    margin-bottom: 0.75rem;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    background-color: #ffecec;
    color: #b00020;
    font-size: 0.9rem;
}

.server-actions {
    display: flex;
    gap: 0.6rem;
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

/* Две колонки для полей профиля: "Название + IP" и "Порт + Unit ID" */
.form-row-grid {
    flex-direction: row;
    gap: 1rem;
}

.form-field {
    flex: 1 1 0;
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

/* Отключённая кнопка "Сохранить профиль" */
.btn.primary:disabled {
    background-color: #9bb4f0;
    cursor: default;
    opacity: 0.8;
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

.btn.small {
    padding: 0.2rem 0.5rem;
    font-size: 0.8rem;
}

.btn.danger {
    background-color: #e53935;
    color: #ffffff;
}

.btn.danger:hover {
    background-color: #c62828;
}

.save-info {
    margin: 0.3rem 0 0;
    font-size: 0.85rem;
    color: #666666;
}

/* Таблица переменных */

.vars-summary {
    margin-bottom: 0.5rem;
    font-size: 0.9rem;
    color: #444;
}

.table-wrapper {
    width: 100%;
    overflow-x: auto;
}

.vars-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.9rem;
}

.vars-table thead {
    background-color: #f0f2f8;
}

.vars-table th,
.vars-table td {
    border: 1px solid #e0e0e0;
    padding: 0.3rem 0.4rem;
    text-align: left;
    white-space: nowrap;
}

.vars-table th {
    font-weight: 600;
}

.cell-input,
.cell-select {
    width: 100%;
    box-sizing: border-box;
    border-radius: 4px;
    border: 1px solid #d0d0d0;
    padding: 0.2rem 0.3rem;
    font-size: 0.85rem;
    font-family: inherit;
    outline: none;
    background-color: #ffffff;
}

.cell-input:focus,
.cell-select:focus {
    border-color: #396cd8;
    box-shadow: 0 0 0 1px rgba(57, 108, 216, 0.25);
}

.cell-input-number {
    text-align: right;
}

.cell-checkbox {
    width: 16px;
    height: 16px;
}

.value-cell {
    transition: background-color 0.3s ease;
}

.value-cell.value-changed {
    background-color: #fff3cd !important;
    animation: pulse-highlight 0.5s ease-in-out;
}

@keyframes pulse-highlight {
    0% {
        background-color: #ffc107;
    }
    100% {
        background-color: #fff3cd;
    }
}

.cell-center {
    text-align: center;
}

.cell-actions {
    text-align: center;
}

.empty-row {
    text-align: center;
    padding: 0.6rem;
    color: #777;
    font-style: italic;
}

.vars-actions {
    margin-top: 0.7rem;
    display: flex;
    justify-content: flex-start;
}

/* Logging panel styles */
.logs-card {
    margin-top: 0.5rem;
}

.logs-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.5rem;
}

.logs-title-row {
    display: flex;
    align-items: center;
    gap: 1rem;
}

.logs-count {
    font-size: 0.85rem;
    color: #666;
    font-weight: normal;
}

.logs-actions {
    display: flex;
    gap: 0.5rem;
}

.logs-container {
    max-height: 400px;
    overflow-y: auto;
    border: 1px solid #e0e0e0;
    border-radius: 6px;
    background-color: #fafafa;
    font-family: "Consolas", "Monaco", "Courier New", monospace;
    font-size: 0.82rem;
}

.log-entry {
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid #eee;
}

.log-entry:last-child {
    border-bottom: none;
}

.log-entry:hover {
    background-color: #f5f5f5;
}

.log-entry-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
    margin-bottom: 0.25rem;
}

.log-time {
    color: #888;
    font-size: 0.8rem;
}

.log-type {
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
}

.log-type.request {
    background-color: #e3f2fd;
    color: #1565c0;
}

.log-type.response {
    background-color: #e8f5e9;
    color: #2e7d32;
}

.log-type.error {
    background-color: #ffebee;
    color: #c62828;
}

.log-type.info {
    background-color: #fff3e0;
    color: #e65100;
}

.log-function {
    font-weight: 500;
    color: #333;
}

.log-function-code {
    color: #888;
    font-weight: normal;
}

.log-client {
    color: #666;
    font-size: 0.8rem;
}

.log-duration {
    color: #888;
    font-size: 0.8rem;
    margin-left: auto;
}

.log-entry-summary {
    color: #333;
    margin-bottom: 0.2rem;
}

.log-entry-raw {
    font-size: 0.75rem;
    color: #666;
    margin-top: 0.25rem;
}

.log-entry-raw .raw-label {
    color: #999;
    margin-right: 0.3rem;
}

.log-entry-raw code {
    background-color: #f0f0f0;
    padding: 0.1rem 0.3rem;
    border-radius: 3px;
    word-break: break-all;
}

.log-request {
    border-left: 3px solid #1565c0;
}

.log-response {
    border-left: 3px solid #2e7d32;
}

.log-error {
    border-left: 3px solid #c62828;
    background-color: #fff8f8;
}

.log-info {
    border-left: 3px solid #e65100;
}

.logs-empty {
    padding: 2rem;
    text-align: center;
    color: #888;
    font-style: italic;
    font-family: inherit;
}

@media (max-width: 768px) {
    .container {
        padding: 2vh 3vw;
    }

    .card {
        padding: 1.25rem 1.3rem;
    }

    .form-row-grid {
        flex-direction: column;
    }

    .vars-table th,
    .vars-table td {
        font-size: 0.8rem;
    }
}
</style>
