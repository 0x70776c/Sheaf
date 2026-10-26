export type Language = 'en' | 'de';

export interface Translations {
  // App Header & Branding
  appName: string;
  appSubtitle: string;
  
  // Navigation & Actions
  switchTool: string;
  settings: string;
  about: string;
  theme: string;
  language: string;
  openFile: string;
  clearFiles: string;
  chooseFiles: string;
  chooseDirectory: string;
  cancel: string;
  close: string;
  save: string;
  showInFinder: string;
  openResult: string;
  
  // Status & Badges
  unencrypted: string;
  unencryptedDesc: string;
  protected: string;
  protectedDesc: string;
  unlocked: string;
  unlockedDesc: string;
  readOnly: string;
  readOnlyDesc: string;
  
  // Tools
  merge: string;
  mergeDesc: string;
  split: string;
  splitDesc: string;
  pages: string;
  pagesDesc: string;
  compress: string;
  compressDesc: string;
  convert: string;
  convertDesc: string;
  watermark: string;
  watermarkDesc: string;
  number: string;
  numberDesc: string;
  protect: string;
  protectDesc: string;
  unlock: string;
  unlockDesc: string;
  meta: string;
  metaDesc: string;
  info: string;
  infoDesc: string;

  // Options & Form Controls
  outputDestination: string;
  defaultDestination: string;
  pagesSelection: string;
  allPages: string;
  selectedCount: string;
  pageRangesHint: string;
  ranges: string;
  everyNPages: string;
  bookmarks: string;
  combineRanges: string;
  rotateDegrees: string;
  degrees90: string;
  degrees180: string;
  degrees270: string;
  deleteSelected: string;
  reorderSequence: string;
  extractSubset: string;
  presetLow: string;
  presetMedium: string;
  presetHigh: string;
  grayscale: string;
  watermarkText: string;
  numberFormat: string;
  position: string;
  fontSize: string;
  opacity: string;
  color: string;
  margin: string;

  // Password Modal
  passwordRequired: string;
  passwordPrompt: string;
  passwordPlaceholder: string;
  passwordError: string;
  unlockDocument: string;

  // Settings Modal
  settingsTitle: string;
  interfaceLanguage: string;
  english: string;
  german: string;
  appearance: string;
  themeSystem: string;
  themeLight: string;
  themeDark: string;
  aboutSheaf: string;
  licenseInfo: string;
}

