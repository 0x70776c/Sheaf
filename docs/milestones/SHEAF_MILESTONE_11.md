# SHEAF – Milestone 11: Nacharbeit M10 + Metadaten-Engine

> Basis: Branch `feat/m10-hardening` (HEAD `ab8c11b`), nach Merge in `main`.
> Teil **A** schließt Lücken, die in der Abnahme von M10 aufgefallen sind. Teil **B** baut `sheaf meta`.
> Abschnitt **1** als Prompt an den Agent, Abschnitt **2** an `AGENTS.md` anhängen.

---

# 0. Abnahme M10 (Fakten aus dem Agent-Bericht)

| Punkt | Befund | Bewertung |
|---|---|---|
| Overlay-Audit 10/10 mit Testnamen | belegt | 🟢 |
| `sheaf number` | existiert, nutzt Overlay-Engine | 🟢 |
| Argv-Passwörter | entfernt, Quellen env/file/stdin, `SecretString` | 🟢 |
| `load_pdf` + `clippy.toml` | umgesetzt | 🟢 |
| AES-256 (R6) | laut Bericht gegen `qpdf` und PDFium geprüft | 🟢 (siehe manuelle Prüfung unten) |
| Docs/`capabilities` aus clap | umgesetzt, Sync-Test | 🟢 mit Lücke (nächste Zeile) |
| **`capabilities`: `pages` und `convert` mit `args: []`** | Subcommands (`pages rotate`, `convert to-images` …) werden nicht introspiziert | 🔴 Agents sehen diese Befehle nicht |
| **`capabilities`: Flags vs. Optionen** | Boolesche Flags (`--combine`, `--no-cache`) haben `value_name`, kein `kind`/`default`/`possible_values` | 🟡 für Agents mehrdeutig |
| **Bericht: "Abweichungen: Keine"** | `number` weicht ab: Default-Format `{n} / {total}` (Spec: `{n}`), Schriftgröße `11` (Spec: `10`), Farbe `#111827` (Spec: `#444444`), Bereich `6..=300` (Spec: `4..=72`), zusätzliches `--opacity` | 🟡 Selbstbericht unzuverlässig |
| Desktop-Extras | Hinweis nach 3 Fehlversuchen, farbige Status-Badge-Logik (Spec: Schloss + Algorithmus) | 🟡 Scope-Creep, klein |
| **Testabdeckung** | `cli_password_sources_test`: 1 Test, `cli_password_migration_test`: 1, `cli_security_leak_test`: 2. **Kein eigener `number`-Test** in der Liste. Kein Beleg für: Permission-Bits, qpdf-Encryption-Check, Datei-Rechte-Warnung, Pipeline-Matrix je Befehl | 🟡 evtl. table-driven, aber nicht belegt |
| **UI-Checks** | `npm run check` und `npm run build` im Bericht nicht gezeigt | 🟡 |
| Hilfetext `protect` | Hinweis "Berechtigungen sind nur Hinweise an den Viewer" fehlt | 🟡 |
| Milestone-Dokumente | Agent fand sie nicht im Repo (`find MILESTONE files` Exit 1) und las aus dem Transcript | 🟡 Dokumente ins Repo legen |

## Manuelle Prüfung durch dich (vor Merge)

```bash
# Verschlüsselte Datei erzeugen
export PW=test1234
./target/release/sheaf protect tests/fixtures/fixture_12p.pdf --new-password-env PW -o /tmp/locked.pdf --force
open -a Preview /tmp/locked.pdf        # Passwort-Abfrage? Öffnet mit "test1234"?
```

- Zusätzlich in Chrome/Safari und, falls vorhanden, Adobe Acrobat Reader öffnen.
- AES-256/R6 wird nicht von jedem Viewer gleich robust unterstützt. Qpdf und PDFium zu bestehen reicht nicht als Kompatibilitätsnachweis.
- Schlägt Preview fehl: Befund an den Agent geben (Fallback AES-128 prüfen, Option `--algorithm aes128|aes256` erwägen).

## Repo-Hygiene

- Milestone-Dokumente nach `docs/milestones/` ins Repo committen (`SHEAF_MILESTONE_*.md`).
- Merge von `feat/m10-hardening` per PR nach Durchsicht der Diffs, CI muss grün sein.

---

# 1. Prompt an den Agent

