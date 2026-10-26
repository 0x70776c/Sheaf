<script lang="ts">
  import { Check, Globe, Moon, Sun, Monitor, X, ExternalLink } from '@lucide/svelte';
  import type { Language } from '../i18n';
  import { translations } from '../i18n';
  import type { ThemePreference } from '../types';
  import { openPathInSystem } from '../tauri';

  interface Props {
    open: boolean;
    lang: Language;
    theme: ThemePreference;
    onClose: () => void;
    onSelectLanguage: (lang: Language) => void;
    onSelectTheme: (theme: ThemePreference) => void;
  }

  let {
    open,
    lang = 'en',
    theme = 'system',
    onClose,
    onSelectLanguage,
    onSelectTheme,
  }: Props = $props();

  let t = $derived(translations[lang]);

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      onClose();
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-backdrop" onclick={onClose} onkeydown={handleKeydown}>
    <div class="modal-card" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <div class="modal-title-row">
          <Globe size={18} strokeWidth={2} class="modal-icon" />
          <h2 class="modal-title">{t.settingsTitle}</h2>
        </div>
        <button
          type="button"
          class="modal-close"
          onclick={onClose}
          aria-label={t.close}
        >
          <X size={16} strokeWidth={2} />
        </button>
      </div>

      <div class="modal-body">
        <!-- Language Selection -->
        <div class="setting-group">
          <label class="setting-label" for="lang-select">
            <Globe size={15} strokeWidth={1.5} />
            <span>{t.interfaceLanguage}</span>
          </label>
          <div class="option-grid" id="lang-select">
            <button
              type="button"
              class="option-pill"
              class:active={lang === 'en'}
              onclick={() => onSelectLanguage('en')}
            >
              <span>{t.english}</span>
              {#if lang === 'en'}
                <Check size={14} strokeWidth={2.5} />
              {/if}
            </button>
            <button
              type="button"
              class="option-pill"
              class:active={lang === 'de'}
              onclick={() => onSelectLanguage('de')}
            >
              <span>{t.german}</span>
              {#if lang === 'de'}
                <Check size={14} strokeWidth={2.5} />
              {/if}
            </button>
          </div>
        </div>

        <!-- Theme Selection -->
        <div class="setting-group">
          <div class="setting-label">
            <Monitor size={15} strokeWidth={1.5} />
            <span>{t.appearance}</span>
          </div>
          <div class="option-grid">
            <button
              type="button"
              class="option-pill"
              class:active={theme === 'system'}
              onclick={() => onSelectTheme('system')}
            >
              <Monitor size={14} strokeWidth={1.5} />
              <span>{t.themeSystem}</span>
            </button>
            <button
              type="button"
              class="option-pill"
              class:active={theme === 'light'}
              onclick={() => onSelectTheme('light')}
            >
              <Sun size={14} strokeWidth={1.5} />
              <span>{t.themeLight}</span>
            </button>
            <button
              type="button"
              class="option-pill"
              class:active={theme === 'dark'}
              onclick={() => onSelectTheme('dark')}
            >
              <Moon size={14} strokeWidth={1.5} />
              <span>{t.themeDark}</span>
            </button>
          </div>
        </div>

        <div class="divider"></div>

        <!-- About Sheaf Box -->
        <div class="about-section">
          <div class="about-hero">
            <img src="/icon.png" alt="Sheaf" class="about-icon" width="48" height="48" />
            <div class="about-titles">
              <div class="app-heading">
                <span class="app-name">Sheaf</span>
                <span class="app-version">v0.1.0</span>
              </div>
              <p class="app-sub">{t.appSubtitle}</p>
            </div>
          </div>

          <div class="about-meta-grid">
            <div class="meta-row">
              <span class="meta-label">Version</span>
              <span class="meta-val">0.1.0</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Commit</span>
              <button
                type="button"
                class="link-val"
                onclick={() => void openPathInSystem('https://github.com/0x70776c/Sheaf/commit/2907e7f')}
                title="View commit on GitHub"
              >
                <code>2907e7f</code>
                <ExternalLink size={11} strokeWidth={2} />
              </button>
            </div>
            <div class="meta-row">
              <span class="meta-label">{lang === 'de' ? 'Lizenz' : 'License'}</span>
              <button
                type="button"
                class="link-val"
                onclick={() => void openPathInSystem('https://github.com/0x70776c/Sheaf/blob/main/LICENSE-MIT')}
                title="View MIT / Apache 2.0 License"
              >
                <span>MIT / Apache-2.0</span>
                <ExternalLink size={11} strokeWidth={2} />
              </button>
            </div>
            <div class="meta-row">
              <span class="meta-label">{lang === 'de' ? 'Drittanbieter' : 'Third-Party'}</span>
              <button
                type="button"
                class="link-val"
                onclick={() => void openPathInSystem('https://github.com/0x70776c/Sheaf/blob/main/THIRD_PARTY_LICENSES.md')}
                title="View THIRD_PARTY_LICENSES.md"
              >
                <span>THIRD_PARTY_LICENSES</span>
                <ExternalLink size={11} strokeWidth={2} />
              </button>
            </div>
          </div>

          <p class="license-note">{t.licenseInfo}</p>
        </div>
      </div>

      <div class="modal-footer">
        <button type="button" class="action-btn primary" onclick={onClose}>
          {t.close}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 200;
    background-color: rgba(0, 0, 0, 0.35);
    backdrop-filter: var(--glass-blur);
    -webkit-backdrop-filter: var(--glass-blur);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-4);
  }

  .modal-card {
    width: 100%;
    max-width: 460px;
    background-color: var(--surface);
    backdrop-filter: blur(36px) saturate(190%);
    -webkit-backdrop-filter: blur(36px) saturate(190%);
    border: 1px solid var(--glass-border);
    border-radius: var(--radius-card);
    box-shadow: var(--glass-glow), var(--shadow-floating);
    overflow: hidden;
    animation: modal-enter 160ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes modal-enter {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(6px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-4) var(--space-4) var(--space-3);
    border-bottom: 1px solid var(--border);
  }

  .modal-title-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--accent);
  }

  .modal-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }

  .modal-close {
    border: none;
    background: transparent;
    color: var(--text-muted);
    padding: 4px;
    border-radius: var(--radius-control);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background-color var(--transition-fast), color var(--transition-fast);
  }

  .modal-close:hover {
    background-color: var(--surface-hover);
    color: var(--text);
  }

  .modal-body {
    padding: var(--space-4);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .setting-group {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .setting-label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
    text-transform: uppercase;
    letter-spacing: 0.4px;
  }

  .option-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: var(--space-2);
  }

  .option-pill {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    padding: 8px 12px;
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    box-shadow: var(--glass-glow), 0 1px 2px rgba(0, 0, 0, 0.03);
    transition:
      border-color var(--transition-fast),
      background-color var(--transition-fast),
      color var(--transition-fast);
  }

  .option-pill:hover {
    background-color: var(--surface-hover);
    border-color: var(--accent);
    color: var(--text);
  }

  .option-pill.active {
    background-color: var(--accent-subtle);
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }

  .divider {
    height: 1px;
    background-color: var(--border);
    margin: var(--space-1) 0;
    opacity: 0.7;
  }

  .about-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    padding: var(--space-3);
    box-shadow: var(--glass-glow);
  }

  .about-hero {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .about-icon {
    border-radius: 10px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.12);
  }

  .app-heading {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
  }

  .app-name {
    font-size: 16px;
    font-weight: 700;
    color: var(--text);
  }

  .app-version {
    font-size: 12px;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }

  .app-sub {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
  }

  .about-meta-grid {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: var(--space-3);
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-surface);
  }

  .meta-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
  }

  .meta-label {
    color: var(--text-muted);
    font-weight: 500;
  }

  .meta-val {
    color: var(--text);
    font-weight: 500;
  }

  .link-val {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: transparent;
    border: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    font-size: 12px;
    font-weight: 500;
    text-decoration: none;
    transition: opacity var(--transition-fast);
  }

  .link-val:hover {
    opacity: 0.8;
    text-decoration: underline;
  }

  .link-val code {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    padding: 1px 4px;
    background-color: var(--surface-hover);
    border-radius: 4px;
  }

  .license-note {
    font-size: 11px;
    color: var(--text-muted);
    line-height: 1.4;
  }

  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: var(--space-3) var(--space-4);
    border-top: 1px solid var(--border);
    background-color: var(--sidebar-surface);
  }

  .action-btn {
    padding: 6px 16px;
    font-size: 12px;
    font-weight: 500;
    border-radius: var(--radius-control);
    cursor: pointer;
    border: 1px solid var(--border);
    background-color: var(--surface);
    color: var(--text);
    transition: background-color var(--transition-fast), border-color var(--transition-fast);
  }

  .action-btn.primary {
    background-color: var(--accent);
    color: white;
    border-color: transparent;
  }

  .action-btn.primary:hover {
    filter: brightness(1.1);
  }
</style>