export const translations: Record<Language, Translations> = {
  en: {
    appName: 'Sheaf',
    appSubtitle: 'Fast, local PDF toolkit for macOS',

    switchTool: 'Switch tool...',
    settings: 'Settings',
    about: 'About Sheaf',
    theme: 'Theme',
    language: 'Language',
    openFile: 'Open…',
    clearFiles: 'Clear',
    chooseFiles: 'Choose files…',
    chooseDirectory: 'Choose folder…',
    cancel: 'Cancel',
    close: 'Close',
    save: 'Save',
    showInFinder: 'Show in Finder',
    openResult: 'Open',

    unencrypted: 'Unencrypted',
    unencryptedDesc: 'Document is not encrypted',
    protected: 'Protected',
    protectedDesc: 'Document is encrypted and password-protected',
    unlocked: 'Unlocked',
    unlockedDesc: 'Document unlocked with full edit rights',
    readOnly: 'Read-Only',
    readOnlyDesc: 'Document is restricted or read-only',

    merge: 'Merge',
    mergeDesc: 'Combine multiple PDF documents into one',
    split: 'Split',
    splitDesc: 'Split into parts by ranges, intervals, or bookmarks',
    pages: 'Pages',
    pagesDesc: 'Rotate, delete, reorder, or extract individual pages',
    compress: 'Compress',
    compressDesc: 'Optimize file size with image downsampling and deflate',
    convert: 'Convert',
    convertDesc: 'Convert between PDF, images, and Markdown',
    watermark: 'Watermark',
    watermarkDesc: 'Stamp text overlays with customizable opacity and position',
    number: 'Number',
    numberDesc: 'Add dynamic page numbers ({n} of {total})',
    protect: 'Protect',
    protectDesc: 'Encrypt with AES-256 and granular permissions',
    unlock: 'Unlock',
    unlockDesc: 'Remove password protection from PDF',
    meta: 'Metadata',
    metaDesc: 'Inspect, modify, or strip document Info and XMP metadata',
    info: 'Info',
    infoDesc: 'View detailed document structure and properties',

    outputDestination: 'Output Destination',
    defaultDestination: 'Default (same directory as input)',
    pagesSelection: 'Pages',
    allPages: 'All pages',
    selectedCount: 'selected',
    pageRangesHint: 'e.g. 1-3, 5, 8-',
    ranges: 'Ranges',
    everyNPages: 'Every N pages',
    bookmarks: 'Bookmarks',
    combineRanges: 'Combine into single file',
    rotateDegrees: 'Rotation',
    degrees90: '90° Clockwise',
    degrees180: '180° Flip',
    degrees270: '270° Counter-Clockwise',
    deleteSelected: 'Delete selected pages',
    reorderSequence: 'New page sequence (e.g. 3, 1, 2)',
    extractSubset: 'Extract selected pages',
    presetLow: 'Low (light compression)',
    presetMedium: 'Medium (recommended)',
    presetHigh: 'High (maximum reduction)',
    grayscale: 'Convert color to grayscale',
    watermarkText: 'Watermark Text',
    numberFormat: 'Format pattern ({n}, {total})',
    position: 'Position',
    fontSize: 'Font Size',
    opacity: 'Opacity',
    color: 'Color',
    margin: 'Margin',

    passwordRequired: 'Password Protected PDF',
    passwordPrompt: 'Please enter the password to open this document.',
    passwordPlaceholder: 'Enter password…',
    passwordError: 'Incorrect password. Please try again.',
    unlockDocument: 'Unlock PDF',

    settingsTitle: 'Settings & About',
    interfaceLanguage: 'Interface Language',
    english: 'English (Default)',
    german: 'Deutsch (German)',
    appearance: 'Appearance',
    themeSystem: 'System',
    themeLight: 'Light',
    themeDark: 'Dark',
    aboutSheaf: 'About Sheaf',
    licenseInfo: 'Open source under MIT & Apache 2.0 licenses.',
  },
  de: {
    appName: 'Sheaf',
    appSubtitle: 'Schnelles, lokales PDF-Toolkit für macOS',

    switchTool: 'Werkzeug wechseln…',
    settings: 'Einstellungen',
    about: 'Über Sheaf',
    theme: 'Erscheinungsbild',
    language: 'Sprache',
    openFile: 'Öffnen…',
    clearFiles: 'Leeren',
    chooseFiles: 'Dateien wählen…',
    chooseDirectory: 'Ordner wählen…',
    cancel: 'Abbrechen',
    close: 'Schließen',
    save: 'Speichern',
    showInFinder: 'Im Finder anzeigen',
    openResult: 'Öffnen',

    unencrypted: 'Unverschlüsselt',
    unencryptedDesc: 'Datei ist nicht verschlüsselt',
    protected: 'Geschützt',
    protectedDesc: 'Datei ist passwortgeschützt',
    unlocked: 'Entschlüsselt',
    unlockedDesc: 'Datei mit Rechten entschlüsselt',
    readOnly: 'Schreibgeschützt',
    readOnlyDesc: 'Eingeschränkte Zugriffsrechte',

    merge: 'Zusammenführen',
    mergeDesc: 'Mehrere PDFs in einer Datei kombinieren',
    split: 'Teilen',
    splitDesc: 'Nach Bereichen, Intervallen oder Lesezeichen aufteilen',
    pages: 'Seiten',
    pagesDesc: 'Seiten drehen, löschen, umordnen oder extrahieren',
    compress: 'Komprimieren',
    compressDesc: 'Dateigröße durch Bildkompression optimieren',
    convert: 'Konvertieren',
    convertDesc: 'Zwischen PDF, Bildern und Markdown umwandeln',
    watermark: 'Wasserzeichen',
    watermarkDesc: 'Textstempel mit Deckkraft und Position überlagern',
    number: 'Seitenzahlen',
    numberDesc: 'Dynamische Seitenzahlen ({n} von {total}) hinzufügen',
    protect: 'Schützen',
    protectDesc: 'Mit AES-256 und Berechtigungen verschlüsseln',
    unlock: 'Entsperren',
    unlockDesc: 'Passwortschutz aus PDF entfernen',
    meta: 'Metadaten',
    metaDesc: 'Info- und XMP-Metadaten anzeigen, anpassen oder bereinigen',
    info: 'Information',
    infoDesc: 'Struktur, Abmessungen und Details analysieren',

    outputDestination: 'Zielordner / Ausgabedatei',
    defaultDestination: 'Standard (gleicher Ordner wie Eingabe)',
    pagesSelection: 'Seiten',
    allPages: 'Alle Seiten',
    selectedCount: 'ausgewählt',
    pageRangesHint: 'z. B. 1-3, 5, 8-',
    ranges: 'Bereiche',
    everyNPages: 'Alle N Seiten',
    bookmarks: 'Lesezeichen',
    combineRanges: 'Zu einer Datei zusammenfügen',
    rotateDegrees: 'Drehung',
    degrees90: '90° im Uhrzeigersinn',
    degrees180: '180° Drehung',
    degrees270: '270° gegen Uhrzeigersinn',
    deleteSelected: 'Ausgewählte Seiten löschen',
    reorderSequence: 'Neue Reihenfolge (z. B. 3, 1, 2)',
    extractSubset: 'Ausgewählte Seiten extrahieren',
    presetLow: 'Gering (leichte Kompression)',
    presetMedium: 'Mittel (empfohlen)',
    presetHigh: 'Stark (maximale Ersparnis)',
    grayscale: 'Farben in Graustufen konvertieren',
    watermarkText: 'Wasserzeichen-Text',
    numberFormat: 'Formatierung ({n}, {total})',
    position: 'Position',
    fontSize: 'Schriftgröße',
    opacity: 'Deckkraft',
    color: 'Farbe',
    margin: 'Randabstand',

    passwordRequired: 'Geschützte PDF-Datei',
    passwordPrompt: 'Bitte gib das Passwort ein, um dieses Dokument zu öffnen.',
    passwordPlaceholder: 'Passwort eingeben…',
    passwordError: 'Falsches Passwort. Bitte erneut versuchen.',
    unlockDocument: 'PDF entsperren',

    settingsTitle: 'Einstellungen & Info',
    interfaceLanguage: 'Sprache / Language',
    english: 'English (Englisch)',
    german: 'Deutsch (German)',
    appearance: 'Erscheinungsbild',
    themeSystem: 'System',
    themeLight: 'Hell',
    themeDark: 'Dunkel',
    aboutSheaf: 'Über Sheaf',
    licenseInfo: 'Open Source unter MIT- und Apache 2.0-Lizenzen.',
  },
};

const STORAGE_KEY = 'sheaf_language';

export function getInitialLanguage(): Language {
  if (typeof window !== 'undefined') {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === 'de' || saved === 'en') {
      return saved;
    }
  }
  return 'en'; // English is default
}

export function saveLanguage(lang: Language): void {
  if (typeof window !== 'undefined') {
    localStorage.setItem(STORAGE_KEY, lang);
  }
}