```text
Milestone 10 ist abgenommen (siehe SHEAF_MILESTONE_11.md, Abschnitt 0 für die Befunde).
Starte Milestone 11. Lies zuerst docs/milestones/SHEAF_MILESTONE_11.md komplett
(falls nicht vorhanden: committe die Datei und die früheren Milestone-Dokumente dorthin; ich lege sie bereit).

Branch: `feat/m11-meta`. Pushe den Branch, merge NICHT nach main.

Teil A (Nacharbeit), je Schritt ein Commit:
11A.1  capabilities: Subcommands rekursiv, Argumenttyp (flag/option/positional), default, possible_values, help
11A.2  `number` an die Spezifikation angleichen (Defaults, Bereiche), `--opacity` bleibt
11A.3  Testlücken schließen (Abschnitt 4) und UI-Checks ausführen (npm run check, npm run build)
11A.4  Hilfetext/Doku: Berechtigungen sind Hinweise, kein echter Schutz

Teil B (Metadaten), je Schritt ein Commit:
11B.1  Metadaten-Grundlagen: Textstrings (PDFDocEncoding/UTF-16BE), PDF-Datumsformat
11B.2  `meta get`
11B.3  `meta set` inkl. XMP-Modi
11B.4  `meta strip` inkl. `--deep`
11B.5  Desktop: Info-Panel um Metadaten-Editor und "Bereinigen" erweitern
11B.6  Tests laut Abschnitt 8

Regeln:
- Keine Crates oder APIs raten, docs.rs prüfen, Versionen und Wartungsstatus nennen.
- Alle PDFs ausschließlich über `load_pdf` laden (clippy erzwingt das), Passwort-Quellen wie in M10.
- Keine Features außerhalb dieses Dokuments. Ideen am Ende als Vorschlag.
- BERICHTSFORMAT (verbindlich):
  1. Tabelle "Spec-Punkt | Ist | Beleg (Testname oder Befehlsausgabe)". Abweichungen sind dort aufzuführen,
     "keine Abweichungen" nur, wenn die Tabelle vollständig und belegt ist.
  2. Rohe Ausgabe von `sheaf capabilities --json` (gekürzt auf `meta`, `pages`, `convert`), `sheaf meta --help`,
     `sheaf meta set --help`, `sheaf meta strip --help`.
  3. `cargo test --workspace -- --list | wc -l` und Zusammenfassung des Testlaufs, `npm run check` und `npm run build`.

Am Ende: Vorschlag für Milestone 12 (Release). Starte Milestone 12 NICHT ohne meine Freigabe.
```

---

# 2. Ergänzung für `AGENTS.md`

````markdown
## Berichte
- Abschlussberichte enthalten eine Spec-Ist-Tabelle mit Belegen und rohe Tool-Ausgaben.
- "Keine Abweichungen" ist nur zulässig, wenn die Tabelle jeden Punkt der Spezifikation abdeckt.
- Milestone-Dokumente liegen in `docs/milestones/` und sind Teil des Repos.

## Metadaten
- Textstrings im Info-Dict: lesen als PDFDocEncoding oder UTF-16BE (BOM `FE FF`) oder UTF-8 (BOM `EF BB BF`).
  Schreiben: PDFDocEncoding, wenn alle Zeichen darstellbar, sonst UTF-16BE mit BOM.
- Datumswerte: PDF-Format `D:YYYYMMDDHHmmSSOHH'mm'`, Ausgabe in JSON immer ISO-8601 plus Rohwert.
- `SOURCE_DATE_EPOCH` (Sekunden seit Epoch) überschreibt "jetzt" für deterministische Tests und Builds.
- Metadaten-Operationen ändern nie Seiteninhalte.
````

---

# 3. Teil A – Nacharbeit

## 11A.1 `capabilities` vollständig

Aktuell: `pages` und `convert` erscheinen mit `args: []`. Ziel: jeder Blatt-Befehl ist einzeln gelistet.

```json
{
  "name": "pages rotate",
  "args": [
    {"name": "input", "kind": "positional", "required": true},
    {"long": "pages", "kind": "option", "value_name": "PAGES", "required": false},
    {"long": "degrees", "kind": "option", "value_name": "DEGREES", "required": true, "possible_values": ["90","180","270","-90"]},
    {"long": "password-env", "kind": "option", "value_name": "VAR", "secret": true},
    {"long": "force", "kind": "flag"}
  ],
  "status": "stable"
}
```

