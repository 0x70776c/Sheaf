<script lang="ts">
  import {
    AlertCircle,
    Check,
    ChevronDown,
    ChevronRight,
    FileText,
    Plus,
    RefreshCw,
    Shield,
    Trash2,
  } from '@lucide/svelte';
  import { basename, formatBytes } from '../tools';
  import type { FileInfo, MetaDiffEntryUi, MetaFileResultUi } from '../types';
  import {
    extractErrorMessage,
    getMetaCommand,
    runMetaSetCommand,
    runMetaStripCommand,
  } from '../tauri';

  interface Props {
    file: FileInfo;
    sessionPassword: string | null;
    isRunning: boolean;
    onSaveSuccess: (path: string, noticeTitle: string, noticeSubtitle: string) => void;
    onRequestPassword: () => void;
  }

  let { file, sessionPassword, isRunning, onSaveSuccess, onRequestPassword }: Props = $props();

  // Loading and metadata states
  let metaLoading = $state(false);
  let metaError = $state<string | null>(null);
  let originalMeta = $state<MetaFileResultUi | null>(null);

  // Form values
  let title = $state('');
  let author = $state('');
  let subject = $state('');
  let keywords = $state('');
  let creator = $state('');
  let producer = $state('');
  let customFields = $state<Array<{ key: string; value: string }>>([]);

  // New custom key inputs
  let newKey = $state('');
  let newValue = $state('');
  let customKeyError = $state<string | null>(null);

  // Advanced options
  let showAdvanced = $state(false);
  let xmpMode = $state<'sync' | 'keep' | 'drop'>('sync');

  // Dialog states
  let diffModalOpen = $state(false);
  let pendingDiff = $state<MetaDiffEntryUi[]>([]);
  let isSaving = $state(false);

  let stripModalOpen = $state(false);
  let stripDeep = $state(false);
  let isStripping = $state(false);

  // Fetch metadata when file or session password changes
  $effect(() => {
    const currentPath = file.path;
    const currentPw = sessionPassword;
    void loadMetadata(currentPath, currentPw);
  });

  async function loadMetadata(path: string, password: string | null) {
    if (file.encrypted && !password) {
      metaError = 'Datei ist verschlüsselt. Bitte Passwort eingeben.';
      return;
    }
    metaLoading = true;
    metaError = null;
    try {
      const rep = await getMetaCommand(path, password);
      const res = rep.files[0];
      if (res) {
        originalMeta = res;
        title = res.metadata.title ?? '';
        author = res.metadata.author ?? '';
        subject = res.metadata.subject ?? '';
        keywords = res.metadata.keywords ?? '';
        creator = res.metadata.creator ?? '';
        producer = res.metadata.producer ?? '';
        customFields = Object.entries(res.custom ?? {}).map(([k, v]) => ({
          key: k,
          value: v,
        }));
      }
    } catch (e) {
      const err = extractErrorMessage(e);
      metaError = err.message;
      if (err.code === 'wrong_password' || err.code === 'encrypted') {
        onRequestPassword();
      }
    } finally {
      metaLoading = false;
    }
  }

  function handleAddCustomField() {
    customKeyError = null;
    const trimmedKey = newKey.trim();
    if (!trimmedKey) {
      customKeyError = 'Schlüssel darf nicht leer sein.';
      return;
    }
    if (!/^[A-Za-z0-9_-]{1,64}$/.test(trimmedKey)) {
      customKeyError = 'Erlaubt: 1–64 Zeichen (A-Z, a-z, 0-9, _, -).';
      return;
    }
    const standardKeys = ['title', 'author', 'subject', 'keywords', 'creator', 'producer'];
    if (standardKeys.includes(trimmedKey.toLowerCase())) {
      customKeyError = `„${trimmedKey}“ ist ein Standardfeld.`;
      return;
    }
    if (customFields.some((f) => f.key.toLowerCase() === trimmedKey.toLowerCase())) {
      customKeyError = `Schlüssel „${trimmedKey}“ existiert bereits.`;
      return;
    }

    customFields = [...customFields, { key: trimmedKey, value: newValue }];
    newKey = '';
    newValue = '';
  }

  function handleRemoveCustomField(index: number) {
    customFields = customFields.filter((_, i) => i !== index);
  }

  function computeLocalDiff(): MetaDiffEntryUi[] {
    const diff: MetaDiffEntryUi[] = [];
    const orig = originalMeta?.metadata;

    const checkField = (field: string, currentVal: string, origVal: string | null) => {
      const cur = currentVal.trim() || null;
      const old = origVal ?? null;
      if (cur !== old) {
        diff.push({ field, old, new: cur });
      }
    };

    checkField('Title', title, orig?.title ?? null);
    checkField('Author', author, orig?.author ?? null);
    checkField('Subject', subject, orig?.subject ?? null);
    checkField('Keywords', keywords, orig?.keywords ?? null);
    checkField('Creator', creator, orig?.creator ?? null);
    checkField('Producer', producer, orig?.producer ?? null);

    // Custom keys
    const origCustom = originalMeta?.custom ?? {};
    const curCustomMap: Record<string, string> = {};
    for (const f of customFields) {
      curCustomMap[f.key] = f.value;
    }

    // New or changed custom keys
    for (const [k, v] of Object.entries(curCustomMap)) {
      const oldV = origCustom[k] ?? null;
      if (oldV !== v) {
        diff.push({ field: `Custom:${k}`, old: oldV, new: v });
      }
    }

    // Removed custom keys
    for (const [k, oldV] of Object.entries(origCustom)) {
      if (!(k in curCustomMap)) {
        diff.push({ field: `Custom:${k}`, old: oldV, new: null });
      }
    }

    return diff;
  }

  async function handleOpenSavePreview() {
    const diff = computeLocalDiff();
    if (diff.length === 0) {
      metaError = 'Keine Änderungen vorgenommen.';
      return;
    }
    pendingDiff = diff;
    diffModalOpen = true;
  }

  async function handleConfirmSave() {
    diffModalOpen = false;
    isSaving = true;
    metaError = null;

    try {
      // Build custom and unset
      const customPairs: [string, string][] = customFields.map((f) => [f.key, f.value]);
      const unsetList: string[] = [];
      const orig = originalMeta?.metadata;

      if (orig?.title && !title.trim()) unsetList.push('Title');
      if (orig?.author && !author.trim()) unsetList.push('Author');
      if (orig?.subject && !subject.trim()) unsetList.push('Subject');
      if (orig?.keywords && !keywords.trim()) unsetList.push('Keywords');
      if (orig?.creator && !creator.trim()) unsetList.push('Creator');
      if (orig?.producer && !producer.trim()) unsetList.push('Producer');

      const origCustom = originalMeta?.custom ?? {};
      for (const k of Object.keys(origCustom)) {
        if (!customFields.some((f) => f.key === k)) {
          unsetList.push(k);
        }
      }

      const rep = await runMetaSetCommand({
        input: file.path,
        title: title.trim() || null,
        author: author.trim() || null,
        subject: subject.trim() || null,
        keywords: keywords.trim() || null,
        creator: creator.trim() || null,
        producer: producer.trim() || null,
        custom: customPairs,
        unset: unsetList,
        xmp: xmpMode,
        password: sessionPassword,
      });

      onSaveSuccess(
        rep.output,
        'Metadaten aktualisiert',
        `${rep.diff.length} Feld(er) erfolgreich gespeichert.`,
      );
      void loadMetadata(rep.output, sessionPassword);
    } catch (e) {
      const err = extractErrorMessage(e);
      metaError = err.message;
    } finally {
      isSaving = false;
    }
  }

  async function handleConfirmStrip() {
    stripModalOpen = false;
    isStripping = true;
    metaError = null;

    try {
      const rep = await runMetaStripCommand({
        input: file.path,
        deep: stripDeep,
        password: sessionPassword,
      });

      const count =
        rep.removed.info_keys.length +
        rep.removed.piece_info +
        rep.removed.object_metadata +
        rep.removed.annotation_fields;

      onSaveSuccess(
        rep.output,
        'Metadaten bereinigt',
        `${count} Metadaten-Einträge entfernt. Neue Trailer-ID erzeugt.`,
      );
      void loadMetadata(rep.output, sessionPassword);
    } catch (e) {
      const err = extractErrorMessage(e);
      metaError = err.message;
    } finally {
      isStripping = false;
    }
  }