| Feld je Argument | Regel |
|---|---|
| `kind` | `positional`, `option` (nimmt Wert), `flag` (boolesch) |
| `value_name` | nur bei `option`/`positional` |
| `default` | falls clap-Default vorhanden (String) |
| `possible_values` | bei Enums (`--preset`, `--position`, `--progress`) |
| `help` | erste Zeile des Hilfetexts |
| `secret` | wie bisher |
| `global_args` | Top-Level-Liste der globalen Flags (`--json`, `--dry-run`, …), nicht je Befehl wiederholt |

- Rekursion über `get_subcommands()`; Namen mit Leerzeichen (`convert to-images`).
- Behalte `schema: 1`: Felder werden **nur ergänzt**, bestehende bleiben (`value_name`, `required`, `secret`).
- `exit_codes`: `wrong_password` ergänzen (Exit 4, zweiter Code), `invalid_selection`/`empty_selection`/`usage` unter 2.
- Snapshot-Test (`insta`) und `docs_sync_test` aktualisieren.

## 11A.2 `number` angleichen

| Option | Spec (M9 §5) | Aktuell | Aktion |
|---|---|---|---|
| `--format` Default | `{n}` | `{n} / {total}` | auf `{n}` setzen |
| `--font-size` Default / Bereich | `10` / `4..=72` | `11` / `6..=300` | angleichen |
| `--color` Default | `#444444` | `#111827` | angleichen |
| `--opacity` | nicht vorgesehen | vorhanden (Default 1) | **bleibt**, in README vermerken |

- Snapshot- und CLI-Tests an die neuen Defaults anpassen.

## 11A.3 Testlücken (siehe Abschnitt 4)

## 11A.4 Berechtigungen: Hinweistext

In `protect --help`, `docs/CLI.md` und README:

> `--no-print`, `--no-copy`, `--no-modify` sind Hinweise an den PDF-Viewer. Manche Programme ignorieren sie. Echter Schutz entsteht nur durch das User-Passwort.

Zusätzlich: Ruft jemand `protect` ohne `--new-password-*` auf, muss **Exit 2** (Usage) mit klarem Hinweis kommen. Per Test belegen, ggf. `ArgGroup` mit `required(true)`.

---

# 4. Testlücken aus M10 schließen

Der Agent liefert eine Matrix **Anforderung → Testname**. Fehlende Tests werden ergänzt.

| Anforderung | Erwarteter Test |
|---|---|
| Canary in `protect`, `unlock`, `info`, `split`, `watermark`, `number`, jeweils `--json`, `--verbose`, `--progress json`, Fehlerfall, `--dry-run` | `cli_security_leak_test` (table-driven, ein Fall je Zeile) |
| Quellen: env fehlt/leer, file fehlt, stdin ist TTY, zwei stdin-Flags, leeres Passwort | `cli_password_sources_test` je Fall ein Eintrag |
| Entfernte Flags `--password`, `--owner-password`, `--user-password` | `cli_password_migration_test` je Flag |
| Pipeline-Matrix: **jeder** lesende Befehl (inkl. `pages *`, `convert to-images`, `convert to-markdown`, `render`, `compress`, `merge`) ohne Passwort → 4, falsch → 4 `wrong_password`, richtig → Erfolg + Warning | eigener Test mit Befehlsliste |
| `/P`-Bits für `--no-print/--no-copy/--no-modify` | Core-Test |
| `qpdf --show-encryption` bestätigt AES-256 und Berechtigungen | Core- oder CLI-Test |
| `--password-file` mit Mode `0644` → Warning | Unix-Test |
| `number`: Platzhalter, `--start`, `--pages`, Rotation 90/180/270, Rendering | `number_test.rs` |
| `info` Snapshot mit `encryption`-Block (ohne und mit Passwort) | `insta` |
| `protect → unlock` Roundtrip: Seitenzahl und Text gleich | Core-Test |
| Kein Thumbnail auf Disk bei verschlüsseltem Render | vorhanden (`cli_security_leak_test`), im Bericht belegen |
| `npm run check`, `npm run build` | Ausgabe im Bericht |

---

# 5. Teil B – `sheaf meta`

## Befehlsübersicht

```bash
sheaf meta get   <input>... [--xmp] [--json]
sheaf meta set   <input> [Felder] [--custom K=V]... [--unset KEY]... [--mod-date now|keep|ISO] [--xmp sync|keep|drop] [-o out]
sheaf meta strip <input> [--deep] [--keep FIELD]... [-o out]
```