</script>

<div class="info-panel">
  <!-- Document summary header -->
  <section class="section-card" aria-label="Dokumentinformationen">
    <div class="card-header">
      <FileText size={16} strokeWidth={1.5} />
      <span class="card-title">Dokumentübersicht</span>
    </div>
    <div class="info-grid">
      <div class="info-row">
        <span class="lbl">Datei:</span>
        <span class="val truncate" title={file.path}>{basename(file.path)}</span>
      </div>
      <div class="info-row">
        <span class="lbl">Dateigröße:</span>
        <span class="val">{formatBytes(file.size_bytes)}</span>
      </div>
      <div class="info-row">
        <span class="lbl">PDF-Version:</span>
        <span class="val">{file.pdf_version ?? 'Unbekannt'}</span>
      </div>
      <div class="info-row">
        <span class="lbl">Seiten:</span>
        <span class="val">{file.pages ?? 'Verschlüsselt'}</span>
      </div>
      <div class="info-row">
        <span class="lbl">Lesezeichen:</span>
        <span class="val">{file.has_outlines ? `${file.outline_count} Einträge` : 'Keine'}</span>
      </div>
      {#if originalMeta?.dates.creation_date.iso}
        <div class="info-row">
          <span class="lbl">Erstellt:</span>
          <span class="val date-val" title={originalMeta.dates.creation_date.raw ?? ''}>
            {originalMeta.dates.creation_date.iso}
          </span>
        </div>
      {/if}
      {#if originalMeta?.dates.mod_date.iso}
        <div class="info-row">
          <span class="lbl">Geändert:</span>
          <span class="val date-val" title={originalMeta.dates.mod_date.raw ?? ''}>
            {originalMeta.dates.mod_date.iso}
          </span>
        </div>
      {/if}
      {#if originalMeta?.xmp.present}
        <div class="info-row">
          <span class="lbl">XMP-Status:</span>
          <span class="val xmp-badge" class:in-sync={originalMeta.xmp.in_sync}>
            {originalMeta.xmp.in_sync ? 'XMP synchron' : 'XMP weicht ab'}
          </span>
        </div>
      {/if}
    </div>
  </section>

  <!-- Error display -->
  {#if metaError}
    <div class="meta-error-banner" role="alert">
      <AlertCircle size={15} />
      <span>{metaError}</span>
    </div>
  {/if}

  <!-- Standard Metadata Editor -->
  <section class="section-card" aria-label="Standard-Metadaten">
    <div class="card-header">
      <span class="card-title">Standard-Metadaten</span>
      {#if metaLoading}
        <RefreshCw size={13} class="spin" />
      {/if}
    </div>

    <div class="form-fields">
      <label class="field-label">
        <span>Titel</span>
        <input
          type="text"
          class="text-input"
          placeholder="Dokumenttitel"
          bind:value={title}
          disabled={metaLoading || isRunning}
        />
      </label>

      <label class="field-label">
        <span>Autor</span>
        <input
          type="text"
          class="text-input"
          placeholder="Verfasser / Urheber"
          bind:value={author}
          disabled={metaLoading || isRunning}
        />
      </label>

      <label class="field-label">
        <span>Betreff</span>
        <input
          type="text"
          class="text-input"
          placeholder="Dokumentthema / Betreff"
          bind:value={subject}
          disabled={metaLoading || isRunning}
        />
      </label>

      <label class="field-label">
        <span>Stichwörter</span>
        <input
          type="text"
          class="text-input"
          placeholder="Kommagetrennte Schlagwörter"
          bind:value={keywords}
          disabled={metaLoading || isRunning}
        />
      </label>

      <label class="field-label">
        <span>Creator</span>
        <input
          type="text"
          class="text-input"
          placeholder="Erstellendes Programm"
          bind:value={creator}
          disabled={metaLoading || isRunning}
        />
      </label>

      <label class="field-label">
        <span>Producer</span>
        <input
          type="text"
          class="text-input"
          placeholder="PDF-Erzeuger / Engine"
          bind:value={producer}
          disabled={metaLoading || isRunning}
        />
      </label>
    </div>
  </section>

  <!-- Custom Metadata Keys -->
  <section class="section-card" aria-label="Benutzerdefinierte Felder">
    <div class="card-header">
      <span class="card-title">Benutzerdefinierte Felder</span>
      <span class="count-pill">{customFields.length}</span>
    </div>

    {#if customFields.length > 0}
      <div class="custom-fields-list">
        {#each customFields as field, idx (field.key)}
          <div class="custom-field-row">
            <span class="custom-key" title={field.key}>{field.key}</span>
            <input
              type="text"
              class="text-input custom-val-input"
              bind:value={field.value}
              disabled={metaLoading || isRunning}
            />
            <button
              type="button"
              class="icon-btn delete-btn"
              title="Feld entfernen"
              onclick={() => handleRemoveCustomField(idx)}
              disabled={metaLoading || isRunning}
            >
              <Trash2 size={14} />
            </button>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Add custom key form -->
    <div class="add-custom-row">
      <input
        type="text"
        class="text-input key-add-input"
        placeholder="Schlüssel (z. B. Abteilung)"
        bind:value={newKey}
        disabled={metaLoading || isRunning}
      />
      <input
        type="text"
        class="text-input val-add-input"
        placeholder="Wert"
        bind:value={newValue}
        disabled={metaLoading || isRunning}
        onkeydown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            handleAddCustomField();
          }
        }}
      />
      <button
        type="button"
        class="add-key-btn"
        onclick={handleAddCustomField}
        disabled={metaLoading || isRunning || !newKey.trim()}
      >
        <Plus size={14} />
        <span>Hinzufügen</span>
      </button>
    </div>
    {#if customKeyError}
      <span class="error-hint">{customKeyError}</span>
    {/if}
  </section>

  <!-- Advanced Options (XMP) -->
  <section class="advanced-section">
    <button
      type="button"
      class="accordion-trigger"
      onclick={() => (showAdvanced = !showAdvanced)}
    >
      {#if showAdvanced}
        <ChevronDown size={14} />
      {:else}
        <ChevronRight size={14} />
      {/if}
      <span>Erweiterte XMP-Einstellungen</span>
    </button>

    {#if showAdvanced}
      <div class="advanced-body">
        <label class="field-label">
          <span>XMP-Synchronisationsmodus</span>
          <select class="select-input" bind:value={xmpMode}>
            <option value="sync">Synchronisieren (Standard) – Info & XMP abgleichen</option>
            <option value="keep">Beibehalten (Keep) – Bestehendes XMP unberührt lassen</option>
            <option value="drop">Entfernen (Drop) – XMP-Paket aus Katalog löschen</option>
          </select>
        </label>
        <p class="field-help">
          Wenn die Datei kein XMP-Paket enthält, wird im Modus „sync“ kein neues erzeugt.
        </p>
      </div>
    {/if}
  </section>

  <!-- Actions -->
  <div class="panel-actions">
    <button
      type="button"
      class="strip-btn"
      disabled={metaLoading || isRunning || isSaving || isStripping}
      onclick={() => (stripModalOpen = true)}
    >
      <Shield size={14} />
      <span>Metadaten bereinigen…</span>
    </button>

    <button
      type="button"
      class="save-btn"
      disabled={metaLoading || isRunning || isSaving || isStripping}
      onclick={() => void handleOpenSavePreview()}
    >
      <Check size={14} />
      <span>{isSaving ? 'Speichern…' : 'Speichern…'}</span>
    </button>
  </div>
</div>

<!-- Diff Confirmation Modal -->
{#if diffModalOpen}
  <div class="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="diff-title">
    <div class="modal-card">
      <h3 id="diff-title" class="modal-title">Änderungen überprüfen</h3>
      <p class="modal-desc">
        Folgende Metadaten-Felder werden beim Speichern in eine neue PDF-Datei geschrieben:
      </p>

      <div class="diff-list">
        {#each pendingDiff as d (d.field)}
          <div class="diff-item">
            <span class="diff-field-name">{d.field}:</span>
            <div class="diff-values">
              <span class="diff-old">{d.old ?? '— leer —'}</span>
              <span class="diff-arrow">→</span>
              <span class="diff-new">{d.new ?? '— entfernt —'}</span>
            </div>
          </div>
        {/each}
      </div>

      <div class="modal-footer">
        <button
          type="button"
          class="ghost-btn"
          onclick={() => (diffModalOpen = false)}
        >
          Abbrechen
        </button>
        <button
          type="button"
          class="primary-action-btn"
          onclick={() => void handleConfirmSave()}
        >
          Änderungen anwenden
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Strip Confirmation Modal -->
{#if stripModalOpen}
  <div class="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="strip-title">
    <div class="modal-card">
      <h3 id="strip-title" class="modal-title">Metadaten bereinigen</h3>
      <p class="modal-desc">
        Diese Aktion entfernt alle identifizierenden Dokument-Metadaten:
      </p>

      <ul class="strip-checklist">
        <li>Info-Dictionary (alle Standard- und benutzerdefinierten Felder)</li>
        <li>XMP-Metadatenpaket im Dokument-Katalog</li>
        <li>PieceInfo-Dictionaries (Katalog & Seiten)</li>
        <li>Objekt-Metadaten an Einzelseiten und XObjects</li>
        <li>Generiert eine neue, anonymisierte Dokument-ID (/ID)</li>
      </ul>

      <label class="deep-checkbox-label">
        <input type="checkbox" bind:checked={stripDeep} />
        <span>Erweiterte Bereinigung (--deep): Autoren & Daten aus Annotationen entfernen</span>
      </label>

      <div class="strip-warning-box">
        <strong>Hinweis:</strong> Eingebettete JPEG-Bilder können EXIF-Metadaten enthalten. Sichtbare Seiteninhalte, Schriftarten und Anhänge bleiben unverändert.
      </div>

      <div class="modal-footer">
        <button
          type="button"
          class="ghost-btn"
          onclick={() => (stripModalOpen = false)}
        >
          Abbrechen
        </button>
        <button
          type="button"
          class="danger-action-btn"
          onclick={() => void handleConfirmStrip()}
        >
          Jetzt bereinigen
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .info-panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-3);
    overflow-y: auto;
  }

  .section-card {
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    padding: var(--space-3);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
    border-bottom: 1px solid var(--border);
    padding-bottom: var(--space-2);
  }

  .card-title {
    flex: 1;
  }

  .count-pill {
    font-size: 11px;
    background-color: var(--surface-muted);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 1px 6px;
    color: var(--text-muted);
  }

  .info-grid {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
  }

  .info-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-2);
  }

  .lbl {
    color: var(--text-muted);
    min-width: 90px;
  }

  .val {
    color: var(--text);
    font-weight: 500;
    text-align: right;
  }

  .val.truncate {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 180px;
  }

  .val.date-val {
    font-family: var(--font-mono, monospace);
    font-size: 11px;
  }

  .xmp-badge {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 4px;
    background-color: #fef3c7;
    color: #92400e;
  }

  .xmp-badge.in-sync {
    background-color: #ecfdf5;
    color: #065f46;
  }

  .form-fields {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .field-label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .text-input,
  .select-input {
    width: 100%;
    padding: 6px 8px;
    font-size: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    background-color: var(--bg);
    color: var(--text);
    outline: none;
    transition: border-color var(--transition-fast);
  }

  .text-input:focus,
  .select-input:focus {
    border-color: var(--accent);
  }

  .custom-fields-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .custom-field-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .custom-key {
    font-size: 11px;
    font-weight: 600;
    color: var(--text);
    min-width: 75px;
    max-width: 90px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .custom-val-input {
    flex: 1;
  }

  .add-custom-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
  }

  .key-add-input {
    width: 100px;
    flex-shrink: 0;
  }

  .val-add-input {
    flex: 1;
  }

  .add-key-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 6px 10px;
    font-size: 11px;
    font-weight: 500;
    border-radius: var(--radius-control);
    background-color: var(--surface-muted);
    border: 1px solid var(--border);
    color: var(--text);
    cursor: pointer;
    white-space: nowrap;
  }

  .add-key-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .icon-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .icon-btn:hover {
    color: #ef4444;
  }

  .error-hint {
    font-size: 11px;
    color: #ef4444;
  }

  .meta-error-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background-color: #fef2f2;
    border: 1px solid #fee2e2;
    border-radius: var(--radius-control);
    color: #991b1b;
    font-size: 12px;
  }

  .advanced-section {
    border-top: 1px solid var(--border);
    padding-top: var(--space-2);
  }

  .accordion-trigger {
    display: flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    padding: 4px 0;
  }

  .accordion-trigger:hover {
    color: var(--text);
  }

  .advanced-body {
    padding: var(--space-2) 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field-help {
    font-size: 11px;
    color: var(--text-muted);
    margin: 0;
  }

  .panel-actions {
    display: flex;
    gap: var(--space-2);
    margin-top: var(--space-2);
    padding-bottom: var(--space-3);
  }

  .strip-btn {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px 12px;
    font-size: 12px;
    font-weight: 500;
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text);
    cursor: pointer;
    transition: background-color var(--transition-fast);
  }

  .strip-btn:hover:not(:disabled) {
    background-color: var(--surface-muted);
    border-color: #dc2626;
    color: #dc2626;
  }

  .save-btn {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px 12px;
    font-size: 12px;
    font-weight: 600;
    background-color: var(--accent);
    border: 1px solid var(--accent);
    border-radius: var(--radius-control);
    color: #fff;
    cursor: pointer;
  }

  .strip-btn:disabled,
  .save-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Modals */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background-color: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal-card {
    background-color: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    width: 480px;
    max-width: 90vw;
    padding: var(--space-4);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.2);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .modal-title {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }

  .modal-desc {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .diff-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 220px;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    padding: var(--space-2);
    background-color: var(--bg);
  }

  .diff-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12px;
    border-bottom: 1px dashed var(--border);
    padding-bottom: 4px;
  }

  .diff-item:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  .diff-field-name {
    font-weight: 600;
    color: var(--text);
  }

  .diff-values {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
  }

  .diff-old {
    color: #991b1b;
    background-color: #fee2e2;
    padding: 1px 4px;
    border-radius: 3px;
  }

  .diff-arrow {
    color: var(--text-muted);
  }

  .diff-new {
    color: #065f46;
    background-color: #d1fae5;
    padding: 1px 4px;
    border-radius: 3px;
  }

  .strip-checklist {
    margin: 0;
    padding-left: 18px;
    font-size: 12px;
    color: var(--text);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .deep-checkbox-label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text);
    cursor: pointer;
  }

  .strip-warning-box {
    font-size: 11px;
    color: #92400e;
    background-color: #fef3c7;
    border: 1px solid #fde68a;
    border-radius: var(--radius-control);
    padding: 8px;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  .ghost-btn {
    padding: 6px 12px;
    font-size: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    background-color: transparent;
    color: var(--text);
    cursor: pointer;
  }

  .primary-action-btn {
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 600;
    border: none;
    border-radius: var(--radius-control);
    background-color: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  .danger-action-btn {
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 600;
    border: none;
    border-radius: var(--radius-control);
    background-color: #dc2626;
    color: #fff;
    cursor: pointer;
  }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }
</style>