- Alle Befehle: globale Flags, Open-Passwort-Quellen (`--password-env|file|stdin`), CLI-Vertrag aus `AGENTS.md`.
- Default-Ausgabe: `set` → `{stem}_meta.pdf`, `strip` → `{stem}_clean.pdf`.
- Ausgabe bei verschlüsselter Eingabe: unverschlüsselt + Warning (wie bei allen Befehlen).
- `meta get` ohne Passwort bei verschlüsselter Datei: Exit 4 (Strings sind verschlüsselt). `info` bleibt wie in M10.
- `info.metadata` und `meta get` nutzen **denselben** Core-Code (eine Quelle der Wahrheit).

## Standardfelder

| CLI-Name | Info-Key | XMP-Property (bei `sync`) |
|---|---|---|
| `title` | `Title` | `dc:title` |
| `author` | `Author` | `dc:creator` |
| `subject` | `Subject` | `dc:description` |
| `keywords` | `Keywords` | `pdf:Keywords` |
| `creator` | `Creator` | `xmp:CreatorTool` |
| `producer` | `Producer` | `pdf:Producer` |
| `creation-date` | `CreationDate` | `xmp:CreateDate` |
| `mod-date` | `ModDate` | `xmp:ModifyDate` |

## Textstrings und Datum (Pflicht-Korrektheit)

| Thema | Regel |
|---|---|
| Lesen | PDFDocEncoding, UTF-16BE mit BOM `FE FF`, UTF-8 mit BOM `EF BB BF` |
| Schreiben | PDFDocEncoding, wenn alle Zeichen darstellbar, sonst UTF-16BE mit BOM (inkl. Surrogatpaare für Emoji) |
| Datum lesen | tolerant: `D:2026`, `D:20261022`, `D:20261022100000`, mit `Z`, `+02'00'`, `+02'`, `+0200` |
| Datum schreiben | `D:YYYYMMDDHHmmSS+HH'mm'` (bei UTC `Z00'00'`), Eingabe ISO-8601 |
| Unlesbares Datum | `iso: null`, `raw` bleibt, Warning |
| Determinismus | `SOURCE_DATE_EPOCH` ersetzt "jetzt" |

Crates (Wartungsstatus und API prüfen): ein Datums-Crate (`time` oder `jiff`), für XMP `quick-xml`.

## `meta get`

| Feld (`data.files[]`) | Typ |
|---|---|
| `path` | string |
| `info` | `{title, author, subject, keywords, creator, producer}` (nullable Strings) |
| `dates` | `{creation_date: {iso, raw}, mod_date: {iso, raw}}` |
| `custom` | Objekt, alle Info-Keys außerhalb der Standardfelder |
| `xmp` | `{present, size_bytes, in_sync}`, bei `--xmp` zusätzlich `packet` (String, max. 1 MB) |

- `in_sync`: Vergleich der Standardfelder aus Info mit den XMP-Properties (`null` bei fehlendem XMP).
- Mehrere Dateien erlaubt, ein Fehler bricht nicht ab: je Datei `ok`/`error`, Gesamt-Exit nach schwerstem Fehler.

```json
{
  "schema": 1, "ok": true, "op": "meta get",
  "data": { "files": [{
    "path": "report.pdf",
    "info": {"title": "Q3 Bericht", "author": "Müller", "subject": null, "keywords": "q3,finance", "creator": "Word", "producer": "Sheaf"},
    "dates": {"creation_date": {"iso": "2026-10-22T10:00:00+02:00", "raw": "D:20261022100000+02'00'"}, "mod_date": {"iso": null, "raw": null}},
    "custom": {"Company": "Acme"},
    "xmp": {"present": true, "size_bytes": 2048, "in_sync": true}
  }]},
  "warnings": [], "duration_ms": 12
}
```

## `meta set`

| Option | Beschreibung |
|---|---|
| `--title`, `--author`, `--subject`, `--keywords`, `--creator`, `--producer` | Wert setzen (leerer String → Usage-Fehler, Hint: `--unset` nutzen) |
| `--creation-date <ISO>` | Erstellungsdatum setzen |
| `--mod-date <now\|keep\|ISO>` | Default `now` |
| `--custom KEY=VALUE` | wiederholbar, KEY nur `[A-Za-z0-9_-]{1,64}`, nicht gleich einem Standardschlüssel |
| `--unset KEY` | wiederholbar, Standardnamen (case-insensitive) oder Custom-Key |
| `--xmp <sync\|keep\|drop>` | Default `sync` |

- Keine Änderung angegeben → Usage-Fehler (Exit 2).
- **XMP-Modi:**
  - `sync`: bestehendes XMP-Paket parsen und bekannte Properties (Tabelle oben) aktualisieren, übrige Inhalte bleiben unangetastet. **Existiert kein XMP, wird keines erzeugt.** Ist das Paket nicht parsebar: Warning, Paket unverändert, im Report `xmp_in_sync: false`.
  - `keep`: XMP unangetastet. Weichen Werte danach ab: Warning.
  - `drop`: `/Metadata` im Catalog entfernen.
- Ausgabe nur auf `Info`-Dict und optional XMP, Seiteninhalte bleiben byte-identisch (Test).
- `--dry-run`: Diff als Liste `{field, old, new}` ausgeben, nichts schreiben.

## `meta strip`

Entfernt identifizierende Metadaten auf Dokumentebene.

| Entfernt (Standard) | Detail |
|---|---|
| Info-Dict | alle Keys (außer `--keep`) |
| XMP | Catalog `/Metadata` |
| PieceInfo | Catalog und Seiten (`/PieceInfo`) |
| Objekt-Metadaten | `/Metadata` an Seiten und XObjects |
| Trailer `/ID` | neu erzeugt (zufällig, Ausgabe ist unverschlüsselt) |

| Zusätzlich bei `--deep` | Detail |
|---|---|
| Annotationen | `/T` (Autor), `/M` (Änderungsdatum), `/CreationDate`, `/Subj` |

- `--keep <field>` (wiederholbar): Standardfelder erhalten (z. B. `--keep title`).
- Report: `{removed: {info_keys: [...], xmp: bool, piece_info: n, object_metadata: n, annotation_fields: n, trailer_id_regenerated: bool}}`.
- **Pflicht-Warning**, wenn DCT-Bilder (JPEG) enthalten sind: `"n JPEG-Bilder können EXIF-Daten enthalten, diese bleiben erhalten."`
- Dokumentation (README und `--help`): Strip entfernt **keine** sichtbaren Inhalte, eingebettete Schriftnamen, Bild-EXIF und Dateinamen eingebetteter Anhänge.

---

# 6. Desktop (`apps/desktop`)

- **Kein neuer Sidebar-Eintrag.** Das Info-Panel (`⌘8`) wird zu "Info & Metadaten".
- Felder: Titel, Autor, Betreff, Stichwörter, Creator, Producer. Datumsfelder read-only.
- Benutzerdefinierte Felder als Liste (Key/Value, hinzufügen/entfernen).
- **Änderungsvorschau:** vor dem Speichern Diff "Feld: alt → neu".
- Button **"Metadaten bereinigen"** mit Bestätigungsdialog, der auflistet, was entfernt wird (Info-Keys, XMP vorhanden?, PieceInfo). Hinweis auf EXIF in JPEG, wenn zutreffend.
- XMP-Modus unter "Erweitert" (Default `sync`).
- Verschlüsselte PDFs: bestehender Passwort-Dialog, Passwort nur im Speicher.
- Fortschritt, Toast, "Im Finder zeigen" wie bei den anderen Panels. `npm run check` und `npm run build` grün.

---

# 7. Core-API (Skizze)

```rust
pub struct MetaFields { pub title: Option<String>, pub author: Option<String>, /* … */ }

pub enum XmpMode { Sync, Keep, Drop }

pub struct MetaSetOptions {
    pub set: MetaFields, pub custom: Vec<(String, String)>, pub unset: Vec<String>,
    pub creation_date: Option<DateTime>, pub mod_date: ModDate, pub xmp: XmpMode,
    pub input: LoadOptions, pub out: PathBuf, pub force: bool, pub dry_run: bool,
}

#[derive(Serialize)] pub struct MetaGetReport { pub files: Vec<MetaFileReport> }
#[derive(Serialize)] pub struct MetaSetReport { pub output: PathBuf, pub changes: Vec<FieldChange>, pub xmp_in_sync: Option<bool> }
#[derive(Serialize)] pub struct MetaStripReport { pub output: PathBuf, pub removed: StripRemoved }

pub fn meta_get(src: &Path, opts: &MetaGetOptions) -> Result<MetaFileReport, SheafError>;
pub fn meta_set(src: &Path, opts: &MetaSetOptions, p: &dyn Progress) -> Result<MetaSetReport, SheafError>;
pub fn meta_strip(src: &Path, opts: &MetaStripOptions, p: &dyn Progress) -> Result<MetaStripReport, SheafError>;
```

---

# 8. Tests

| Ebene | Werkzeug | Inhalt |
|---|---|---|
| Unit | `cargo test` | PDF-Datum parsen/formatieren (alle tolerierten Varianten), Textstring-Encode/Decode (PDFDocEncoding, UTF-16BE, Surrogatpaare), Custom-Key-Validierung |
| Property | `proptest` | `decode(encode(s)) == s` für beliebige Unicode-Strings; Datumsparser panict nie |
| Roundtrip | Core | `set` → `get` liefert gesetzte Werte, inkl. `Müller`, `日本語`, Emoji |
| Determinismus | Test | mit `SOURCE_DATE_EPOCH` identische Ausgabe-Bytes bei zwei Läufen |
| XMP | Fixture mit XMP | `sync` aktualisiert nur bekannte Properties, fremde bleiben; `keep` unverändert; `drop` entfernt; nicht parsebares XMP → Warning, `xmp_in_sync: false`; ohne XMP wird keines erzeugt |
| Strip | Fixtures mit Info, XMP, PieceInfo, Annotationen mit `/T`, JPEG | nach `strip` (Scan über alle Objekte): kein `/Metadata`, kein `/PieceInfo`, Info leer (außer `--keep`), mit `--deep` keine Annotation-Autoren, neues `/ID`, JPEG-Warning |
| Inhalt unverändert | Core | Seitenzahl und extrahierter Text identisch, Content-Streams byte-identisch |
| Struktur | `qpdf --check`, `sheaf verify --deep` | jede Ausgabe |
| Passwort | CLI | verschlüsselte Eingabe: ohne Passwort 4, falsch 4 `wrong_password`, richtig Erfolg + Warning |
| Secret-Leak | CLI | Canary taucht in `meta`-Befehlen nicht auf (alle Modi) |
| CLI | `assert_cmd` | keine Änderung → 2, leerer Wert → 2, ungültiger Custom-Key → 2, Ziel existiert → 7, `--dry-run` schreibt nichts und zeigt Diff, stdout bei `--json` ist ein Objekt |
| Mehrfach-`get` | CLI | eine kaputte Datei unter mehreren: Exit 3, übrige Ergebnisse vorhanden |
| Snapshots | `insta` | JSON-Envelopes `get`/`set`/`strip` (Dauer maskiert) |
| `capabilities` | Snapshot | `meta get`, `meta set`, `meta strip` rekursiv gelistet |
| Docs | CI | `docs/CLI.md` aktuell, jeder Befehl hat Beispiel in der README |
| UI | `npm run check`, `npm run build` | grün |

---

# 9. Akzeptanzkriterien

- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` grün
- [ ] `capabilities` listet **alle** Blatt-Befehle (inkl. `pages *`, `convert *`, `meta *`) mit `kind`, `default`, `possible_values`, `help`
- [ ] `number`-Defaults entsprechen der Spezifikation, `--opacity` dokumentiert
- [ ] Testmatrix aus Abschnitt 4 vollständig belegt, `number_test.rs` existiert
- [ ] `npm run check` und `npm run build` im Bericht gezeigt und grün
- [ ] Hilfetext/Doku weist darauf hin, dass Berechtigungen nur Hinweise sind; `protect` ohne neues Passwort → Exit 2
- [ ] `meta get|set|strip` erfüllen den CLI-Vertrag, nutzen `load_pdf` und die Passwort-Quellen
- [ ] Unicode-Roundtrip (Umlaute, CJK, Emoji) und Datums-Toleranz getestet
- [ ] XMP-Modi `sync|keep|drop` getestet, kein neues XMP bei fehlendem Paket
- [ ] `strip` entfernt Info, XMP, PieceInfo, Objekt-Metadaten, erneuert `/ID`, `--deep` entfernt Annotation-Autoren, JPEG-Warning vorhanden
- [ ] Seiteninhalte bleiben byte-identisch
- [ ] Desktop: Info & Metadaten-Panel mit Diff-Vorschau und "Bereinigen"
- [ ] Bericht im vorgeschriebenen Format (Spec-Ist-Tabelle, rohe Ausgaben)
- [ ] Arbeit auf `feat/m11-meta`, Branch gepusht, nicht in `main` gemerged

---

# 10. Roadmap danach

| # | Milestone | Inhalt |
|---|---|---|
| 12 | Release | Universal Binary, PDFium x86_64 + arm64 bundlen, Code Signing, Notarization, `.dmg`, Homebrew Cask, `completions`, Stresstests (500+ Seiten), Benchmarks |
